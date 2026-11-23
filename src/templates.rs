//! Templates de jogos (HTML/CSS/JS puros) embutidos no binário, e a rotina que os instancia num projeto.

use crate::spec::{GameSpec, Genre};
use anyhow::{Result, anyhow};
use rust_embed::RustEmbed;
use std::path::Path;

#[derive(RustEmbed)]
#[folder = "templates/"]
struct Templates;

/// Ordem dos scripts no `index.html` (o `custom.js` do usuário/IA sempre vem por último).
pub struct Plan {
    /// (origem no template, destino no projeto)
    pub files: Vec<(&'static str, &'static str)>,
    pub scripts: Vec<&'static str>,
}

pub fn plan(genre: Genre) -> Plan {
    let mut files = vec![("common/index.html", "index.html"), ("common/style.css", "style.css"), ("common/js/gameflow.js", "js/gameflow.js"), ("common/js/custom.js", "js/custom.js")];
    let mut scripts = vec!["js/data.js", "js/gameflow.js"];
    let add = |src: &'static str, dst: &'static str, files: &mut Vec<(&'static str, &'static str)>, scripts: &mut Vec<&'static str>| {
        files.push((src, dst));
        scripts.push(dst);
    };
    match genre {
        Genre::Rpg2d => {
            add("rpg2d/js/render.js", "js/render.js", &mut files, &mut scripts);
            add("rpg/js/game.js", "js/game.js", &mut files, &mut scripts);
        }
        Genre::Rpg3d => {
            add("3d/js/engine3d.js", "js/engine3d.js", &mut files, &mut scripts);
            add("rpg3d/js/render.js", "js/render.js", &mut files, &mut scripts);
            add("rpg/js/game.js", "js/game.js", &mut files, &mut scripts);
        }
        Genre::VisualNovel => add("vn/js/game.js", "js/game.js", &mut files, &mut scripts),
        Genre::Platformer2d => add("platformer2d/js/game.js", "js/game.js", &mut files, &mut scripts),
        Genre::Platformer3d => {
            add("3d/js/engine3d.js", "js/engine3d.js", &mut files, &mut scripts);
            add("platformer3d/js/game.js", "js/game.js", &mut files, &mut scripts);
        }
        Genre::Fps => add("fps/js/game.js", "js/game.js", &mut files, &mut scripts),
        Genre::Tps => {
            add("3d/js/engine3d.js", "js/engine3d.js", &mut files, &mut scripts);
            add("tps/js/game.js", "js/game.js", &mut files, &mut scripts);
        }
        Genre::Arcade => add("arcade/js/game.js", "js/game.js", &mut files, &mut scripts),
    }
    scripts.push("js/custom.js");
    Plan { files, scripts }
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn read_template(path: &str) -> Result<String> {
    let f = Templates::get(path).ok_or_else(|| anyhow!("template ausente: {path}"))?;
    Ok(String::from_utf8_lossy(&f.data).to_string())
}

/// Texto original (sem personalização) de um arquivo de template, para restaurar arquivos quebrados.
pub fn pristine(genre: Genre, dest: &str) -> Option<String> {
    let p = plan(genre);
    let (src, _) = p.files.iter().find(|(_, d)| *d == dest)?;
    let raw = read_template(src).ok()?;
    Some(raw)
}

pub fn render(raw: &str, spec: &GameSpec, scripts: &[&str]) -> String {
    let tags: String = scripts.iter().map(|s| format!("  <script src=\"{s}\"></script>")).collect::<Vec<_>>().join("\n");
    let p = &spec.palette;
    raw.replace("{{TITLE}}", &html_escape(&spec.title))
        .replace("{{TAGLINE}}", &html_escape(&spec.tagline))
        .replace("{{LANG}}", if spec.lang == "en" { "en" } else { "pt-BR" })
        .replace("{{SCRIPTS}}", &tags)
        .replace("{{BG}}", &p.bg)
        .replace("{{BG2}}", &p.bg2)
        .replace("{{GROUND}}", &p.ground)
        .replace("{{PRIMARY}}", &p.primary)
        .replace("{{SECONDARY}}", &p.secondary)
        .replace("{{ACCENT}}", &p.accent)
        .replace("{{DANGER}}", &p.danger)
        .replace("{{TEXT}}", &p.text)
}

/// Escreve os arquivos do template do gênero + `game.json`. Retorna os caminhos escritos.
/// Com `only_missing`, preserva arquivos existentes (não sobrescreve edições do usuário).
pub fn materialize(spec: &GameSpec, dir: &Path, only_missing: bool) -> Result<Vec<String>> {
    let plan = plan(spec.genre);
    let mut written = vec![];
    for (src, dst) in &plan.files {
        let target = dir.join(dst);
        if only_missing && target.exists() {
            continue;
        }
        let raw = read_template(src)?;
        let text = if dst.ends_with(".js") && !dst.ends_with("custom.js") { raw } else { render(&raw, spec, &plan.scripts) };
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&target, text)?;
        written.push(dst.to_string());
    }
    write_game_json(spec, dir)?;
    written.push("game.json".into());
    write_data_js(dir)?;
    written.push("js/data.js".into());
    Ok(written)
}

pub fn write_game_json(spec: &GameSpec, dir: &Path) -> Result<()> {
    std::fs::write(dir.join("game.json"), serde_json::to_vec_pretty(spec)?)?;
    Ok(())
}

/// Gera `js/data.js` a partir de `game.json` + `assets/manifest.json`, para o jogo funcionar abrindo o
/// `index.html` direto do disco (`file://`), onde navegadores bloqueiam `fetch` de arquivos locais.
pub fn write_data_js(dir: &Path) -> Result<()> {
    let game: serde_json::Value = std::fs::read_to_string(dir.join("game.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
    let manifest: serde_json::Value =
        std::fs::read_to_string(dir.join("assets/manifest.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({"images": {}, "audio": {}}));
    // "</script" dentro de strings não pode aparecer em <script>; aqui é arquivo externo, mas escapamos por segurança.
    let esc = |v: &serde_json::Value| serde_json::to_string(v).unwrap_or_default().replace("</", "<\\/");
    let js = format!(
        "// Gerado automaticamente pelo GameFlow a partir de game.json e assets/manifest.json — não edite aqui.\nwindow.GAME_DATA = {};\nwindow.GAME_MANIFEST = {};\n",
        esc(&game),
        esc(&manifest)
    );
    if let Some(p) = dir.join("js/data.js").parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(dir.join("js/data.js"), js)?;
    Ok(())
}

/// Reescreve `index.html` e `style.css` (cores/título) mantendo os scripts atuais do gênero.
pub fn refresh_shell(spec: &GameSpec, dir: &Path) -> Result<()> {
    let plan = plan(spec.genre);
    for (src, dst) in plan.files.iter().filter(|(_, d)| *d == "index.html" || *d == "style.css") {
        std::fs::write(dir.join(dst), render(&read_template(src)?, spec, &plan.scripts))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::heuristic_spec;

    #[test]
    fn every_genre_materializes_all_planned_files() {
        for g in Genre::ALL {
            let spec = heuristic_spec("jogo \"Teste <b>\"", Some(g), None);
            let dir = std::env::temp_dir().join(format!("gf-tpl-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let files = materialize(&spec, &dir, false).unwrap();
            assert!(files.contains(&"index.html".to_string()), "{g:?}");
            let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
            assert!(!html.contains("{{"), "placeholder sobrando em {g:?}");
            assert!(html.contains("js/game.js"));
            assert!(html.contains("js/custom.js"));
            assert!(!html.contains("<b>"), "título não foi escapado");
            for (_, dst) in plan(g).files {
                assert!(dir.join(dst).is_file(), "{dst} ausente em {g:?}");
            }
            std::fs::remove_dir_all(dir).ok();
        }
    }
}
