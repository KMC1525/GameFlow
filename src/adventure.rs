//! Text adventures (interactive fiction): world model, procedural generator and validator.
//!
//! The generator builds a connected map with one guaranteed puzzle chain and a **walkthrough**
//! (list of commands) that the tests replay in the real game engine:
//!
//! 1. an *offering* lies in some room, a *keeper* (NPC) stands in another and trades the *key* for it;
//! 2. the key opens a locked exit that leads to the *vault* holding the *relic*;
//! 3. the game is won by bringing the relic back to the starting room.
//!
//! The validator (also used by the Tester agent, so it covers hand-edited and AI-edited worlds) checks that every
//! reference exists and that the goal is reachable by a simple fixpoint simulation.

use crate::spec::GameSpec;
use crate::util::{Rng, fold_accent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const DIRS: [&str; 4] = ["north", "south", "east", "west"];

pub fn opposite(d: &str) -> &'static str {
    match d {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        "up" => "down",
        _ => "up",
    }
}

fn dir_word(d: &str, lang: &str) -> &'static str {
    match (d, lang == "en") {
        ("north", false) => "norte",
        ("south", false) => "sul",
        ("east", false) => "leste",
        ("west", false) => "oeste",
        ("up", false) => "cima",
        ("down", false) => "baixo",
        ("north", true) => "north",
        ("south", true) => "south",
        ("east", true) => "east",
        ("west", true) => "west",
        ("up", true) => "up",
        _ => "down",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Lock {
    /// id of the item that opens this exit
    pub key: String,
    pub msg: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub desc: String,
    /// direction (north|south|east|west|up|down) → room id
    pub exits: BTreeMap<String, String>,
    /// direction → lock on that exit
    pub locks: BTreeMap<String, Lock>,
    pub items: Vec<String>,
    pub npcs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Thing {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub desc: String,
    pub take: bool,
    /// text shown by "read"
    pub read: String,
}

impl Default for Thing {
    fn default() -> Self {
        Thing { id: String::new(), name: String::new(), aliases: vec![], desc: String::new(), take: true, read: String::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub desc: String,
    pub talk: Vec<String>,
    /// item id the person wants (empty = nothing)
    pub wants: String,
    /// item id handed over (into the inventory) when satisfied
    pub gives: String,
    /// flag set when the trade happens
    pub flag: String,
    pub thanks: String,
    pub refuse: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Goal {
    /// the player wins holding this item…
    pub item: String,
    /// …while standing in this room
    pub room: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Hint {
    /// all conditions must hold: `has:item`, `!has:item`, `flag:x`, `!flag:x`, `at:room`
    pub when: Vec<String>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct World {
    pub start: String,
    pub intro: String,
    /// `{moves}` is replaced by the number of moves
    pub win_text: String,
    pub goal: Goal,
    pub rooms: Vec<Room>,
    pub items: Vec<Thing>,
    pub npcs: Vec<Person>,
    pub hints: Vec<Hint>,
    /// commands that win the game (documentation and automated tests)
    pub walkthrough: Vec<String>,
}

// ───────────────────────── vocabulary ─────────────────────────

fn t<'a>(lang: &str, pt: &'a str, en: &'a str) -> &'a str {
    if lang == "en" { en } else { pt }
}

/// (name pt, name en, description pt, description en) — `{a}` is replaced by the theme atmosphere.
const ARCHETYPES: [(&str, &str, &str, &str); 11] = [
    ("Grande Salão", "Great Hall", "Um salão amplo onde cada passo ecoa. {a}", "A wide hall where every step echoes. {a}"),
    ("Jardim Esquecido", "Forgotten Garden", "Canteiros tomados pelo mato cercam um velho banco de pedra. {a}", "Overgrown beds surround an old stone bench. {a}"),
    ("Porão Úmido", "Damp Cellar", "Barris vazios e teias de aranha enchem o lugar. {a}", "Empty barrels and cobwebs fill the place. {a}"),
    ("Torre Alta", "High Tower", "Uma escada em espiral termina numa sala com janelas estreitas. {a}", "A spiral stair ends in a room with narrow windows. {a}"),
    ("Biblioteca Empoeirada", "Dusty Library", "Estantes tortas guardam livros que ninguém abre há anos. {a}", "Crooked shelves hold books nobody has opened in years. {a}"),
    ("Ponte Estreita", "Narrow Bridge", "Uma ponte de tábuas range sobre o vazio. {a}", "A plank bridge creaks over the void. {a}"),
    ("Caverna Escura", "Dark Cave", "Gotas caem de um teto que você mal enxerga. {a}", "Drops fall from a ceiling you can barely see. {a}"),
    ("Oficina Abandonada", "Abandoned Workshop", "Ferramentas enferrujadas jazem sobre uma bancada. {a}", "Rusty tools lie on a workbench. {a}"),
    ("Pequeno Santuário", "Small Shrine", "Velas apagadas cercam um altar simples. {a}", "Unlit candles surround a simple altar. {a}"),
    ("Pátio Interno", "Inner Courtyard", "Um pátio aberto, com um poço seco no centro. {a}", "An open courtyard with a dry well at its center. {a}"),
    ("Encruzilhada", "Crossroads", "Caminhos se cruzam sob uma placa quebrada. {a}", "Paths cross under a broken sign. {a}"),
];

fn atmosphere(theme: &str, lang: &str) -> &'static str {
    let en = lang == "en";
    match (theme, en) {
        ("floresta", false) => "Folhas sussurram ao seu redor.",
        ("floresta", true) => "Leaves whisper all around you.",
        ("deserto", false) => "O calor e a areia deixam tudo sonolento.",
        ("deserto", true) => "Heat and sand make everything drowsy.",
        ("espaco", false) => "Luzes frias piscam ao longe.",
        ("espaco", true) => "Cold lights blink in the distance.",
        ("cyberpunk", false) => "Néon e chuva pintam as paredes.",
        ("cyberpunk", true) => "Neon and rain paint the walls.",
        ("gelo", false) => "Seu hálito vira fumaça no frio.",
        ("gelo", true) => "Your breath turns to smoke in the cold.",
        ("vulcao", false) => "O ar queima e o chão estremece.",
        ("vulcao", true) => "The air burns and the ground trembles.",
        ("oceano", false) => "Cheiro de sal e madeira molhada.",
        ("oceano", true) => "A smell of salt and wet wood.",
        ("assombrado", false) => "Sombras se movem onde ninguém está.",
        ("assombrado", true) => "Shadows move where no one stands.",
        (_, false) => "Uma magia antiga paira no ar.",
        (_, true) => "An ancient magic hangs in the air.",
    }
}

/// (name pt, name en, description pt, description en)
const OFFERINGS: [(&str, &str, &str, &str); 6] = [
    ("Maçã dourada", "Golden apple", "Uma maçã que brilha como ouro polido.", "An apple that shines like polished gold."),
    ("Flauta de osso", "Bone flute", "Uma flauta delicada, ainda com um fiapo de som.", "A delicate flute that still hums a thread of sound."),
    ("Carta lacrada", "Sealed letter", "Um envelope pesado, fechado com cera vermelha.", "A heavy envelope sealed with red wax."),
    ("Amuleto de prata", "Silver amulet", "Frio ao toque, com um símbolo gravado.", "Cold to the touch, with an engraved symbol."),
    ("Garrafa de mel", "Bottle of honey", "O mel lá dentro ainda parece fresco.", "The honey inside still looks fresh."),
    ("Retrato antigo", "Old portrait", "Um pequeno retrato de alguém que sorri para você.", "A tiny portrait of someone smiling at you."),
];

const DECOYS: [(&str, &str, &str, &str); 4] = [
    ("Lanterna enferrujada", "Rusty lantern", "Não acende mais, mas é bonita.", "It no longer lights, but it is pretty."),
    ("Pedra lisa", "Smooth stone", "Perfeita para quicar na água. Se houvesse água.", "Perfect for skipping on water. If there were any."),
    ("Pena colorida", "Colorful feather", "Leve como um suspiro.", "Light as a sigh."),
    ("Moeda sem valor", "Worthless coin", "Tão gasta que nem dá para ver o rosto.", "So worn you cannot see the face."),
];

const SCENERY: [(&str, &str, &str, &str); 3] = [
    ("Estátua rachada", "Cracked statue", "Uma estátua de rosto apagado. Parece observar você.", "A statue with a worn face. It seems to watch you."),
    ("Mural desbotado", "Faded mural", "Figuras antigas, quase invisíveis, dançam em círculo.", "Ancient, nearly invisible figures dance in a circle."),
    ("Velho baú", "Old chest", "Está vazio e sem tampa. Alguém chegou antes.", "Empty and lidless. Someone got here first."),
];

const STOP: [&str; 14] = ["de", "da", "do", "das", "dos", "the", "of", "com", "para", "uma", "um", "old", "sem", "a"];

/// Lowercase, accent-free text with single spaces (what the game parser compares against).
pub fn norm(s: &str) -> String {
    let folded: String = s.chars().map(|c| fold_accent(c).to_ascii_lowercase()).map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn aliases_for(name: &str) -> Vec<String> {
    let full = norm(name);
    let mut out = vec![full.clone()];
    for w in full.split(' ') {
        if w.len() >= 4 && !STOP.contains(&w) && !out.contains(&w.to_string()) {
            out.push(w.to_string());
        }
    }
    out
}

// ───────────────────────── generator ─────────────────────────

fn free_dirs(r: &Room) -> Vec<&'static str> {
    DIRS.iter().copied().filter(|d| !r.exits.contains_key(*d)).collect()
}

fn link(rooms: &mut [Room], a: usize, dir: &str, b: usize) {
    let (ida, idb) = (rooms[a].id.clone(), rooms[b].id.clone());
    rooms[a].exits.insert(dir.to_string(), idb);
    rooms[b].exits.insert(opposite(dir).to_string(), ida);
}

/// Shortest list of directions between two rooms; the room `avoid` is never entered (unless it is the target).
fn path(rooms: &[Room], from: &str, to: &str, avoid: &str) -> Option<Vec<String>> {
    let mut prev: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::from([from.to_string()]);
    let mut q = VecDeque::from([from.to_string()]);
    while let Some(cur) = q.pop_front() {
        if cur == to {
            let mut dirs = vec![];
            let mut at = cur;
            while at != from {
                let (p, d) = prev[&at].clone();
                dirs.push(d);
                at = p;
            }
            dirs.reverse();
            return Some(dirs);
        }
        let room = rooms.iter().find(|r| r.id == cur)?;
        for (d, next) in &room.exits {
            if seen.contains(next) || (next == avoid && next != to) {
                continue;
            }
            seen.insert(next.clone());
            prev.insert(next.clone(), (cur.clone(), d.clone()));
            q.push_back(next.clone());
        }
    }
    None
}

pub fn generate(spec: &GameSpec) -> Value {
    serde_json::to_value(build(spec)).unwrap_or(Value::Null)
}

pub fn build(spec: &GameSpec) -> World {
    let lang = spec.lang.as_str();
    let mut rng = Rng::new(spec.seed ^ 0xAD_7E_47);
    let n: usize = match spec.difficulty.as_str() {
        "easy" => 7,
        "hard" => 11,
        _ => 9,
    };
    let atm = atmosphere(&spec.theme, lang);
    let place = |i: usize| spec.places.get(i).cloned().unwrap_or_else(|| t(lang, "Lugar", "Place").to_string());
    let relic_name = spec.items.first().map(|i| i.name.clone()).unwrap_or_else(|| t(lang, "Relíquia", "Relic").to_string());
    let elder_name = spec.npcs.first().map(|p| p.name.clone()).unwrap_or_else(|| t(lang, "Ancião", "Elder").to_string());
    let keeper_name = spec.npcs.get(1).map(|p| p.name.clone()).unwrap_or_else(|| t(lang, "Guardião", "Keeper").to_string());

    // ── rooms ──
    let mut arche: Vec<usize> = (0..ARCHETYPES.len()).collect();
    rng.shuffle(&mut arche);
    let mut rooms: Vec<Room> = vec![];
    for i in 0..n {
        let id = format!("r{}", i + 1);
        let (name, desc) = if i == 0 {
            (place(0), format!("{} {}", t(lang, "É aqui que tudo começa.", "This is where it all begins."), atm))
        } else if i == n - 1 {
            (
                t(lang, "Câmara Selada", "Sealed Chamber").to_string(),
                format!("{} {}", t(lang, "Uma câmara silenciosa, guardada por muito tempo.", "A silent chamber, guarded for a very long time."), atm),
            )
        } else if i == 1 {
            (place(1), t(lang, "Um lugar que parece ter muita história.", "A place that seems to have a lot of history.").to_string())
        } else {
            let a = &ARCHETYPES[arche[i % ARCHETYPES.len()]];
            // the theme sentence only on some rooms, so descriptions do not all end the same way
            let flavor = if i % 3 == 0 { atm } else { "" };
            (t(lang, a.0, a.1).to_string(), t(lang, a.2, a.3).replace("{a}", flavor).trim().to_string())
        };
        rooms.push(Room { id, name, desc, ..Default::default() });
    }
    // spanning tree over rooms 0..n-2 (the vault is attached later with a lock)
    for i in 1..n - 1 {
        loop {
            let parent = rng.range(0, i as i32 - 1) as usize;
            let free = free_dirs(&rooms[parent]);
            if let Some(&d) = (!free.is_empty()).then(|| rng.pick(&free)) {
                link(&mut rooms, parent, d, i);
                break;
            }
        }
    }
    // one extra loop so the map is not a pure tree
    if n >= 8 {
        for _ in 0..30 {
            let (a, b) = (rng.range(0, n as i32 - 2) as usize, rng.range(0, n as i32 - 2) as usize);
            if a == b || rooms[a].exits.values().any(|x| *x == rooms[b].id) {
                continue;
            }
            let d = *rng.pick(&DIRS);
            if !rooms[a].exits.contains_key(d) && !rooms[b].exits.contains_key(opposite(d)) {
                link(&mut rooms, a, d, b);
                break;
            }
        }
    }
    // the vault hangs behind a locked exit of a room that is at least two steps from the start
    let vault = n - 1;
    let deep: Vec<usize> = (1..n - 1).filter(|&i| path(&rooms, "r1", &rooms[i].id, "").map(|p| p.len() >= 2).unwrap_or(false) && !free_dirs(&rooms[i]).is_empty()).collect();
    let any: Vec<usize> = (1..n - 1).filter(|&i| !free_dirs(&rooms[i]).is_empty()).collect();
    let a_room = *rng.pick(if deep.is_empty() { &any } else { &deep });
    let a_dir = *rng.pick(&free_dirs(&rooms[a_room]));
    link(&mut rooms, a_room, a_dir, vault);
    let a_id = rooms[a_room].id.clone();
    rooms[a_room]
        .locks
        .insert(a_dir.to_string(), Lock { key: "key".into(), msg: t(lang, "A porta está trancada. Parece precisar de uma chave.", "The door is locked. It looks like it needs a key.").into() });

    // ── things and people ──
    let off = OFFERINGS[rng.range(0, OFFERINGS.len() as i32 - 1) as usize];
    let normal: Vec<usize> = (1..n - 1).collect();
    let keeper_room = *rng.pick(&normal);
    let offering_room = *rng.pick(&normal.iter().copied().filter(|&i| i != keeper_room).chain([0usize]).collect::<Vec<_>>());
    let note_room = *rng.pick(&(0..n - 1).filter(|&i| path(&rooms, "r1", &rooms[i].id, &rooms[vault].id).map(|p| p.len() <= 2).unwrap_or(false)).collect::<Vec<_>>());

    let mk_thing = |id: &str, name: &str, desc: &str, take: bool, read: &str| Thing { id: id.into(), name: name.into(), aliases: aliases_for(name), desc: desc.into(), take, read: read.into() };
    let mut items: Vec<Thing> = vec![
        mk_thing("key", t(lang, "Chave antiga", "Old key"), t(lang, "Pesada e fria, com um entalhe estranho.", "Heavy and cold, with a strange notch."), true, ""),
        mk_thing("offering", t(lang, off.0, off.1), t(lang, off.2, off.3), true, ""),
        mk_thing("relic", &relic_name, t(lang, "O objeto que você veio buscar. Parece zumbir de leve.", "The object you came for. It seems to hum faintly."), true, ""),
        mk_thing(
            "note",
            t(lang, "Bilhete amassado", "Crumpled note"),
            t(lang, "Um papel dobrado várias vezes.", "A paper folded many times."),
            true,
            &format!(
                "{} {}",
                t(lang, "“Quem guarda a chave só a entrega em troca de um presente. Dizem que", "“Whoever keeps the key only hands it over for a gift. They say that"),
                t(lang, &format!("{} gosta de {}.”", keeper_name, t(lang, off.0, off.1).to_lowercase()), &format!("{} likes {}.”", keeper_name, t(lang, off.0, off.1).to_lowercase()))
            ),
        ),
    ];
    let mut decoys: Vec<usize> = (0..DECOYS.len()).collect();
    rng.shuffle(&mut decoys);
    let decoy_ids: Vec<String> = decoys.iter().take(2).map(|&i| format!("decoy{}", i + 1)).collect();
    for (k, &di) in decoys.iter().take(2).enumerate() {
        let d = DECOYS[di];
        items.push(mk_thing(&decoy_ids[k], t(lang, d.0, d.1), t(lang, d.2, d.3), true, ""));
    }
    let mut sc: Vec<usize> = (0..SCENERY.len()).collect();
    rng.shuffle(&mut sc);
    let scenery_ids: Vec<String> = sc.iter().take(2).map(|&i| format!("scenery{}", i + 1)).collect();
    for (k, &si) in sc.iter().take(2).enumerate() {
        let s = SCENERY[si];
        items.push(mk_thing(&scenery_ids[k], t(lang, s.0, s.1), t(lang, s.2, s.3), false, ""));
    }

    let npcs = vec![
        Person {
            id: "elder".into(),
            name: elder_name.clone(),
            aliases: aliases_for(&elder_name),
            desc: t(lang, "Alguém que parece saber mais do que diz.", "Someone who seems to know more than they say.").into(),
            talk: vec![
                format!("{} {}", t(lang, "Bem-vindo(a)! Você veio atrás de", "Welcome! You came for"), format!("{}.", relic_name)),
                t(lang, "Ela está numa câmara selada. A chave está com alguém que gosta de trocas.", "It lies in a sealed chamber. The key belongs to someone who likes trades.").into(),
                t(lang, "Dica: digite 'ajuda' para ver os comandos — ou clique nas palavras destacadas.", "Tip: type 'help' to see the commands — or click the highlighted words.").into(),
            ],
            ..Default::default()
        },
        Person {
            id: "keeper".into(),
            name: keeper_name.clone(),
            aliases: aliases_for(&keeper_name),
            desc: t(lang, "Tem um molho de chaves... e um olhar interesseiro.", "Carries a bunch of keys... and a scheming look.").into(),
            talk: vec![
                t(lang, "Ninguém passa pela porta trancada sem a minha chave.", "Nobody passes the locked door without my key.").into(),
                format!("{} {}.", t(lang, "Mas eu aceitaria um presente... algo como", "But I would accept a gift... something like"), t(lang, off.0, off.1).to_lowercase()),
            ],
            wants: "offering".into(),
            gives: "key".into(),
            flag: "key_given".into(),
            thanks: t(lang, "Que gentileza! Tome a chave — e boa sorte.", "How kind! Take the key — and good luck.").into(),
            refuse: t(lang, "Isso não me interessa.", "That does not interest me.").into(),
        },
    ];
    rooms[0].npcs.push("elder".into());
    rooms[keeper_room].npcs.push("keeper".into());
    rooms[offering_room].items.push("offering".into());
    rooms[note_room].items.push("note".into());
    rooms[vault].items.push("relic".into());
    for id in decoy_ids.iter().chain(scenery_ids.iter()) {
        let r = rng.range(0, n as i32 - 2) as usize;
        rooms[r].items.push(id.clone());
    }

    // ── hints (first one whose conditions hold is shown) ──
    let (keeper_room_name, offering_room_name, a_name) = (rooms[keeper_room].name.clone(), rooms[offering_room].name.clone(), rooms[a_room].name.clone());
    let hints = vec![
        Hint { when: vec!["has:relic".into()], text: format!("{} {}.", t(lang, "Você já tem o que veio buscar. Volte para", "You already have what you came for. Go back to"), rooms[0].name) },
        Hint {
            when: vec!["has:key".into(), "!has:relic".into()],
            text: format!("{} {} ({}).", t(lang, "A chave abre uma porta trancada em", "The key opens a locked door in"), a_name, dir_word(a_dir, lang)),
        },
        Hint {
            when: vec!["has:offering".into(), "!flag:key_given".into()],
            text: format!("{} {}, {} {}.", t(lang, "Leve o presente a", "Take the gift to"), keeper_name, t(lang, "que está em", "who is in"), keeper_room_name),
        },
        Hint {
            when: vec!["!has:offering".into(), "!flag:key_given".into()],
            text: format!(
                "{} {}. {} {}?",
                t(lang, "Procure algo que", "Look for something"),
                t(lang, &format!("{} possa querer", keeper_name), &format!("{} might want", keeper_name)),
                t(lang, "Já olhou em", "Have you looked in"),
                offering_room_name
            ),
        },
    ];

    // ── walkthrough ──
    let vault_id = rooms[vault].id.clone();
    let mut cmds: Vec<String> = vec![];
    let go = |cmds: &mut Vec<String>, rooms: &[Room], from: &str, to: &str, avoid: &str| {
        for d in path(rooms, from, to, avoid).unwrap_or_default() {
            cmds.push(format!("{} {}", t(lang, "ir", "go"), dir_word(&d, lang)));
        }
    };
    let (off_id, keeper_id) = (rooms[offering_room].id.clone(), rooms[keeper_room].id.clone());
    go(&mut cmds, &rooms, "r1", &off_id, &vault_id);
    cmds.push(format!("{} {}", t(lang, "pegar", "take"), norm(t(lang, off.0, off.1))));
    go(&mut cmds, &rooms, &off_id, &keeper_id, &vault_id);
    cmds.push(format!("{} {} {} {}", t(lang, "dar", "give"), norm(t(lang, off.0, off.1)), t(lang, "para", "to"), norm(&keeper_name)));
    go(&mut cmds, &rooms, &keeper_id, &a_id, &vault_id);
    cmds.push(format!("{} {}", t(lang, "ir", "go"), dir_word(a_dir, lang)));
    cmds.push(format!("{} {}", t(lang, "pegar", "take"), norm(&relic_name)));
    go(&mut cmds, &rooms, &vault_id, "r1", "");

    let start_name = rooms[0].name.clone();
    let intro = if lang == "en" {
        format!("{}. You are {}. Your mission: recover {} and bring it back to {}.", capitalize_first(&spec.setting), spec.player.name, relic_name, start_name)
    } else {
        format!("{}. Você é {}. Sua missão: recuperar {} e trazê-lo de volta a {}.", capitalize_first(&spec.setting), spec.player.name, relic_name, start_name)
    };
    let win_text = if lang == "en" {
        format!("You are back at {} with {}. Mission complete in {{moves}} moves!", start_name, relic_name)
    } else {
        format!("Você voltou a {} com {}. Missão cumprida em {{moves}} movimentos!", start_name, relic_name)
    };
    World { start: "r1".into(), intro, win_text, goal: Goal { item: "relic".into(), room: "r1".into() }, rooms, items, npcs, hints, walkthrough: cmds }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Compact JSON with the texts an AI may polish (ids and structure stay untouched).
pub fn text_snapshot(world: &Value) -> String {
    let w: World = serde_json::from_value(world.clone()).unwrap_or_default();
    serde_json::json!({
        "intro": w.intro,
        "win_text": w.win_text,
        "rooms": w.rooms.iter().map(|r| serde_json::json!({"id": r.id, "name": r.name, "desc": r.desc})).collect::<Vec<_>>(),
        "items": w.items.iter().map(|i| serde_json::json!({"id": i.id, "name": i.name, "desc": i.desc})).collect::<Vec<_>>(),
        "npcs": w.npcs.iter().map(|n| serde_json::json!({"id": n.id, "name": n.name, "desc": n.desc, "talk": n.talk})).collect::<Vec<_>>(),
    })
    .to_string()
}

fn clean(s: &str, max: usize) -> Option<String> {
    let c: String = s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect::<String>().trim().to_string();
    (!c.is_empty()).then_some(c)
}

/// Applies AI-written texts onto existing ids only. Never touches structure, names of items/people, the key's
/// trade dialogue or the clue note, so the puzzle chain survives. Returns how many texts changed.
pub fn merge_texts(world: &mut Value, patch: &Value) -> usize {
    let mut w: World = match serde_json::from_value(world.clone()) {
        Ok(w) => w,
        Err(_) => return 0,
    };
    let mut n = 0;
    let set = |dst: &mut String, src: Option<&Value>, max: usize| -> usize {
        match src.and_then(|v| v.as_str()).and_then(|s| clean(s, max)) {
            Some(v) if *dst != v => {
                *dst = v;
                1
            }
            _ => 0,
        }
    };
    n += set(&mut w.intro, patch.get("intro"), 600);
    if let Some(v) = patch.get("win_text").and_then(|v| v.as_str()) {
        if v.contains("{moves}") {
            n += set(&mut w.win_text, patch.get("win_text"), 400);
        }
    }
    let by_id = |key: &str, id: &str| patch.get(key).and_then(|a| a.as_array()).and_then(|a| a.iter().find(|x| x.get("id").and_then(|i| i.as_str()) == Some(id))).cloned();
    for r in w.rooms.iter_mut() {
        if let Some(p) = by_id("rooms", &r.id) {
            n += set(&mut r.name, p.get("name"), 60);
            n += set(&mut r.desc, p.get("desc"), 500);
        }
    }
    for i in w.items.iter_mut() {
        if let Some(p) = by_id("items", &i.id) {
            n += set(&mut i.desc, p.get("desc"), 300);
        }
    }
    for npc in w.npcs.iter_mut() {
        if let Some(p) = by_id("npcs", &npc.id) {
            n += set(&mut npc.desc, p.get("desc"), 300);
            // people who trade carry the clue about the gift: keep their lines
            if npc.wants.is_empty() {
                if let Some(lines) = p.get("talk").and_then(|t| t.as_array()) {
                    let new: Vec<String> = lines.iter().filter_map(|l| l.as_str()).filter_map(|l| clean(l, 220)).take(6).collect();
                    if !new.is_empty() && new != npc.talk {
                        npc.talk = new;
                        n += 1;
                    }
                }
            }
        }
    }
    if let Ok(v) = serde_json::to_value(&w) {
        *world = v;
    }
    n
}

/// Author-facing solution document (spoilers!).
pub fn walkthrough_markdown(world: &Value, lang: &str) -> String {
    let w: World = serde_json::from_value(world.clone()).unwrap_or_default();
    let en = lang == "en";
    let mut s = format!("# {}\n\n", if en { "Walkthrough (spoilers!)" } else { "Passo a passo (spoilers!)" });
    s.push_str(&format!(
        "{}\n\n",
        if en {
            "Type these commands in order to win. Delete `world.walkthrough` from `game.json` if you do not want the solution shipped with the game."
        } else {
            "Digite estes comandos em ordem para vencer. Apague `world.walkthrough` do `game.json` se não quiser distribuir a solução junto com o jogo."
        }
    ));
    for (i, c) in w.walkthrough.iter().enumerate() {
        s.push_str(&format!("{}. `{}`\n", i + 1, c));
    }
    s.push_str(&format!("\n## {}\n\n", if en { "Map" } else { "Mapa" }));
    for r in &w.rooms {
        let exits: Vec<String> = r
            .exits
            .iter()
            .map(|(d, to)| format!("{} → {}{}", dir_word(d, lang), w.rooms.iter().find(|x| &x.id == to).map(|x| x.name.as_str()).unwrap_or(to), if r.locks.contains_key(d) { " 🔒" } else { "" }))
            .collect();
        s.push_str(&format!("- **{}** ({}): {}\n", r.name, r.id, exits.join(", ")));
    }
    s
}

// ───────────────────────── validator ─────────────────────────

/// Checks references and simulates reachability. Returns human-readable problems (empty = fine).
pub fn validate(world: &Value) -> Vec<String> {
    let w: World = match serde_json::from_value(world.clone()) {
        Ok(w) => w,
        Err(e) => return vec![format!("o mundo da aventura está inválido: {e}")],
    };
    let mut p: Vec<String> = vec![];
    let rooms: BTreeMap<&str, &Room> = w.rooms.iter().map(|r| (r.id.as_str(), r)).collect();
    let items: BTreeMap<&str, &Thing> = w.items.iter().map(|i| (i.id.as_str(), i)).collect();
    let npcs: BTreeMap<&str, &Person> = w.npcs.iter().map(|n| (n.id.as_str(), n)).collect();
    if rooms.len() != w.rooms.len() {
        p.push("há salas com o mesmo id".into());
    }
    if w.rooms.is_empty() {
        p.push("a aventura não tem salas".into());
        return p;
    }
    if !rooms.contains_key(w.start.as_str()) {
        p.push(format!("a sala inicial “{}” não existe", w.start));
    }
    for r in &w.rooms {
        for (d, to) in &r.exits {
            if !rooms.contains_key(to.as_str()) {
                p.push(format!("a saída {d} de “{}” leva a “{to}”, que não existe", r.id));
            }
            if !matches!(d.as_str(), "north" | "south" | "east" | "west" | "up" | "down") {
                p.push(format!("direção desconhecida “{d}” em “{}”", r.id));
            }
        }
        for (d, l) in &r.locks {
            if !r.exits.contains_key(d) {
                p.push(format!("a sala “{}” tem um cadeado em {d}, mas não há saída nessa direção", r.id));
            }
            if !items.contains_key(l.key.as_str()) {
                p.push(format!("o cadeado de “{}” precisa do item “{}”, que não existe", r.id, l.key));
            }
        }
        for i in &r.items {
            if !items.contains_key(i.as_str()) {
                p.push(format!("a sala “{}” contém o item “{i}”, que não existe", r.id));
            }
        }
        for n in &r.npcs {
            if !npcs.contains_key(n.as_str()) {
                p.push(format!("a sala “{}” contém o personagem “{n}”, que não existe", r.id));
            }
        }
    }
    for n in &w.npcs {
        for (what, id) in [("quer", &n.wants), ("entrega", &n.gives)] {
            if !id.is_empty() && !items.contains_key(id.as_str()) {
                p.push(format!("o personagem “{}” {what} o item “{id}”, que não existe", n.id));
            }
        }
    }
    if !items.contains_key(w.goal.item.as_str()) {
        p.push(format!("o objetivo exige o item “{}”, que não existe", w.goal.item));
    }
    if !rooms.contains_key(w.goal.room.as_str()) {
        p.push(format!("o objetivo exige a sala “{}”, que não existe", w.goal.room));
    }
    if !p.is_empty() {
        return p;
    }
    // reachability fixpoint
    let mut inv: BTreeSet<&str> = BTreeSet::new();
    let mut reach: BTreeSet<&str> = BTreeSet::from([w.start.as_str()]);
    loop {
        let before = (inv.len(), reach.len());
        let cur: Vec<&str> = reach.iter().copied().collect();
        for id in cur {
            let r = rooms[id];
            for (d, to) in &r.exits {
                if r.locks.get(d).is_none_or(|l| inv.contains(l.key.as_str())) {
                    reach.insert(to.as_str());
                }
            }
            for i in &r.items {
                if items[i.as_str()].take {
                    inv.insert(i.as_str());
                }
            }
            for n in &r.npcs {
                let person = npcs[n.as_str()];
                if !person.gives.is_empty() && (person.wants.is_empty() || inv.contains(person.wants.as_str())) {
                    inv.insert(person.gives.as_str());
                }
            }
        }
        if (inv.len(), reach.len()) == before {
            break;
        }
    }
    if !items[w.goal.item.as_str()].take {
        p.push(format!("o item do objetivo “{}” não pode ser pego", w.goal.item));
    } else if !inv.contains(w.goal.item.as_str()) {
        p.push(format!("é impossível conseguir o item “{}”: confira chaves, trocas e saídas trancadas", w.goal.item));
    }
    for r in &w.rooms {
        if !reach.contains(r.id.as_str()) {
            p.push(format!("a sala “{}” não pode ser alcançada a partir do início", r.id));
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Genre, heuristic_spec};

    fn spec(prompt: &str, seed: u64, diff: &str) -> GameSpec {
        let mut s = heuristic_spec(prompt, Some(Genre::TextAdventure), None);
        s.seed = seed;
        s.difficulty = diff.into();
        s
    }

    #[test]
    fn generated_worlds_are_valid_and_solvable_for_many_seeds() {
        for lang_prompt in ["uma aventura de texto na floresta", "a text adventure in a haunted castle"] {
            for diff in ["easy", "normal", "hard"] {
                for seed in 0..40u64 {
                    let s = spec(lang_prompt, seed, diff);
                    let w = generate(&s);
                    assert_eq!(validate(&w), Vec::<String>::new(), "seed {seed} {diff} {lang_prompt}");
                    let world: World = serde_json::from_value(w).unwrap();
                    assert!(world.walkthrough.len() >= 6, "{:?}", world.walkthrough);
                    assert!(world.rooms.len() >= 7);
                }
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let s = spec("aventura de texto", 5, "normal");
        assert_eq!(generate(&s), generate(&s));
        assert_ne!(generate(&s), generate(&spec("aventura de texto", 6, "normal")));
    }

    #[test]
    fn validator_catches_broken_worlds() {
        let good = generate(&spec("aventura de texto", 1, "normal"));
        // exit to nowhere
        let mut w = good.clone();
        w["rooms"][0]["exits"]["north"] = "nowhere".into();
        assert!(validate(&w).iter().any(|m| m.contains("não existe")), "{:?}", validate(&w));
        // key that can never be obtained: the keeper no longer gives it
        let mut w = good.clone();
        for n in w["npcs"].as_array_mut().unwrap() {
            n["gives"] = "".into();
        }
        assert!(validate(&w).iter().any(|m| m.contains("impossível")), "{:?}", validate(&w));
        // unreachable room
        let mut w = good.clone();
        w["rooms"].as_array_mut().unwrap().push(serde_json::json!({"id": "island", "name": "Ilha", "desc": "x"}));
        assert!(validate(&w).iter().any(|m| m.contains("island")));
        // garbage
        assert!(!validate(&Value::Null).is_empty());
        assert!(!validate(&serde_json::json!({"rooms": []})).is_empty());
    }

    #[test]
    fn merge_texts_keeps_structure_and_puzzle() {
        let s = spec("aventura de texto", 3, "normal");
        let mut w = generate(&s);
        let before = w.clone();
        let rid = w["rooms"][1]["id"].as_str().unwrap().to_string();
        let patch = serde_json::json!({
            "intro": "Nova abertura.",
            "win_text": "sem o marcador",
            "rooms": [{"id": rid, "name": "Salão Novo", "desc": "Descrição nova."}, {"id": "inexistente", "name": "X", "desc": "Y"}],
            "items": [{"id": "key", "name": "RENOMEADA", "desc": "Chave reluzente."}, {"id": "note", "read": "sem pista"}],
            "npcs": [{"id": "keeper", "desc": "Vendedor.", "talk": ["não conte nada"]}, {"id": "elder", "talk": ["Olá novo", "Segunda"]}]
        });
        let n = merge_texts(&mut w, &patch);
        assert!(n >= 5, "{n}");
        assert_eq!(w["intro"], "Nova abertura.");
        assert_eq!(w["win_text"], before["win_text"], "win_text sem {{moves}} deve ser ignorado");
        assert_eq!(w["rooms"][1]["name"], "Salão Novo");
        assert_eq!(w["items"][0]["name"], before["items"][0]["name"], "itens não podem ser renomeados");
        assert_eq!(w["items"][0]["desc"], "Chave reluzente.");
        let keeper = w["npcs"].as_array().unwrap().iter().find(|n| n["id"] == "keeper").unwrap().clone();
        let keeper0 = before["npcs"].as_array().unwrap().iter().find(|n| n["id"] == "keeper").unwrap().clone();
        assert_eq!(keeper["talk"], keeper0["talk"], "as falas de quem negocia guardam a pista");
        assert_eq!(w["rooms"].as_array().unwrap().len(), before["rooms"].as_array().unwrap().len());
        assert_eq!(w["walkthrough"], before["walkthrough"]);
        assert_eq!(validate(&w), Vec::<String>::new());
        assert!(text_snapshot(&w).contains("Salão Novo"));
    }

    #[test]
    fn aliases_and_norm() {
        assert_eq!(norm("Maçã dourada!"), "maca dourada");
        assert_eq!(aliases_for("Chave de ferro"), vec!["chave de ferro", "chave", "ferro"]);
    }

    #[test]
    fn walkthrough_wins_in_the_real_engine() {
        // Needs Node.js; skipped (with a notice) where it is not installed.
        if std::process::Command::new("node").arg("--version").output().is_err() {
            eprintln!("node não encontrado: teste do motor ignorado");
            return;
        }
        let root = std::env::temp_dir().join(format!("gf-ta-{}", uuid::Uuid::new_v4()));
        let mut dirs = vec![];
        for (i, (prompt, diff)) in [
            ("uma aventura de texto na floresta", "easy"),
            ("aventura de texto no deserto", "normal"),
            ("jogo de texto no espaço difícil", "hard"),
            ("a text adventure in a frozen world", "normal"),
            ("text adventure in a haunted house", "hard"),
            ("interactive fiction on a pirate island", "easy"),
        ]
        .iter()
        .enumerate()
        {
            for seed in 0..6u64 {
                let mut s = spec(prompt, seed * 7 + i as u64, diff);
                s.lang = if prompt.starts_with("a ") || prompt.starts_with("text") || prompt.starts_with("interactive") { "en".into() } else { "pt".into() };
                s.world = Value::Null;
                crate::spec::fill_defaults(&mut s);
                let dir = root.join(format!("g{i}-{seed}"));
                std::fs::create_dir_all(&dir).unwrap();
                crate::templates::materialize(&s, &dir, false).unwrap();
                dirs.push(dir);
            }
        }
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test-textadventure.js");
        let out = std::process::Command::new("node").arg(&script).args(&dirs).output().unwrap();
        assert!(out.status.success(), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        std::fs::remove_dir_all(root).ok();
    }
}
