//! Prompts dos agentes e parsing das respostas dos modelos (que raramente seguem o formato à risca).

use crate::spec::{GameSpec, Genre};

pub fn director_system(genre: Genre, lang: &str) -> String {
    format!(
        r##"Você é o Diretor de Jogo de um estúdio indie. Transforme o pedido do usuário em um documento de design.
Responda SOMENTE com um objeto JSON válido (sem explicações, sem markdown), usando estas chaves (todas opcionais; omita o que não souber):
{{
 "title": "nome curto e marcante",
 "tagline": "uma frase de efeito",
 "setting": "o mundo do jogo em uma frase",
 "art_style": "estilo visual, ex.: pixel art sombria",
 "difficulty": "easy|normal|hard",
 "palette": {{"bg":"#rrggbb","bg2":"#rrggbb","ground":"#rrggbb","primary":"#rrggbb","secondary":"#rrggbb","accent":"#rrggbb","danger":"#rrggbb","text":"#rrggbb"}},
 "player": {{"name":"","description":""}},
 "npcs": [{{"name":"","role":"quest|villager","lines":["fala 1","fala 2"]}}],
 "enemies": [{{"name":"","description":""}}],
 "boss": {{"name":"","description":""}},
 "items": [{{"name":"","description":""}}],
 "places": ["lugar 1","lugar 2","lugar 3"],
 "quest": {{"title":"","intro":"","goal":"","win_text":"","lose_text":""}},
 "mechanics": ["mecânica 1","mecânica 2"]
}}
Gênero (já decidido, não mude): {}. Escreva todos os textos em: {}. Use cores harmoniosas com bom contraste."##,
        genre.label(),
        if lang == "en" { "inglês" } else { "português do Brasil" }
    )
}

pub fn writer_system(lang: &str) -> String {
    format!(
        r##"Você é o Roteirista de um jogo. Melhore os textos do jogo mantendo coerência com o tema.
Responda SOMENTE com JSON válido (sem markdown) com estas chaves:
{{
 "npcs": [{{"name":"","role":"quest|villager","lines":["2 a 4 falas curtas"]}}],
 "quest": {{"title":"","intro":"","goal":"","win_text":"","lose_text":""}},
 "enemies": [{{"name":"","description":""}}],
 "boss": {{"name":"","description":""}},
 "items": [{{"name":"","description":""}}]
}}
Idioma dos textos: {}. Falas curtas (até 120 caracteres)."##,
        if lang == "en" { "inglês" } else { "português do Brasil" }
    )
}

pub fn vn_writer_system(spec: &GameSpec) -> String {
    format!(
        r##"Você é o Roteirista de uma Visual Novel. Escreva um roteiro ramificado curto (12 a 22 nós) com 2 escolhas importantes e 3 finais.
Responda SOMENTE com JSON válido: {{"story": [ NÓ, NÓ, ... ]}} onde cada NÓ tem:
 "id": texto único (o primeiro nó deve ser "start"),
 "bg": um destes cenários: "bg_a" (1º lugar), "bg_b" (2º lugar), "bg_c" (3º lugar),
 "character": "char_player" | "char_ally" | "char_rival" | "" (quem aparece na tela),
 "speaker": nome de quem fala ("" para narrador),
 "text": a fala/narração (máx. 200 caracteres),
 "next": id do próximo nó (omita se houver choices/branch/ending),
 "choices": [{{"text":"","goto":"id","effect":{{"confianca":1}}}}] (opcional; 2 ou 3 opções),
 "branch": [{{"cond":"confianca>=2","goto":"id"}}, {{"cond":"","goto":"id_padrao"}}] (opcional; escolhe o final conforme variáveis),
 "ending": "good|neutral|bad" (somente nos nós finais).
Todo "goto"/"next" deve apontar para um id existente. Personagens: protagonista {} (char_player), aliado {} (char_ally), rival {} (char_rival). Lugares: {}.
Idioma: {}."##,
        spec.player.name,
        spec.npcs.first().map(|n| n.name.as_str()).unwrap_or("Aliado"),
        if spec.boss.name.is_empty() { "Rival" } else { spec.boss.name.as_str() },
        spec.places.join(", "),
        if spec.lang == "en" { "inglês" } else { "português do Brasil" }
    )
}

pub fn architect_system(genre: Genre) -> String {
    let allowed = allowed_features(genre);
    format!(
        r##"Você é o Arquiteto de software de um jogo HTML5. Escolha quais recursos opcionais do template de {} devem ser ativados para atender ao pedido.
Recursos disponíveis (use somente estes): {}.
Responda SOMENTE com JSON válido: {{"features": ["..."], "notes": "1 a 3 frases sobre a arquitetura/decisões"}}"##,
        genre.label(),
        if allowed.is_empty() { "nenhum".to_string() } else { allowed.join(", ") }
    )
}

pub fn allowed_features(genre: Genre) -> &'static [&'static str] {
    match genre {
        Genre::Platformer2d | Genre::Platformer3d => &["double_jump"],
        Genre::Rpg2d | Genre::Rpg3d => &["boss"],
        Genre::Fps | Genre::Tps => &["waves"],
        Genre::Arcade => &["spread_shot", "shield", "boss"],
        Genre::VisualNovel => &[],
    }
}

pub const ENGINEER_SYSTEM: &str = r##"Você é um programador sênior de jogos HTML5 (JavaScript puro, Canvas 2D). O jogo já funciona: foi gerado a partir de um template. Seu trabalho é adicionar ou alterar mecânicas conforme o pedido.

API disponível (arquivo js/custom.js é carregado depois de todo o resto):
- GF.on('start', fn)            // quando o jogo inicia (ou reinicia)
- GF.on('update', (dt) => {})   // todo quadro; dt em segundos
- GF.on('draw', (ctx) => {})    // depois que o template desenhou o quadro (ctx = canvas 2D)
- GF.on('event', (nome, dados) => {})  // eventos do jogo (ex.: 'coin', 'hit', 'kill', 'win', 'lose')
- GF.game   // estado atual do jogo (player, score, entidades... conforme o gênero)
- GF.cfg    // conteúdo de game.json (título, paleta, textos, mundo)
- GF.input.down('KeyW') // true se a tecla está pressionada; GF.input.pressed('Space') (uma vez por toque)
- GF.play('coin') / GF.img('player') / GF.palette / GF.W, GF.H (tamanho do canvas) / GF.rand()

Formato OBRIGATÓRIO da resposta (um bloco por arquivo, conteúdo COMPLETO do arquivo, sem explicações fora dos blocos):
<<<FILE js/custom.js>>>
...código...
<<<END>>>

Regras: JavaScript puro, sem bibliotecas externas, sem import/export, sem rede. Não quebre o que já existe. Prefira editar apenas js/custom.js; só altere outros arquivos se for realmente necessário. Código curto, comentado em português."##;

/// Trecho curto do estado do jogo para dar contexto ao engenheiro.
pub fn engineer_context(spec: &GameSpec, files: &[(String, String)], issues: &[String]) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Jogo: \"{}\" — gênero {} — tema: {}.\nMecânicas: {}.\nRecursos ativos: {}.\nJogador: {} ({}).\nInimigos: {}. Chefe: {}.\nPaleta: primary {}, accent {}, danger {}.\n",
        spec.title,
        spec.genre.label(),
        spec.setting,
        spec.mechanics.join("; "),
        spec.features.join(", "),
        spec.player.name,
        spec.player.description,
        spec.enemies.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", "),
        spec.boss.name,
        spec.palette.primary,
        spec.palette.accent,
        spec.palette.danger,
    ));
    for (path, content) in files {
        let c: String = content.chars().take(9000).collect();
        s.push_str(&format!("\n--- {} ---\n{}\n", path, c));
    }
    if !issues.is_empty() {
        s.push_str("\nProblemas encontrados pelo testador (corrija-os):\n");
        for i in issues {
            s.push_str(&format!("- {i}\n"));
        }
    }
    s
}

/// Extrai blocos `<<<FILE caminho>>> ... <<<END>>>`. Aceita fim ausente e cercas ``` dentro do bloco.
pub fn parse_file_blocks(text: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(i) = rest.find("<<<FILE") {
        rest = &rest[i + "<<<FILE".len()..];
        let Some(close) = rest.find(">>>") else { break };
        let path = rest[..close].trim().trim_matches('`').trim().to_string();
        rest = &rest[close + 3..];
        let end = [rest.find("<<<END>>>"), rest.find("<<<FILE")].into_iter().flatten().min().unwrap_or(rest.len());
        let body = strip_fence(&rest[..end]);
        if !path.is_empty() {
            out.push((path, body));
        }
        rest = &rest[end..];
        rest = rest.strip_prefix("<<<END>>>").unwrap_or(rest);
    }
    if out.is_empty() {
        // Fallback: um único bloco de código vira o custom.js
        let blocks: Vec<&str> = text.split("```").enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, b)| b).collect();
        if blocks.len() == 1 {
            out.push(("js/custom.js".to_string(), strip_fence(&format!("```{}```", blocks[0]))));
        }
    }
    out
}

fn strip_fence(s: &str) -> String {
    let t = s.trim_matches(['\n', '\r']);
    let t = t.trim();
    if let Some(stripped) = t.strip_prefix("```") {
        let body = stripped.split_once('\n').map(|(_, b)| b).unwrap_or("");
        return body.trim_end().trim_end_matches("```").trim_end().to_string() + "\n";
    }
    format!("{}\n", t)
}

/// Caminhos que a IA pode escrever dentro do projeto.
pub fn allowed_write_path(rel: &str) -> bool {
    let r = rel.replace('\\', "/");
    if r.contains("..") || r.starts_with('/') || r.len() > 120 {
        return false;
    }
    let is_js = r.starts_with("js/") && r.ends_with(".js") && r.matches('/').count() == 1;
    is_js || r == "style.css" || r == "index.html" || r == "game.json"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks_with_fences_and_missing_end() {
        let t = "Claro!\n<<<FILE js/custom.js>>>\n```js\nGF.on('start', () => {});\n```\n<<<END>>>\ntexto\n<<<FILE style.css>>>\nbody{color:red}\n";
        let b = parse_file_blocks(t);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].0, "js/custom.js");
        assert_eq!(b[0].1, "GF.on('start', () => {});\n");
        assert_eq!(b[1].0, "style.css");
        assert!(b[1].1.contains("color:red"));
    }

    #[test]
    fn single_fenced_block_fallback() {
        let b = parse_file_blocks("Aqui:\n```javascript\nconsole.log(1)\n```\nPronto");
        assert_eq!(b, vec![("js/custom.js".to_string(), "console.log(1)\n".to_string())]);
    }

    #[test]
    fn write_paths_are_restricted() {
        assert!(allowed_write_path("js/custom.js"));
        assert!(allowed_write_path("style.css"));
        assert!(!allowed_write_path("../x.js"));
        assert!(!allowed_write_path("js/../../x.js"));
        assert!(!allowed_write_path("assets/images/a.png"));
        assert!(!allowed_write_path("/etc/passwd"));
        assert!(!allowed_write_path("js/sub/x.js"));
    }
}
