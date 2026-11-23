//! Execução dos modelos locais. Cada tarefa (texto, código, imagem, som) pode usar um runtime diferente:
//!
//! * `llama_cpp`: o GameFlow inicia o `llama-server` (llama.cpp) com um GGUF baixado do Hugging Face;
//! * `openai`: qualquer servidor local compatível com a API da OpenAI (Ollama, LM Studio, vLLM...);
//! * `sd_cpp`: stable-diffusion.cpp para imagens;
//! * `command`: comando próprio (Piper, scripts...), com placeholders `{prompt}`, `{out}`, `{model}`;
//! * `builtin`: sem IA — os agentes usam geradores embutidos.

use crate::config::{AgentModel, Paths, RuntimeKind, Settings};
use crate::util::{safe_join, split_args};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Text,
    Code,
    Image,
    Sound,
}

impl Task {
    pub fn parse(s: &str) -> Option<Task> {
        match s {
            "text" => Some(Task::Text),
            "code" => Some(Task::Code),
            "image" => Some(Task::Image),
            "sound" => Some(Task::Sound),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Task::Text => "texto",
            Task::Code => "código",
            Task::Image => "imagem",
            Task::Sound => "som",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatMsg {
    pub role: &'static str,
    pub content: String,
}

impl ChatMsg {
    pub fn system(s: impl Into<String>) -> Self {
        ChatMsg { role: "system", content: s.into() }
    }
    pub fn user(s: impl Into<String>) -> Self {
        ChatMsg { role: "user", content: s.into() }
    }
}

struct LlamaProc {
    port: u16,
    child: Child,
    last_used: Instant,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Tools {
    pub llama_server: Option<String>,
    pub sd: Option<String>,
    pub node: Option<String>,
}

pub struct Runtimes {
    paths: Paths,
    settings: Arc<RwLock<Settings>>,
    http: reqwest::Client,
    llama: tokio::sync::Mutex<HashMap<String, LlamaProc>>,
}

/// Procura um executável no PATH (e em `extra`).
pub fn which(names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = extra.to_vec();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    for d in dirs {
        for n in names {
            for ext in if cfg!(windows) { vec![".exe", ".cmd", ".bat", ""] } else { vec![""] } {
                let p = d.join(format!("{n}{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

impl Runtimes {
    pub fn new(paths: Paths, settings: Arc<RwLock<Settings>>) -> Self {
        let http = reqwest::Client::builder().user_agent(concat!("GameFlow/", env!("CARGO_PKG_VERSION"))).build().expect("cliente HTTP");
        Runtimes { paths, settings, http, llama: Default::default() }
    }

    fn snapshot(&self) -> Settings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn agent(&self, task: Task) -> AgentModel {
        let s = self.snapshot();
        match task {
            Task::Text => s.agents.text,
            Task::Code => s.agents.code,
            Task::Image => s.agents.image,
            Task::Sound => s.agents.sound,
        }
    }

    /// Verdadeiro quando a tarefa usa o gerador embutido (sem modelo de IA).
    pub fn is_builtin(&self, task: Task) -> bool {
        self.agent(task).runtime == RuntimeKind::Builtin
    }

    pub fn tools(&self) -> Tools {
        let s = self.snapshot();
        let extra = vec![self.paths.root.join("runtimes")];
        let pick = |custom: &str, names: &[&str]| -> Option<String> {
            if !custom.is_empty() {
                return Some(custom.to_string());
            }
            which(names, &extra).map(|p| p.display().to_string())
        };
        Tools { llama_server: pick(&s.llama_server_path, &["llama-server"]), sd: pick(&s.sd_cli_path, &["sd-cli", "sd"]), node: pick(&s.node_path, &["node"]) }
    }

    // ───────────── Texto / código ─────────────

    /// Conversa com o modelo configurado para a tarefa. `Ok(None)` => runtime embutido (sem IA).
    pub async fn chat(&self, task: Task, msgs: &[ChatMsg], max_tokens: u32) -> Result<Option<String>> {
        let a = self.agent(task);
        let timeout = Duration::from_secs(self.snapshot().request_timeout_secs);
        let (endpoint, model, key) = match a.runtime {
            RuntimeKind::Builtin => return Ok(None),
            RuntimeKind::LlamaCpp => {
                if a.model.is_empty() {
                    bail!("nenhum modelo escolhido para o agente de {}", task.label());
                }
                let port = self.ensure_llama(&a).await?;
                (format!("http://127.0.0.1:{port}/v1"), a.model.clone(), String::new())
            }
            RuntimeKind::Openai => {
                if a.endpoint.is_empty() {
                    bail!("informe a URL do servidor (ex.: http://127.0.0.1:11434/v1) para o agente de {}", task.label());
                }
                (a.endpoint.trim_end_matches('/').to_string(), a.model.clone(), a.api_key.clone())
            }
            _ => bail!("o runtime escolhido não serve para a tarefa de {}", task.label()),
        };
        let body = json!({
            "model": if model.is_empty() { "default".to_string() } else { model },
            "messages": msgs.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
            "temperature": a.temperature,
            "max_tokens": max_tokens,
            "stream": false,
        });
        let mut req = self.http.post(format!("{endpoint}/chat/completions")).timeout(timeout).json(&body);
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await.with_context(|| format!("não consegui falar com o modelo de {} ({endpoint})", task.label()))?;
        let status = resp.status();
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("o modelo respondeu {status}: {}", v.get("error").map(|e| e.to_string()).unwrap_or_default());
        }
        let text = v["choices"][0]["message"]["content"].as_str().ok_or_else(|| anyhow!("resposta do modelo sem conteúdo"))?;
        Ok(Some(text.to_string()))
    }

    async fn ensure_llama(&self, a: &AgentModel) -> Result<u16> {
        let model_path = safe_join(&self.paths.models, &a.model).ok_or_else(|| anyhow!("caminho de modelo inválido"))?;
        if !model_path.is_file() {
            bail!("o arquivo do modelo não existe: {} (baixe-o em Configurações → Modelos)", a.model);
        }
        let mut map = self.llama.lock().await;
        if let Some(p) = map.get_mut(&a.model) {
            if p.child.try_wait()?.is_none() {
                p.last_used = Instant::now();
                return Ok(p.port);
            }
            map.remove(&a.model);
        }
        // limita a 2 servidores simultâneos (RAM): encerra o menos recente
        while map.len() >= 2 {
            let oldest = map.iter().min_by_key(|(_, p)| p.last_used).map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                if let Some(mut p) = map.remove(&k) {
                    let _ = p.child.kill().await;
                }
            }
        }
        let s = self.snapshot();
        let bin = if !s.llama_server_path.is_empty() {
            PathBuf::from(&s.llama_server_path)
        } else {
            which(&["llama-server"], &[self.paths.root.join("runtimes")])
                .ok_or_else(|| anyhow!("não encontrei o `llama-server` (llama.cpp). Instale-o (ex.: `brew install llama.cpp`) ou informe o caminho em Configurações → Agentes → Runtimes"))?
        };
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0")?;
            l.local_addr()?.port()
        };
        let mut cmd = Command::new(&bin);
        cmd.arg("-m").arg(&model_path).args(["--host", "127.0.0.1", "--port", &port.to_string(), "-c", &a.context.to_string()]);
        cmd.args(split_args(&a.command));
        cmd.kill_on_drop(true).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut child = cmd.spawn().with_context(|| format!("não consegui iniciar {}", bin.display()))?;
        let started = Instant::now();
        let url = format!("http://127.0.0.1:{port}/health");
        loop {
            if let Some(st) = child.try_wait()? {
                bail!("o llama-server encerrou ao iniciar ({st}). Verifique se o modelo cabe na memória.");
            }
            if let Ok(r) = self.http.get(&url).timeout(Duration::from_secs(2)).send().await {
                if r.status().is_success() {
                    break;
                }
            }
            if started.elapsed() > Duration::from_secs(300) {
                let _ = child.kill().await;
                bail!("tempo esgotado carregando o modelo no llama-server");
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        map.insert(a.model.clone(), LlamaProc { port, child, last_used: Instant::now() });
        Ok(port)
    }

    pub async fn shutdown(&self) {
        let mut map = self.llama.lock().await;
        for (_, mut p) in map.drain() {
            let _ = p.child.kill().await;
        }
    }

    // ───────────── Imagem ─────────────

    /// Gera uma imagem em `out`. `Ok(false)` => runtime embutido (use o gerador procedural).
    pub async fn generate_image(&self, prompt: &str, w: u32, h: u32, out: &Path, reference: Option<&Path>) -> Result<bool> {
        let a = self.agent(Task::Image);
        let timeout = Duration::from_secs(self.snapshot().request_timeout_secs);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        match a.runtime {
            RuntimeKind::Builtin => Ok(false),
            RuntimeKind::SdCpp => {
                let s = self.snapshot();
                let bin = if !s.sd_cli_path.is_empty() {
                    PathBuf::from(&s.sd_cli_path)
                } else {
                    which(&["sd-cli", "sd"], &[self.paths.root.join("runtimes")])
                        .ok_or_else(|| anyhow!("não encontrei o `sd`/`sd-cli` (stable-diffusion.cpp). Informe o caminho em Configurações → Agentes"))?
                };
                let model = safe_join(&self.paths.models, &a.model).filter(|p| p.is_file()).ok_or_else(|| anyhow!("escolha um modelo de imagem baixado em Configurações → Agentes"))?;
                let (iw, ih) = (w.clamp(256, 768) / 64 * 64, h.clamp(256, 768) / 64 * 64);
                let mut cmd = Command::new(bin);
                cmd.arg("-m").arg(&model).arg("-p").arg(prompt).arg("-o").arg(out).args(["-W", &iw.to_string(), "-H", &ih.to_string()]);
                if let Some(r) = reference.filter(|r| r.is_file()) {
                    cmd.arg("-i").arg(r).args(["--strength", "0.65"]);
                }
                cmd.args(split_args(&a.command));
                run_cmd(cmd, timeout, None).await?;
                Ok(out.is_file())
            }
            RuntimeKind::Command => {
                let model = if a.model.is_empty() { String::new() } else { safe_join(&self.paths.models, &a.model).map(|p| p.display().to_string()).unwrap_or_default() };
                self.run_template(&a, prompt, out, &model, &[("w", w.to_string()), ("h", h.to_string()), ("kind", "image".into())], timeout).await?;
                Ok(out.is_file())
            }
            RuntimeKind::Openai => {
                if a.endpoint.is_empty() {
                    bail!("informe a URL do servidor de imagens");
                }
                let body = json!({"model": a.model, "prompt": prompt, "n": 1, "size": format!("{}x{}", w.clamp(256, 1024), h.clamp(256, 1024)), "response_format": "b64_json"});
                let mut req = self.http.post(format!("{}/images/generations", a.endpoint.trim_end_matches('/'))).timeout(timeout).json(&body);
                if !a.api_key.is_empty() {
                    req = req.bearer_auth(&a.api_key);
                }
                let v: Value = req.send().await?.error_for_status()?.json().await?;
                let b64 = v["data"][0]["b64_json"].as_str().ok_or_else(|| anyhow!("o servidor não devolveu a imagem"))?;
                std::fs::write(out, base64::engine::general_purpose::STANDARD.decode(b64)?)?;
                Ok(true)
            }
            RuntimeKind::LlamaCpp => bail!("llama.cpp não gera imagens; escolha stable-diffusion.cpp, comando ou servidor compatível"),
        }
    }

    // ───────────── Som ─────────────

    /// `Ok(false)` => runtime embutido (use o sintetizador procedural).
    pub async fn generate_sound(&self, prompt: &str, kind: &str, out: &Path) -> Result<bool> {
        let a = self.agent(Task::Sound);
        let timeout = Duration::from_secs(self.snapshot().request_timeout_secs);
        match a.runtime {
            RuntimeKind::Builtin => Ok(false),
            RuntimeKind::Command => {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let model = if a.model.is_empty() { String::new() } else { safe_join(&self.paths.models, &a.model).map(|p| p.display().to_string()).unwrap_or_default() };
                let dur = if kind == "music" { "30" } else { "2" };
                self.run_template(&a, prompt, out, &model, &[("kind", kind.to_string()), ("duration", dur.into())], timeout).await?;
                Ok(out.is_file())
            }
            _ => bail!("para sons use o runtime \"comando\" (ex.: Piper ou um script) ou o gerador embutido"),
        }
    }

    /// Executa o modelo de comando. Se o template não usa `{prompt}`, o texto é enviado pela entrada padrão.
    async fn run_template(&self, a: &AgentModel, prompt: &str, out: &Path, model: &str, extra: &[(&str, String)], timeout: Duration) -> Result<()> {
        let tpl = split_args(&a.command);
        if tpl.is_empty() {
            bail!("defina o comando do agente (use {{prompt}}, {{out}} e {{model}})");
        }
        let uses_prompt = tpl.iter().any(|t| t.contains("{prompt}"));
        let args: Vec<String> = tpl
            .iter()
            .map(|t| {
                let mut s = t.replace("{prompt}", prompt).replace("{out}", &out.display().to_string()).replace("{model}", model);
                for (k, v) in extra {
                    s = s.replace(&format!("{{{k}}}"), v);
                }
                s
            })
            .collect();
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        run_cmd(cmd, timeout, if uses_prompt { None } else { Some(prompt.to_string()) }).await
    }

    // ───────────── Teste rápido ─────────────

    pub async fn test(&self, task: Task) -> Result<String> {
        match task {
            Task::Text | Task::Code => {
                let t0 = Instant::now();
                let r = self.chat(task, &[ChatMsg::user("Responda apenas com a palavra: OK")], 16).await?;
                Ok(match r {
                    None => "Gerador embutido ativo (sem IA). Escolha um runtime para usar um modelo.".into(),
                    Some(t) => format!("Modelo respondeu em {:.1}s: “{}”", t0.elapsed().as_secs_f32(), t.trim().chars().take(60).collect::<String>()),
                })
            }
            Task::Image => {
                let out = std::env::temp_dir().join(format!("gameflow-test-{}.png", uuid::Uuid::new_v4()));
                let ok = self.generate_image("a tiny red apple, pixel art", 256, 256, &out, None).await?;
                let _ = std::fs::remove_file(&out);
                Ok(if ok { "Imagem gerada com sucesso.".into() } else { "Gerador embutido ativo (arte procedural).".into() })
            }
            Task::Sound => {
                let out = std::env::temp_dir().join(format!("gameflow-test-{}.wav", uuid::Uuid::new_v4()));
                let ok = self.generate_sound("short beep", "sfx", &out).await?;
                let _ = std::fs::remove_file(&out);
                Ok(if ok { "Som gerado com sucesso.".into() } else { "Gerador embutido ativo (sons procedurais).".into() })
            }
        }
    }
}

async fn run_cmd(mut cmd: Command, timeout: Duration, stdin: Option<String>) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    cmd.kill_on_drop(true).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
    cmd.stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    let mut child = cmd.spawn().context("não consegui iniciar o comando do agente")?;
    if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
        si.write_all(text.as_bytes()).await.ok();
        si.write_all(b"\n").await.ok();
        drop(si);
    }
    let out = tokio::time::timeout(timeout, child.wait_with_output()).await.map_err(|_| anyhow!("o comando demorou mais que {}s", timeout.as_secs()))??;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("o comando falhou ({}): {}", out.status, err.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(f: impl FnOnce(&mut Settings)) -> Runtimes {
        let dir = std::env::temp_dir().join(format!("gf-rt-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root: dir.clone(), projects: dir.join("p"), models: dir.join("m"), settings_file: dir.join("s.json") };
        let mut s = Settings::default();
        f(&mut s);
        Runtimes::new(paths, Arc::new(RwLock::new(s)))
    }

    #[tokio::test]
    async fn builtin_returns_none() {
        let r = rt(|_| {});
        assert!(r.chat(Task::Code, &[ChatMsg::user("oi")], 10).await.unwrap().is_none());
        assert!(!r.generate_image("x", 64, 64, Path::new("/tmp/x.png"), None).await.unwrap());
        assert!(r.is_builtin(Task::Text));
    }

    #[tokio::test]
    async fn command_runtime_writes_output() {
        if cfg!(windows) {
            return;
        }
        let r = rt(|s| {
            s.agents.sound.runtime = RuntimeKind::Command;
            // usa `cp` para simular um gerador: copia /etc/hostname → {out}
            s.agents.sound.command = "cp /etc/hostname {out}".into();
        });
        let out = std::env::temp_dir().join(format!("gf-snd-{}.wav", uuid::Uuid::new_v4()));
        assert!(r.generate_sound("beep {x}", "sfx", &out).await.unwrap());
        assert!(out.is_file());
        std::fs::remove_file(out).ok();
    }

    #[tokio::test]
    async fn command_failure_is_reported() {
        if cfg!(windows) {
            return;
        }
        let r = rt(|s| {
            s.agents.sound.runtime = RuntimeKind::Command;
            s.agents.sound.command = "false".into();
        });
        assert!(r.generate_sound("x", "sfx", Path::new("/tmp/never.wav")).await.is_err());
    }
}
