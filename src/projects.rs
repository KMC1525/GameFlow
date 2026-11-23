//! Projetos: cada jogo é uma pasta com todo o código/assets gerados + `gameflow.json` (metadados).

use crate::spec::Genre;
use crate::util::*;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

pub const META_FILE: &str = "gameflow.json";
pub const HIDDEN_DIR: &str = ".gameflow";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub genre: Option<Genre>,
    pub prompt: String,
    pub created_at: String,
    pub updated_at: String,
    pub generated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TreeEntry {
    pub path: String,
    pub kind: &'static str, // "dir" | "file"
    pub size: u64,
}

#[derive(Clone)]
pub struct Projects {
    root: PathBuf,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}

impl Projects {
    pub fn new(root: PathBuf) -> Self {
        Projects { root }
    }

    /// Pasta do projeto (se o id for válido e a pasta existir).
    pub fn dir(&self, id: &str) -> Option<PathBuf> {
        if !valid_id(id) {
            return None;
        }
        let d = self.root.join(id);
        d.is_dir().then_some(d)
    }

    pub fn get(&self, id: &str) -> Option<ProjectMeta> {
        let d = self.dir(id)?;
        let mut m: ProjectMeta = std::fs::read_to_string(d.join(META_FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        m.id = id.to_string();
        if m.name.is_empty() {
            m.name = id.to_string();
        }
        Some(m)
    }

    pub fn list(&self) -> Vec<ProjectMeta> {
        let mut out: Vec<ProjectMeta> =
            std::fs::read_dir(&self.root).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).filter_map(|e| self.get(&e.file_name().to_string_lossy())).collect()).unwrap_or_default();
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
        out
    }

    pub fn save_meta(&self, meta: &ProjectMeta) -> Result<()> {
        let d = self.dir(&meta.id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        std::fs::write(d.join(META_FILE), serde_json::to_vec_pretty(meta)?)?;
        Ok(())
    }

    pub fn touch(&self, id: &str) {
        if let Some(mut m) = self.get(id) {
            m.updated_at = now_iso();
            let _ = self.save_meta(&m);
        }
    }

    pub fn create(&self, name: &str, genre: Option<Genre>) -> Result<ProjectMeta> {
        let name = name.trim();
        let name = if name.is_empty() { "Novo jogo" } else { name };
        std::fs::create_dir_all(&self.root)?;
        let base = slugify(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.root.join(&id).exists() {
            id = format!("{}-{}", base, n);
            n += 1;
        }
        std::fs::create_dir_all(self.root.join(&id).join(HIDDEN_DIR))?;
        let now = now_iso();
        let meta = ProjectMeta { id: id.clone(), name: name.chars().take(80).collect(), genre, prompt: String::new(), created_at: now.clone(), updated_at: now, generated: false };
        self.save_meta(&meta)?;
        Ok(meta)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<ProjectMeta> {
        let mut m = self.get(id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        m.name = name.trim().chars().take(80).collect();
        if m.name.is_empty() {
            bail!("o nome não pode ficar vazio");
        }
        self.save_meta(&m)?;
        Ok(m)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let d = self.dir(id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        std::fs::remove_dir_all(d)?;
        Ok(())
    }

    /// Resolve um caminho relativo dentro do projeto, sem permitir escapar dele (nem via symlink).
    pub fn resolve(&self, id: &str, rel: &str) -> Result<PathBuf> {
        let d = self.dir(id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        let p = safe_join(&d, rel).ok_or_else(|| anyhow!("caminho inválido"))?;
        // Se algum ancestral existente é um link simbólico que sai do projeto, recusa.
        let canon_root = d.canonicalize()?;
        let mut probe = p.clone();
        while !probe.exists() {
            if !probe.pop() {
                break;
            }
        }
        if probe.exists() && !probe.canonicalize()?.starts_with(&canon_root) {
            bail!("caminho fora do projeto");
        }
        Ok(p)
    }

    pub fn tree(&self, id: &str) -> Result<Vec<TreeEntry>> {
        let d = self.dir(id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        let mut out = vec![];
        walk(&d, &d, 0, &mut out);
        Ok(out)
    }

    pub fn read_text(&self, id: &str, rel: &str) -> Result<String> {
        let p = self.resolve(id, rel)?;
        let meta = std::fs::metadata(&p).with_context(|| format!("arquivo não encontrado: {}", rel))?;
        if !meta.is_file() {
            bail!("não é um arquivo");
        }
        if meta.len() > 4 * 1024 * 1024 {
            bail!("arquivo grande demais para o editor");
        }
        let bytes = std::fs::read(&p)?;
        String::from_utf8(bytes).map_err(|_| anyhow!("arquivo binário"))
    }

    pub fn write(&self, id: &str, rel: &str, data: &[u8]) -> Result<()> {
        let p = self.resolve(id, rel)?;
        if rel.trim_matches('/').is_empty() {
            bail!("caminho inválido");
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, data)?;
        self.touch(id);
        Ok(())
    }

    pub fn mkdir(&self, id: &str, rel: &str) -> Result<()> {
        let p = self.resolve(id, rel)?;
        std::fs::create_dir_all(p)?;
        Ok(())
    }

    pub fn remove(&self, id: &str, rel: &str) -> Result<()> {
        let p = self.resolve(id, rel)?;
        if rel.trim_matches('/').is_empty() || rel.trim_matches('/') == META_FILE {
            bail!("este item não pode ser removido");
        }
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
        } else {
            std::fs::remove_file(p)?;
        }
        self.touch(id);
        Ok(())
    }

    pub fn rename_path(&self, id: &str, from: &str, to: &str) -> Result<()> {
        let a = self.resolve(id, from)?;
        let b = self.resolve(id, to)?;
        if from.trim_matches('/') == META_FILE || to.trim_matches('/').is_empty() {
            bail!("movimento inválido");
        }
        if b.exists() {
            bail!("já existe um item com esse nome");
        }
        if let Some(p) = b.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::rename(a, b)?;
        self.touch(id);
        Ok(())
    }

    /// Zip do jogo pronto para distribuir (sem pastas internas do GameFlow).
    pub fn export_zip(&self, id: &str) -> Result<Vec<u8>> {
        let d = self.dir(id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        let mut entries = vec![];
        walk(&d, &d, 0, &mut entries);
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for e in entries {
            if e.path.starts_with("references") || e.path == META_FILE {
                continue;
            }
            if e.kind == "dir" {
                zw.add_directory(format!("{}/", e.path), opts)?;
            } else {
                zw.start_file(&e.path, opts)?;
                zw.write_all(&std::fs::read(d.join(&e.path))?)?;
            }
        }
        Ok(zw.finish()?.into_inner())
    }
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<TreeEntry>) {
    if depth > 8 || out.len() > 5000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut items: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| (!e.path().is_dir(), e.file_name().to_string_lossy().to_lowercase()));
    for e in items {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let rel = rel_slash(root, &p);
        if ft.is_dir() {
            out.push(TreeEntry { path: rel, kind: "dir", size: 0 });
            walk(root, &p, depth + 1, out);
        } else {
            out.push(TreeEntry { path: rel, kind: "file", size: e.metadata().map(|m| m.len()).unwrap_or(0) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("gf-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn create_list_write_delete() {
        let root = tmp();
        let ps = Projects::new(root.clone());
        let a = ps.create("Meu Jogo", Some(Genre::Rpg2d)).unwrap();
        let b = ps.create("Meu Jogo", None).unwrap();
        assert_eq!(a.id, "meu-jogo");
        assert_eq!(b.id, "meu-jogo-2");
        ps.write(&a.id, "js/game.js", b"console.log(1)").unwrap();
        assert_eq!(ps.read_text(&a.id, "js/game.js").unwrap(), "console.log(1)");
        let t = ps.tree(&a.id).unwrap();
        assert!(t.iter().any(|e| e.path == "js" && e.kind == "dir"));
        assert!(t.iter().any(|e| e.path == "js/game.js"));
        assert!(ps.write(&a.id, "../evil.txt", b"x").is_err());
        assert!(ps.read_text(&a.id, "/etc/passwd").is_err());
        let zip = ps.export_zip(&a.id).unwrap();
        assert!(zip.len() > 20);
        assert_eq!(ps.list().len(), 2);
        ps.delete(&a.id).unwrap();
        assert_eq!(ps.list().len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_blocked() {
        let root = tmp();
        let outside = tmp();
        let ps = Projects::new(root.clone());
        let a = ps.create("x", None).unwrap();
        let d = ps.dir(&a.id).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("link")).unwrap();
        assert!(ps.write(&a.id, "link/pwn.txt", b"x").is_err());
        assert!(!outside.join("pwn.txt").exists());
    }

    #[test]
    fn id_validation() {
        assert!(valid_id("abc-1"));
        assert!(!valid_id("../x"));
        assert!(!valid_id("A"));
        assert!(!valid_id(""));
    }
}
