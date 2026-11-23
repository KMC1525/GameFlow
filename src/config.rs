//! Pastas de dados e configurações persistentes (temas, escala da interface, modelos por agente).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Locais no disco usados pelo GameFlow.
///
/// * `GAMEFLOW_HOME` definido => tudo fica dentro dele (útil para testes e instalações portáteis).
/// * Caso contrário: configurações/modelos no diretório de dados do SO e projetos em `Documentos/GameFlow`.
#[derive(Clone, Debug, Serialize)]
pub struct Paths {
    pub root: PathBuf,
    pub projects: PathBuf,
    pub models: PathBuf,
    pub presets: PathBuf,
    pub settings_file: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        let (root, projects) = match std::env::var_os("GAMEFLOW_HOME") {
            Some(h) => {
                let root = PathBuf::from(h);
                let projects = root.join("projects");
                (root, projects)
            }
            None => {
                let root = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("GameFlow");
                let projects = dirs::document_dir().unwrap_or_else(|| root.clone()).join("GameFlow");
                (root, projects)
            }
        };
        Paths { models: root.join("models"), presets: root.join("presets"), settings_file: root.join("settings.json"), projects, root }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(&self.projects)?;
        std::fs::create_dir_all(&self.models)?;
        Ok(())
    }
}

/// Como um agente executa o seu modelo.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    /// Gerador embutido (sem IA): modelos prontos + arte/sons procedurais. Funciona offline e sem GPU.
    #[default]
    Builtin,
    /// `llama-server` (llama.cpp) iniciado pelo GameFlow com um GGUF baixado do Hugging Face.
    LlamaCpp,
    /// Qualquer servidor compatível com a API da OpenAI (Ollama, LM Studio, vLLM, llama.cpp...).
    Openai,
    /// stable-diffusion.cpp (`sd`/`sd-cli`) para imagens.
    SdCpp,
    /// Comando próprio com placeholders `{prompt}`, `{out}`, `{model}` (ex.: Piper, scripts Python).
    Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentModel {
    pub runtime: RuntimeKind,
    /// Arquivo do modelo relativo à pasta de modelos (llama_cpp / sd_cpp / command) ou nome do modelo (openai).
    pub model: String,
    /// URL base (openai). Ex.: http://127.0.0.1:11434/v1
    pub endpoint: String,
    pub api_key: String,
    /// Comando (command) ou argumentos extras (sd_cpp / llama_cpp).
    pub command: String,
    pub temperature: f32,
    pub context: u32,
}

impl Default for AgentModel {
    fn default() -> Self {
        AgentModel { runtime: RuntimeKind::Builtin, model: String::new(), endpoint: String::new(), api_key: String::new(), command: String::new(), temperature: 0.7, context: 8192 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentsConfig {
    pub text: AgentModel,
    pub code: AgentModel,
    pub image: AgentModel,
    pub sound: AgentModel,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    /// Escala da interface (0.8 – 1.6).
    pub ui_scale: f32,
    pub editor_font_size: u32,
    pub agents: AgentsConfig,
    pub hf_token: String,
    pub llama_server_path: String,
    pub sd_cli_path: String,
    pub node_path: String,
    /// Quantas vezes o testador pode devolver o jogo para o engenheiro corrigir.
    pub max_review_rounds: u32,
    /// Tempo máximo (s) de cada chamada a um modelo.
    pub request_timeout_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "grafite".into(),
            ui_scale: 1.0,
            editor_font_size: 13,
            agents: AgentsConfig::default(),
            hf_token: String::new(),
            llama_server_path: String::new(),
            sd_cli_path: String::new(),
            node_path: String::new(),
            max_review_rounds: 2,
            request_timeout_secs: 900,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// Valida/normaliza valores vindos da UI.
    pub fn sanitize(&mut self) {
        self.ui_scale = self.ui_scale.clamp(0.7, 1.8);
        self.editor_font_size = self.editor_font_size.clamp(9, 28);
        self.max_review_rounds = self.max_review_rounds.min(5);
        self.request_timeout_secs = self.request_timeout_secs.clamp(10, 7200);
    }
}
