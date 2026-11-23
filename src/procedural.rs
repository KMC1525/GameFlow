//! Arte (PNG pixel-art) e sons (WAV chiptune) procedurais.
//!
//! São o "plano B" dos agentes Artista e Compositor: quando nenhum modelo de imagem/som está configurado
//! (ou ele falha), o jogo continua completo e coerente com a paleta/tema do projeto.

use crate::spec::{AssetReq, Palette};
use crate::util::*;
use image::{ImageBuffer, Rgba, RgbaImage};

type C = [u8; 4];

fn rgb(hex: &str) -> C {
    let (r, g, b) = parse_hex(hex);
    [r, g, b, 255]
}
fn mixc(a: C, b: C, t: f32) -> C {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), 255]
}
fn dark(c: C, t: f32) -> C {
    mixc(c, [0, 0, 0, 255], t)
}
fn light(c: C, t: f32) -> C {
    mixc(c, [255, 255, 255, 255], t)
}
fn lum(c: C) -> f32 {
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

/// Tela de pixels de baixa resolução, ampliada com "vizinho mais próximo" no final (visual pixel-art nítido).
struct Px {
    w: i32,
    h: i32,
    d: Vec<C>,
}

impl Px {
    fn new(w: i32, h: i32) -> Self {
        Px { w, h, d: vec![[0, 0, 0, 0]; (w * h) as usize] }
    }
    fn fill(&mut self, c: C) {
        self.d.iter_mut().for_each(|p| *p = c);
    }
    fn set(&mut self, x: i32, y: i32, c: C) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.d[(y * self.w + x) as usize] = c;
        }
    }
    fn get(&self, x: i32, y: i32) -> C {
        if x >= 0 && y >= 0 && x < self.w && y < self.h { self.d[(y * self.w + x) as usize] } else { [0, 0, 0, 0] }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: C) {
        for y in 0..self.h {
            for x in 0..self.w {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, c);
                }
            }
        }
    }
    /// Contorno de 1px ao redor de tudo que é opaco.
    fn outline(&mut self, c: C) {
        let src = self.d.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                if src[(y * self.w + x) as usize][3] == 0 {
                    let n = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                        let (nx, ny) = (x + dx, y + dy);
                        nx >= 0 && ny >= 0 && nx < self.w && ny < self.h && src[(ny * self.w + nx) as usize][3] > 0
                    });
                    if n {
                        self.set(x, y, c);
                    }
                }
            }
        }
    }
    fn mirror_x(&mut self) {
        for y in 0..self.h {
            for x in 0..self.w / 2 {
                let c = self.get(x, y);
                self.set(self.w - 1 - x, y, c);
            }
        }
    }
    fn scale_to(&self, tw: u32, th: u32) -> RgbaImage {
        ImageBuffer::from_fn(tw, th, |x, y| {
            let sx = ((x as u64 * self.w as u64) / tw as u64) as i32;
            let sy = ((y as u64 * self.h as u64) / th as u64) as i32;
            Rgba(self.get(sx, sy))
        })
    }
}

struct Pal {
    bg: C,
    bg2: C,
    ground: C,
    primary: C,
    secondary: C,
    accent: C,
    danger: C,
    text: C,
}

impl Pal {
    fn from(p: &Palette) -> Pal {
        Pal { bg: rgb(&p.bg), bg2: rgb(&p.bg2), ground: rgb(&p.ground), primary: rgb(&p.primary), secondary: rgb(&p.secondary), accent: rgb(&p.accent), danger: rgb(&p.danger), text: rgb(&p.text) }
    }
}

const SKINS: [&str; 6] = ["#f1c9a5", "#e0ac84", "#c68a60", "#8d5a3b", "#f7d9c4", "#a3c97a"];

// ───────────────────────── Imagens ─────────────────────────

/// Gera a imagem para um pedido de asset. Determinística para (seed, id).
pub fn generate_image(req: &AssetReq, palette: &Palette, seed: u64) -> RgbaImage {
    let pal = Pal::from(palette);
    let mut rng = Rng::new(seed ^ hash_str(&req.id));
    let id = req.id.as_str();
    let prompt = normalize(&req.prompt);
    let (w, h) = (req.w.max(8), req.h.max(8));
    let ship = has_any(&prompt, &["spaceship", "ship", "nave", "top view"]) && !has_any(&prompt, &["front view", "side view"]);
    match req.kind.as_str() {
        "background" => background(&pal, &mut rng, w, h, id),
        "portrait" => portrait(&pal, &mut rng, w, h, id),
        "texture" => texture(&pal, &mut rng, w, h, id),
        "tile" => tile(&pal, &mut rng, w, h, id),
        "item" => item(&pal, &mut rng, w, h, id),
        _ => {
            // sprite
            let boss = id.contains("boss");
            let grid = if boss { 32 } else { 16 };
            let mut px = Px::new(grid, grid);
            if ship {
                ship_sprite(&mut px, &pal, &mut rng, id.contains("player"), boss);
            } else if id.contains("player") || id.contains("hero") {
                humanoid(&mut px, &pal, &mut rng, pal.primary, true);
            } else if id.contains("npc") {
                humanoid(&mut px, &pal, &mut rng, pal.secondary, false);
            } else {
                let color = if id.contains("2") { mixc(pal.danger, pal.accent, 0.45) } else { pal.danger };
                monster(&mut px, &pal, &mut rng, color, boss);
            }
            let (tw, th) = (w, h);
            px.scale_to(tw, th)
        }
    }
}

fn humanoid(px: &mut Px, pal: &Pal, rng: &mut Rng, cloth: C, hero: bool) {
    let skin = rgb(rng.pick(&SKINS));
    let hair = if hero { dark(pal.secondary, 0.35) } else { *rng.pick(&[dark(pal.accent, 0.5), dark(pal.ground, 0.4), [60, 40, 30, 255], light(pal.secondary, 0.2)]) };
    // pernas e sapatos
    px.rect(5, 11, 2, 3, dark(pal.bg2, 0.2));
    px.rect(9, 11, 2, 3, dark(pal.bg2, 0.2));
    px.rect(4, 14, 3, 1, dark(cloth, 0.6));
    px.rect(9, 14, 3, 1, dark(cloth, 0.6));
    // tronco e braços
    px.rect(4, 7, 8, 5, cloth);
    px.rect(4, 7, 8, 1, light(cloth, 0.25));
    px.rect(4, 11, 8, 1, dark(cloth, 0.3));
    px.rect(3, 8, 1, 3, skin);
    px.rect(12, 8, 1, 3, skin);
    // cabeça
    px.rect(5, 2, 6, 5, skin);
    px.rect(5, 1, 6, 2, hair);
    px.rect(4, 2, 1, 3, hair);
    px.rect(11, 2, 1, 3, hair);
    px.set(6, 4, [20, 20, 30, 255]);
    px.set(9, 4, [20, 20, 30, 255]);
    px.set(7, 5, dark(skin, 0.25));
    px.set(8, 5, dark(skin, 0.25));
    if hero {
        // lenço/capa para dar identidade
        px.rect(5, 6, 6, 1, pal.accent);
        px.rect(3, 7, 1, 4, dark(pal.accent, 0.2));
    }
    px.outline(dark(cloth, 0.75));
}

fn monster(px: &mut Px, pal: &Pal, rng: &mut Rng, color: C, boss: bool) {
    let n = px.w;
    let half = n / 2;
    let margin = if boss { 3 } else { 2 };
    for y in margin..n - margin {
        for x in margin..half {
            let (cx, cy) = ((x - margin) as f32 / (half - margin) as f32, (y - margin) as f32 / (n - 2 * margin) as f32);
            let blob = 1.0 - ((1.0 - cx).powi(2) * 0.6 + (cy - 0.5).powi(2) * 3.2).sqrt();
            if blob > 0.0 && (rng.f32() < 0.25 + blob * 0.9) {
                px.set(x, y, color);
            }
        }
    }
    // centro sólido para evitar sprites vazios
    for y in n / 4..n * 3 / 4 {
        for x in n / 4..half {
            px.set(x, y, color);
        }
    }
    px.mirror_x();
    // sombreamento simples
    for y in 0..n {
        for x in 0..n {
            let c = px.get(x, y);
            if c[3] > 0 {
                if y > n * 2 / 3 {
                    px.set(x, y, dark(c, 0.25));
                } else if y < n / 3 && (x + y) % 2 == 0 {
                    px.set(x, y, light(c, 0.12));
                }
            }
        }
    }
    let ey = n * 2 / 5;
    let (ex1, ex2) = (n / 2 - n / 5, n / 2 + n / 5 - 1);
    let s = if boss { 2 } else { 1 };
    for (ex, _) in [(ex1, 0), (ex2, 0)] {
        px.rect(ex, ey, s + 1, s + 1, [255, 255, 255, 255]);
        px.rect(ex + 1, ey + s - 1, 1, 1 + (s - 1), [10, 10, 20, 255]);
    }
    // boca
    px.rect(n / 2 - n / 6, ey + s * 3, n / 3, 1, dark(color, 0.7));
    if boss {
        // coroa/chifres
        for k in 0..3 {
            px.rect(7 + k * 8, 2, 2, 4, pal.accent);
        }
    }
    px.outline(dark(color, 0.8));
}

fn ship_sprite(px: &mut Px, pal: &Pal, rng: &mut Rng, player: bool, boss: bool) {
    let n = px.w;
    let half = n / 2;
    let base = if player { pal.primary } else { pal.danger };
    for y in 1..n - 1 {
        // fuselagem afunila na frente
        let t = y as f32 / n as f32;
        let wdt = if player { (1.0 - (t - 0.2).abs() * 0.0).min(1.0) * (0.25 + t * 0.75) } else { 0.25 + (1.0 - t) * 0.75 };
        let reach = (wdt * half as f32).round() as i32;
        for x in (half - reach).max(1)..half {
            if x > half - 2 || rng.f32() < 0.9 {
                px.set(x, y, base);
            }
        }
    }
    px.rect(half - 2, 3, 2, n - 6, light(base, 0.15));
    // cabine
    let cy = if player { n / 3 } else { n * 2 / 3 };
    px.rect(half - 2, cy, 2, 3, if player { pal.accent } else { light(pal.accent, 0.2) });
    // asas
    for k in 0..(n / 4) {
        px.set(1 + k / 2, if player { n - 3 - k } else { 2 + k }, dark(base, 0.25));
    }
    px.mirror_x();
    if boss {
        px.rect(half - 3, n / 2, 6, 2, pal.accent);
    }
    px.outline(dark(base, 0.8));
}

fn item(pal: &Pal, rng: &mut Rng, w: u32, h: u32, id: &str) -> RgbaImage {
    let g = 16;
    let (gw, gh) = if h >= w * 2 { (8, 16) } else { (g, g) };
    let mut px = Px::new(gw, gh);
    if id == "goal" {
        px.rect(1, 0, 1, 16, [220, 220, 230, 255]);
        px.rect(2, 1, 5, 4, pal.danger);
        px.rect(2, 1, 5, 1, light(pal.danger, 0.3));
        px.rect(2, 5, 3, 1, dark(pal.danger, 0.3));
        px.outline([20, 20, 30, 255]);
    } else if id.contains("health") {
        px.rect(2, 2, 12, 12, [245, 245, 245, 255]);
        px.rect(6, 4, 4, 8, pal.danger);
        px.rect(4, 6, 8, 4, pal.danger);
        px.outline([40, 40, 50, 255]);
    } else if id.contains("ammo") {
        px.rect(2, 5, 12, 8, dark(pal.ground, 0.2));
        px.rect(2, 5, 12, 2, light(pal.ground, 0.15));
        for k in 0..4 {
            px.rect(3 + k * 3, 2, 2, 4, pal.accent);
        }
        px.outline([20, 20, 30, 255]);
    } else if id == "logo" {
        px.ellipse(8.0, 8.0, 7.0, 7.0, pal.primary);
        px.ellipse(8.0, 8.0, 4.0, 4.0, pal.accent);
        px.outline(dark(pal.primary, 0.7));
    } else {
        // gema / relíquia: losango com brilho
        let c = if rng.chance(0.5) { pal.accent } else { mixc(pal.accent, pal.secondary, 0.4) };
        for y in 0..16 {
            let span = if y < 8 { y } else { 15 - y };
            px.rect(8 - span.min(7) - 1, y, (span.min(7) + 1) * 2, 1, if y < 8 { light(c, 0.15) } else { dark(c, 0.15) });
        }
        px.rect(6, 4, 2, 2, [255, 255, 255, 220]);
        px.outline(dark(c, 0.75));
    }
    px.scale_to(w, h)
}

fn tile(pal: &Pal, rng: &mut Rng, w: u32, h: u32, id: &str) -> RgbaImage {
    let mut px = Px::new(16, 16);
    if id.contains("water") {
        let base = mixc([40, 100, 190, 255], pal.primary, 0.25);
        px.fill(base);
        for y in 0..16 {
            for x in 0..16 {
                if (x + y * 2 + (rng.f32() * 1.5) as i32) % 8 == 0 {
                    px.set(x, y, light(base, 0.25));
                }
            }
        }
    } else if id.contains("wall") {
        // árvore/arbusto denso (RPG) em bloco
        let base = dark(mixc(pal.ground, pal.primary, 0.4), 0.25);
        px.fill(base);
        for _ in 0..40 {
            let (x, y) = (rng.range(0, 15), rng.range(0, 15));
            px.rect(x, y, 2, 2, if rng.chance(0.5) { light(base, 0.18) } else { dark(base, 0.3) });
        }
        px.rect(0, 14, 16, 2, dark(base, 0.5));
    } else {
        let top = id.contains("top");
        let base = if top { mixc(pal.ground, pal.primary, 0.35) } else { pal.ground };
        px.fill(base);
        for _ in 0..30 {
            let (x, y) = (rng.range(0, 15), rng.range(0, 15));
            px.set(x, y, if rng.chance(0.5) { light(base, 0.14) } else { dark(base, 0.2) });
        }
        if top {
            px.rect(0, 0, 16, 3, light(base, 0.22));
            for x in 0..16 {
                if rng.chance(0.5) {
                    px.set(x, 3, light(base, 0.22));
                }
            }
        } else {
            px.rect(0, 15, 16, 1, dark(base, 0.25));
        }
    }
    px.scale_to(w, h)
}

fn texture(pal: &Pal, rng: &mut Rng, w: u32, h: u32, id: &str) -> RgbaImage {
    let n = 32;
    let mut px = Px::new(n, n);
    let variant = id.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(1);
    let base = match variant {
        2 => mixc(pal.ground, pal.danger, 0.35),
        3 => mixc(pal.bg2, pal.primary, 0.25),
        _ => pal.ground,
    };
    let base = if lum(base) < 0.22 { light(base, 0.25) } else { base };
    px.fill(dark(base, 0.35));
    match variant {
        3 => {
            // painéis metálicos
            px.rect(1, 1, n - 2, n - 2, base);
            px.rect(0, n / 2, n, 1, dark(base, 0.5));
            px.rect(n / 2, 0, 1, n, dark(base, 0.5));
            for (x, y) in [(3, 3), (n - 4, 3), (3, n - 4), (n - 4, n - 4)] {
                px.rect(x, y, 2, 2, light(base, 0.4));
            }
        }
        _ => {
            // tijolos
            let bh = 8;
            for row in 0..n / bh {
                let off = if row % 2 == 0 { 0 } else { 8 };
                for col in -1..(n / 16 + 1) {
                    let tint = rng.f32() * 0.18 - 0.06;
                    let c = if tint > 0.0 { light(base, tint) } else { dark(base, -tint) };
                    px.rect(col * 16 + off + 1, row * bh + 1, 15, bh - 1, c);
                }
            }
        }
    }
    for _ in 0..60 {
        let (x, y) = (rng.range(0, n - 1), rng.range(0, n - 1));
        let c = px.get(x, y);
        px.set(x, y, if rng.chance(0.5) { dark(c, 0.15) } else { light(c, 0.1) });
    }
    px.scale_to(w, h)
}

fn background(pal: &Pal, rng: &mut Rng, w: u32, h: u32, id: &str) -> RgbaImage {
    let scale = (w / 160).max(1);
    let (gw, gh) = ((w / scale) as i32, (h / scale) as i32);
    let mut px = Px::new(gw, gh);
    // variações por id (VN usa bg_a/b/c)
    let variant = match id {
        "bg_b" => 1,
        "bg_c" => 2,
        _ => 0,
    };
    let top = [pal.bg, mixc(pal.bg, pal.secondary, 0.35), dark(pal.bg, 0.3)][variant];
    let bottom = [pal.bg2, mixc(pal.bg2, pal.accent, 0.25), mixc(pal.bg2, pal.danger, 0.3)][variant];
    for y in 0..gh {
        let t = y as f32 / gh as f32;
        for x in 0..gw {
            px.set(x, y, mixc(top, bottom, t));
        }
    }
    let dark_sky = lum(pal.bg) < 0.25;
    if dark_sky {
        for _ in 0..(gw * gh / 90) {
            let (x, y) = (rng.range(0, gw - 1), rng.range(0, gh * 2 / 3));
            px.set(x, y, if rng.chance(0.25) { pal.text } else { light(top, 0.5) });
        }
    }
    // astro
    let (ax, ay, ar) = (rng.range(gw / 5, gw * 4 / 5) as f32, gh as f32 * (0.18 + rng.f32() * 0.15), gh as f32 * 0.08);
    px.ellipse(ax, ay, ar, ar, if dark_sky { light(pal.accent, 0.5) } else { pal.accent });
    // camadas de silhueta (parallax estático)
    let layers = 3;
    for l in 0..layers {
        let t = (l + 1) as f32 / layers as f32;
        let col = mixc(pal.bg2, pal.ground, 0.25 + 0.5 * t);
        let col = dark(col, 0.5 - 0.35 * t);
        let base = gh as f32 * (0.50 + 0.14 * l as f32);
        let (f1, f2, p1, p2) = (0.03 + rng.f32() * 0.05, 0.09 + rng.f32() * 0.08, rng.f32() * std::f32::consts::TAU, rng.f32() * std::f32::consts::TAU);
        let amp = gh as f32 * (0.16 - 0.03 * l as f32);
        for x in 0..gw {
            let xf = x as f32;
            let mut top_y = base - amp * (0.6 * (xf * f1 + p1).sin() + 0.4 * (xf * f2 + p2).sin()).abs();
            if variant == 2 && l == 2 {
                // cidade/ruínas: prédios retangulares
                top_y = base - ((x / 9) as f32 * 7.31).sin().abs() * amp * 1.4;
            }
            for y in (top_y as i32).max(0)..gh {
                px.set(x, y, col);
            }
        }
    }
    px.scale_to(w, h)
}

fn portrait(pal: &Pal, rng: &mut Rng, w: u32, h: u32, id: &str) -> RgbaImage {
    let (gw, gh) = (64, 96);
    let mut px = Px::new(gw, gh);
    let skin = rgb(rng.pick(&SKINS));
    let natural = [rgb("#2b1b17"), rgb("#4a2c17"), rgb("#1c1c24"), rgb("#8a5a2b"), rgb("#c9a54e"), dark(pal.secondary, 0.45)];
    let hair = *rng.pick(&natural);
    let cloth = match id {
        "char_rival" => pal.danger,
        "char_ally" => pal.secondary,
        _ => pal.primary,
    };
    let cx = 32.0;
    // ombros / roupa
    px.ellipse(cx, 90.0, 30.0, 26.0, dark(cloth, 0.1));
    px.ellipse(cx, 92.0, 25.0, 22.0, cloth);
    // pescoço e gola
    px.rect(28, 50, 8, 14, dark(skin, 0.15));
    px.rect(24, 63, 16, 3, pal.accent);
    // cabelo atrás
    px.ellipse(cx, 33.0, 17.0, 21.0, hair);
    // rosto
    px.ellipse(cx, 36.0, 13.0, 16.0, skin);
    // franja e mechas laterais
    px.ellipse(cx, 23.0, 15.0, 7.0, hair);
    px.rect(17, 25, 6, 18, hair);
    px.rect(41, 25, 6, 18, hair);
    // olhos
    for ex in [25, 36] {
        px.rect(ex, 36, 4, 3, [255, 255, 255, 255]);
        px.rect(ex + 1, 36, 2, 3, [30, 30, 50, 255]);
    }
    // sobrancelhas, nariz e boca
    px.rect(24, 33, 6, 1, dark(hair, 0.2));
    px.rect(35, 33, 6, 1, dark(hair, 0.2));
    px.rect(31, 42, 2, 2, dark(skin, 0.2));
    px.rect(28, 47, 8, 1, dark(skin, 0.45));
    px.outline(dark(cloth, 0.8));
    px.scale_to(w, h)
}

pub fn png_bytes(img: &RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(buf.into_inner())
}

/// Remove o fundo liso de imagens geradas por modelos (preenche a partir das bordas com a cor dos cantos).
pub fn cutout_background(img: &mut RgbaImage) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    if w < 4 || h < 4 {
        return;
    }
    let corner = |x: i32, y: i32| -> C {
        let p = img.get_pixel(x as u32, y as u32);
        [p[0], p[1], p[2], p[3]]
    };
    let cs = [corner(0, 0), corner(w - 1, 0), corner(0, h - 1), corner(w - 1, h - 1)];
    let d = |a: C, b: C| -> f32 { (0..3).map(|i| (a[i] as f32 - b[i] as f32).powi(2)).sum::<f32>().sqrt() };
    if cs.iter().any(|c| d(*c, cs[0]) > 40.0) {
        return; // cantos diferentes: não é um fundo liso
    }
    let bg = cs[0];
    let mut seen = vec![false; (w * h) as usize];
    let mut stack: Vec<(i32, i32)> = vec![];
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h || seen[(y * w + x) as usize] {
            continue;
        }
        let p = img.get_pixel(x as u32, y as u32);
        if d([p[0], p[1], p[2], 255], bg) > 48.0 {
            continue;
        }
        seen[(y * w + x) as usize] = true;
        img.put_pixel(x as u32, y as u32, Rgba([p[0], p[1], p[2], 0]));
        stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
    }
}

// ───────────────────────── Paleta a partir de imagens de referência ─────────────────────────

/// Extrai uma paleta coerente de imagens anexadas pelo usuário (PNG/JPEG).
pub fn palette_from_images(paths: &[std::path::PathBuf], fallback: &Palette) -> Option<Palette> {
    use std::collections::HashMap;
    let mut hist: HashMap<(u8, u8, u8), u32> = HashMap::new();
    let mut any = false;
    for p in paths {
        let Ok(img) = image::open(p) else { continue };
        let small = img.thumbnail(64, 64).to_rgba8();
        for px in small.pixels() {
            if px[3] < 128 {
                continue;
            }
            any = true;
            *hist.entry((px[0] >> 4, px[1] >> 4, px[2] >> 4)).or_insert(0) += 1;
        }
    }
    if !any {
        return None;
    }
    let mut buckets: Vec<((u8, u8, u8), u32)> = hist.into_iter().collect();
    buckets.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let to_c = |b: (u8, u8, u8)| -> C { [b.0 * 16 + 8, b.1 * 16 + 8, b.2 * 16 + 8, 255] };
    let dist = |a: C, b: C| -> f32 { (0..3).map(|i| (a[i] as f32 - b[i] as f32).powi(2)).sum::<f32>().sqrt() };
    let mut top: Vec<C> = vec![];
    for (b, _) in buckets {
        let c = to_c(b);
        if top.iter().all(|t| dist(*t, c) > 60.0) {
            top.push(c);
            if top.len() >= 10 {
                break;
            }
        }
    }
    let sat = |c: C| -> f32 {
        let (mx, mn) = (c[0].max(c[1]).max(c[2]) as f32, c[0].min(c[1]).min(c[2]) as f32);
        if mx == 0.0 { 0.0 } else { (mx - mn) / mx }
    };
    let vivid = |c: &C| sat(*c) * (0.4 + lum(*c));
    let darkest = top.iter().copied().min_by(|a, b| lum(*a).partial_cmp(&lum(*b)).unwrap())?;
    let bg = if lum(darkest) > 0.22 { dark(darkest, 0.6) } else { darkest };
    let bg2 = mixc(bg, [255, 255, 255, 255], 0.14);
    let ground = top.iter().copied().min_by(|a, b| (lum(*a) - 0.38).abs().partial_cmp(&(lum(*b) - 0.38).abs()).unwrap()).unwrap_or(rgb(&fallback.ground));
    let mut by_vivid = top.clone();
    by_vivid.sort_by(|a, b| vivid(b).partial_cmp(&vivid(a)).unwrap());
    let primary = by_vivid.first().copied().unwrap_or(rgb(&fallback.primary));
    let secondary = by_vivid.iter().copied().find(|c| dist(*c, primary) > 90.0).unwrap_or(rgb(&fallback.secondary));
    let accent = top.iter().copied().filter(|c| sat(*c) > 0.3).max_by(|a, b| lum(*a).partial_cmp(&lum(*b)).unwrap()).unwrap_or(rgb(&fallback.accent));
    let danger = top.iter().copied().find(|c| c[0] > 150 && (c[1] as i32) < c[0] as i32 - 60 && (c[2] as i32) < c[0] as i32 - 60).unwrap_or(rgb(&fallback.danger));
    let text = mixc([255, 255, 255, 255], bg, 0.06);
    let hex = |c: C| to_hex(c[0], c[1], c[2]);
    Some(Palette {
        bg: hex(bg),
        bg2: hex(bg2),
        ground: hex(ground),
        primary: hex(light(primary, if lum(primary) < 0.4 { 0.25 } else { 0.0 })),
        secondary: hex(secondary),
        accent: hex(accent),
        danger: hex(danger),
        text: hex(text),
    })
}

// ───────────────────────── Áudio ─────────────────────────

const SR: u32 = 22050;

pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SR.to_le_bytes());
    out.extend_from_slice(&(SR * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    out
}

#[derive(Clone, Copy)]
enum Wave {
    Square(f32),
    Tri,
    Saw,
}

fn osc(w: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    match w {
        Wave::Square(duty) => {
            if p < duty {
                0.6
            } else {
                -0.6
            }
        }
        Wave::Tri => (if p < 0.5 { p * 4.0 - 1.0 } else { 3.0 - p * 4.0 }) * 0.8,
        Wave::Saw => (p * 2.0 - 1.0) * 0.5,
    }
}

/// Nota com glissando de f0 → f1 e envelope de ataque rápido/decaimento exponencial.
fn tone(out: &mut Vec<f32>, w: Wave, f0: f32, f1: f32, dur: f32, vol: f32, decay: f32) {
    let n = (dur * SR as f32) as usize;
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let f = f0 + (f1 - f0) * t;
        phase += f / SR as f32;
        let env = (1.0 - t).powf(decay) * (i as f32 / 120.0).min(1.0);
        out.push(osc(w, phase) * env * vol);
    }
}

fn noise(out: &mut Vec<f32>, rng: &mut Rng, dur: f32, vol: f32, decay: f32, lowpass: f32) {
    let n = (dur * SR as f32) as usize;
    let mut y = 0.0f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let x = rng.f32() * 2.0 - 1.0;
        y += (x - y) * lowpass;
        out.push(y * (1.0 - t).powf(decay) * vol);
    }
}

fn note_hz(semi_from_a4: i32) -> f32 {
    440.0 * 2f32.powf(semi_from_a4 as f32 / 12.0)
}

pub fn synth_sfx(id: &str, prompt: &str, seed: u64) -> Vec<u8> {
    let mut rng = Rng::new(seed ^ hash_str(id));
    let key = format!("{} {}", id, normalize(prompt));
    let mut s: Vec<f32> = vec![];
    let has = |ws: &[&str]| has_any(&key, ws);
    if has(&["coin", "moeda", "pickup", "item", "collect"]) {
        tone(&mut s, Wave::Square(0.5), note_hz(14), note_hz(14), 0.07, 0.35, 0.3);
        tone(&mut s, Wave::Square(0.5), note_hz(19), note_hz(19), 0.22, 0.35, 1.5);
    } else if has(&["jump", "pulo", "boing"]) {
        tone(&mut s, Wave::Square(0.25), 260.0, 640.0, 0.2, 0.35, 0.8);
    } else if has(&["win", "victory", "vitoria", "level_up", "levelup", "up"]) {
        for (i, st) in [0, 4, 7, 12, 16].iter().enumerate() {
            let last = i == 4;
            tone(&mut s, Wave::Square(0.5), note_hz(*st - 9), note_hz(*st - 9), if last { 0.4 } else { 0.1 }, 0.3, if last { 1.2 } else { 0.4 });
        }
    } else if has(&["lose", "defeat", "gameover", "game_over", "derrota"]) {
        for (i, st) in [3, 0, -4, -9].iter().enumerate() {
            tone(&mut s, Wave::Tri, note_hz(*st - 9), note_hz(*st - 9 - 1), if i == 3 { 0.5 } else { 0.18 }, 0.5, 0.9);
        }
    } else if has(&["explosion", "explod", "boom"]) {
        noise(&mut s, &mut rng, 0.7, 0.9, 1.6, 0.12);
    } else if has(&["shoot", "laser", "tiro", "shot", "blaster"]) {
        tone(&mut s, Wave::Saw, 1100.0, 180.0, 0.16, 0.4, 1.0);
    } else if has(&["attack", "sword", "espada", "swing", "swoosh"]) {
        noise(&mut s, &mut rng, 0.18, 0.7, 1.0, 0.35);
        tone(&mut s, Wave::Saw, 500.0, 220.0, 0.12, 0.2, 1.0);
    } else if has(&["stomp", "pisao", "squash"]) {
        tone(&mut s, Wave::Square(0.5), 220.0, 70.0, 0.12, 0.5, 1.0);
    } else if has(&["hit", "hurt", "dano", "impact", "damage"]) {
        noise(&mut s, &mut rng, 0.18, 0.7, 1.2, 0.5);
        tone(&mut s, Wave::Square(0.5), 160.0, 60.0, 0.15, 0.35, 1.0);
    } else if has(&["choice", "select", "menu"]) {
        tone(&mut s, Wave::Square(0.5), 660.0, 880.0, 0.08, 0.3, 0.8);
    } else if has(&["blip", "text", "dialog"]) {
        let f = 520.0 + rng.f32() * 120.0;
        tone(&mut s, Wave::Square(0.5), f, f, 0.045, 0.25, 0.5);
    } else {
        tone(&mut s, Wave::Square(0.5), 440.0, 330.0, 0.15, 0.3, 1.0);
    }
    wav_bytes(&s)
}

/// Trilha curta em loop: baixo + arpejo + melodia pentatônica. Modo menor para temas sombrios.
pub fn synth_music(seed: u64, dark_theme: bool) -> Vec<u8> {
    let mut rng = Rng::new(seed ^ 0x6D75_7369);
    let bpm = 96.0 + rng.f32() * 40.0;
    let beat = 60.0 / bpm;
    let scale: [i32; 5] = if dark_theme { [0, 3, 5, 7, 10] } else { [0, 2, 4, 7, 9] };
    let triad: [i32; 3] = if dark_theme { [0, 3, 7] } else { [0, 4, 7] };
    let roots = [0, if dark_theme { -4 } else { -3 }, 5, 7];
    let root_base = -21 + rng.range(0, 5);
    let mut tracks: [Vec<f32>; 3] = [vec![], vec![], vec![]];
    for (bar, r) in roots.iter().cycle().take(8).enumerate() {
        let root = root_base + r;
        // baixo
        for b in 0..4 {
            let n = if b % 2 == 0 { root } else { root + 7 };
            tone(&mut tracks[0], Wave::Tri, note_hz(n), note_hz(n), beat, 0.55, 0.7);
        }
        // arpejo
        for k in 0..8 {
            let n = root + 12 + triad[k % 3] + if k >= 6 { 12 } else { 0 };
            tone(&mut tracks[1], Wave::Square(0.25), note_hz(n), note_hz(n), beat / 2.0, 0.16, 1.1);
        }
        // melodia (só nos compassos pares, para respirar)
        let mut left = 4.0;
        while left > 0.0 {
            let dur = *rng.pick(&[0.5f32, 1.0, 1.0, 2.0]);
            let dur = dur.min(left);
            if bar % 2 == 0 || rng.chance(0.4) {
                let n = root + 24 + scale[rng.range(0, 4) as usize];
                tone(&mut tracks[2], Wave::Square(0.5), note_hz(n), note_hz(n), dur * beat, 0.16, 0.9);
            } else {
                tracks[2].extend(std::iter::repeat_n(0.0, (dur * beat * SR as f32) as usize));
            }
            left -= dur;
        }
    }
    let len = tracks.iter().map(|t| t.len()).max().unwrap_or(0);
    let mix: Vec<f32> = (0..len).map(|i| tracks.iter().map(|t| t.get(i).copied().unwrap_or(0.0)).sum::<f32>() * 0.75).collect();
    wav_bytes(&mix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::*;

    #[test]
    fn all_default_assets_render_non_empty() {
        for g in Genre::ALL {
            let spec = heuristic_spec("jogo teste", Some(g), None);
            for a in &spec.assets {
                let img = generate_image(a, &spec.palette, spec.seed);
                assert_eq!((img.width(), img.height()), (a.w.max(8), a.h.max(8)), "{}", a.id);
                if a.kind != "background" {
                    assert!(img.pixels().any(|p| p[3] > 0), "{} vazio", a.id);
                }
            }
        }
    }

    #[test]
    fn wav_header_is_valid() {
        let w = synth_sfx("coin", "", 1);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        let data_len = u32::from_le_bytes([w[40], w[41], w[42], w[43]]) as usize;
        assert_eq!(w.len(), 44 + data_len);
        let m = synth_music(7, true);
        assert!(m.len() > 44 + SR as usize * 2 * 5);
    }

    #[test]
    fn palette_from_image() {
        let dir = std::env::temp_dir().join(format!("gf-pal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut img = RgbaImage::new(32, 32);
        for (x, _y, p) in img.enumerate_pixels_mut() {
            *p = if x < 16 { Rgba([20, 30, 60, 255]) } else { Rgba([240, 90, 40, 255]) };
        }
        let f = dir.join("ref.png");
        img.save(&f).unwrap();
        let pal = palette_from_images(&[f], &Palette::default()).unwrap();
        let (r, _, b) = parse_hex(&pal.primary);
        assert!(r > b, "primária deveria ser laranja: {}", pal.primary);
        assert!(lum(rgb(&pal.bg)) < 0.25);
    }
}
