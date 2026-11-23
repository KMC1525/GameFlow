//! Execuções (runs) do pipeline de agentes: eventos em tempo real (SSE) com histórico para quem conectar depois.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    NodeAdded {
        id: String,
        role: String,
        label: String,
        task: String,
        deps: Vec<String>,
        round: u32,
    },
    NodeStatus {
        id: String,
        status: String,
        detail: Option<String>,
    },
    Log {
        node: String,
        level: String,
        text: String,
    },
    /// Mensagem entre agentes (pedidos de ajuda, feedback do testador...).
    Message {
        from: String,
        to: String,
        text: String,
    },
    File {
        path: String,
        action: String,
    },
    Done {
        ok: bool,
        summary: String,
    },
}

#[derive(Serialize, Clone, Debug)]
pub struct Envelope {
    pub seq: usize,
    #[serde(flatten)]
    pub event: RunEvent,
}

pub struct Run {
    pub id: String,
    pub project: String,
    events: Mutex<Vec<Envelope>>,
    tx: broadcast::Sender<Envelope>,
    pub cancel: AtomicBool,
    finished: AtomicBool,
}

impl Run {
    pub fn new(id: String, project: String) -> Arc<Run> {
        let (tx, _) = broadcast::channel(512);
        Arc::new(Run { id, project, events: Mutex::new(vec![]), tx, cancel: AtomicBool::new(false), finished: AtomicBool::new(false) })
    }

    pub fn emit(&self, event: RunEvent) {
        let mut evs = self.events.lock().unwrap();
        if matches!(event, RunEvent::Done { .. }) {
            self.finished.store(true, Ordering::SeqCst);
        }
        let env = Envelope { seq: evs.len(), event };
        evs.push(env.clone());
        let _ = self.tx.send(env);
    }

    pub fn log(&self, node: &str, level: &str, text: impl Into<String>) {
        self.emit(RunEvent::Log { node: node.into(), level: level.into(), text: text.into() });
    }

    /// Eventos a partir de `from` (inclusive).
    pub fn snapshot(&self, from: usize) -> Vec<Envelope> {
        self.events.lock().unwrap().iter().skip(from).cloned().collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Envelope> {
        self.tx.subscribe()
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AttachmentReq {
    /// Caminho relativo dentro do projeto (ex.: references/heroi.png)
    pub path: String,
    /// "reference" (inspiração) ou "use" (usar no jogo como está)
    pub mode: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct GenerateRequest {
    pub prompt: String,
    /// "auto" ou o id de um gênero
    pub genre: Option<String>,
    /// "create" (padrão no primeiro prompt) ou "modify"
    pub mode: Option<String>,
    /// Style preset id from the library (see `presets.rs`).
    pub preset: Option<String>,
    pub attachments: Vec<AttachmentReq>,
}
