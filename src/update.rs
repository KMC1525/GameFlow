//! `gameflow update`: baixa a última versão publicada no GitHub Releases e substitui o executável atual.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const REPO: &str = "KMC1525/GameFlow";

/// Triplo do alvo de compilação atual, igual ao usado nos nomes dos arquivos de release.
pub fn current_target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        _ => return None,
    })
}

pub fn asset_name(target: &str) -> String {
    if target.contains("windows") { format!("gameflow-{target}.zip") } else { format!("gameflow-{target}.tar.gz") }
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().unwrap_or(Some(0))?, it.next().unwrap_or(Some(0))?))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

pub async fn run(check_only: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let http = reqwest::Client::builder().user_agent(concat!("GameFlow/", env!("CARGO_PKG_VERSION"))).timeout(std::time::Duration::from_secs(60)).build()?;
    println!("Procurando atualizações (versão atual: {current})…");
    let resp = http.get(format!("https://api.github.com/repos/{REPO}/releases/latest")).header("Accept", "application/vnd.github+json").send().await.context("não consegui acessar o GitHub")?;
    if resp.status().as_u16() == 404 {
        println!("Ainda não há versões publicadas. Para instalar a partir do código-fonte:\n  cargo install --git https://github.com/{REPO} --force");
        return Ok(());
    }
    let rel: Value = resp.error_for_status()?.json().await?;
    let tag = rel["tag_name"].as_str().ok_or_else(|| anyhow!("resposta inesperada do GitHub"))?;
    if !is_newer(tag, current) {
        println!("Você já está na versão mais recente ({current}).");
        return Ok(());
    }
    println!("Nova versão disponível: {tag}");
    if check_only {
        println!("Rode `gameflow update` para instalar.");
        return Ok(());
    }
    let target = current_target().ok_or_else(|| anyhow!("plataforma sem build oficial; instale pelo código-fonte"))?;
    let name = asset_name(target);
    let assets = rel["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |n: &str| assets.iter().find(|a| a["name"].as_str() == Some(n)).and_then(|a| a["browser_download_url"].as_str()).map(String::from);
    let url = url_of(&name).ok_or_else(|| anyhow!("a versão {tag} não tem o arquivo {name}"))?;
    println!("Baixando {name}…");
    let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?;
    if let Some(sum_url) = url_of(&format!("{name}.sha256")) {
        let text = http.get(sum_url).send().await?.error_for_status()?.text().await?;
        let want = text.split_whitespace().next().unwrap_or("").to_lowercase();
        let got = hex::encode(Sha256::digest(&bytes));
        if want != got {
            bail!("a verificação de integridade falhou (esperado {want}, obtido {got}); nada foi alterado");
        }
        println!("Integridade verificada (SHA-256).");
    } else {
        println!("Aviso: esta versão não publicou checksum; pulando verificação.");
    }
    let bin_name = if cfg!(windows) { "gameflow.exe" } else { "gameflow" };
    let new_bin = extract_binary(&bytes, bin_name, target.contains("windows"))?;
    replace_current_exe(&new_bin)?;
    println!("Atualizado para {tag}! Reinicie o GameFlow.");
    Ok(())
}

fn extract_binary(archive: &[u8], bin_name: &str, zip: bool) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if zip {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
        for i in 0..z.len() {
            let mut f = z.by_index(i)?;
            if f.name().map(|n| n.rsplit('/').next() == Some(bin_name)).unwrap_or(false) {
                f.read_to_end(&mut out)?;
                return Ok(out);
            }
        }
    } else {
        let gz = flate2::read::GzDecoder::new(archive);
        let mut tar = tar::Archive::new(gz);
        for e in tar.entries()? {
            let mut e = e?;
            let p = e.path()?.to_path_buf();
            if p.file_name().map(|n| n == bin_name).unwrap_or(false) {
                e.read_to_end(&mut out)?;
                return Ok(out);
            }
        }
    }
    bail!("o arquivo baixado não contém {bin_name}")
}

fn replace_current_exe(new_bin: &[u8]) -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let dir = exe.parent().ok_or_else(|| anyhow!("pasta do executável desconhecida"))?;
    let tmp: PathBuf = dir.join(".gameflow.new");
    std::fs::write(&tmp, new_bin).with_context(|| format!("sem permissão para escrever em {} (tente com sudo ou reinstale pelo script)", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    swap(&tmp, &exe)
}

#[cfg(unix)]
fn swap(new: &Path, exe: &Path) -> Result<()> {
    std::fs::rename(new, exe)?;
    Ok(())
}

#[cfg(windows)]
fn swap(new: &Path, exe: &Path) -> Result<()> {
    // Windows não deixa sobrescrever o .exe em uso, mas permite renomeá-lo.
    let old = exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(new, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.10"), Some((0, 10, 0)));
        assert_eq!(parse_version("1.0.0-beta.1"), Some((1, 0, 0)));
        assert_eq!(parse_version("abc"), None);
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn asset_names() {
        assert_eq!(asset_name("x86_64-pc-windows-msvc"), "gameflow-x86_64-pc-windows-msvc.zip");
        assert_eq!(asset_name("aarch64-apple-darwin"), "gameflow-aarch64-apple-darwin.tar.gz");
        assert!(current_target().is_some());
    }

    #[test]
    fn extracts_from_targz_and_zip() {
        // tar.gz
        let mut tar_buf = Vec::new();
        {
            let gz = flate2::write::GzEncoder::new(&mut tar_buf, flate2::Compression::fast());
            let mut b = tar::Builder::new(gz);
            let data = b"BINARY";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            b.append_data(&mut h, "gameflow-x/gameflow", &data[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        assert_eq!(extract_binary(&tar_buf, "gameflow", false).unwrap(), b"BINARY");
        assert!(extract_binary(&tar_buf, "outro", false).is_err());
        // zip
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        z.start_file("gameflow.exe", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"EXE").unwrap();
        let bytes = z.finish().unwrap().into_inner();
        assert_eq!(extract_binary(&bytes, "gameflow.exe", true).unwrap(), b"EXE");
    }
}
