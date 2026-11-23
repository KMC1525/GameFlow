//! Integração com o Hugging Face: busca de modelos, listagem de arquivos e download com
//! retomada/progresso para a pasta de modelos local.

use crate::config::{Paths, Settings};
use crate::util::safe_join;
use anyhow::{Result, anyhow, bail};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::io::AsyncWriteExt;

const MODEL_EXTS: [&str; 7] = ["gguf", "safetensors", "onnx", "bin", "ckpt", "pt", "pth"];

#[derive(Serialize, Clone, Debug)]
pub struct HfModel {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub pipeline_tag: Option<String>,
    pub tags: Vec<String>,
    pub gated: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct HfFile {
    pub path: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Download {
    pub id: String,
    pub repo: String,
    pub file: String,
    pub total: u64,
    pub done: u64,
    /// queued | downloading | done | error | cancelled
    pub status: String,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LocalModel {
    /// Caminho relativo à pasta de modelos (com `/`).
    pub path: String,
    pub size: u64,
    /// Tarefas para as quais o arquivo provavelmente serve: text, code, image, sound.
    pub suits: Vec<&'static str>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Recommended {
    pub task: &'static str,
    pub repo: &'static str,
    pub title: &'static str,
    pub note: &'static str,
}

pub const RECOMMENDED: [Recommended; 8] = [
    Recommended { task: "code", repo: "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF", title: "Qwen2.5-Coder 1.5B", note: "Leve (~1 GB). Roda em qualquer computador; bom para começar." },
    Recommended { task: "code", repo: "Qwen/Qwen2.5-Coder-3B-Instruct-GGUF", title: "Qwen2.5-Coder 3B", note: "Equilíbrio entre velocidade e qualidade (~2 GB)." },
    Recommended { task: "code", repo: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF", title: "Qwen2.5-Coder 7B", note: "Melhor qualidade de código (~5 GB, recomenda-se 16 GB de RAM)." },
    Recommended { task: "text", repo: "bartowski/Llama-3.2-3B-Instruct-GGUF", title: "Llama 3.2 3B Instruct", note: "Leve e multilíngue; ótimo para roteiro e diálogos." },
    Recommended { task: "text", repo: "Qwen/Qwen2.5-7B-Instruct-GGUF", title: "Qwen2.5 7B Instruct", note: "Roteiros mais ricos (~5 GB)." },
    Recommended { task: "image", repo: "second-state/stable-diffusion-v1-5-GGUF", title: "Stable Diffusion 1.5 (GGUF)", note: "Para usar com stable-diffusion.cpp. Bom para sprites e fundos." },
    Recommended { task: "image", repo: "city96/FLUX.1-schnell-gguf", title: "FLUX.1 schnell (GGUF)", note: "Alta qualidade, mas pesado (precisa de boa GPU/RAM)." },
    Recommended { task: "sound", repo: "rhasspy/piper-voices", title: "Piper voices (TTS)", note: "Vozes para falas (inclui pt_BR). Use com o comando `piper` no agente de som." },
];

#[derive(Clone)]
pub struct Hf {
    http: reqwest::Client,
    paths: Paths,
    settings: Arc<RwLock<Settings>>,
    base: String,
    downloads: Arc<Mutex<HashMap<String, Download>>>,
    cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

pub fn valid_repo(repo: &str) -> bool {
    let ok = |s: &str| !s.is_empty() && s.len() <= 96 && !s.starts_with('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    let mut it = repo.split('/');
    matches!((it.next(), it.next(), it.next()), (Some(a), Some(b), None) if ok(a) && ok(b))
}

fn valid_file(file: &str) -> bool {
    !file.is_empty() && !file.starts_with('/') && !file.contains("..") && !file.contains('\\') && file.len() < 300
}

impl Hf {
    pub fn new(paths: Paths, settings: Arc<RwLock<Settings>>) -> Self {
        let base = std::env::var("GAMEFLOW_HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".into());
        let http = reqwest::Client::builder().user_agent(concat!("GameFlow/", env!("CARGO_PKG_VERSION"))).connect_timeout(std::time::Duration::from_secs(20)).build().expect("cliente HTTP");
        Hf { http, paths, settings, base: base.trim_end_matches('/').to_string(), downloads: Default::default(), cancels: Default::default() }
    }

    fn token(&self) -> String {
        self.settings.read().map(|s| s.hf_token.clone()).unwrap_or_default()
    }

    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        let r = self.http.get(url);
        let t = self.token();
        if t.is_empty() { r } else { r.bearer_auth(t) }
    }

    pub async fn search(&self, query: &str, task: &str) -> Result<Vec<HfModel>> {
        let mut req = self.get(&format!("{}/api/models", self.base)).query(&[("search", query), ("sort", "downloads"), ("direction", "-1"), ("limit", "24")]);
        match task {
            "text" | "code" => req = req.query(&[("filter", "gguf")]),
            "image" => req = req.query(&[("pipeline_tag", "text-to-image")]),
            "sound" => req = req.query(&[("pipeline_tag", "text-to-speech")]),
            _ => {}
        }
        let v: Value = req.send().await?.error_for_status()?.json().await?;
        let arr = v.as_array().ok_or_else(|| anyhow!("resposta inesperada do Hugging Face"))?;
        Ok(arr
            .iter()
            .filter_map(|m| {
                Some(HfModel {
                    id: m.get("id")?.as_str()?.to_string(),
                    downloads: m.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
                    likes: m.get("likes").and_then(|x| x.as_u64()).unwrap_or(0),
                    pipeline_tag: m.get("pipeline_tag").and_then(|x| x.as_str()).map(String::from),
                    tags: m.get("tags").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).take(12).collect()).unwrap_or_default(),
                    gated: m.get("gated").map(|g| !matches!(g, Value::Bool(false) | Value::Null)).unwrap_or(false),
                })
            })
            .collect())
    }

    /// Arquivos de modelo de um repositório (somente extensões relevantes).
    pub async fn files(&self, repo: &str) -> Result<Vec<HfFile>> {
        if !valid_repo(repo) {
            bail!("nome de repositório inválido (use dono/nome)");
        }
        let url = format!("{}/api/models/{}/tree/main?recursive=true", self.base, repo);
        let v: Value = self.get(&url).send().await?.error_for_status()?.json().await?;
        let mut out: Vec<HfFile> = v
            .as_array()
            .ok_or_else(|| anyhow!("resposta inesperada do Hugging Face"))?
            .iter()
            .filter(|f| f.get("type").and_then(|t| t.as_str()) == Some("file"))
            .filter_map(|f| {
                let path = f.get("path")?.as_str()?.to_string();
                let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                if !MODEL_EXTS.contains(&ext.as_str()) && !path.ends_with(".onnx.json") {
                    return None;
                }
                let size = f.get("lfs").and_then(|l| l.get("size")).and_then(|s| s.as_u64()).or_else(|| f.get("size").and_then(|s| s.as_u64())).unwrap_or(0);
                Some(HfFile { path, size })
            })
            .collect();
        out.sort_by(|a, b| a.size.cmp(&b.size).then(a.path.cmp(&b.path)));
        Ok(out)
    }

    pub fn start_download(&self, repo: &str, file: &str) -> Result<Download> {
        if !valid_repo(repo) || !valid_file(file) {
            bail!("repositório ou arquivo inválido");
        }
        let target = safe_join(&self.paths.models.join(repo), file).ok_or_else(|| anyhow!("caminho inválido"))?;
        let id = format!("{}:{}", repo, file);
        {
            let dl = self.downloads.lock().unwrap();
            if let Some(d) = dl.get(&id) {
                if d.status == "downloading" || d.status == "queued" {
                    return Ok(d.clone());
                }
            }
        }
        let d = Download { id: id.clone(), repo: repo.into(), file: file.into(), total: 0, done: 0, status: "queued".into(), error: None };
        self.downloads.lock().unwrap().insert(id.clone(), d.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap().insert(id.clone(), cancel.clone());
        let me = self.clone();
        let (repo, file) = (repo.to_string(), file.to_string());
        tokio::spawn(async move {
            let res = me.run_download(&id, &repo, &file, target, cancel.clone()).await;
            let mut dl = me.downloads.lock().unwrap();
            if let Some(d) = dl.get_mut(&id) {
                match res {
                    Ok(()) if cancel.load(Ordering::Relaxed) => d.status = "cancelled".into(),
                    Ok(()) => d.status = "done".into(),
                    Err(e) => {
                        d.status = "error".into();
                        d.error = Some(e.to_string());
                    }
                }
            }
        });
        Ok(d)
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut Download)) {
        if let Some(d) = self.downloads.lock().unwrap().get_mut(id) {
            f(d);
        }
    }

    async fn run_download(&self, id: &str, repo: &str, file: &str, target: PathBuf, cancel: Arc<AtomicBool>) -> Result<()> {
        if let Some(p) = target.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        let part = PathBuf::from(format!("{}.part", target.display()));
        let have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        let url = format!("{}/{}/resolve/main/{}", self.base, repo, file.split('/').map(urlenc).collect::<Vec<_>>().join("/"));
        let mut req = self.get(&url);
        if have > 0 {
            req = req.header("Range", format!("bytes={}-", have));
        }
        self.update(id, |d| d.status = "downloading".into());
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            if status.as_u16() == 416 {
                // já estava completo
                tokio::fs::rename(&part, &target).await?;
                return Ok(());
            }
            if status.as_u16() == 401 || status.as_u16() == 403 {
                bail!("acesso negado ({status}). Este modelo exige aceitar a licença no site e um token do Hugging Face nas Configurações.");
            }
            bail!("o Hugging Face respondeu {status}");
        }
        let resumed = status.as_u16() == 206;
        let total = resp.content_length().map(|l| l + if resumed { have } else { 0 }).unwrap_or(0);
        let mut done = if resumed { have } else { 0 };
        self.update(id, |d| {
            d.total = total;
            d.done = done;
        });
        let mut f = if resumed { tokio::fs::OpenOptions::new().append(true).open(&part).await? } else { tokio::fs::File::create(&part).await? };
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                f.flush().await.ok();
                return Ok(());
            }
            let chunk = chunk?;
            f.write_all(&chunk).await?;
            done += chunk.len() as u64;
            self.update(id, |d| d.done = done);
        }
        f.flush().await?;
        drop(f);
        if total > 0 && done < total {
            bail!("download incompleto ({} de {} bytes)", done, total);
        }
        tokio::fs::rename(&part, &target).await?;
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> bool {
        match self.cancels.lock().unwrap().get(id) {
            Some(c) => {
                c.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    pub fn downloads(&self) -> Vec<Download> {
        let mut v: Vec<Download> = self.downloads.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn clear_finished(&self) {
        self.downloads.lock().unwrap().retain(|_, d| d.status == "downloading" || d.status == "queued");
    }

    pub fn local_models(&self) -> Vec<LocalModel> {
        let mut out = vec![];
        scan(&self.paths.models, &self.paths.models, 0, &mut out);
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    pub fn delete_local(&self, rel: &str) -> Result<()> {
        let p = safe_join(&self.paths.models, rel).ok_or_else(|| anyhow!("caminho inválido"))?;
        if !p.is_file() {
            bail!("modelo não encontrado");
        }
        std::fs::remove_file(p)?;
        Ok(())
    }
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}

fn scan(root: &std::path::Path, dir: &std::path::Path, depth: usize, out: &mut Vec<LocalModel>) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            scan(root, &p, depth + 1, out);
        } else if ft.is_file() {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            let ext = name.rsplit('.').next().unwrap_or("");
            if !MODEL_EXTS.contains(&ext) {
                continue;
            }
            let rel = crate::util::rel_slash(root, &p);
            out.push(LocalModel { suits: suits(&rel.to_lowercase(), ext), size: e.metadata().map(|m| m.len()).unwrap_or(0), path: rel });
        }
    }
}

fn suits(rel: &str, ext: &str) -> Vec<&'static str> {
    let imagey = ["diffusion", "sdxl", "sd-", "sd_", "sd1", "sd3", "flux", "stable", "turbo", "lcm"].iter().any(|k| rel.contains(k));
    if ext == "onnx" || rel.contains("piper") || rel.contains("tts") {
        vec!["sound"]
    } else if imagey {
        vec!["image"]
    } else if ext == "gguf" {
        if rel.contains("coder") || rel.contains("code") { vec!["code", "text"] } else { vec!["text", "code"] }
    } else {
        vec!["text", "code", "image", "sound"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_validation() {
        assert!(valid_repo("Qwen/Qwen2.5-Coder-7B-Instruct-GGUF"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("../etc"));
        assert!(!valid_repo("a/b c"));
        assert!(!valid_file("../x.gguf"));
        assert!(!valid_file("/x.gguf"));
        assert!(valid_file("dir/x.gguf"));
    }

    #[test]
    fn suits_guesses() {
        assert_eq!(suits("qwen/qwen2.5-coder-7b/x.gguf", "gguf")[0], "code");
        assert_eq!(suits("second-state/stable-diffusion-v1-5-gguf/x.gguf", "gguf"), vec!["image"]);
        assert_eq!(suits("rhasspy/piper-voices/pt/x.onnx", "onnx"), vec!["sound"]);
    }

    #[test]
    fn recommended_repos_are_valid() {
        for r in RECOMMENDED.iter() {
            assert!(valid_repo(r.repo), "{}", r.repo);
        }
    }
}
