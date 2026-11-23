//! Style presets: portable "looks" (art style + palette + image-prompt hints) that can be exported to a
//! `.gfstyle.json` file, shared, imported, and reused across projects.
//!
//! File format (version 1):
//! ```json
//! { "format": "gameflow-style-preset", "version": 1, "name": "Neon Noir", "description": "…",
//!   "author": "…", "art_style": "pixel art, neon cyberpunk", "prompt_suffix": "high contrast, glowing edges",
//!   "negative_prompt": "blurry, text", "palette": { "bg": "#0b0716", … 8 colors … } }
//! ```

use crate::spec::{GameSpec, Palette};
use crate::util::{now_iso, slugify};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const FORMAT: &str = "gameflow-style-preset";
pub const VERSION: u32 = 1;
pub const MAX_BYTES: usize = 256 * 1024;
pub const FILE_SUFFIX: &str = ".gfstyle.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StylePreset {
    /// Must be present in files; `serde(default)` here is the empty string so that random JSON is not accepted.
    #[serde(default = "String::new")]
    pub format: String,
    #[serde(default)]
    pub version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    /// Visual style sentence used by the artist agent and to decide pixel/smooth rendering ("pixel art…" → crisp pixels).
    pub art_style: String,
    /// Extra words appended to every image prompt.
    pub prompt_suffix: String,
    pub negative_prompt: String,
    pub palette: Palette,
    pub created_at: String,
    /// Shipped with GameFlow (read-only). Never read from files.
    #[serde(skip_deserializing)]
    pub builtin: bool,
}

impl Default for StylePreset {
    fn default() -> Self {
        StylePreset {
            format: FORMAT.into(),
            version: VERSION,
            id: String::new(),
            name: String::new(),
            description: String::new(),
            author: String::new(),
            art_style: "pixel art".into(),
            prompt_suffix: String::new(),
            negative_prompt: String::new(),
            palette: Palette::default(),
            created_at: String::new(),
            builtin: false,
        }
    }
}

fn clip(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == ' ').take(max).collect::<String>().trim().to_string()
}

/// Normalizes `#abc`/`#aabbcc`/`aabbcc` to `#aabbcc`; None if invalid.
pub fn normalize_color(c: &str) -> Option<String> {
    let h = c.trim().trim_start_matches('#');
    let h = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => h.to_string(),
        _ => return None,
    };
    h.chars().all(|c| c.is_ascii_hexdigit()).then(|| format!("#{}", h.to_lowercase()))
}

impl StylePreset {
    /// Validates and cleans an untrusted preset (imported file, API body). Keeps `id` empty — the library assigns it.
    pub fn sanitized(mut self) -> Result<StylePreset> {
        if self.format != FORMAT {
            bail!("este arquivo não é um estilo do GameFlow (falta \"format\": \"{FORMAT}\")");
        }
        if self.version == 0 || self.version > VERSION {
            bail!("versão {} do estilo não suportada (este GameFlow entende até a {VERSION}); atualize com `gameflow update`", self.version);
        }
        self.name = clip(&self.name, 60);
        if self.name.is_empty() {
            bail!("o estilo precisa de um nome");
        }
        self.description = clip(&self.description, 300);
        self.author = clip(&self.author, 60);
        self.art_style = clip(&self.art_style, 200);
        if self.art_style.is_empty() {
            bail!("o estilo precisa de um \"art_style\"");
        }
        self.prompt_suffix = clip(&self.prompt_suffix, 300);
        self.negative_prompt = clip(&self.negative_prompt, 300);
        let p = &mut self.palette;
        for (label, c) in [
            ("bg", &mut p.bg),
            ("bg2", &mut p.bg2),
            ("ground", &mut p.ground),
            ("primary", &mut p.primary),
            ("secondary", &mut p.secondary),
            ("accent", &mut p.accent),
            ("danger", &mut p.danger),
            ("text", &mut p.text),
        ] {
            *c = normalize_color(c).ok_or_else(|| anyhow!("cor inválida em palette.{label}: use #rrggbb"))?;
        }
        self.format = FORMAT.into();
        self.version = VERSION;
        self.id.clear();
        self.builtin = false;
        Ok(self)
    }

    /// Captures the current look of a project (its `game.json`) as a preset.
    pub fn from_spec(spec: &GameSpec, name: &str, description: &str) -> Result<StylePreset> {
        StylePreset {
            name: name.into(),
            description: description.into(),
            art_style: spec.art_style.clone(),
            prompt_suffix: spec.style.prompt_suffix.clone(),
            negative_prompt: spec.style.negative_prompt.clone(),
            palette: spec.palette.clone(),
            created_at: now_iso(),
            ..Default::default()
        }
        .sanitized()
    }

    pub fn to_file_json(&self) -> String {
        let mut p = self.clone();
        p.builtin = false;
        serde_json::to_string_pretty(&p).unwrap_or_default() + "\n"
    }
}

fn pal(c: [&str; 8]) -> Palette {
    Palette { bg: c[0].into(), bg2: c[1].into(), ground: c[2].into(), primary: c[3].into(), secondary: c[4].into(), accent: c[5].into(), danger: c[6].into(), text: c[7].into() }
}

/// Presets that ship with GameFlow.
pub fn builtin() -> Vec<StylePreset> {
    let mk = |id: &str, name: &str, desc: &str, art: &str, suffix: &str, neg: &str, c: [&str; 8]| StylePreset {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        author: "GameFlow".into(),
        art_style: art.into(),
        prompt_suffix: suffix.into(),
        negative_prompt: neg.into(),
        palette: pal(c),
        builtin: true,
        ..Default::default()
    };
    vec![
        mk(
            "neon-noir",
            "Neon Noir",
            "Noites escuras na cidade com magenta e ciano brilhantes.",
            "pixel art, neon cyberpunk, dark background",
            "high contrast, glowing edges, limited palette",
            "blurry, photo, text, watermark",
            ["#0d0820", "#241447", "#2f1b5c", "#00e5ff", "#ff2d95", "#f5ff3b", "#ff3d3d", "#f2e9ff"],
        ),
        mk(
            "cozy-pastel",
            "Pastel Aconchegante",
            "Suave, quente e amigável — bom para jogos relaxantes.",
            "pixel art, soft pastel colors, cozy and cute",
            "soft shading, rounded shapes, gentle colors",
            "dark, gritty, horror, text",
            ["#3b3552", "#564a73", "#8fbf9f", "#f2a7b8", "#a5d8e6", "#ffe29a", "#e86b7c", "#fff6ee"],
        ),
        mk(
            "gameboy-green",
            "Verde Portátil",
            "Quatro tons de verde, como num portátil clássico.",
            "pixel art, 4-color monochrome green, retro handheld",
            "strictly four green shades, chunky pixels",
            "gradients, smooth, realistic, text",
            ["#0f380f", "#306230", "#8bac0f", "#9bbc0f", "#8bac0f", "#e0f8d0", "#306230", "#e0f8d0"],
        ),
        mk(
            "storybook-watercolor",
            "Aquarela de Livro Infantil",
            "Visual pintado à mão, com bordas suaves (sem pixels).",
            "hand-painted watercolor storybook illustration, smooth",
            "soft brush strokes, paper texture, warm light",
            "pixelated, 3d render, text",
            ["#2d3a3a", "#476055", "#9bb58a", "#d98c5f", "#7fa7b8", "#f2d27a", "#c0504d", "#fbf3e4"],
        ),
        mk(
            "ember-dusk",
            "Crepúsculo de Brasa",
            "Pores do sol quentes, lava e brasas.",
            "pixel art, volcanic sunset, warm ember glow",
            "warm rim light, dramatic silhouettes",
            "cold colors, blurry, text",
            ["#1c0a0a", "#4a1b14", "#6b3a2c", "#ff9f43", "#ffd166", "#fff1a8", "#ff3b30", "#ffece0"],
        ),
    ]
}

#[derive(Clone)]
pub struct Presets {
    dir: PathBuf,
}

impl Presets {
    pub fn new(dir: PathBuf) -> Self {
        Presets { dir }
    }

    pub fn list(&self) -> Vec<StylePreset> {
        let mut out = builtin();
        let mut user: Vec<StylePreset> = std::fs::read_dir(&self.dir).map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| self.read_file(&e.path())).collect()).unwrap_or_default();
        user.sort_by_key(|p| p.name.to_lowercase());
        out.extend(user);
        out
    }

    fn read_file(&self, path: &std::path::Path) -> Option<StylePreset> {
        let name = path.file_name()?.to_string_lossy().to_string();
        let id = name.strip_suffix(FILE_SUFFIX)?;
        let text = std::fs::read_to_string(path).ok()?;
        let mut p = serde_json::from_str::<StylePreset>(&text).ok()?.sanitized().ok()?;
        p.id = id.to_string();
        Some(p)
    }

    pub fn get(&self, id: &str) -> Option<StylePreset> {
        self.list().into_iter().find(|p| p.id == id)
    }

    fn free_id(&self, name: &str) -> String {
        let base = slugify(name);
        let taken: Vec<String> = self.list().into_iter().map(|p| p.id).collect();
        let mut id = base.clone();
        let mut n = 2;
        while taken.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// Imports untrusted preset text (a `.gfstyle.json` file) into the library.
    pub fn import_text(&self, text: &str) -> Result<StylePreset> {
        if text.len() > MAX_BYTES {
            bail!("arquivo de estilo grande demais (máx. {} KB)", MAX_BYTES / 1024);
        }
        let raw: StylePreset = serde_json::from_str(text).map_err(|e| anyhow!("arquivo de estilo inválido: {e}"))?;
        self.save(raw.sanitized()?)
    }

    /// Saves an already-sanitized preset under a fresh id (never overwrites).
    pub fn save(&self, mut p: StylePreset) -> Result<StylePreset> {
        std::fs::create_dir_all(&self.dir)?;
        p.id = self.free_id(&p.name);
        if p.created_at.is_empty() {
            p.created_at = now_iso();
        }
        std::fs::write(self.dir.join(format!("{}{FILE_SUFFIX}", p.id)), p.to_file_json())?;
        Ok(p)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        if builtin().iter().any(|p| p.id == id) {
            bail!("estilos embutidos não podem ser excluídos");
        }
        let valid = !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let path = self.dir.join(format!("{id}{FILE_SUFFIX}"));
        if !valid || !path.is_file() {
            bail!("estilo não encontrado");
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::heuristic_spec;

    fn lib() -> Presets {
        Presets::new(std::env::temp_dir().join(format!("gf-presets-{}", uuid::Uuid::new_v4())))
    }

    #[test]
    fn builtins_are_valid_and_unique() {
        let b = builtin();
        let mut ids: Vec<_> = b.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), b.len());
        for p in b {
            let id = p.id.clone();
            let again = StylePreset { builtin: false, ..p }.sanitized().unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(!again.name.is_empty());
        }
    }

    #[test]
    fn export_import_roundtrip() {
        let l = lib();
        let spec = heuristic_spec("jogo de plataforma no gelo", None, None);
        let p = StylePreset::from_spec(&spec, "Meu Gelo", "frio e limpo").unwrap();
        let saved = l.save(p).unwrap();
        assert_eq!(saved.id, "meu-gelo");
        let text = l.get("meu-gelo").unwrap().to_file_json();
        // outra máquina importa o arquivo: mesmo conteúdo, id novo sem sobrescrever
        let other = lib();
        let imported = other.import_text(&text).unwrap();
        assert_eq!(imported.name, "Meu Gelo");
        assert_eq!(imported.palette.primary, spec.palette.primary);
        assert_eq!(imported.art_style, spec.art_style);
        let twice = other.import_text(&text).unwrap();
        assert_eq!(twice.id, "meu-gelo-2");
        assert_eq!(other.list().len(), builtin().len() + 2);
    }

    #[test]
    fn import_rejects_bad_input() {
        let l = lib();
        assert!(l.import_text("not json").is_err());
        assert!(l.import_text("{}").is_err(), "missing format");
        let mut no_format: serde_json::Value = serde_json::from_str(&builtin()[0].to_file_json()).unwrap();
        no_format.as_object_mut().unwrap().remove("format");
        assert!(l.import_text(&no_format.to_string()).unwrap_err().to_string().contains("não é um estilo"), "a file without \"format\" must be rejected");
        let mut p = builtin()[0].clone();
        p.version = 99;
        assert!(l.import_text(&p.to_file_json()).unwrap_err().to_string().contains("versão"));
        let mut p = builtin()[0].clone();
        p.palette.accent = "red; background:url(x)".into();
        assert!(l.import_text(&p.to_file_json()).unwrap_err().to_string().contains("palette.accent"));
        let mut p = builtin()[0].clone();
        p.name = "   ".into();
        assert!(l.import_text(&p.to_file_json()).is_err());
        assert!(l.import_text(&" ".repeat(MAX_BYTES + 1)).is_err());
    }

    #[test]
    fn import_cannot_choose_its_id_or_builtin_flag() {
        let l = lib();
        let mut p = builtin()[0].clone();
        p.id = "../../etc/passwd".into();
        let mut json: serde_json::Value = serde_json::from_str(&p.to_file_json()).unwrap();
        json["builtin"] = true.into();
        let saved = l.import_text(&json.to_string()).unwrap();
        assert_eq!(saved.id, "neon-noir-2", "id is assigned by the library");
        assert!(!saved.builtin);
        assert!(l.delete("neon-noir").is_err(), "built-ins are protected");
        assert!(l.delete("../x").is_err());
        l.delete("neon-noir-2").unwrap();
    }

    #[test]
    fn colors_are_normalized() {
        assert_eq!(normalize_color("#ABC").as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_color("FFAA00").as_deref(), Some("#ffaa00"));
        assert!(normalize_color("#12345").is_none());
        assert!(normalize_color("#gggggg").is_none());
    }
}
