#![allow(clippy::collapsible_if, clippy::collapsible_match, clippy::too_many_arguments, clippy::type_complexity, clippy::needless_range_loop)]

mod adventure;
mod agents;
mod config;
mod hf;
mod levelgen;
mod presets;
mod procedural;
mod projects;
mod runtime;
mod server;
mod spec;
mod story;
mod templates;
mod update;
mod util;

use agents::run::{GenerateRequest, RunEvent};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "gameflow", version, about = "GameFlow — IDE low/no-code para criar jogos completos com IA local")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Inicia o GameFlow e abre a interface no navegador (comando padrão)
    Start {
        /// Porta da interface
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Não abrir o navegador automaticamente
        #[arg(long)]
        no_open: bool,
    },
    /// Atualiza o GameFlow para a versão mais recente
    Update {
        /// Apenas verifica se há versão nova
        #[arg(long)]
        check: bool,
    },
    /// Mostra diagnóstico do ambiente (pastas, runtimes de IA encontrados)
    Doctor,
    /// Gera um jogo pelo terminal, sem abrir a interface
    Generate {
        /// Descrição do jogo
        prompt: String,
        /// Nome do projeto
        #[arg(long)]
        name: Option<String>,
        /// Gênero: rpg2d, rpg3d, visual_novel, platformer2d, platformer3d, fps, tps, arcade
        #[arg(long)]
        genre: Option<String>,
        /// Imagens/sons de referência (use `usar:caminho` para incluir no jogo)
        #[arg(long)]
        attach: Vec<String>,
        /// Estilo visual da biblioteca (veja `gameflow preset list`)
        #[arg(long)]
        preset: Option<String>,
    },
    /// Gerencia presets de estilo visual (importar, exportar, listar)
    Preset {
        #[command(subcommand)]
        action: PresetCmd,
    },
    /// Mostra as pastas usadas pelo GameFlow
    Paths,
}

#[derive(Subcommand)]
enum PresetCmd {
    /// Lista os presets (embutidos e importados)
    List,
    /// Importa um arquivo .gfstyle.json para a biblioteca
    Import { file: PathBuf },
    /// Exporta um preset para um arquivo .gfstyle.json
    Export {
        id: String,
        /// Arquivo de saída (padrão: ./<id>.gfstyle.json)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Remove um preset importado
    Delete { id: String },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).with_target(false).init();
    let cli = Cli::parse();
    let res = match cli.cmd.unwrap_or(Cmd::Start { port: 7878, no_open: false }) {
        Cmd::Start { port, no_open } => start(port, !no_open).await,
        Cmd::Update { check } => update::run(check).await,
        Cmd::Doctor => doctor().await,
        Cmd::Generate { prompt, name, genre, attach, preset } => generate(prompt, name, genre, attach, preset).await,
        Cmd::Preset { action } => preset_cmd(action),
        Cmd::Paths => {
            let p = config::Paths::discover();
            println!("dados e modelos : {}\nprojetos        : {}\nconfigurações   : {}", p.root.display(), p.projects.display(), p.settings_file.display());
            Ok(())
        }
    };
    if let Err(e) = res {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}

async fn start(port: u16, open: bool) -> anyhow::Result<()> {
    let st = server::build_state(config::Paths::discover())?;
    server::serve(st, "127.0.0.1", port, open).await
}

async fn doctor() -> anyhow::Result<()> {
    let paths = config::Paths::discover();
    let st = server::build_state(paths.clone())?;
    println!("GameFlow {} ({} / {})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    println!("  dados e modelos : {}", paths.root.display());
    println!("  projetos        : {}", paths.projects.display());
    let t = st.rt.tools();
    let show = |o: &Option<String>| o.clone().unwrap_or_else(|| "não encontrado".into());
    println!("\nRuntimes locais:");
    println!("  llama-server (llama.cpp)   : {}", show(&t.llama_server));
    println!("  stable-diffusion.cpp (sd)  : {}", show(&t.sd));
    println!("  node (validação de código) : {}", show(&t.node));
    let s = st.settings.read().unwrap().clone();
    println!("\nAgentes:");
    for (name, a) in [("texto", &s.agents.text), ("código", &s.agents.code), ("imagem", &s.agents.image), ("som", &s.agents.sound)] {
        println!("  {name:<7}: {:?} {}", a.runtime, a.model);
    }
    println!("\nModelos baixados: {}", st.hf.local_models().len());
    let ok = reqwest::Client::new().head("https://huggingface.co").timeout(Duration::from_secs(6)).send().await.map(|r| r.status().as_u16() < 500).unwrap_or(false);
    println!("Hugging Face    : {}", if ok { "acessível" } else { "inacessível (sem internet ou bloqueado)" });
    Ok(())
}

async fn generate(prompt: String, name: Option<String>, genre: Option<String>, attach: Vec<String>, preset: Option<String>) -> anyhow::Result<()> {
    let st = server::build_state(config::Paths::discover())?;
    let genre_enum = genre.as_deref().and_then(spec::Genre::from_id);
    if genre.is_some() && genre_enum.is_none() {
        anyhow::bail!("gênero desconhecido: {} (use: {})", genre.unwrap_or_default(), spec::Genre::ALL.iter().map(|g| g.id()).collect::<Vec<_>>().join(", "));
    }
    let name = name.unwrap_or_else(|| util::first_words(&prompt, 4));
    let meta = st.projects.create(&name, genre_enum)?;
    let mut atts = vec![];
    for a in attach {
        let (mode, path) = match a.strip_prefix("usar:") {
            Some(p) => ("use", p.to_string()),
            None => ("reference", a),
        };
        let src = PathBuf::from(&path);
        let fname = src.file_name().ok_or_else(|| anyhow::anyhow!("anexo inválido: {path}"))?.to_string_lossy().to_string();
        let rel = format!("references/{fname}");
        st.projects.write(&meta.id, &rel, &std::fs::read(&src)?)?;
        atts.push(agents::run::AttachmentReq { path: rel, mode: mode.into() });
    }
    let run = st.orch.start(&meta.id, GenerateRequest { prompt, genre, mode: None, preset, attachments: atts })?;
    let mut next = 0;
    loop {
        for env in run.snapshot(next) {
            next = env.seq + 1;
            match env.event {
                RunEvent::NodeStatus { id, status, detail } if status != "pending" => println!("[{id}] {status}{}", detail.map(|d| format!(" — {d}")).unwrap_or_default()),
                RunEvent::Log { node, level, text } => println!("  {node}: {}{text}", if level == "warn" { "⚠ " } else { "" }),
                RunEvent::Message { from, to, text } => println!("  {from} → {to}: {text}"),
                RunEvent::Done { ok, summary } => {
                    println!("\n{summary}");
                    println!("Projeto: {}", st.paths.projects.join(&meta.id).display());
                    st.rt.shutdown().await;
                    if !ok {
                        std::process::exit(2);
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

fn preset_cmd(action: PresetCmd) -> anyhow::Result<()> {
    let lib = presets::Presets::new(config::Paths::discover().presets);
    match action {
        PresetCmd::List => {
            for p in lib.list() {
                println!("{:<24} {}{}  —  {}", p.id, p.name, if p.builtin { " (embutido)" } else { "" }, p.art_style);
            }
        }
        PresetCmd::Import { file } => {
            let text = std::fs::read_to_string(&file).map_err(|e| anyhow::anyhow!("não consegui ler {}: {e}", file.display()))?;
            let p = lib.import_text(&text)?;
            println!("Importado: {} (id: {})", p.name, p.id);
        }
        PresetCmd::Export { id, out } => {
            let p = lib.get(&id).ok_or_else(|| anyhow::anyhow!("preset \"{id}\" não encontrado (veja `gameflow preset list`)"))?;
            let out = out.unwrap_or_else(|| PathBuf::from(format!("{}{}", p.id, presets::FILE_SUFFIX)));
            std::fs::write(&out, p.to_file_json())?;
            println!("Exportado para {}", out.display());
        }
        PresetCmd::Delete { id } => {
            lib.delete(&id)?;
            println!("Removido: {id}");
        }
    }
    Ok(())
}
