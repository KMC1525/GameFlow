//! Lógica de cada agente. Todos têm um caminho "com IA" (modelo local) e um caminho embutido que
//! sempre funciona — assim o pipeline nunca fica travado por falta de modelo.

use super::graph::{NodeSpec, Outcome, Role};
use super::llm::*;
use super::{Attachment, Ctx, checks};
use crate::runtime::{ChatMsg, Task};
use crate::spec::*;
use crate::util::*;
use crate::{levelgen, procedural, story, templates};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn run(ctx: Arc<Ctx>, node: NodeSpec) -> Result<Outcome> {
    match node.role {
        Role::Director => director(&ctx, &node).await,
        Role::Writer => writer(&ctx, &node).await,
        Role::Architect => architect(&ctx, &node).await,
        Role::Artist => artist(&ctx, &node).await,
        Role::Composer => composer(&ctx, &node).await,
        Role::Engineer => engineer(&ctx, &node).await,
        Role::Integrator => integrator(&ctx, &node).await,
        Role::Tester => tester(&ctx, &node).await,
        Role::Publisher => publisher(&ctx, &node).await,
    }
}

// ───────────────────────── utilidades ─────────────────────────

fn strip_think(t: &str) -> String {
    let mut s = t.to_string();
    while let (Some(a), Some(b)) = (s.find("<think>"), s.find("</think>")) {
        if b < a {
            break;
        }
        s.replace_range(a..b + "</think>".len(), "");
    }
    s
}

/// Pergunta ao modelo configurado. `None` => runtime embutido ou falha (já registrada como aviso).
async fn ask(ctx: &Ctx, node: &str, task: Task, system: String, user: String, max_tokens: u32) -> Option<String> {
    if ctx.rt.is_builtin(task) || ctx.run.cancelled() {
        return None;
    }
    ctx.log(node, "info", format!("Consultando o modelo de {}…", task.label()));
    match ctx.rt.chat(task, &[ChatMsg::system(system), ChatMsg::user(user)], max_tokens).await {
        Ok(Some(t)) => {
            ctx.bb.lock().unwrap().used_ai.insert(task.label().to_string());
            Some(strip_think(&t))
        }
        Ok(None) => None,
        Err(e) => {
            ctx.warn(node, format!("O modelo de {} falhou ({e:#}). Usando o gerador embutido.", task.label()));
            None
        }
    }
}

fn contains_any(prompt: &str, words: &[&str]) -> bool {
    let p = normalize(prompt);
    words.iter().any(|w| p.contains(w))
}

fn wants_narrative(p: &str) -> bool {
    contains_any(p, &["historia", "roteiro", "dialog", "personagem", "npc", "missao", "quest", "final", "texto", "fala", "story", "character", "ending", "text", "enredo", "narrativ"])
}
fn wants_visual(p: &str) -> bool {
    contains_any(p, &["arte", "sprite", "imagem", "cor", "visual", "estilo", "fundo", "cenario", "textura", "art", "image", "color", "colour", "background", "style", "look", "tema", "theme"])
}
fn wants_audio(p: &str) -> bool {
    contains_any(p, &["som", "musica", "trilha", "efeito sonoro", "audio", "sound", "music", "sfx"])
}
fn wants_new_map(p: &str) -> bool {
    contains_any(p, &["novo mapa", "nova fase", "outro mapa", "outra fase", "gerar de novo", "new map", "new level", "regenerate", "novas fases", "mapa diferente"])
}

fn luminance(hex: &str) -> f32 {
    let (r, g, b) = parse_hex(hex);
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

fn sources_path() -> &'static str {
    ".gameflow/asset_sources.json"
}

fn load_sources(ctx: &Ctx) -> BTreeMap<String, String> {
    ctx.read(sources_path()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_sources(ctx: &Ctx, m: &BTreeMap<String, String>) {
    if let Ok(p) = ctx.projects.resolve(&ctx.project_id, sources_path()) {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, serde_json::to_vec_pretty(m).unwrap_or_default());
    }
}

fn find_asset(ctx: &Ctx, sub: &str, id: &str) -> Option<String> {
    let dir = ctx.dir.join(sub);
    for e in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if name.rsplit_once('.').map(|(s, _)| s) == Some(id) {
            return Some(format!("{sub}/{name}"));
        }
    }
    None
}

fn base_id(id: &str) -> String {
    id.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

fn aliases(id: &str) -> &'static [&'static str] {
    match id {
        "player" | "char_player" => &["player", "hero", "heroi", "jogador", "protagonista", "personagem", "character"],
        "npc" => &["npc", "aldeao", "villager", "aldeia"],
        "enemy" => &["enemy", "inimigo", "monster", "monstro", "slime"],
        "boss" => &["boss", "chefe"],
        "item" => &["item", "coin", "moeda", "gem", "gema", "relic", "reliquia", "crystal", "cristal"],
        "bg" => &["bg", "background", "fundo", "cenario", "parallax"],
        "tile" => &["tile", "ground", "chao", "terra"],
        "tile_top" => &["top", "grass", "grama"],
        "tile_wall" | "wall" => &["wall", "parede", "arvore", "tree", "muro"],
        "tile_floor" => &["floor", "piso", "chao"],
        "tile_water" => &["water", "agua"],
        "goal" => &["goal", "flag", "bandeira", "meta"],
        "health" => &["health", "vida", "heart", "coracao"],
        "ammo" => &["ammo", "municao"],
        "char_ally" => &["ally", "aliado", "amigo"],
        "char_rival" => &["rival", "vilao", "villain"],
        "music" => &["music", "musica", "trilha", "bgm", "theme", "tema"],
        "coin" => &["coin", "moeda", "pickup"],
        "jump" => &["jump", "pulo"],
        "hit" => &["hit", "dano", "hurt", "damage"],
        "shoot" => &["shoot", "tiro", "laser", "disparo"],
        "explosion" => &["explosion", "explosao", "boom"],
        "win" => &["win", "vitoria", "victory"],
        "lose" => &["lose", "derrota", "gameover", "game_over"],
        "attack" => &["sword", "espada", "attack", "ataque"],
        "blip" => &["blip", "dialog"],
        _ => &[],
    }
}

fn match_attachment<'a>(ctx: &'a Ctx, id: &str, used: &HashSet<usize>, audio: bool) -> Option<(usize, &'a Attachment)> {
    let base = base_id(id);
    ctx.attachments.iter().enumerate().find(|(i, a)| {
        if used.contains(i) || !a.use_in_game || a.is_audio() != audio || !(a.is_audio() || a.is_image()) {
            return false;
        }
        let s = normalize(&a.stem).replace(['-', ' '], "_");
        s == id || s == base || aliases(id).iter().chain(aliases(&base).iter()).any(|k| s.contains(k))
    })
}

fn file_ext_safe(ext: &str) -> &str {
    match ext {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "wav" | "ogg" | "mp3" | "m4a" => ext,
        _ => "bin",
    }
}

fn design_markdown(spec: &GameSpec) -> String {
    let en = spec.lang == "en";
    let mut s = format!("# {}\n\n_{}_\n\n", spec.title, spec.tagline);
    s.push_str(&format!(
        "- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n- **{}**: {}\n\n",
        if en { "Genre" } else { "Gênero" },
        spec.genre.label(),
        if en { "Setting" } else { "Cenário" },
        spec.setting,
        if en { "Art style" } else { "Estilo de arte" },
        spec.art_style,
        if en { "Difficulty" } else { "Dificuldade" },
        spec.difficulty
    ));
    s.push_str(&format!("## {}\n{} — {}\n\n", if en { "Hero" } else { "Protagonista" }, spec.player.name, spec.player.description));
    if !spec.mechanics.is_empty() {
        s.push_str(&format!("## {}\n", if en { "Mechanics" } else { "Mecânicas" }));
        for m in &spec.mechanics {
            s.push_str(&format!("- {m}\n"));
        }
        s.push('\n');
    }
    if spec.genre != Genre::VisualNovel {
        s.push_str(&format!("## {}\n{}\n\n{}\n\n", if en { "Quest" } else { "Missão" }, spec.quest.title, spec.quest.intro));
        s.push_str(&format!("## {}\n", if en { "Characters" } else { "Personagens" }));
        for n in &spec.npcs {
            s.push_str(&format!("- **{}** ({}): {}\n", n.name, n.role, n.lines.first().cloned().unwrap_or_default()));
        }
        s.push_str(&format!("\n## {}\n", if en { "Enemies" } else { "Inimigos" }));
        for e in &spec.enemies {
            s.push_str(&format!("- {}\n", e.name));
        }
        s.push_str(&format!("- **{}** ({})\n", spec.boss.name, if en { "boss" } else { "chefe" }));
    }
    s
}

// ───────────────────────── Diretor ─────────────────────────

/// Edições sem IA: só o que dá para entender por palavras-chave (tema/cores, título, dificuldade, recursos).
fn apply_simple_edits(spec: &mut GameSpec, prompt: &str, keep_palette: bool) -> Vec<String> {
    let mut changes = vec![];
    if let Some((id, pal, art)) = detect_theme_explicit(prompt) {
        if id != spec.theme {
            spec.theme = id.into();
            spec.art_style = art.into();
            if !keep_palette {
                spec.palette = pal;
            }
            changes.push(format!("tema → {id}"));
        }
    }
    let p = normalize(prompt);
    for (words, diff) in [(&["dificil", "hard", "mais dificil", "harder"][..], "hard"), (&["facil", "easy", "mais facil", "easier", "relaxante"][..], "easy")] {
        if words.iter().any(|w| p.contains(w)) && spec.difficulty != diff {
            spec.difficulty = diff.into();
            changes.push(format!("dificuldade → {diff}"));
        }
    }
    if let Some(start) = prompt.find('"') {
        if let Some(len) = prompt[start + 1..].find('"') {
            let t = prompt[start + 1..start + 1 + len].trim();
            if (2..=48).contains(&t.chars().count()) && t != spec.title {
                spec.title = t.to_string();
                changes.push("novo título".into());
            }
        }
    }
    if has_any(&p, &["pulo duplo", "double jump"]) && !spec.features.iter().any(|f| f == "double_jump") && matches!(spec.genre, Genre::Platformer2d | Genre::Platformer3d) {
        spec.features.push("double_jump".into());
        changes.push("pulo duplo".into());
    }
    changes
}

async fn director(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let prompt = ctx.req.prompt.trim().to_string();
    let hint = ctx.req.genre.as_deref().and_then(Genre::from_id);
    let mut modify = ctx.bb.lock().unwrap().modify;
    let img_paths: Vec<PathBuf> = ctx.attachments.iter().filter(|a| a.decodable()).map(|a| a.abs.clone()).collect();
    let ref_palette = if img_paths.is_empty() { None } else { procedural::palette_from_images(&img_paths, &Palette::default()) };

    let mut spec: GameSpec = if modify {
        match ctx.read("game.json").and_then(|t| serde_json::from_str::<GameSpec>(&t).ok()) {
            Some(s) => s,
            None => {
                ctx.warn(&node.id, "Não consegui ler o game.json atual; vou recriar o jogo do zero.");
                modify = false;
                ctx.bb.lock().unwrap().modify = false;
                heuristic_spec(&prompt, hint, ref_palette.clone())
            }
        }
    } else {
        heuristic_spec(&prompt, hint, ref_palette.clone())
    };
    let original_genre = spec.genre;

    if modify {
        let keep = ref_palette.is_some();
        let changes = apply_simple_edits(&mut spec, &prompt, keep);
        if !changes.is_empty() {
            ctx.log(&node.id, "info", format!("Ajustes automáticos: {}", changes.join(", ")));
        }
    }

    // referências visuais => paleta do jogo
    if let Some(p) = ref_palette {
        spec.palette = p;
        ctx.bb.lock().unwrap().palette_from_refs = true;
        ctx.log(&node.id, "info", format!("Paleta extraída de {} imagem(ns) de referência.", img_paths.len()));
    }

    // refinamento por IA
    let refs_desc = if ctx.attachments.is_empty() {
        String::new()
    } else {
        format!("\nO usuário anexou: {}.", ctx.attachments.iter().map(|a| format!("{} ({})", a.rel, if a.use_in_game { "usar no jogo" } else { "referência" })).collect::<Vec<_>>().join(", "))
    };
    let user = if modify {
        format!(
            "Pedido de alteração do usuário:\n{}\n{}\nJogo atual: \"{}\" ({}), cenário: {}. Mecânicas: {}.\nDevolva APENAS as chaves que devem mudar.",
            prompt,
            refs_desc,
            spec.title,
            spec.genre.label(),
            spec.setting,
            spec.mechanics.join("; ")
        )
    } else {
        format!("Pedido do usuário:\n{}\n{}", prompt, refs_desc)
    };
    if let Some(text) = ask(ctx, &node.id, Task::Text, director_system(spec.genre, &spec.lang), user, 2000).await {
        match extract_json(&text) {
            Some(mut v) if v.is_object() => {
                if ctx.bb.lock().unwrap().palette_from_refs {
                    v.as_object_mut().map(|o| o.remove("palette"));
                }
                spec = merge_patch(&spec, &v);
                ctx.log(&node.id, "info", "O modelo refinou o design do jogo.");
            }
            _ => ctx.warn(&node.id, "O modelo de texto não devolveu um JSON válido; mantive o design embutido."),
        }
    }
    spec.genre = hint.unwrap_or(original_genre);
    fill_defaults(&mut spec);

    let md = design_markdown(&spec);
    ctx.write("docs/design.md", md.as_bytes())?;
    ctx.msg(&node.id, "all", format!("Vamos fazer “{}” — {} ({}). Cenário: {}", spec.title, spec.genre.label(), spec.art_style, spec.setting));
    let detail = format!("{} · {}", spec.genre.label(), spec.theme);
    ctx.bb.lock().unwrap().spec = Some(spec);
    Ok(Outcome { detail: Some(detail), ..Default::default() })
}

// ───────────────────────── Roteirista ─────────────────────────

async fn writer(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let modify = ctx.bb.lock().unwrap().modify;
    if modify && !wants_narrative(&ctx.req.prompt) {
        ctx.log(&node.id, "info", "Nenhuma mudança de história pedida; textos mantidos.");
        return Ok(Outcome { detail: Some("sem mudanças".into()), ..Default::default() });
    }
    if ctx.rt.is_builtin(Task::Text) {
        let detail = if spec.genre == Genre::VisualNovel { format!("{} cenas de roteiro", spec.story.len()) } else { format!("{} NPCs, missão “{}”", spec.npcs.len(), spec.quest.title) };
        ctx.log(&node.id, "info", format!("Textos gerados pelo gerador embutido ({detail}). Configure um modelo de texto para histórias mais ricas."));
        return Ok(Outcome { detail: Some(detail), ..Default::default() });
    }
    let brief = format!(
        "Jogo \"{}\" ({}), cenário: {}. Pedido original: {}\nPersonagem principal: {} — {}.",
        spec.title,
        spec.genre.label(),
        spec.setting,
        spec.prompt,
        spec.player.name,
        spec.player.description
    );
    if spec.genre == Genre::VisualNovel {
        let user = format!("{brief}\nPedido atual: {}", ctx.req.prompt);
        if let Some(text) = ask(ctx, &node.id, Task::Text, vn_writer_system(&spec), user, 4000).await {
            let nodes: Option<Vec<VnNode>> = extract_json(&text).and_then(|v| v.get("story").cloned()).and_then(|s| serde_json::from_value(s).ok());
            match nodes {
                Some(n) if n.len() >= 6 && story::validate(&n).is_empty() => {
                    let count = n.len();
                    ctx.update_spec(|s| s.story = n);
                    ctx.log(&node.id, "info", format!("Roteiro escrito pelo modelo: {count} cenas."));
                }
                Some(n) => ctx.warn(&node.id, format!("O roteiro do modelo tinha problemas ({}); mantive o roteiro embutido.", story::validate(&n).into_iter().take(2).collect::<Vec<_>>().join("; "))),
                None => ctx.warn(&node.id, "O modelo não devolveu um roteiro válido; mantive o roteiro embutido."),
            }
        }
    } else {
        let user = format!("{brief}\nPedido atual: {}\nNPCs atuais: {}", ctx.req.prompt, spec.npcs.iter().map(|n| n.name.as_str()).collect::<Vec<_>>().join(", "));
        if let Some(text) = ask(ctx, &node.id, Task::Text, writer_system(&spec.lang), user, 2500).await {
            match extract_json(&text) {
                Some(v) if v.is_object() => {
                    let mut only = serde_json::Map::new();
                    for k in ["npcs", "quest", "enemies", "boss", "items"] {
                        if let Some(x) = v.get(k) {
                            only.insert(k.into(), x.clone());
                        }
                    }
                    let patch = Value::Object(only);
                    ctx.update_spec(|s| *s = merge_patch(s, &patch));
                    ctx.log(&node.id, "info", "Falas, missão e personagens reescritos pelo modelo.");
                }
                _ => ctx.warn(&node.id, "O modelo não devolveu JSON válido; mantive os textos embutidos."),
            }
        }
    }
    Ok(Outcome { detail: Some("texto pelo modelo".into()), ..Default::default() })
}

// ───────────────────────── Arquiteto ─────────────────────────

async fn architect(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let modify = ctx.bb.lock().unwrap().modify;
    let allowed = allowed_features(spec.genre);
    let mut features: Vec<String> = spec.features.iter().filter(|f| allowed.contains(&f.as_str())).cloned().collect();
    let mut notes = String::new();
    let user = format!("Jogo \"{}\" ({}). Pedido: {}\nMecânicas: {}", spec.title, spec.genre.label(), ctx.req.prompt, spec.mechanics.join("; "));
    if !allowed.is_empty() {
        if let Some(text) = ask(ctx, &node.id, Task::Code, architect_system(spec.genre), user, 600).await {
            if let Some(v) = extract_json(&text) {
                if let Some(arr) = v.get("features").and_then(|f| f.as_array()) {
                    features = arr.iter().filter_map(|x| x.as_str()).filter(|f| allowed.contains(f)).map(String::from).collect();
                    features.dedup();
                }
                notes = v.get("notes").and_then(|n| n.as_str()).unwrap_or("").to_string();
            }
        }
    }
    let regen = !modify || spec.world.is_null() || wants_new_map(&ctx.req.prompt);
    let new_seed = if modify && wants_new_map(&ctx.req.prompt) { spec.seed.wrapping_add(1) } else { spec.seed };
    let world = if regen {
        let mut s = spec.clone();
        s.seed = new_seed;
        let w = levelgen::generate_world(&s);
        ctx.log(
            &node.id,
            "info",
            match spec.genre {
                Genre::VisualNovel => "Gênero narrativo: sem mapas, apenas roteiro.".to_string(),
                Genre::Arcade => "Ondas de inimigos configuradas.".to_string(),
                _ => "Mapas/fases gerados proceduralmente (editáveis em game.json → world).".to_string(),
            },
        );
        w
    } else {
        ctx.log(&node.id, "info", "Mapas existentes mantidos.");
        spec.world.clone()
    };
    ctx.update_spec(|s| {
        s.features = features.clone();
        s.world = world;
        s.seed = new_seed;
    });
    let md = format!(
        "# Arquitetura\n\n- Gênero: **{}**\n- Arquivos: `index.html`, `style.css`, `game.json` (conteúdo editável), `js/gameflow.js` (mini-engine), `js/game.js` (lógica), `js/custom.js` (seus ajustes), `js/data.js` (gerado a partir do game.json).\n- Recursos ativos: {}\n- Pontos de extensão em `js/custom.js`: `GF.on('start'|'update'|'draw'|'event', fn)`.\n\n{}\n",
        spec.genre.label(),
        if features.is_empty() { "nenhum".to_string() } else { features.join(", ") },
        notes
    );
    ctx.write("docs/architecture.md", md.as_bytes())?;
    ctx.msg(&node.id, "engineer", format!("Template: {} · recursos: {}", spec.genre.id(), if features.is_empty() { "—".into() } else { features.join(", ") }));
    Ok(Outcome { detail: Some(format!("{} recursos", features.len())), ..Default::default() })
}

// ───────────────────────── Artista ─────────────────────────

async fn artist(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let (modify, requested) = {
        let mut bb = ctx.bb.lock().unwrap();
        let req = if node.round > 1 { Some(std::mem::take(&mut bb.missing_images)) } else { None };
        (bb.modify, req)
    };
    let has_new_imgs = ctx.attachments.iter().any(|a| a.is_image() && a.use_in_game);
    let regenerate_all = !modify || wants_visual(&ctx.req.prompt) || has_new_imgs;
    let mut sources = load_sources(ctx);
    let reference = ctx.attachments.iter().find(|a| a.decodable()).map(|a| a.abs.clone());
    let use_ai = !ctx.rt.is_builtin(Task::Image);
    let mut ai_ok = use_ai;
    let mut used: HashSet<usize> = HashSet::new();
    let (mut n_user, mut n_ai, mut n_proc) = (0, 0, 0);

    if use_ai {
        ctx.log(&node.id, "info", "Gerando imagens com o modelo de imagem configurado…");
    }
    for req in &spec.assets {
        if ctx.run.cancelled() {
            break;
        }
        let existing = find_asset(ctx, "assets/images", &req.id);
        match &requested {
            Some(r) if !r.contains(&req.id) => continue,
            None => {
                if existing.is_some() && (!regenerate_all || sources.get(&req.id).map(|s| s == "user").unwrap_or(false)) {
                    continue;
                }
            }
            _ => {}
        }
        // 1) arquivo do usuário
        if let Some((idx, att)) = match_attachment(ctx, &req.id, &used, false) {
            let rel = format!("assets/images/{}.{}", req.id, file_ext_safe(&att.ext));
            ctx.write(&rel, &std::fs::read(&att.abs)?)?;
            used.insert(idx);
            sources.insert(req.id.clone(), "user".into());
            n_user += 1;
            continue;
        }
        let rel = format!("assets/images/{}.png", req.id);
        // 2) modelo de imagem
        if ai_ok {
            let style = if matches!(req.kind.as_str(), "sprite" | "item") { ", single object centered on a plain white background, game asset" } else { "" };
            let prompt = format!("{}, {}{}", req.prompt, spec.art_style, style);
            let tmp = ctx.dir.join(".gameflow/tmp").join(format!("{}.png", req.id));
            match ctx.rt.generate_image(&prompt, req.w, req.h, &tmp, reference.as_deref()).await {
                Ok(true) => {
                    if let Ok(mut img) = image::open(&tmp).map(|i| i.to_rgba8()) {
                        if matches!(req.kind.as_str(), "sprite" | "item" | "portrait") {
                            procedural::cutout_background(&mut img);
                        }
                        ctx.write(&rel, &procedural::png_bytes(&img)?)?;
                        sources.insert(req.id.clone(), "ai".into());
                        ctx.bb.lock().unwrap().used_ai.insert("imagem".into());
                        n_ai += 1;
                        let _ = std::fs::remove_file(&tmp);
                        continue;
                    }
                }
                Ok(false) => ai_ok = false,
                Err(e) => {
                    ctx.warn(&node.id, format!("O modelo de imagem falhou ({e:#}). Usando arte procedural para o restante."));
                    ai_ok = false;
                }
            }
        }
        // 3) procedural
        let img = procedural::generate_image(req, &spec.palette, spec.seed);
        ctx.write(&rel, &procedural::png_bytes(&img)?)?;
        // remove versões antigas com outra extensão
        if let Some(old) = existing.filter(|o| o != &rel) {
            let _ = std::fs::remove_file(ctx.dir.join(old));
        }
        sources.insert(req.id.clone(), "procedural".into());
        n_proc += 1;
    }
    // anexos "usar no jogo" que não casaram com nenhum id: ficam disponíveis como user_<nome>
    if requested.is_none() {
        for (i, a) in ctx.attachments.iter().enumerate() {
            if a.use_in_game && a.is_image() && !used.contains(&i) {
                let id = format!("user_{}", slugify(&a.stem).replace('-', "_"));
                ctx.write(&format!("assets/images/{}.{}", id, file_ext_safe(&a.ext)), &std::fs::read(&a.abs)?)?;
                sources.insert(id.clone(), "user".into());
                ctx.log(&node.id, "info", format!("Seu arquivo “{}” foi adicionado como assets/images/{}.", a.rel, id));
                n_user += 1;
            }
        }
    }
    save_sources(ctx, &sources);
    let mut parts = vec![];
    if n_user > 0 {
        parts.push(format!("{n_user} seus"));
    }
    if n_ai > 0 {
        parts.push(format!("{n_ai} por IA"));
    }
    if n_proc > 0 {
        parts.push(format!("{n_proc} procedurais"));
    }
    let detail = if parts.is_empty() { "nada a gerar".to_string() } else { parts.join(" · ") };
    ctx.log(&node.id, "info", format!("Imagens: {detail}."));
    Ok(Outcome { detail: Some(detail), ..Default::default() })
}

// ───────────────────────── Compositor ─────────────────────────

async fn composer(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let (modify, requested) = {
        let mut bb = ctx.bb.lock().unwrap();
        let req = if node.round > 1 { Some(std::mem::take(&mut bb.missing_sounds)) } else { None };
        (bb.modify, req)
    };
    let has_new = ctx.attachments.iter().any(|a| a.is_audio() && a.use_in_game);
    let regenerate_all = !modify || wants_audio(&ctx.req.prompt) || has_new;
    let mut sources = load_sources(ctx);
    let mut ai_ok = !ctx.rt.is_builtin(Task::Sound);
    let dark = luminance(&spec.palette.bg) < 0.25;
    let mut used: HashSet<usize> = HashSet::new();
    let (mut n_user, mut n_ai, mut n_proc) = (0, 0, 0);
    for req in &spec.sounds {
        if ctx.run.cancelled() {
            break;
        }
        let key = format!("snd:{}", req.id);
        let existing = find_asset(ctx, "assets/audio", &req.id);
        match &requested {
            Some(r) if !r.contains(&req.id) => continue,
            None => {
                if existing.is_some() && (!regenerate_all || sources.get(&key).map(|s| s == "user").unwrap_or(false)) {
                    continue;
                }
            }
            _ => {}
        }
        if let Some((idx, att)) = match_attachment(ctx, &req.id, &used, true) {
            ctx.write(&format!("assets/audio/{}.{}", req.id, file_ext_safe(&att.ext)), &std::fs::read(&att.abs)?)?;
            used.insert(idx);
            sources.insert(key, "user".into());
            n_user += 1;
            continue;
        }
        let rel = format!("assets/audio/{}.wav", req.id);
        if ai_ok {
            let tmp = ctx.dir.join(".gameflow/tmp").join(format!("{}.wav", req.id));
            match ctx.rt.generate_sound(&format!("{}, {}", req.prompt, spec.setting), &req.kind, &tmp).await {
                Ok(true) => {
                    ctx.write(&rel, &std::fs::read(&tmp)?)?;
                    let _ = std::fs::remove_file(&tmp);
                    sources.insert(key, "ai".into());
                    ctx.bb.lock().unwrap().used_ai.insert("som".into());
                    n_ai += 1;
                    continue;
                }
                Ok(false) => ai_ok = false,
                Err(e) => {
                    ctx.warn(&node.id, format!("O modelo de som falhou ({e:#}). Usando sons procedurais para o restante."));
                    ai_ok = false;
                }
            }
        }
        let bytes = if req.kind == "music" { procedural::synth_music(spec.seed, dark) } else { procedural::synth_sfx(&req.id, &req.prompt, spec.seed) };
        ctx.write(&rel, &bytes)?;
        if let Some(old) = existing.filter(|o| o != &rel) {
            let _ = std::fs::remove_file(ctx.dir.join(old));
        }
        sources.insert(key, "procedural".into());
        n_proc += 1;
    }
    save_sources(ctx, &sources);
    let mut parts = vec![];
    if n_user > 0 {
        parts.push(format!("{n_user} seus"));
    }
    if n_ai > 0 {
        parts.push(format!("{n_ai} por IA"));
    }
    if n_proc > 0 {
        parts.push(format!("{n_proc} procedurais"));
    }
    let detail = if parts.is_empty() { "nada a gerar".to_string() } else { parts.join(" · ") };
    ctx.log(&node.id, "info", format!("Sons: {detail}."));
    Ok(Outcome { detail: Some(detail), ..Default::default() })
}

// ───────────────────────── Engenheiro ─────────────────────────

fn backup(ctx: &Ctx, rel: &str) {
    if let (Ok(src), Some(content)) = (ctx.projects.resolve(&ctx.project_id, rel), ctx.read(rel)) {
        let _ = src;
        let dst = ctx.dir.join(".gameflow/backups").join(&ctx.run.id).join(rel);
        if let Some(d) = dst.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(dst, content);
    }
}

/// Restaura um arquivo problemático: backup desta execução → template original → stub.
fn restore_file(ctx: &Ctx, rel: &str, spec: &GameSpec) -> bool {
    let bak = ctx.dir.join(".gameflow/backups").join(&ctx.run.id).join(rel);
    if let Ok(t) = std::fs::read_to_string(&bak) {
        return ctx.write(rel, t.as_bytes()).is_ok();
    }
    if rel == "game.json" {
        return templates::write_game_json(spec, &ctx.dir).is_ok();
    }
    if let Some(raw) = templates::pristine(spec.genre, rel) {
        let plan = templates::plan(spec.genre);
        let text = if rel.ends_with(".js") { raw } else { templates::render(&raw, spec, &plan.scripts) };
        return ctx.write(rel, text.as_bytes()).is_ok();
    }
    false
}

async fn engineer(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let modify = ctx.bb.lock().unwrap().modify;
    let prompt = ctx.req.prompt.clone();

    if node.round == 1 {
        if !modify {
            let files = templates::materialize(&spec, &ctx.dir, false)?;
            for f in &files {
                ctx.run.emit(super::run::RunEvent::File { path: f.clone(), action: "created".into() });
                ctx.bb.lock().unwrap().files.insert(f.clone());
            }
            ctx.log(&node.id, "info", format!("Template “{}” instanciado ({} arquivos).", spec.genre.id(), files.len()));
        } else {
            let before: Option<GameSpec> = ctx.read("game.json").and_then(|t| serde_json::from_str(&t).ok());
            let missing = templates::materialize(&spec, &ctx.dir, true)?;
            let shell_changed = before.as_ref().map(|b| b.title != spec.title || serde_json::to_string(&b.palette).ok() != serde_json::to_string(&spec.palette).ok()).unwrap_or(true);
            if shell_changed {
                backup(ctx, "index.html");
                backup(ctx, "style.css");
                templates::refresh_shell(&spec, &ctx.dir)?;
                ctx.log(&node.id, "info", "Título/cores mudaram: index.html e style.css foram atualizados (cópia em .gameflow/backups).");
            }
            ctx.run.emit(super::run::RunEvent::File { path: "game.json".into(), action: "modified".into() });
            ctx.log(&node.id, "info", format!("Projeto existente atualizado ({} arquivos de base recriados).", missing.len().saturating_sub(1)));
        }
    } else {
        let issues = ctx.bb.lock().unwrap().issues.clone();
        let mut fixed = HashSet::new();
        for i in &issues {
            if let Some((f, _)) = i.split_once('|') {
                if !f.is_empty() && fixed.insert(f.to_string()) && restore_file(ctx, f, &spec) {
                    ctx.log(&node.id, "info", format!("Restaurei {f} para a versão estável."));
                }
            }
        }
    }

    // ── IA de código: ajustes/mecânicas pedidos no prompt ──
    let builtin = ctx.rt.is_builtin(Task::Code);
    if builtin {
        if modify && node.round == 1 {
            ctx.log(
                &node.id,
                "info",
                "Sem modelo de código configurado: apliquei só ajustes automáticos (título, cores, dificuldade). Para mudar mecânicas livremente, escolha um modelo em Configurações → Agentes.",
            );
        }
        return Ok(Outcome { detail: Some("template".into()), ..Default::default() });
    }
    let issues = ctx.bb.lock().unwrap().issues.clone();
    let mut ctx_files: Vec<(String, String)> = vec![("js/custom.js".into(), ctx.read("js/custom.js").unwrap_or_default())];
    if modify || node.round > 1 {
        if let Some(g) = ctx.read("js/game.js") {
            ctx_files.push(("js/game.js".into(), g));
        }
    }
    let mut user = format!("Pedido do usuário: {}\n\n{}", prompt, engineer_context(&spec, &ctx_files, &issues));
    let mut written: Vec<String> = vec![];
    for attempt in 0..2 {
        let Some(text) = ask(ctx, &node.id, Task::Code, ENGINEER_SYSTEM.to_string(), user.clone(), 6000).await else { break };
        let blocks = parse_file_blocks(&text);
        if blocks.is_empty() {
            ctx.warn(&node.id, "O modelo de código não devolveu arquivos no formato esperado.");
            break;
        }
        let mut errors: Vec<String> = vec![];
        for (path, content) in blocks {
            if !allowed_write_path(&path) || content.len() > 300_000 {
                ctx.warn(&node.id, format!("Ignorei a escrita em “{path}” (caminho não permitido)."));
                continue;
            }
            if path.ends_with(".js") {
                if let Err(e) = checks::js_balanced(&content) {
                    errors.push(format!("{path}: {e}"));
                    continue;
                }
            }
            if path == "game.json" {
                match serde_json::from_str::<GameSpec>(&content) {
                    Ok(g) if g.genre == spec.genre && !g.world.is_null() || g.genre == Genre::VisualNovel => {}
                    _ => {
                        errors.push("game.json: JSON inválido ou sem o campo world".into());
                        continue;
                    }
                }
            }
            backup(ctx, &path);
            ctx.write(&path, content.as_bytes())?;
            ctx.bb.lock().unwrap().llm_files.insert(path.clone());
            written.push(path);
        }
        if errors.is_empty() || attempt == 1 {
            for e in errors {
                ctx.warn(&node.id, format!("Descartei código com erro de sintaxe — {e}"));
            }
            break;
        }
        ctx.log(&node.id, "warn", format!("Código com erro de sintaxe ({}); pedindo correção ao modelo…", errors.join("; ")));
        user = format!("{}\n\nSua resposta anterior tinha erros de sintaxe: {}\nCorrija e devolva os arquivos completos novamente.", user, errors.join("; "));
    }
    let detail = if written.is_empty() { "template".to_string() } else { format!("IA: {}", written.join(", ")) };
    if !written.is_empty() {
        ctx.log(&node.id, "info", format!("Arquivos escritos pelo modelo: {}.", written.join(", ")));
    }
    Ok(Outcome { detail: Some(detail), ..Default::default() })
}

// ───────────────────────── Integrador ─────────────────────────

fn list_dir(ctx: &Ctx, sub: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(ctx.dir.join(sub)) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || !e.path().is_file() {
                continue;
            }
            if let Some((stem, _)) = name.rsplit_once('.') {
                out.push((stem.to_string(), format!("{sub}/{name}")));
            }
        }
    }
    out.sort();
    out
}

pub fn write_manifest(ctx: &Ctx, spec: &GameSpec) -> Result<(Vec<String>, Vec<String>)> {
    let imgs = list_dir(ctx, "assets/images");
    let auds = list_dir(ctx, "assets/audio");
    let kinds: BTreeMap<&str, &str> = spec.sounds.iter().map(|s| (s.id.as_str(), s.kind.as_str())).collect();
    let images: BTreeMap<&str, &str> = imgs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let audio: BTreeMap<&str, Value> = auds.iter().map(|(k, v)| (k.as_str(), json!({"path": v, "kind": kinds.get(k.as_str()).copied().unwrap_or("sfx")}))).collect();
    let man = json!({"images": images, "audio": audio});
    ctx.write("assets/manifest.json", serde_json::to_string_pretty(&man)?.as_bytes())?;
    let missing_i: Vec<String> = spec.assets.iter().filter(|a| !images.contains_key(a.id.as_str())).map(|a| a.id.clone()).collect();
    let missing_s: Vec<String> = spec.sounds.iter().filter(|a| !audio.contains_key(a.id.as_str())).map(|a| a.id.clone()).collect();
    Ok((missing_i, missing_s))
}

async fn integrator(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let (missing_i, missing_s) = write_manifest(ctx, &spec)?;
    // garante que todos os scripts do gênero estão no index.html
    if let Some(mut html) = ctx.read("index.html") {
        let plan = templates::plan(spec.genre);
        let mut changed = false;
        for s in &plan.scripts {
            if ctx.exists(s) && !html.contains(&format!("src=\"{s}\"")) {
                let tag = format!("  <script src=\"{s}\"></script>\n");
                match html.rfind("</body>") {
                    Some(i) => html.insert_str(i, &tag),
                    None => html.push_str(&tag),
                }
                changed = true;
                ctx.log(&node.id, "info", format!("Adicionei {s} ao index.html."));
            }
        }
        if changed {
            ctx.write("index.html", html.as_bytes())?;
        }
    }
    let mut out = Outcome::default();
    let already = ctx.bb.lock().unwrap().asset_requests;
    if (!missing_i.is_empty() || !missing_s.is_empty()) && already < 1 && !ctx.run.cancelled() {
        ctx.bb.lock().unwrap().asset_requests += 1;
        let mut deps = vec![];
        {
            let mut bb = ctx.bb.lock().unwrap();
            bb.missing_images = missing_i.clone();
            bb.missing_sounds = missing_s.clone();
        }
        if !missing_i.is_empty() {
            let id = ctx.next_id("artist");
            ctx.msg(&node.id, &id, format!("Preciso destas imagens: {}", missing_i.join(", ")));
            out.spawn.push(NodeSpec { id: id.clone(), role: Role::Artist, deps: vec![node.id.clone()], round: 2 });
            deps.push(id);
        }
        if !missing_s.is_empty() {
            let id = ctx.next_id("composer");
            ctx.msg(&node.id, &id, format!("Preciso destes sons: {}", missing_s.join(", ")));
            out.spawn.push(NodeSpec { id: id.clone(), role: Role::Composer, deps: vec![node.id.clone()], round: 2 });
            deps.push(id);
        }
        let again = ctx.next_id("integrator");
        out.spawn.push(NodeSpec { id: again.clone(), role: Role::Integrator, deps, round: node.round + 1 });
        out.redirect.push((node.id.clone(), vec![again]));
        out.detail = Some(format!("faltam {} assets", missing_i.len() + missing_s.len()));
        return Ok(out);
    }
    if !missing_i.is_empty() || !missing_s.is_empty() {
        ctx.warn(&node.id, format!("Ainda faltam assets: {}. O jogo usa formas simples no lugar.", missing_i.iter().chain(missing_s.iter()).cloned().collect::<Vec<_>>().join(", ")));
    }
    templates::write_data_js(&ctx.dir)?;
    ctx.run.emit(super::run::RunEvent::File { path: "js/data.js".into(), action: "modified".into() });
    out.detail = Some("manifest + data.js".into());
    Ok(out)
}

// ───────────────────────── Testador ─────────────────────────

async fn tester(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let mut issues: Vec<String> = vec![];

    match ctx.read("index.html") {
        None => issues.push("index.html|arquivo ausente".into()),
        Some(html) => {
            if !html.contains("<canvas") {
                issues.push("index.html|não há elemento <canvas>".into());
            }
            for r in checks::html_local_refs(&html) {
                if !ctx.exists(&r) {
                    issues.push(format!("index.html|referência a “{r}” que não existe"));
                }
            }
        }
    }
    match ctx.read("game.json").map(|t| serde_json::from_str::<GameSpec>(&t)) {
        None => issues.push("game.json|arquivo ausente".into()),
        Some(Err(e)) => issues.push(format!("game.json|JSON inválido: {e}")),
        Some(Ok(g)) => {
            if g.genre == Genre::VisualNovel {
                for p in story::validate(&g.story) {
                    issues.push(format!("game.json|{p}"));
                }
            }
            if let Some(m) = world_problem(&g) {
                issues.push(format!("game.json|{m}"));
            }
        }
    }
    if let Some(m) = ctx.read("assets/manifest.json") {
        match serde_json::from_str::<Value>(&m) {
            Ok(v) => {
                for sect in ["images", "audio"] {
                    if let Some(o) = v.get(sect).and_then(|x| x.as_object()) {
                        for (_, val) in o {
                            let p = val.as_str().or_else(|| val.get("path").and_then(|p| p.as_str())).unwrap_or("");
                            if !ctx.exists(p) {
                                issues.push(format!("assets/manifest.json|“{p}” não existe"));
                            }
                        }
                    }
                }
            }
            Err(e) => issues.push(format!("assets/manifest.json|JSON inválido: {e}")),
        }
    } else {
        issues.push("assets/manifest.json|manifesto ausente".into());
    }
    // sintaxe dos scripts
    let node_bin = ctx.rt.tools().node;
    let mut used_node = false;
    if let Ok(rd) = std::fs::read_dir(ctx.dir.join("js")) {
        let mut files: Vec<_> = rd.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|x| x == "js").unwrap_or(false)).collect();
        files.sort_by_key(|e| e.file_name());
        for e in files {
            let name = format!("js/{}", e.file_name().to_string_lossy());
            let res = match &node_bin {
                Some(n) => match checks::node_check(n, &e.path()).await {
                    Some(r) => {
                        used_node = true;
                        r
                    }
                    None => std::fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|s| checks::js_balanced(&s)),
                },
                None => std::fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|s| checks::js_balanced(&s)),
            };
            if let Err(msg) = res {
                issues.push(format!("{name}|erro de sintaxe: {msg}"));
            }
        }
    } else {
        issues.push("js|pasta js ausente".into());
    }

    let method = if used_node { "node --check" } else { "verificação estática" };
    if issues.is_empty() {
        ctx.bb.lock().unwrap().issues.clear();
        ctx.log(&node.id, "info", format!("Tudo certo ({method}): HTML, game.json, manifesto e scripts válidos."));
        return Ok(Outcome { detail: Some("sem problemas".into()), ..Default::default() });
    }
    for i in &issues {
        ctx.log(&node.id, "warn", i.replace('|', ": "));
    }
    let max = ctx.settings.max_review_rounds;
    let rework = ctx.bb.lock().unwrap().rework;
    ctx.bb.lock().unwrap().issues = issues.clone();
    if rework < max && !ctx.run.cancelled() {
        ctx.bb.lock().unwrap().rework += 1;
        let eng = ctx.next_id("engineer");
        let integ = ctx.next_id("integrator");
        let test = ctx.next_id("tester");
        ctx.msg(&node.id, &eng, format!("Encontrei {} problema(s); por favor corrija: {}", issues.len(), issues.iter().take(3).map(|i| i.replace('|', ": ")).collect::<Vec<_>>().join(" • ")));
        let r = node.round + 1;
        let mut out = Outcome { detail: Some(format!("{} problema(s) → retrabalho", issues.len())), ..Default::default() };
        out.spawn.push(NodeSpec { id: eng.clone(), role: Role::Engineer, deps: vec![node.id.clone()], round: r });
        out.spawn.push(NodeSpec { id: integ.clone(), role: Role::Integrator, deps: vec![eng], round: r });
        out.spawn.push(NodeSpec { id: test.clone(), role: Role::Tester, deps: vec![integ], round: r });
        out.redirect.push((node.id.clone(), vec![test]));
        return Ok(out);
    }
    // sem mais rodadas: isola o código opcional problemático para o jogo base continuar jogável
    let mut quarantined = vec![];
    for i in &issues {
        if let Some((f, _)) = i.split_once('|') {
            if f == "js/custom.js" || ctx.bb.lock().unwrap().llm_files.contains(f) {
                if restore_file(ctx, f, &spec) {
                    quarantined.push(f.to_string());
                }
            }
        }
    }
    if !quarantined.is_empty() {
        ctx.warn(&node.id, format!("Não consegui corrigir {}; restaurei a versão estável.", quarantined.join(", ")));
    }
    ctx.warn(&node.id, format!("{} problema(s) restante(s) após {} rodada(s) de correção.", issues.len(), rework));
    Ok(Outcome { detail: Some(format!("{} aviso(s)", issues.len())), ..Default::default() })
}

fn world_problem(g: &GameSpec) -> Option<String> {
    let rows = |v: &Value| -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|r| r.as_str().map(String::from)).collect()).unwrap_or_default() };
    match g.genre {
        Genre::Platformer2d => {
            let lv = g.world.get("levels")?.as_array()?;
            if lv.is_empty() {
                return Some("world.levels está vazio".into());
            }
            for (i, l) in lv.iter().enumerate() {
                let r = rows(&l["rows"]);
                let all: String = r.concat();
                if !all.contains('P') || !all.contains('G') {
                    return Some(format!("a fase {} precisa de um 'P' (início) e um 'G' (bandeira)", i + 1));
                }
            }
            None
        }
        Genre::Rpg2d | Genre::Rpg3d | Genre::Fps => {
            let r = rows(&g.world["rows"]);
            if r.is_empty() || !r.concat().contains('P') { Some("o mapa (world.rows) precisa de um 'P' para o início do jogador".into()) } else { None }
        }
        Genre::Platformer3d => g.world.get("platforms").and_then(|p| p.as_array()).filter(|p| p.len() >= 2).map(|_| None).unwrap_or(Some("world.platforms precisa de ao menos 2 plataformas".into())),
        _ => None,
    }
}

// ───────────────────────── Publicador ─────────────────────────

async fn publisher(ctx: &Ctx, node: &NodeSpec) -> Result<Outcome> {
    let spec = ctx.spec();
    let en = spec.lang == "en";
    let controls_key = match spec.genre {
        Genre::Rpg2d => "controls_rpg",
        Genre::Rpg3d => "controls_rpg3d",
        Genre::VisualNovel => "controls_vn",
        Genre::Platformer2d | Genre::Platformer3d => "controls_platformer",
        Genre::Fps => "controls_fps",
        Genre::Tps => "controls_tps",
        Genre::Arcade => "controls_arcade",
    };
    let controls = spec.ui.get(controls_key).cloned().unwrap_or_default();
    let readme = if en {
        format!(
            "# {t}\n\n_{tag}_\n\nA {g} game made with **GameFlow**.\n\n## How to play\nOpen `index.html` in any modern browser.\n\n**Controls:** {c}\n\n## Customizing\n- `game.json`: story, characters, colors and maps (plain text — edit freely).\n- `js/custom.js`: your own tweaks using `GF.on('update', ...)`.\n- `assets/`: replace images/sounds keeping the same file names.\n",
            t = spec.title,
            tag = spec.tagline,
            g = spec.genre.label(),
            c = controls
        )
    } else {
        format!(
            "# {t}\n\n_{tag}_\n\nUm jogo de {g} criado com o **GameFlow**.\n\n## Como jogar\nAbra o `index.html` em qualquer navegador moderno.\n\n**Controles:** {c}\n\n## Personalizando\n- `game.json`: história, personagens, cores e mapas (texto simples — edite à vontade).\n- `js/custom.js`: seus próprios ajustes usando `GF.on('update', ...)`.\n- `assets/`: troque imagens/sons mantendo os mesmos nomes de arquivo.\n",
            t = spec.title,
            tag = spec.tagline,
            g = spec.genre.label(),
            c = controls
        )
    };
    ctx.write("README.md", readme.as_bytes())?;
    templates::write_data_js(&ctx.dir)?;

    let mut meta = ctx.projects.get(&ctx.project_id).unwrap_or_default();
    if meta.name.trim().is_empty() || meta.name == "Novo jogo" || meta.name == meta.id {
        meta.name = spec.title.clone();
    }
    meta.genre = Some(spec.genre);
    if !ctx.bb.lock().unwrap().modify {
        meta.prompt = ctx.req.prompt.clone();
    }
    meta.generated = true;
    meta.updated_at = now_iso();
    ctx.projects.save_meta(&meta)?;

    let log = {
        let bb = ctx.bb.lock().unwrap();
        json!({"run": ctx.run.id, "prompt": ctx.req.prompt, "mode": ctx.req.mode, "files": bb.files, "warnings": bb.warnings, "ai": bb.used_ai, "finished": now_iso()})
    };
    ctx.write(&format!(".gameflow/runs/{}.json", ctx.run.id), serde_json::to_string_pretty(&log)?.as_bytes())?;
    ctx.msg(&node.id, "all", "Pronto! Abra a aba ▶ Jogar para testar.");
    Ok(Outcome { detail: Some("projeto salvo".into()), ..Default::default() })
}

/// Texto final mostrado no chat.
pub fn final_summary(ctx: &Ctx, ok: bool) -> String {
    let spec = ctx.spec();
    let bb = ctx.bb.lock().unwrap();
    if ctx.run.cancelled() {
        return "Geração cancelada.".into();
    }
    if !ok {
        return format!("Não consegui concluir a geração{}. Veja o registro dos agentes para detalhes.", if bb.warnings.is_empty() { String::new() } else { format!(" ({})", bb.warnings.join("; ")) });
    }
    let n = bb.files.iter().filter(|f| !f.starts_with(".gameflow")).count();
    let ai = if bb.used_ai.is_empty() { "gerador embutido (sem IA)".to_string() } else { format!("IA local: {}", bb.used_ai.iter().cloned().collect::<Vec<_>>().join(", ")) };
    let genre_part = if bb.modify { String::new() } else { format!(" ({})", spec.genre.label()) };
    let mut s = format!("{} “{}”{} — {} arquivo(s) escrito(s) · {}.", if bb.modify { "Atualizei" } else { "Criei" }, spec.title, genre_part, n, ai);
    if !bb.warnings.is_empty() {
        s.push_str(&format!("\n⚠ {} aviso(s): {}", bb.warnings.len(), bb.warnings.iter().take(3).cloned().collect::<Vec<_>>().join(" • ")));
    }
    s.push_str("\nClique em ▶ Jogar para testar.");
    s
}
