//! Agentes de IA: um grafo **não linear** de especialistas que se ajudam para gerar o jogo inteiro.
//!
//! ```text
//!                ┌─► Roteirista ─────┐
//!   Diretor ─────┼─► Arquiteto ──────┴─► Engenheiro ─┐
//!                ├─► Artista ────────────────────────┼─► Integrador ─► Testador ─► Publicador
//!                └─► Compositor ─────────────────────┘        │            │
//!                          ▲   pedidos de assets faltantes ◄──┘            │
//!                          └────────── retrabalho (Engenheiro #2…) ◄───────┘
//! ```
//!
//! Os nós rodam em paralelo assim que suas dependências terminam e conversam por um quadro-negro
//! compartilhado (`Blackboard`). O Integrador pode pedir assets que faltam ao Artista/Compositor, e o
//! Testador pode devolver o jogo ao Engenheiro com a lista de problemas (até `max_review_rounds`).

pub mod checks;
pub mod graph;
pub mod llm;
pub mod roles;
pub mod run;

use crate::config::Settings;
use crate::projects::{HIDDEN_DIR, Projects};
use crate::runtime::Runtimes;
use crate::spec::GameSpec;
use crate::util::now_iso;
use anyhow::{Result, anyhow};
use run::{GenerateRequest, Run, RunEvent};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone, Debug)]
pub struct Attachment {
    pub rel: String,
    pub abs: PathBuf,
    /// true => usar no jogo como está; false => só inspiração
    pub use_in_game: bool,
    pub stem: String,
    pub ext: String,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        matches!(self.ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg")
    }
    pub fn is_audio(&self) -> bool {
        matches!(self.ext.as_str(), "wav" | "ogg" | "mp3" | "m4a")
    }
    /// Formatos que a biblioteca de imagem consegue decodificar para extrair paleta.
    pub fn decodable(&self) -> bool {
        matches!(self.ext.as_str(), "png" | "jpg" | "jpeg")
    }
}

/// Quadro-negro: estado compartilhado entre os agentes.
#[derive(Default)]
pub struct Blackboard {
    pub spec: Option<GameSpec>,
    pub modify: bool,
    pub palette_from_refs: bool,
    /// Problemas apontados pelo testador: "arquivo|mensagem"
    pub issues: Vec<String>,
    pub files: BTreeSet<String>,
    pub llm_files: BTreeSet<String>,
    pub missing_images: Vec<String>,
    pub missing_sounds: Vec<String>,
    pub warnings: Vec<String>,
    pub rework: u32,
    pub asset_requests: u32,
    pub used_ai: BTreeSet<String>,
}

pub struct Ctx {
    pub run: Arc<Run>,
    pub projects: Projects,
    pub project_id: String,
    pub dir: PathBuf,
    pub rt: Arc<Runtimes>,
    pub settings: Settings,
    pub req: GenerateRequest,
    pub attachments: Vec<Attachment>,
    pub bb: Mutex<Blackboard>,
    ids: Mutex<HashMap<String, u32>>,
}

impl Ctx {
    pub fn log(&self, node: &str, level: &str, text: impl Into<String>) {
        self.run.log(node, level, text);
    }

    pub fn msg(&self, from: &str, to: &str, text: impl Into<String>) {
        self.run.emit(RunEvent::Message { from: from.into(), to: to.into(), text: text.into() });
    }

    pub fn spec(&self) -> GameSpec {
        self.bb.lock().unwrap().spec.clone().unwrap_or_default()
    }

    pub fn update_spec(&self, f: impl FnOnce(&mut GameSpec)) {
        let mut bb = self.bb.lock().unwrap();
        let mut s = bb.spec.take().unwrap_or_default();
        f(&mut s);
        bb.spec = Some(s);
    }

    pub fn warn(&self, node: &str, text: impl Into<String>) {
        let t: String = text.into();
        self.log(node, "warn", t.clone());
        self.bb.lock().unwrap().warnings.push(t);
    }

    /// Escreve um arquivo do projeto (caminho seguro) e emite o evento correspondente.
    pub fn write(&self, rel: &str, data: &[u8]) -> Result<()> {
        let p = self.projects.resolve(&self.project_id, rel)?;
        let existed = p.exists();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, data)?;
        self.bb.lock().unwrap().files.insert(rel.to_string());
        self.run.emit(RunEvent::File { path: rel.to_string(), action: if existed { "modified" } else { "created" }.into() });
        Ok(())
    }

    pub fn read(&self, rel: &str) -> Option<String> {
        let p = self.projects.resolve(&self.project_id, rel).ok()?;
        std::fs::read_to_string(p).ok()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.projects.resolve(&self.project_id, rel).map(|p| p.exists()).unwrap_or(false)
    }

    /// Gera ids de nós: `engineer`, `engineer#2`, `engineer#3`...
    pub fn next_id(&self, role: &str) -> String {
        let mut m = self.ids.lock().unwrap();
        let n = m.entry(role.to_string()).or_insert(0);
        *n += 1;
        if *n == 1 { role.to_string() } else { format!("{}#{}", role, n) }
    }
}

// ───────────────────────── Histórico de conversa ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ChatEntry {
    pub role: String,
    pub text: String,
    pub ts: String,
    pub run_id: Option<String>,
    pub attachments: Vec<String>,
    pub ok: Option<bool>,
}

pub fn chat_load(dir: &std::path::Path) -> Vec<ChatEntry> {
    std::fs::read_to_string(dir.join(HIDDEN_DIR).join("chat.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn chat_append(dir: &std::path::Path, entry: ChatEntry) {
    let mut all = chat_load(dir);
    all.push(entry);
    let keep = all.len().saturating_sub(300);
    let all = &all[keep..];
    let _ = std::fs::create_dir_all(dir.join(HIDDEN_DIR));
    let _ = std::fs::write(dir.join(HIDDEN_DIR).join("chat.json"), serde_json::to_vec_pretty(all).unwrap_or_default());
}

// ───────────────────────── Orquestrador ─────────────────────────

pub struct Orchestrator {
    pub projects: Projects,
    pub rt: Arc<Runtimes>,
    pub settings: Arc<RwLock<Settings>>,
    runs: Mutex<HashMap<String, Arc<Run>>>,
}

impl Orchestrator {
    pub fn new(projects: Projects, rt: Arc<Runtimes>, settings: Arc<RwLock<Settings>>) -> Arc<Self> {
        Arc::new(Orchestrator { projects, rt, settings, runs: Mutex::new(HashMap::new()) })
    }

    pub fn get(&self, run_id: &str) -> Option<Arc<Run>> {
        self.runs.lock().unwrap().get(run_id).cloned()
    }

    pub fn active_for(&self, project: &str) -> Option<Arc<Run>> {
        self.runs.lock().unwrap().values().find(|r| r.project == project && !r.is_finished()).cloned()
    }

    /// Inicia uma geração em segundo plano. Só uma por projeto por vez.
    pub fn start(self: &Arc<Self>, project_id: &str, req: GenerateRequest) -> Result<Arc<Run>> {
        let dir = self.projects.dir(project_id).ok_or_else(|| anyhow!("projeto não encontrado"))?;
        if req.prompt.trim().is_empty() {
            return Err(anyhow!("escreva o que você quer criar"));
        }
        if req.prompt.chars().count() > 8000 {
            return Err(anyhow!("o texto do prompt é grande demais (máx. 8000 caracteres)"));
        }
        if self.active_for(project_id).is_some() {
            return Err(anyhow!("já existe uma geração em andamento neste projeto"));
        }
        let meta = self.projects.get(project_id).unwrap_or_default();
        let mut req = req;
        let existing = dir.join("game.json").is_file() && meta.generated;
        let wants_modify = req.mode.as_deref() == Some("modify") || (req.mode.is_none() && existing);
        req.mode = Some(if wants_modify && existing { "modify" } else { "create" }.into());

        let mut attachments = vec![];
        for a in req.attachments.iter().take(24) {
            let Ok(abs) = self.projects.resolve(project_id, &a.path) else { continue };
            if !abs.is_file() {
                continue;
            }
            let fname = abs.file_name().unwrap_or_default().to_string_lossy().to_string();
            let (stem, ext) = match fname.rsplit_once('.') {
                Some((s, e)) => (s.to_string(), e.to_lowercase()),
                None => (fname.clone(), String::new()),
            };
            attachments.push(Attachment { rel: a.path.clone(), abs, use_in_game: a.mode == "use", stem, ext });
        }

        let run = Run::new(uuid::Uuid::new_v4().simple().to_string(), project_id.to_string());
        {
            let mut runs = self.runs.lock().unwrap();
            // mantém só as últimas execuções
            if runs.len() > 24 {
                let done: Vec<String> = runs.iter().filter(|(_, r)| r.is_finished()).map(|(k, _)| k.clone()).take(runs.len() - 20).collect();
                for k in done {
                    runs.remove(&k);
                }
            }
            runs.insert(run.id.clone(), run.clone());
        }
        let settings = self.settings.read().map(|s| s.clone()).unwrap_or_default();
        let ctx = Arc::new(Ctx {
            run: run.clone(),
            projects: self.projects.clone(),
            project_id: project_id.to_string(),
            dir: dir.clone(),
            rt: self.rt.clone(),
            settings,
            req: req.clone(),
            attachments,
            bb: Mutex::new(Blackboard { modify: wants_modify && existing, ..Default::default() }),
            ids: Mutex::new(HashMap::new()),
        });
        chat_append(
            &dir,
            ChatEntry { role: "user".into(), text: req.prompt.clone(), ts: now_iso(), run_id: Some(run.id.clone()), attachments: req.attachments.iter().map(|a| a.path.clone()).collect(), ok: None },
        );
        let orch = self.clone();
        let run2 = run.clone();
        tokio::spawn(async move {
            let ok = graph::execute(ctx.clone()).await;
            let summary = roles::final_summary(&ctx, ok);
            chat_append(&ctx.dir, ChatEntry { role: "assistant".into(), text: summary.clone(), ts: now_iso(), run_id: Some(run2.id.clone()), attachments: vec![], ok: Some(ok) });
            run2.emit(RunEvent::Done { ok, summary });
            orch.projects.touch(&ctx.project_id);
        });
        Ok(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::server::{S, build_state};
    use crate::spec::Genre;

    fn tmp_state() -> S {
        let root = std::env::temp_dir().join(format!("gf-it-{}", uuid::Uuid::new_v4()));
        build_state(Paths { projects: root.join("projects"), models: root.join("models"), settings_file: root.join("settings.json"), root }).unwrap()
    }

    async fn run_and_wait(st: &S, id: &str, req: GenerateRequest) -> (bool, String) {
        let run = st.orch.start(id, req).expect("start");
        for _ in 0..1200 {
            if run.is_finished() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        match run.snapshot(0).last().map(|e| e.event.clone()) {
            Some(RunEvent::Done { ok, summary }) => (ok, summary),
            other => panic!("a execução não terminou: {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn offline_pipeline_creates_a_playable_project_for_every_genre() {
        let st = tmp_state();
        for g in Genre::ALL {
            let meta = st.projects.create(g.id(), Some(g)).unwrap();
            let (ok, summary) = run_and_wait(&st, &meta.id, GenerateRequest { prompt: "um jogo de teste com chefe".into(), genre: Some(g.id().into()), ..Default::default() }).await;
            assert!(ok, "{g:?}: {summary}");
            let dir = st.projects.dir(&meta.id).unwrap();
            for f in ["index.html", "style.css", "game.json", "js/gameflow.js", "js/game.js", "js/custom.js", "js/data.js", "assets/manifest.json", "README.md", "docs/design.md"] {
                assert!(dir.join(f).is_file(), "{f} ausente em {g:?}");
            }
            // todo asset pedido pela spec existe em disco e está no manifesto
            let spec: crate::spec::GameSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("game.json")).unwrap()).unwrap();
            let man: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("assets/manifest.json")).unwrap()).unwrap();
            for a in &spec.assets {
                assert!(man["images"].get(&a.id).is_some(), "asset {} faltando em {g:?}", a.id);
            }
            for s in &spec.sounds {
                assert!(man["audio"].get(&s.id).is_some(), "som {} faltando em {g:?}", s.id);
            }
            assert!(st.projects.get(&meta.id).unwrap().generated);
            // nenhum aviso de "Ainda faltam assets" no caminho embutido
            assert!(!summary.contains("faltam"), "{summary}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn modify_mode_keeps_user_edits_and_applies_simple_changes() {
        let st = tmp_state();
        let meta = st.projects.create("edit", Some(Genre::Platformer2d)).unwrap();
        let (ok, _) = run_and_wait(&st, &meta.id, GenerateRequest { prompt: "Plataforma 2D na floresta".into(), ..Default::default() }).await;
        assert!(ok);
        let dir = st.projects.dir(&meta.id).unwrap();
        let before: crate::spec::GameSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("game.json")).unwrap()).unwrap();
        // o usuário edita código e escolhe um sprite próprio
        std::fs::write(dir.join("js/custom.js"), "// MEU AJUSTE\nGF.on('start', () => {});\n").unwrap();
        let game_js = std::fs::read_to_string(dir.join("js/game.js")).unwrap() + "\n// edição do usuário\n";
        std::fs::write(dir.join("js/game.js"), &game_js).unwrap();
        std::fs::write(dir.join("assets/images/player.png"), b"not really a png").unwrap();
        let mut src = std::collections::BTreeMap::new();
        src.insert("player".to_string(), "user".to_string());
        std::fs::write(dir.join(".gameflow/asset_sources.json"), serde_json::to_vec(&src).unwrap()).unwrap();

        let (ok, summary) = run_and_wait(&st, &meta.id, GenerateRequest { prompt: "deixe mais difícil e mude para tema gelo".into(), ..Default::default() }).await;
        assert!(ok, "{summary}");
        assert!(summary.starts_with("Atualizei"), "{summary}");
        let after: crate::spec::GameSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("game.json")).unwrap()).unwrap();
        assert_eq!(after.difficulty, "hard");
        assert_eq!(after.theme, "gelo");
        assert_eq!(after.title, before.title);
        assert_eq!(after.world, before.world, "o mapa não deve ser regerado sem pedir");
        assert!(std::fs::read_to_string(dir.join("js/custom.js")).unwrap().contains("MEU AJUSTE"));
        assert_eq!(std::fs::read_to_string(dir.join("js/game.js")).unwrap(), game_js, "game.js do usuário foi sobrescrito");
        assert_eq!(std::fs::read(dir.join("assets/images/player.png")).unwrap(), b"not really a png", "sprite do usuário foi sobrescrito");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn only_one_run_per_project_and_validation() {
        let st = tmp_state();
        let meta = st.projects.create("x", None).unwrap();
        assert!(st.orch.start(&meta.id, GenerateRequest { prompt: "  ".into(), ..Default::default() }).is_err());
        assert!(st.orch.start("nao-existe", GenerateRequest { prompt: "oi".into(), ..Default::default() }).is_err());
        let run = st.orch.start(&meta.id, GenerateRequest { prompt: "jogo".into(), ..Default::default() }).unwrap();
        let second = st.orch.start(&meta.id, GenerateRequest { prompt: "outro".into(), ..Default::default() });
        assert!(second.is_err(), "duas execuções simultâneas no mesmo projeto");
        for _ in 0..1200 {
            if run.is_finished() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(run.is_finished());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn attachments_cannot_escape_the_project() {
        let st = tmp_state();
        let meta = st.projects.create("anexo", None).unwrap();
        let req = GenerateRequest { prompt: "jogo".into(), attachments: vec![run::AttachmentReq { path: "../../etc/passwd".into(), mode: "use".into() }], ..Default::default() };
        let (ok, _) = run_and_wait(&st, &meta.id, req).await;
        assert!(ok);
        let dir = st.projects.dir(&meta.id).unwrap();
        assert!(!dir.join("assets/images/user_passwd.png").exists());
    }
}
