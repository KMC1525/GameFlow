//! Especificação do jogo (`GameSpec`): o "contrato" entre os agentes. É serializada em `game.json`
//! dentro do projeto, e é lida pelos templates HTML/JS. Também contém a análise heurística de prompts
//! (usada quando não há modelo de texto configurado, e como base para completar JSONs parciais da IA).

use crate::util::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ───────────────────────────── Gêneros ─────────────────────────────

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    Rpg2d,
    Rpg3d,
    VisualNovel,
    #[default]
    Platformer2d,
    Platformer3d,
    Fps,
    Tps,
    Arcade,
}

impl Genre {
    pub const ALL: [Genre; 8] = [Genre::Rpg2d, Genre::Rpg3d, Genre::VisualNovel, Genre::Platformer2d, Genre::Platformer3d, Genre::Fps, Genre::Tps, Genre::Arcade];

    pub fn id(self) -> &'static str {
        match self {
            Genre::Rpg2d => "rpg2d",
            Genre::Rpg3d => "rpg3d",
            Genre::VisualNovel => "visual_novel",
            Genre::Platformer2d => "platformer2d",
            Genre::Platformer3d => "platformer3d",
            Genre::Fps => "fps",
            Genre::Tps => "tps",
            Genre::Arcade => "arcade",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Genre::Rpg2d => "RPG 2D",
            Genre::Rpg3d => "RPG 3D",
            Genre::VisualNovel => "Visual Novel",
            Genre::Platformer2d => "Plataforma 2D",
            Genre::Platformer3d => "Plataforma 3D",
            Genre::Fps => "Tiro em 1ª pessoa (3D)",
            Genre::Tps => "Ação em 3ª pessoa (3D)",
            Genre::Arcade => "Arcade (nave)",
        }
    }

    pub fn from_id(s: &str) -> Option<Genre> {
        let s = s.trim().to_lowercase();
        Genre::ALL.iter().copied().find(|g| g.id() == s || g.id().replace('_', "") == s)
    }

    pub fn is_3d(self) -> bool {
        matches!(self, Genre::Rpg3d | Genre::Platformer3d | Genre::Fps | Genre::Tps)
    }
}

/// Descobre o gênero a partir do texto do usuário (pt/en).
pub fn detect_genre(prompt: &str) -> Genre {
    let p = normalize(prompt);
    let has3d = p.contains("3d") || has_any(&p, &["tridimensional"]);
    if has_any(&p, &["visual novel", "novel visual", "dating sim", "otome", "historia interativa", "romance interativo", "livro-jogo", "livro jogo"]) {
        return Genre::VisualNovel;
    }
    if has_any(&p, &["fps", "primeira pessoa", "first person", "first-person", "boomer shooter", "doom", "wolfenstein"]) {
        return Genre::Fps;
    }
    if has_any(&p, &["tps", "terceira pessoa", "third person", "third-person"]) {
        return Genre::Tps;
    }
    if has_any(&p, &["plataforma", "platformer", "platform", "mario", "side-scroller", "sidescroller", "metroidvania"]) {
        return if has3d { Genre::Platformer3d } else { Genre::Platformer2d };
    }
    if has_any(&p, &["rpg", "role-playing", "role playing", "jrpg", "zelda", "dungeon", "masmorra", "aventura de fantasia"]) {
        return if has3d { Genre::Rpg3d } else { Genre::Rpg2d };
    }
    if has_any(&p, &["arcade", "nave", "spaceship", "shoot'em", "shmup", "space invaders", "asteroid", "galaga"]) {
        return Genre::Arcade;
    }
    if has3d {
        return Genre::Tps;
    }
    if has_any(&p, &["romance", "narrativ", "historia", "escolhas", "dialogo"]) {
        return Genre::VisualNovel;
    }
    Genre::Platformer2d
}

// ───────────────────────────── Estruturas ─────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub bg: String,
    pub bg2: String,
    pub ground: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub danger: String,
    pub text: String,
}

impl Default for Palette {
    fn default() -> Self {
        palette_from(THEMES[0].pal)
    }
}

fn palette_from(p: [&str; 8]) -> Palette {
    Palette { bg: p[0].into(), bg2: p[1].into(), ground: p[2].into(), primary: p[3].into(), secondary: p[4].into(), accent: p[5].into(), danger: p[6].into(), text: p[7].into() }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Character {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Npc {
    pub name: String,
    pub role: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Enemy {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Item {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Quest {
    pub title: String,
    pub intro: String,
    pub goal: String,
    pub win_text: String,
    pub lose_text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VnChoice {
    pub text: String,
    pub goto: String,
    /// Variáveis afetadas pela escolha, ex.: {"confianca": 1}
    pub effect: BTreeMap<String, i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VnBranch {
    /// Condição simples: `var>=2`, `var<1`, `var==3`
    pub cond: String,
    pub goto: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VnNode {
    pub id: String,
    pub bg: String,
    /// id do asset do personagem exibido (ex.: char_ally) ou vazio
    pub character: String,
    pub speaker: String,
    pub text: String,
    pub next: String,
    pub choices: Vec<VnChoice>,
    pub branch: Vec<VnBranch>,
    /// Marca final do jogo: "good" | "neutral" | "bad"
    pub ending: String,
    pub music: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AssetReq {
    pub id: String,
    /// sprite | background | tile | texture | item | portrait
    pub kind: String,
    pub prompt: String,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SoundReq {
    pub id: String,
    /// sfx | music
    pub kind: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSpec {
    pub title: String,
    pub tagline: String,
    pub genre: Genre,
    pub lang: String,
    pub theme: String,
    pub setting: String,
    pub art_style: String,
    pub seed: u64,
    pub difficulty: String,
    pub prompt: String,
    pub palette: Palette,
    pub player: Character,
    pub npcs: Vec<Npc>,
    pub enemies: Vec<Enemy>,
    pub boss: Enemy,
    pub items: Vec<Item>,
    pub places: Vec<String>,
    pub quest: Quest,
    pub features: Vec<String>,
    pub mechanics: Vec<String>,
    pub story: Vec<VnNode>,
    pub assets: Vec<AssetReq>,
    pub sounds: Vec<SoundReq>,
    pub ui: BTreeMap<String, String>,
    /// Dados do mundo gerados proceduralmente (mapas, plataformas...). Pode ser editado à mão.
    pub world: Value,
}

impl Default for GameSpec {
    fn default() -> Self {
        GameSpec {
            title: "Meu Jogo".into(),
            tagline: String::new(),
            genre: Genre::default(),
            lang: "pt".into(),
            theme: "fantasia".into(),
            setting: String::new(),
            art_style: "pixel art".into(),
            seed: 1,
            difficulty: "normal".into(),
            prompt: String::new(),
            palette: Palette::default(),
            player: Character::default(),
            npcs: vec![],
            enemies: vec![],
            boss: Enemy::default(),
            items: vec![],
            places: vec![],
            quest: Quest::default(),
            features: vec![],
            mechanics: vec![],
            story: vec![],
            assets: vec![],
            sounds: vec![],
            ui: BTreeMap::new(),
            world: Value::Null,
        }
    }
}

// ───────────────────────────── Temas ─────────────────────────────

struct ThemeDef {
    id: &'static str,
    kw: &'static [&'static str],
    /// bg, bg2, ground, primary, secondary, accent, danger, text
    pal: [&'static str; 8],
    setting: (&'static str, &'static str),
    places: [(&'static str, &'static str); 3],
    title_nouns: [(&'static str, &'static str); 3],
    enemies: [(&'static str, &'static str); 3],
    item: (&'static str, &'static str),
    boss: (&'static str, &'static str),
    art: &'static str,
}

static THEMES: [ThemeDef; 9] = [
    ThemeDef {
        id: "fantasia",
        kw: &["fantasia", "fantasy", "medieval", "magia", "magic", "castelo", "castle", "dragao", "dragon", "reino", "kingdom", "elfo", "elf"],
        pal: ["#1d2433", "#2c3a52", "#4a6b3f", "#6fa8dc", "#c27ba0", "#ffd966", "#e06666", "#f3f3f3"],
        setting: ("um reino antigo ameaçado por uma magia sombria", "an ancient kingdom threatened by dark magic"),
        places: [("Vila das Brumas", "Misty Village"), ("Bosque Antigo", "Old Grove"), ("Torre do Eclipse", "Eclipse Tower")],
        title_nouns: [("Crônicas de", "Chronicles of"), ("A Lenda de", "The Legend of"), ("O Reino de", "The Realm of")],
        enemies: [("Slime", "Slime"), ("Goblin", "Goblin"), ("Esqueleto", "Skeleton")],
        item: ("Cristal arcano", "Arcane crystal"),
        boss: ("Mago Sombrio", "Dark Wizard"),
        art: "pixel art fantasy",
    },
    ThemeDef {
        id: "floresta",
        kw: &["floresta", "forest", "bosque", "selva", "jungle", "natureza", "arvore", "tree", "druida", "druid"],
        pal: ["#14261c", "#1f3d2b", "#3f6b3a", "#93c47d", "#b6d7a8", "#ffe599", "#cc4125", "#eef5e8"],
        setting: ("uma floresta viva onde as árvores sussurram segredos", "a living forest where the trees whisper secrets"),
        places: [("Clareira Esmeralda", "Emerald Glade"), ("Pântano Musgoso", "Mossy Swamp"), ("Coração da Floresta", "Heart of the Forest")],
        title_nouns: [("Sussurros de", "Whispers of"), ("O Guardião de", "Guardian of"), ("Raízes de", "Roots of")],
        enemies: [("Cogumelo bravo", "Angry Mushroom"), ("Lobo das sombras", "Shadow Wolf"), ("Aranha gigante", "Giant Spider")],
        item: ("Semente mágica", "Magic seed"),
        boss: ("Ent Corrompido", "Corrupted Ent"),
        art: "pixel art forest",
    },
    ThemeDef {
        id: "deserto",
        kw: &["deserto", "desert", "areia", "sand", "egito", "egypt", "piramide", "pyramid", "oasis", "faraó", "farao"],
        pal: ["#3a2a1a", "#7a5a34", "#c9a15a", "#e69138", "#f6b26b", "#ffe599", "#a61c00", "#fff4e0"],
        setting: ("um deserto infinito com ruínas esquecidas", "an endless desert with forgotten ruins"),
        places: [("Oásis Perdido", "Lost Oasis"), ("Pirâmide Dourada", "Golden Pyramid"), ("Dunas Ardentes", "Burning Dunes")],
        title_nouns: [("Areias de", "Sands of"), ("O Segredo de", "The Secret of"), ("Miragem de", "Mirage of")],
        enemies: [("Escorpião", "Scorpion"), ("Múmia", "Mummy"), ("Chacal", "Jackal")],
        item: ("Escaravelho dourado", "Golden scarab"),
        boss: ("Faraó Maldito", "Cursed Pharaoh"),
        art: "pixel art desert",
    },
    ThemeDef {
        id: "espaco",
        kw: &["espaco", "space", "galaxia", "galaxy", "alien", "planeta", "planet", "estrela", "star", "astronauta", "astronaut", "sci-fi", "ficcao cientifica", "nave"],
        pal: ["#0b1020", "#1a2145", "#3a4270", "#6fa8dc", "#8e7cc3", "#ffd966", "#ff5577", "#e8ecff"],
        setting: ("uma galáxia distante cheia de perigos e mistérios", "a distant galaxy full of danger and mystery"),
        places: [("Estação Orion", "Orion Station"), ("Anel de Asteroides", "Asteroid Ring"), ("Planeta Vermelho", "Red Planet")],
        title_nouns: [("Odisseia", "Odyssey of"), ("Fronteira", "Frontier of"), ("Órbita de", "Orbit of")],
        enemies: [("Drone", "Drone"), ("Alienígena", "Alien"), ("Meteoro vivo", "Living Meteor")],
        item: ("Cristal de energia", "Energy crystal"),
        boss: ("Mente Colmeia", "Hive Mind"),
        art: "pixel art space",
    },
    ThemeDef {
        id: "cyberpunk",
        kw: &["cyberpunk", "neon", "futurista", "futuristic", "robo", "robot", "hacker", "cidade futura", "distopia", "dystopia", "androide", "android"],
        pal: ["#120b22", "#2a1b4d", "#35205f", "#00e5ff", "#ff2d95", "#f5ff3b", "#ff3d3d", "#f2e9ff"],
        setting: ("uma megacidade iluminada por neon e controlada por corporações", "a neon-lit megacity ruled by corporations"),
        places: [("Distrito Neon", "Neon District"), ("Mercado Negro", "Black Market"), ("Torre Corporativa", "Corporate Tower")],
        title_nouns: [("Neon", "Neon"), ("Código", "Code"), ("Circuito de", "Circuit of")],
        enemies: [("Drone de segurança", "Security Drone"), ("Ciborgue", "Cyborg"), ("Vírus vivo", "Living Virus")],
        item: ("Chip de dados", "Data chip"),
        boss: ("IA Mestra", "Master AI"),
        art: "pixel art cyberpunk neon",
    },
    ThemeDef {
        id: "gelo",
        kw: &["gelo", "ice", "neve", "snow", "inverno", "winter", "glacial", "artico", "arctic", "congelad", "frozen"],
        pal: ["#1b2a3a", "#2f4d6b", "#a9c9e0", "#9fd6ff", "#d9ecff", "#ffe08a", "#e45858", "#f0f8ff"],
        setting: ("um mundo congelado onde o inverno nunca termina", "a frozen world where winter never ends"),
        places: [("Vale Gelado", "Frozen Valley"), ("Caverna de Cristal", "Crystal Cave"), ("Pico Eterno", "Eternal Peak")],
        title_nouns: [("Inverno de", "Winter of"), ("Cristais de", "Crystals of"), ("Geada de", "Frost of")],
        enemies: [("Boneco de neve", "Snowman"), ("Lobo de gelo", "Ice Wolf"), ("Golem de gelo", "Ice Golem")],
        item: ("Fragmento de gelo", "Ice shard"),
        boss: ("Rainha do Gelo", "Ice Queen"),
        art: "pixel art ice snow",
    },
    ThemeDef {
        id: "vulcao",
        kw: &["vulcao", "volcano", "lava", "fogo", "fire", "inferno", "magma", "demonio", "demon", "dragao de fogo"],
        pal: ["#200c0c", "#4a1b14", "#5a3a30", "#ff9f43", "#ffd166", "#fff1a8", "#ff3b30", "#ffece0"],
        setting: ("um vulcão em erupção cheio de criaturas de fogo", "an erupting volcano full of fire creatures"),
        places: [("Cratera Ardente", "Burning Crater"), ("Rio de Lava", "Lava River"), ("Forja Ancestral", "Ancient Forge")],
        title_nouns: [("Cinzas de", "Ashes of"), ("A Forja de", "The Forge of"), ("Chamas de", "Flames of")],
        enemies: [("Salamandra", "Salamander"), ("Golem de lava", "Lava Golem"), ("Diabrete", "Imp")],
        item: ("Brasa eterna", "Eternal ember"),
        boss: ("Senhor das Chamas", "Flame Lord"),
        art: "pixel art volcano lava",
    },
    ThemeDef {
        id: "oceano",
        kw: &["oceano", "ocean", "mar ", "sea", "pirata", "pirate", "submarin", "underwater", "ilha", "island", "navio", "ship", "praia", "beach"],
        pal: ["#0a2540", "#12466b", "#c2b280", "#4fc3f7", "#80deea", "#ffd54f", "#ef5350", "#e6f7ff"],
        setting: ("um arquipélago misterioso cercado por águas profundas", "a mysterious archipelago surrounded by deep waters"),
        places: [("Porto das Marés", "Tide Harbor"), ("Recife Coral", "Coral Reef"), ("Ilha do Tesouro", "Treasure Island")],
        title_nouns: [("Marés de", "Tides of"), ("O Tesouro de", "The Treasure of"), ("Ondas de", "Waves of")],
        enemies: [("Caranguejo", "Crab"), ("Água-viva", "Jellyfish"), ("Tubarão", "Shark")],
        item: ("Pérola brilhante", "Shining pearl"),
        boss: ("Kraken", "Kraken"),
        art: "pixel art ocean",
    },
    ThemeDef {
        id: "assombrado",
        kw: &["terror", "horror", "assombrad", "haunted", "fantasma", "ghost", "vampiro", "vampire", "noite", "night", "zumbi", "zombie", "cemiterio", "graveyard", "macabro", "sombrio"],
        pal: ["#17141f", "#2a2438", "#3d3550", "#b39ddb", "#90a4ae", "#ffcc80", "#d32f2f", "#ece7f5"],
        setting: ("uma mansão assombrada onde a noite nunca acaba", "a haunted mansion where the night never ends"),
        places: [("Salão Principal", "Great Hall"), ("Cripta Esquecida", "Forgotten Crypt"), ("Torre do Relógio", "Clock Tower")],
        title_nouns: [("A Maldição de", "The Curse of"), ("Sombras de", "Shadows of"), ("A Noite de", "The Night of")],
        enemies: [("Fantasma", "Ghost"), ("Morcego", "Bat"), ("Zumbi", "Zombie")],
        item: ("Vela sagrada", "Holy candle"),
        boss: ("Conde Vampiro", "Vampire Count"),
        art: "pixel art gothic",
    },
];

fn detect_theme(prompt: &str) -> &'static ThemeDef {
    let p = format!("{} ", normalize(prompt));
    let mut best = (0usize, &THEMES[0]);
    for t in THEMES.iter() {
        let score = t.kw.iter().filter(|k| p.contains(*k)).count();
        if score > best.0 {
            best = (score, t);
        }
    }
    best.1
}

/// Tema citado explicitamente no texto (None se nenhuma palavra-chave aparece). Usado em edições sem IA.
pub fn detect_theme_explicit(prompt: &str) -> Option<(&'static str, Palette, &'static str)> {
    let p = format!("{} ", normalize(prompt));
    let mut best: Option<(usize, &ThemeDef)> = None;
    for t in THEMES.iter() {
        let score = t.kw.iter().filter(|k| p.contains(*k)).count();
        if score > 0 && best.is_none_or(|b| score > b.0) {
            best = Some((score, t));
        }
    }
    best.map(|(_, t)| (t.id, palette_from(t.pal), t.art))
}

pub fn theme_ids() -> Vec<&'static str> {
    THEMES.iter().map(|t| t.id).collect()
}

fn detect_lang(prompt: &str) -> &'static str {
    let p = format!(" {} ", normalize(prompt));
    let pt = [" de ", " que ", " um ", " uma ", " jogo ", " com ", " para ", " nao ", " em ", " do ", " da ", " os ", " as ", " voce ", " crie ", " faca ", " quero ", "cao ", "coes"];
    let en = [" the ", " a ", " an ", " game ", " with ", " and ", " of ", " to ", " make ", " create ", " want ", " in "];
    let sp = pt.iter().filter(|w| p.contains(*w)).count();
    let se = en.iter().filter(|w| p.contains(*w)).count();
    if se > sp { "en" } else { "pt" }
}

fn tr<'a>(lang: &str, pair: (&'a str, &'a str)) -> &'a str {
    if lang == "en" { pair.1 } else { pair.0 }
}

const PLAYER_NAMES: [&str; 12] = ["Aria", "Kael", "Luna", "Theo", "Maya", "Iago", "Nina", "Davi", "Yuri", "Lia", "Bruno", "Zoe"];
const NPC_NAMES: [&str; 10] = ["Sábio Orin", "Dona Marta", "Velho Tobias", "Mestra Iris", "Capitão Ruy", "Pequena Lila", "Ferreiro Gus", "Bruxa Elara", "Mercador Nilo", "Guardiã Sofia"];

// ───────────────────────────── Heurística de prompt ─────────────────────────────

fn quoted(prompt: &str) -> Option<String> {
    for (open, close) in [('"', '"'), ('“', '”'), ('\'', '\''), ('«', '»')] {
        if let Some(i) = prompt.find(open) {
            let rest = &prompt[i + open.len_utf8()..];
            if let Some(j) = rest.find(close) {
                let t = rest[..j].trim();
                if (2..=48).contains(&t.chars().count()) {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// Constrói uma especificação completa e coerente a partir de um prompt, sem nenhum modelo.
pub fn heuristic_spec(prompt: &str, genre_hint: Option<Genre>, palette_override: Option<Palette>) -> GameSpec {
    let genre = genre_hint.unwrap_or_else(|| detect_genre(prompt));
    let theme = detect_theme(prompt);
    let lang = detect_lang(prompt);
    let seed = hash_str(prompt) ^ 0xA5A5;
    let mut rng = Rng::new(seed);
    let en = lang == "en";

    let player_name = rng.pick(&PLAYER_NAMES).to_string();
    let p_norm = normalize(prompt);

    let place_names: Vec<String> = theme.places.iter().map(|p| tr(lang, *p).to_string()).collect();
    let title = quoted(prompt).unwrap_or_else(|| {
        let noun = tr(lang, *rng.pick(&theme.title_nouns));
        let place = place_names[(rng.next_u64() % 3) as usize].clone();
        format!("{} {}", noun, place)
    });

    let difficulty = if has_any(&p_norm, &["dificil", "hard", "desafiador", "challenging", "hardcore"]) {
        "hard"
    } else if has_any(&p_norm, &["facil", "easy", "relaxante", "relaxing", "casual", "infantil"]) {
        "easy"
    } else {
        "normal"
    };

    let mut features: Vec<String> = vec![];
    if has_any(&p_norm, &["pulo duplo", "double jump", "double-jump"]) {
        features.push("double_jump".into());
    }
    if has_any(&p_norm, &["ondas", "waves", "horda", "horde", "sobreviv", "survival"]) {
        features.push("waves".into());
    }
    if has_any(&p_norm, &["escudo", "shield"]) {
        features.push("shield".into());
    }
    if has_any(&p_norm, &["tiro triplo", "triple shot", "spread"]) {
        features.push("spread_shot".into());
    }
    if !has_any(&p_norm, &["sem chefe", "no boss"]) {
        features.push("boss".into());
    }

    let npcs: Vec<Npc> = (0..3)
        .map(|i| {
            let name = if i == 0 && !en {
                "Ancião Eron".to_string()
            } else if i == 0 {
                "Elder Eron".to_string()
            } else {
                rng.pick(&NPC_NAMES).to_string()
            };
            Npc { name, role: if i == 0 { "quest" } else { "villager" }.into(), lines: npc_lines(lang, theme, i, &place_names) }
        })
        .collect();

    let enemies: Vec<Enemy> = theme.enemies.iter().map(|e| Enemy { name: tr(lang, *e).to_string(), description: String::new() }).collect();
    let item_name = tr(lang, theme.item).to_string();
    let boss_name = tr(lang, theme.boss).to_string();

    let quest = Quest {
        title: if en { format!("The {}", item_name) } else { format!("Em busca: {}", item_name) },
        intro: if en {
            format!("{} lies in {}. Gather every {} and face the {}.", title, tr(lang, theme.setting), item_name.to_lowercase(), boss_name)
        } else {
            format!("{} se passa em {}. Reúna todos os itens ({}) e enfrente {}.", title, tr(lang, theme.setting), item_name.to_lowercase(), boss_name)
        },
        goal: if en { format!("Collect the {} and defeat the {}.", item_name.to_lowercase(), boss_name) } else { format!("Colete {} e derrote {}.", item_name.to_lowercase(), boss_name) },
        win_text: if en { "Victory! The land is safe again.".into() } else { "Vitória! A paz voltou a este lugar.".into() },
        lose_text: if en { "You fell... but heroes try again.".into() } else { "Você caiu... mas heróis tentam de novo.".into() },
    };

    let mut spec = GameSpec {
        title,
        tagline: tr(lang, theme.setting).to_string(),
        genre,
        lang: lang.into(),
        theme: theme.id.into(),
        setting: tr(lang, theme.setting).to_string(),
        art_style: theme.art.into(),
        seed,
        difficulty: difficulty.into(),
        prompt: prompt.to_string(),
        palette: palette_override.unwrap_or_else(|| palette_from(theme.pal)),
        player: Character { name: player_name, description: if en { "a brave adventurer".into() } else { "um(a) aventureiro(a) corajoso(a)".into() } },
        npcs,
        enemies,
        boss: Enemy { name: boss_name, description: String::new() },
        items: vec![Item { name: item_name, description: String::new() }],
        places: place_names,
        quest,
        features,
        mechanics: vec![],
        story: vec![],
        assets: vec![],
        sounds: vec![],
        ui: ui_strings(lang),
        world: Value::Null,
    };
    spec.mechanics = default_mechanics(genre, lang);
    fill_defaults(&mut spec);
    spec
}

fn npc_lines(lang: &str, theme: &ThemeDef, idx: usize, places: &[String]) -> Vec<String> {
    let en = lang == "en";
    let item = tr(lang, theme.item).to_lowercase();
    let boss = tr(lang, theme.boss);
    match (idx, en) {
        (0, false) => vec![
            format!("Que bom que você chegou! {} precisa de ajuda.", places[0]),
            format!("Espalhados pelo mapa existem {} que enfraquecem {}.", item, boss),
            "Reúna todos e volte forte. Use ESPAÇO para atacar.".into(),
        ],
        (0, true) => vec![
            format!("Thank goodness you came! {} needs your help.", places[0]),
            format!("Scattered around the map are {} that weaken the {}.", item, boss),
            "Gather them all and come back strong. Press SPACE to attack.".into(),
        ],
        (1, false) => vec![format!("Cuidado em {}: os monstros ficam mais bravos à noite.", places[1]), "Dizem que o herói certo nunca desiste.".into()],
        (1, true) => vec![format!("Careful in {}: monsters get angrier at night.", places[1]), "They say the right hero never gives up.".into()],
        (_, false) => vec![format!("Já ouvi falar de {}... lugar perigoso!", places[2]), "Boa sorte, viajante.".into()],
        (_, true) => vec![format!("I've heard of {}... dangerous place!", places[2]), "Good luck, traveler.".into()],
    }
}

fn default_mechanics(genre: Genre, lang: &str) -> Vec<String> {
    let en = lang == "en";
    let v: Vec<&str> = match (genre, en) {
        (Genre::Rpg2d, false) | (Genre::Rpg3d, false) => vec!["Explorar o mapa", "Conversar com NPCs", "Combate em tempo real", "Coletar relíquias", "Derrotar o chefe"],
        (Genre::Rpg2d, true) | (Genre::Rpg3d, true) => vec!["Explore the map", "Talk to NPCs", "Real-time combat", "Collect relics", "Defeat the boss"],
        (Genre::VisualNovel, false) => vec!["Ler a história", "Fazer escolhas", "Múltiplos finais"],
        (Genre::VisualNovel, true) => vec!["Read the story", "Make choices", "Multiple endings"],
        (Genre::Platformer2d, false) | (Genre::Platformer3d, false) => vec!["Correr e pular", "Coletar moedas", "Pisar nos inimigos", "Chegar à bandeira"],
        (Genre::Platformer2d, true) | (Genre::Platformer3d, true) => vec!["Run and jump", "Collect coins", "Stomp enemies", "Reach the flag"],
        (Genre::Fps, false) | (Genre::Tps, false) => vec!["Atirar", "Sobreviver", "Pegar vida e munição", "Escapar"],
        (Genre::Fps, true) | (Genre::Tps, true) => vec!["Shoot", "Survive", "Pick up health and ammo", "Escape"],
        (Genre::Arcade, false) => vec!["Pilotar a nave", "Atirar", "Desviar", "Coletar power-ups"],
        (Genre::Arcade, true) => vec!["Fly the ship", "Shoot", "Dodge", "Collect power-ups"],
    };
    v.into_iter().map(String::from).collect()
}

/// Garante campos mínimos (assets, sons, UI, história) depois de qualquer alteração vinda da IA.
pub fn fill_defaults(spec: &mut GameSpec) {
    let lang = spec.lang.clone();
    if spec.ui.is_empty() || !spec.ui.contains_key("score") {
        let mut ui = ui_strings(&lang);
        ui.extend(std::mem::take(&mut spec.ui));
        spec.ui = ui;
    }
    if spec.title.trim().is_empty() {
        spec.title = "Meu Jogo".into();
    }
    if spec.enemies.is_empty() {
        spec.enemies.push(Enemy { name: if lang == "en" { "Monster".into() } else { "Monstro".into() }, description: String::new() });
    }
    if spec.items.is_empty() {
        spec.items.push(Item { name: if lang == "en" { "Relic".into() } else { "Relíquia".into() }, description: String::new() });
    }
    if spec.npcs.is_empty() {
        spec.npcs.push(Npc { name: if lang == "en" { "Elder".into() } else { "Ancião".into() }, role: "quest".into(), lines: vec!["...".into()] });
    }
    if spec.places.len() < 3 {
        let t = THEMES.iter().find(|t| t.id == spec.theme).unwrap_or(&THEMES[0]);
        spec.places = t.places.iter().map(|p| tr(&lang, *p).to_string()).collect();
    }
    if spec.genre == Genre::VisualNovel && spec.story.is_empty() {
        spec.story = crate::story::generate(spec);
    }
    // Remove duplicatas mantendo a ordem (a IA às vezes repete ids)
    spec.features.dedup();
    spec.assets = merge_assets(default_assets(spec), std::mem::take(&mut spec.assets));
    spec.sounds = merge_sounds(default_sounds(spec.genre), std::mem::take(&mut spec.sounds));
}

fn merge_assets(defaults: Vec<AssetReq>, custom: Vec<AssetReq>) -> Vec<AssetReq> {
    let mut out = defaults;
    for mut c in custom {
        c.id = slugify(&c.id).replace('-', "_");
        if c.id == "jogo" || c.id.is_empty() {
            continue;
        }
        if c.w == 0 {
            c.w = 64;
        }
        if c.h == 0 {
            c.h = 64;
        }
        c.w = c.w.clamp(8, 1024);
        c.h = c.h.clamp(8, 1024);
        if c.kind.is_empty() {
            c.kind = "sprite".into();
        }
        if let Some(e) = out.iter_mut().find(|a| a.id == c.id) {
            if !c.prompt.is_empty() {
                e.prompt = c.prompt;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn merge_sounds(defaults: Vec<SoundReq>, custom: Vec<SoundReq>) -> Vec<SoundReq> {
    let mut out = defaults;
    for mut c in custom {
        c.id = slugify(&c.id).replace('-', "_");
        if c.id == "jogo" || c.id.is_empty() {
            continue;
        }
        if c.kind.is_empty() {
            c.kind = "sfx".into();
        }
        if let Some(e) = out.iter_mut().find(|a| a.id == c.id) {
            if !c.prompt.is_empty() {
                e.prompt = c.prompt;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn a(id: &str, kind: &str, prompt: String, w: u32, h: u32) -> AssetReq {
    AssetReq { id: id.into(), kind: kind.into(), prompt, w, h }
}

/// Lista de assets que cada gênero espera encontrar (os templates têm fallback caso falte algum).
pub fn default_assets(spec: &GameSpec) -> Vec<AssetReq> {
    let st = &spec.art_style;
    let th = &spec.setting;
    let pl = format!("{}, {}, {}", spec.player.name, spec.player.description, st);
    let en0 = spec.enemies.first().map(|e| e.name.clone()).unwrap_or_default();
    let en1 = spec.enemies.get(1).map(|e| e.name.clone()).unwrap_or_else(|| en0.clone());
    let it = spec.items.first().map(|i| i.name.clone()).unwrap_or_default();
    let boss = spec.boss.name.clone();
    let npc = spec.npcs.first().map(|n| n.name.clone()).unwrap_or_default();
    match spec.genre {
        Genre::VisualNovel => {
            let mut v = vec![
                a("bg_a", "background", format!("{}, {}, background scene, {}", spec.places[0], th, st), 800, 450),
                a("bg_b", "background", format!("{}, {}, background scene, {}", spec.places[1], th, st), 800, 450),
                a("bg_c", "background", format!("{}, {}, background scene, {}", spec.places[2], th, st), 800, 450),
                a("char_ally", "portrait", format!("{}, character portrait, {}", spec.npcs.first().map(|n| n.name.as_str()).unwrap_or("ally"), st), 256, 384),
                a("char_rival", "portrait", format!("{}, rival character portrait, {}", boss, st), 256, 384),
                a("char_player", "portrait", format!("{}, character portrait, {}", pl, st), 256, 384),
            ];
            v.push(a("logo", "item", format!("{} title logo", spec.title), 64, 64));
            v
        }
        Genre::Fps => vec![
            a("wall1", "texture", format!("{} wall texture, seamless, {}", th, st), 64, 64),
            a("wall2", "texture", format!("{} brick wall texture, seamless, {}", th, st), 64, 64),
            a("wall3", "texture", format!("{} metal wall texture, seamless, {}", th, st), 64, 64),
            a("enemy", "sprite", format!("{}, enemy, front view, {}", en0, st), 64, 64),
            a("item", "item", format!("{}, pickup, {}", it, st), 32, 32),
            a("health", "item", format!("health pack, {}", st), 32, 32),
            a("ammo", "item", format!("ammo box, {}", st), 32, 32),
        ],
        Genre::Arcade => vec![
            a("player", "sprite", format!("{} spaceship, top view, {}", spec.player.name, st), 48, 48),
            a("enemy", "sprite", format!("{}, enemy ship, top view, {}", en0, st), 48, 48),
            a("enemy2", "sprite", format!("{}, enemy, top view, {}", en1, st), 48, 48),
            a("boss", "sprite", format!("{}, boss, top view, {}", boss, st), 128, 128),
            a("item", "item", format!("{}, power-up, {}", it, st), 32, 32),
            a("bg", "background", format!("{}, starfield background, {}", th, st), 640, 480),
        ],
        Genre::Rpg2d | Genre::Rpg3d => vec![
            a("player", "sprite", format!("{}, front view, {}", pl, st), 32, 32),
            a("npc", "sprite", format!("{}, villager, {}", npc, st), 32, 32),
            a("enemy", "sprite", format!("{}, enemy, {}", en0, st), 32, 32),
            a("enemy2", "sprite", format!("{}, enemy, {}", en1, st), 32, 32),
            a("boss", "sprite", format!("{}, boss, {}", boss, st), 64, 64),
            a("item", "item", format!("{}, collectible relic, {}", it, st), 32, 32),
            a("tile_floor", "tile", format!("{} ground tile, {}", th, st), 32, 32),
            a("tile_wall", "tile", format!("{} wall or tree tile, {}", th, st), 32, 32),
            a("tile_water", "tile", format!("water tile, {}", st), 32, 32),
        ],
        Genre::Platformer2d | Genre::Platformer3d | Genre::Tps => vec![
            a("player", "sprite", format!("{}, side view, {}", pl, st), 32, 32),
            a("enemy", "sprite", format!("{}, enemy, side view, {}", en0, st), 32, 32),
            a("enemy2", "sprite", format!("{}, flying enemy, {}", en1, st), 32, 32),
            a("item", "item", format!("{}, collectible coin or gem, {}", it, st), 32, 32),
            a("tile", "tile", format!("{} ground tile, {}", th, st), 32, 32),
            a("tile_top", "tile", format!("{} grass top tile, {}", th, st), 32, 32),
            a("goal", "item", format!("goal flag, {}", st), 32, 64),
            a("bg", "background", format!("{}, parallax background, {}", th, st), 640, 360),
        ],
    }
}

pub fn default_sounds(genre: Genre) -> Vec<SoundReq> {
    let s = |id: &str, kind: &str, p: &str| SoundReq { id: id.into(), kind: kind.into(), prompt: p.into() };
    let mut v = vec![
        s("music", "music", "calm looping chiptune adventure theme"),
        s("coin", "sfx", "short bright coin pickup"),
        s("hit", "sfx", "short impact hit"),
        s("win", "sfx", "victory jingle"),
        s("lose", "sfx", "sad defeat jingle"),
    ];
    match genre {
        Genre::VisualNovel => {
            v.push(s("blip", "sfx", "tiny text blip"));
            v.push(s("choice", "sfx", "soft menu select"));
        }
        Genre::Fps | Genre::Tps | Genre::Arcade => {
            v.push(s("shoot", "sfx", "laser shot"));
            v.push(s("explosion", "sfx", "small explosion"));
            v.push(s("jump", "sfx", "jump whoosh"));
        }
        Genre::Rpg2d | Genre::Rpg3d => {
            v.push(s("blip", "sfx", "dialogue blip"));
            v.push(s("attack", "sfx", "sword swing"));
            v.push(s("level_up", "sfx", "level up arpeggio"));
        }
        _ => {
            v.push(s("jump", "sfx", "jump boing"));
            v.push(s("stomp", "sfx", "enemy stomp"));
        }
    }
    v
}

pub fn ui_strings(lang: &str) -> BTreeMap<String, String> {
    let pairs: Vec<(&str, &str, &str)> = vec![
        ("score", "Pontos", "Score"),
        ("lives", "Vidas", "Lives"),
        ("hp", "Vida", "HP"),
        ("level", "Nível", "Level"),
        ("coins", "Moedas", "Coins"),
        ("relics", "Relíquias", "Relics"),
        ("ammo", "Munição", "Ammo"),
        ("wave", "Onda", "Wave"),
        ("enemies", "Inimigos", "Enemies"),
        ("press_start", "Pressione ENTER ou clique para começar", "Press ENTER or click to start"),
        ("paused", "Pausado", "Paused"),
        ("game_over", "Fim de jogo", "Game over"),
        ("you_win", "Você venceu!", "You win!"),
        ("restart", "Pressione R para reiniciar", "Press R to restart"),
        ("next_level", "Próxima fase!", "Next level!"),
        ("talk", "Falar", "Talk"),
        ("continue", "Continuar", "Continue"),
        ("click_continue", "clique ou ENTER para continuar", "click or ENTER to continue"),
        ("ending", "Final", "Ending"),
        ("play_again", "Jogar de novo", "Play again"),
        ("muted", "Som desligado", "Sound off"),
        ("loading", "Carregando...", "Loading..."),
        ("boss_awake", "O chefe despertou!", "The boss has awakened!"),
        ("find_relics", "Colete todas as relíquias para despertar o chefe.", "Collect all relics to awaken the boss."),
        ("level_up", "Subiu de nível!", "Level up!"),
        ("exit_found", "Encontre a saída!", "Find the exit!"),
        ("checkpoint", "Checkpoint!", "Checkpoint!"),
    ];
    let mut m: BTreeMap<String, String> = pairs.iter().map(|(k, pt, en)| (k.to_string(), (if lang == "en" { *en } else { *pt }).to_string())).collect();
    let controls: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        ("controls_rpg", ("WASD/Setas: mover • ESPAÇO: atacar/falar • E: interagir • M: som • P: pausa", "WASD/Arrows: move • SPACE: attack/talk • E: interact • M: sound • P: pause")),
        ("controls_rpg3d", ("WASD: mover • ESPAÇO: atacar/falar • Q/E: girar câmera • M: som • P: pausa", "WASD: move • SPACE: attack/talk • Q/E: rotate camera • M: sound • P: pause")),
        ("controls_vn", ("Clique/ENTER/ESPAÇO: avançar • 1-3: escolher • M: som", "Click/ENTER/SPACE: advance • 1-3: choose • M: sound")),
        ("controls_platformer", ("←/→ ou A/D: mover • ESPAÇO/↑: pular • M: som • P: pausa", "←/→ or A/D: move • SPACE/↑: jump • M: sound • P: pause")),
        ("controls_fps", ("WASD: mover • Mouse: olhar • Clique/ESPAÇO: atirar • M: som", "WASD: move • Mouse: look • Click/SPACE: shoot • M: sound")),
        ("controls_tps", ("WASD: mover • ←/→: câmera • ESPAÇO: atirar • SHIFT: correr • M: som", "WASD: move • ←/→: camera • SPACE: shoot • SHIFT: run • M: sound")),
        ("controls_arcade", ("←/→/↑/↓ ou WASD: mover • ESPAÇO: atirar • M: som", "←/→/↑/↓ or WASD: move • SPACE: shoot • M: sound")),
    ]);
    for (k, (pt, en)) in controls {
        m.insert(k.to_string(), (if lang == "en" { en } else { pt }).to_string());
    }
    m
}

/// Aplica um JSON parcial (vindo da IA) sobre a spec atual e normaliza. Em caso de erro mantém a spec original.
pub fn merge_patch(spec: &GameSpec, patch: &Value) -> GameSpec {
    let mut base = match serde_json::to_value(spec) {
        Ok(v) => v,
        Err(_) => return spec.clone(),
    };
    let mut patch = patch.clone();
    // A IA não deve alterar seed/prompt/genre nem os mundos gerados
    if let Some(o) = patch.as_object_mut() {
        for k in ["seed", "prompt", "world", "ui"] {
            o.remove(k);
        }
        // Só aceita gênero conhecido
        if let Some(g) = o.get("genre").and_then(|g| g.as_str()) {
            if Genre::from_id(g).is_none() {
                o.remove("genre");
            }
        }
        // Listas devem ser listas: descarta tipos errados em vez de falhar tudo
        for k in ["npcs", "enemies", "items", "places", "features", "mechanics", "story", "assets", "sounds"] {
            if o.get(k).is_some_and(|v| !v.is_array()) {
                o.remove(k);
            }
        }
    }
    deep_merge(&mut base, &patch);
    match serde_json::from_value::<GameSpec>(base) {
        Ok(mut s) => {
            s.world = spec.world.clone();
            s.seed = spec.seed;
            s.prompt = spec.prompt.clone();
            s.ui = spec.ui.clone();
            fill_defaults(&mut s);
            s
        }
        Err(_) => spec.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_genres() {
        assert_eq!(detect_genre("Um RPG 2D de fantasia"), Genre::Rpg2d);
        assert_eq!(detect_genre("rpg 3d em um castelo"), Genre::Rpg3d);
        assert_eq!(detect_genre("uma visual novel de romance"), Genre::VisualNovel);
        assert_eq!(detect_genre("jogo de plataforma 3D com lava"), Genre::Platformer3d);
        assert_eq!(detect_genre("plataforma"), Genre::Platformer2d);
        assert_eq!(detect_genre("FPS em primeira pessoa"), Genre::Fps);
        assert_eq!(detect_genre("shooter em terceira pessoa"), Genre::Tps);
        assert_eq!(detect_genre("jogo de nave estilo arcade"), Genre::Arcade);
    }

    #[test]
    fn spec_is_complete() {
        for g in Genre::ALL {
            let s = heuristic_spec("Um jogo no deserto com chefe", Some(g), None);
            assert!(!s.title.is_empty());
            assert!(s.assets.iter().any(|a| a.id.contains("player") || a.id.starts_with("char") || a.id.starts_with("wall")), "{:?}", g);
            assert!(!s.sounds.is_empty());
            assert_eq!(s.theme, "deserto");
            if g == Genre::VisualNovel {
                assert!(s.story.len() >= 10);
            }
        }
    }

    #[test]
    fn quoted_title_wins() {
        let s = heuristic_spec("Crie um jogo chamado \"Pixel Quest\" com neon", None, None);
        assert_eq!(s.title, "Pixel Quest");
        assert_eq!(s.theme, "cyberpunk");
    }

    #[test]
    fn merge_patch_is_resilient() {
        let s = heuristic_spec("jogo de plataforma", None, None);
        let p = serde_json::json!({"title": "Novo", "npcs": "lixo", "genre": "xpto", "palette": {"primary": "#ff0000"}});
        let m = merge_patch(&s, &p);
        assert_eq!(m.title, "Novo");
        assert_eq!(m.genre, s.genre);
        assert_eq!(m.palette.primary, "#ff0000");
        assert!(!m.npcs.is_empty());
    }
}
