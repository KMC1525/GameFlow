//! Geração procedural de fases/mapas (determinística pela semente do jogo).
//! Os mapas são texto puro dentro de `game.json` (campo `world`), então qualquer pessoa pode editar à mão.

use crate::spec::*;
use crate::util::Rng;
use serde_json::{Value, json};
use std::collections::VecDeque;

pub fn generate_world(spec: &GameSpec) -> Value {
    let mut rng = Rng::new(spec.seed ^ 0x1E7E1);
    let diff = match spec.difficulty.as_str() {
        "easy" => 0,
        "hard" => 2,
        _ => 1,
    };
    match spec.genre {
        Genre::Platformer2d => platformer2d(&mut rng, diff),
        Genre::Platformer3d => platformer3d(&mut rng, diff),
        Genre::Rpg2d | Genre::Rpg3d => rpg(&mut rng, diff),
        Genre::Fps => fps(&mut rng, diff),
        Genre::Tps => tps(&mut rng, diff),
        Genre::Arcade => json!({ "waves": 8 + diff * 2 }),
        Genre::VisualNovel => Value::Null,
        Genre::TextAdventure => crate::adventure::generate(spec),
    }
}

// ───────────────────────── Plataforma 2D ─────────────────────────

const PF_H: usize = 16;
const GROUND: usize = 13;

fn platformer2d(rng: &mut Rng, diff: i32) -> Value {
    let levels: Vec<Value> = (0..3).map(|i| json!({ "name": format!("{}", i + 1), "rows": pf_level(rng, i, diff) })).collect();
    json!({ "tile": 32, "levels": levels })
}

fn pf_level(rng: &mut Rng, idx: i32, diff: i32) -> Vec<String> {
    let w = (80 + 22 * idx) as usize;
    let mut g = vec![vec!['.'; w]; PF_H];
    let mut gap = vec![false; w];
    let mut gaps: Vec<(usize, usize)> = vec![];
    let max_gap = if idx == 0 { 3 } else { 4 };
    let mut x = 10usize;
    while x < w - 14 {
        if rng.chance((0.22 + 0.04 * idx as f32 + 0.03 * diff as f32).min(0.4)) {
            let gw = rng.range(2, max_gap) as usize;
            for c in x..x + gw {
                gap[c] = true;
            }
            gaps.push((x, gw));
            x += gw + rng.range(4, 8) as usize;
        } else {
            x += rng.range(2, 4) as usize;
        }
    }
    for c in 0..w {
        if !gap[c] {
            for r in GROUND..PF_H {
                g[r][c] = '#';
            }
        }
    }
    // plataformas sobre buracos largos (rede de segurança) e plataformas flutuantes
    let mut plats: Vec<(usize, usize, usize)> = vec![];
    for &(gx, gw) in &gaps {
        if gw >= 3 {
            plats.push((gx.saturating_sub(1), GROUND - 2, gw + 2));
        }
    }
    let n_float = w / 11;
    for _ in 0..n_float {
        let px = rng.range(8, w as i32 - 16) as usize;
        let len = rng.range(3, 5) as usize;
        let row = rng.range(9, 11) as usize;
        if plats.iter().all(|&(ox, orow, ol)| !(px < ox + ol + 2 && ox < px + len + 2 && row.abs_diff(orow) < 2)) {
            plats.push((px, row, len));
        }
    }
    for &(px, row, len) in &plats {
        for c in px..(px + len).min(w) {
            if g[row][c] == '.' {
                g[row][c] = '#';
            }
        }
        // moedas em cima
        for c in px..(px + len).min(w) {
            if rng.chance(0.7) && g[row - 1][c] == '.' {
                g[row - 1][c] = 'C';
            }
        }
    }
    // arcos de moedas sobre os buracos
    for &(gx, gw) in &gaps {
        for k in 0..gw + 2 {
            let c = gx.saturating_sub(1) + k;
            let up = if k == 0 || k == gw + 1 { 1 } else { 2 + (k % 2) };
            let r = GROUND - 1 - up;
            if c < w && g[r][c] == '.' {
                g[r][c] = 'C';
            }
        }
    }
    // linhas de moedas no chão
    for _ in 0..w / 14 {
        let cx = rng.range(6, w as i32 - 12) as usize;
        for k in 0..rng.range(3, 5) as usize {
            if cx + k < w && !gap[cx + k] && g[GROUND - 1][cx + k] == '.' {
                g[GROUND - 1][cx + k] = 'C';
            }
        }
    }
    // inimigos e espinhos
    let n_enemy = (w / 14) as i32 + diff * 2;
    for _ in 0..n_enemy {
        let ex = rng.range(18, w as i32 - 16) as usize;
        if !gap[ex] && !gap[ex - 1] && !gap[ex + 1] && g[GROUND - 1][ex] != 'E' {
            g[GROUND - 1][ex] = 'E';
        }
    }
    let n_spikes = (w / 30) as i32 + diff;
    for _ in 0..n_spikes {
        let sx = rng.range(20, w as i32 - 18) as usize;
        if (sx - 2..=sx + 2).all(|c| !gap[c]) && (sx - 1..=sx + 1).all(|c| g[GROUND - 1][c] == '.') {
            g[GROUND - 1][sx] = '^';
        }
    }
    g[GROUND - 1][2] = 'P';
    g[GROUND - 1][w - 4] = 'G';
    g.into_iter().map(|r| r.into_iter().collect()).collect()
}

// ───────────────────────── Plataforma 3D ─────────────────────────

fn platformer3d(rng: &mut Rng, diff: i32) -> Value {
    let n = 26 + diff * 4;
    let mut plats = vec![json!({"x": 0.0, "y": 0.0, "z": 0.0, "w": 7.0, "d": 7.0, "kind": "start"})];
    let mut coins: Vec<Value> = vec![];
    let mut enemies: Vec<Value> = vec![];
    let (mut cx, mut cy, mut cz, mut cd) = (0.0f32, 0.0f32, 0.0f32, 7.0f32);
    for i in 1..=n {
        let w = 3.4 + rng.f32() * 2.2;
        let d = 3.4 + rng.f32() * 2.2;
        let gap = 1.6 + rng.f32() * (1.0 + 0.2 * diff as f32);
        let nz = cz - (cd / 2.0 + gap + d / 2.0);
        let nx = (cx + (rng.f32() - 0.5) * 6.0).clamp(-9.0, 9.0);
        let dy = *rng.pick(&[-0.6f32, 0.0, 0.0, 0.5, 0.9]);
        let ny = (cy + dy).clamp(-3.0, 7.0);
        let kind = if i == n {
            "goal"
        } else if i % 8 == 0 {
            "checkpoint"
        } else {
            "normal"
        };
        plats.push(json!({"x": nx, "y": ny, "z": nz, "w": w, "d": d, "kind": kind}));
        if rng.chance(0.8) {
            coins.push(json!({"x": nx + (rng.f32() - 0.5) * w * 0.5, "y": ny + 1.1, "z": nz}));
        }
        // moedas no vão entre plataformas
        if rng.chance(0.5) {
            coins.push(json!({"x": (cx + nx) / 2.0, "y": (cy.max(ny)) + 2.1, "z": (cz + nz) / 2.0}));
        }
        if i > 2 && i != n && d >= 4.4 && rng.chance(0.45 + 0.1 * diff as f32) {
            enemies.push(json!({"x": nx, "y": ny, "z": nz, "range": (w * 0.35).max(1.0), "speed": 1.4 + rng.f32()}));
        }
        cx = nx;
        cy = ny;
        cz = nz;
        cd = d;
    }
    json!({ "platforms": plats, "coins": coins, "enemies": enemies, "killY": -14.0 })
}

// ───────────────────────── TPS ─────────────────────────

fn tps(rng: &mut Rng, diff: i32) -> Value {
    let size = 46.0f32;
    let mut obstacles = vec![];
    for _ in 0..16 {
        let (x, z) = ((rng.f32() - 0.5) * (size * 1.7), (rng.f32() - 0.5) * (size * 1.7));
        if x.abs() < 6.0 && z.abs() < 6.0 {
            continue;
        }
        let (w, d, h) = (2.0 + rng.f32() * 4.0, 2.0 + rng.f32() * 4.0, 1.4 + rng.f32() * 2.8);
        obstacles.push(json!({"x": x, "z": z, "w": w, "d": d, "h": h}));
    }
    json!({ "size": size, "obstacles": obstacles, "waves": 5 + diff, "baseEnemies": 4 + diff * 2 })
}

// ───────────────────────── FPS (labirinto) ─────────────────────────

fn fps(rng: &mut Rng, diff: i32) -> Value {
    let (cw, ch) = (9usize + diff as usize, 9usize + diff as usize);
    let (w, h) = (cw * 2 + 1, ch * 2 + 1);
    let mut wall = vec![vec![true; w]; h];
    // DFS iterativo
    let mut stack = vec![(0usize, 0usize)];
    let mut seen = vec![vec![false; cw]; ch];
    seen[0][0] = true;
    wall[1][1] = false;
    while let Some(&(cx, cy)) = stack.last() {
        let mut nb: Vec<(i32, i32)> = vec![(1, 0), (-1, 0), (0, 1), (0, -1)];
        rng.shuffle(&mut nb);
        let mut moved = false;
        for (dx, dy) in nb {
            let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
            if nx < 0 || ny < 0 || nx >= cw as i32 || ny >= ch as i32 || seen[ny as usize][nx as usize] {
                continue;
            }
            seen[ny as usize][nx as usize] = true;
            wall[ny as usize * 2 + 1][nx as usize * 2 + 1] = false;
            wall[(cy as i32 * 2 + 1 + dy) as usize][(cx as i32 * 2 + 1 + dx) as usize] = false;
            stack.push((nx as usize, ny as usize));
            moved = true;
            break;
        }
        if !moved {
            stack.pop();
        }
    }
    // laços extras e salas
    for _ in 0..(cw * ch) / 7 {
        let (x, y) = (rng.range(2, w as i32 - 3) as usize, rng.range(2, h as i32 - 3) as usize);
        if wall[y][x] && ((!wall[y][x - 1] && !wall[y][x + 1]) || (!wall[y - 1][x] && !wall[y + 1][x])) {
            wall[y][x] = false;
        }
    }
    for _ in 0..3 {
        let (x, y) = (rng.range(3, w as i32 - 6) as usize, rng.range(3, h as i32 - 6) as usize);
        for yy in y..y + 3 {
            for xx in x..x + 3 {
                wall[yy][xx] = false;
            }
        }
    }
    let dist = bfs(&wall.iter().map(|r| r.iter().map(|&b| !b).collect()).collect::<Vec<Vec<bool>>>(), 1, 1);
    let mut cells: Vec<(usize, usize)> = vec![];
    let mut far = (1usize, 1usize, 0i32);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if dist[y][x] >= 0 {
                cells.push((x, y));
                if dist[y][x] > far.2 {
                    far = (x, y, dist[y][x]);
                }
            }
        }
    }
    rng.shuffle(&mut cells);
    let mut rows: Vec<Vec<char>> = (0..h).map(|y| (0..w).map(|x| if wall[y][x] { (b'1' + ((x / 6 + y / 6) % 3) as u8) as char } else { '.' }).collect()).collect();
    rows[1][1] = 'P';
    rows[far.1][far.0] = 'X';
    let put = |rows: &mut Vec<Vec<char>>, ch: char, n: usize, min_d: i32| {
        let mut placed = 0;
        for &(x, y) in &cells {
            if placed >= n {
                break;
            }
            if rows[y][x] == '.' && dist[y][x] >= min_d {
                rows[y][x] = ch;
                placed += 1;
            }
        }
    };
    put(&mut rows, 'E', (7 + diff * 3) as usize, 7);
    put(&mut rows, 'H', 4, 4);
    put(&mut rows, 'A', 5, 3);
    put(&mut rows, 'I', 6, 5);
    json!({ "rows": rows.into_iter().map(|r| r.into_iter().collect::<String>()).collect::<Vec<_>>() })
}

// ───────────────────────── RPG ─────────────────────────

pub fn bfs(open: &[Vec<bool>], sx: usize, sy: usize) -> Vec<Vec<i32>> {
    let h = open.len();
    let w = open[0].len();
    let mut d = vec![vec![-1i32; w]; h];
    if !open[sy][sx] {
        return d;
    }
    d[sy][sx] = 0;
    let mut q = VecDeque::from([(sx, sy)]);
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if open[ny][nx] && d[ny][nx] < 0 {
                d[ny][nx] = d[y][x] + 1;
                q.push_back((nx, ny));
            }
        }
    }
    d
}

fn rpg(rng: &mut Rng, diff: i32) -> Value {
    let (w, h) = (44usize, 30usize);
    let mut g = vec![vec!['.'; w]; h];
    for x in 0..w {
        g[0][x] = '#';
        g[h - 1][x] = '#';
    }
    for row in g.iter_mut() {
        row[0] = '#';
        row[w - 1] = '#';
    }
    let (sx, sy) = (5usize, h / 2);
    let blob = |g: &mut Vec<Vec<char>>, rng: &mut Rng, ch: char, size: i32| {
        let (mut x, mut y) = (rng.range(2, w as i32 - 3), rng.range(2, h as i32 - 3));
        for _ in 0..size {
            if (x - sx as i32).abs() > 3 || (y - sy as i32).abs() > 3 {
                g[y as usize][x as usize] = ch;
            }
            match rng.range(0, 3) {
                0 => x += 1,
                1 => x -= 1,
                2 => y += 1,
                _ => y -= 1,
            }
            x = x.clamp(1, w as i32 - 2);
            y = y.clamp(1, h as i32 - 2);
        }
    };
    for _ in 0..16 {
        let n = rng.range(3, 9);
        blob(&mut g, rng, '#', n);
    }
    for _ in 0..3 {
        let n = rng.range(6, 14);
        blob(&mut g, rng, '~', n);
    }
    let open = |g: &Vec<Vec<char>>| g.iter().map(|r| r.iter().map(|&c| c != '#' && c != '~').collect::<Vec<bool>>()).collect::<Vec<_>>();
    let dist = bfs(&open(&g), sx, sy);
    // regiões inalcançáveis viram árvore
    for y in 0..h {
        for x in 0..w {
            if dist[y][x] < 0 && g[y][x] == '.' {
                g[y][x] = '#';
            }
        }
    }
    let mut cells: Vec<(usize, usize, i32)> = vec![];
    let mut far = (sx, sy, 0);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if dist[y][x] >= 0 && g[y][x] == '.' {
                cells.push((x, y, dist[y][x]));
                if dist[y][x] > far.2 {
                    far = (x, y, dist[y][x]);
                }
            }
        }
    }
    rng.shuffle(&mut cells);
    g[sy][sx] = 'P';
    g[far.1][far.0] = 'B';
    let mut placed: Vec<(usize, usize)> = vec![(sx, sy), (far.0, far.1)];
    let mut put = |g: &mut Vec<Vec<char>>, chars: &[char], n: usize, dmin: i32, dmax: i32, spacing: i32| {
        let mut k = 0;
        for &(x, y, d) in &cells {
            if k >= n {
                break;
            }
            if g[y][x] != '.' || d < dmin || d > dmax {
                continue;
            }
            if placed.iter().any(|&(px, py)| (px as i32 - x as i32).abs() + (py as i32 - y as i32).abs() < spacing) {
                continue;
            }
            g[y][x] = chars[k % chars.len()];
            placed.push((x, y));
            k += 1;
        }
    };
    put(&mut g, &['N'], 3, 3, 12, 3);
    put(&mut g, &['I'], 5, 12, 400, 7);
    put(&mut g, &['M', 'm'], (11 + diff * 3) as usize, 8, 400, 4);
    json!({ "rows": g.into_iter().map(|r| r.into_iter().collect::<String>()).collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &Value) -> Vec<Vec<char>> {
        v.as_array().unwrap().iter().map(|r| r.as_str().unwrap().chars().collect()).collect()
    }

    #[test]
    fn platformer_has_start_and_goal_and_bounded_gaps() {
        for seed in 0..20u64 {
            let spec = GameSpec { genre: Genre::Platformer2d, seed, ..Default::default() };
            let w = generate_world(&spec);
            for lv in w["levels"].as_array().unwrap() {
                let r = rows(&lv["rows"]);
                assert!(r.iter().all(|row| row.len() == r[0].len()));
                assert_eq!(r.iter().flatten().filter(|&&c| c == 'P').count(), 1);
                assert_eq!(r.iter().flatten().filter(|&&c| c == 'G').count(), 1);
                // nenhum buraco maior que 4 colunas no chão
                let mut run = 0;
                for c in 0..r[0].len() {
                    if r[GROUND][c] != '#' {
                        run += 1;
                        assert!(run <= 4, "buraco grande demais (seed {seed})");
                    } else {
                        run = 0;
                    }
                }
            }
        }
    }

    #[test]
    fn rpg_entities_are_reachable() {
        for seed in 0..15u64 {
            let spec = GameSpec { genre: Genre::Rpg2d, seed, ..Default::default() };
            let w = generate_world(&spec);
            let g = rows(&w["rows"]);
            let open: Vec<Vec<bool>> = g.iter().map(|r| r.iter().map(|&c| c != '#' && c != '~').collect()).collect();
            let (mut sx, mut sy) = (0, 0);
            for (y, r) in g.iter().enumerate() {
                for (x, &c) in r.iter().enumerate() {
                    if c == 'P' {
                        (sx, sy) = (x, y);
                    }
                }
            }
            let d = bfs(&open, sx, sy);
            let mut counts = std::collections::HashMap::new();
            for (y, r) in g.iter().enumerate() {
                for (x, &c) in r.iter().enumerate() {
                    if "NIMmB".contains(c) {
                        assert!(d[y][x] >= 0, "{c} inalcançável (seed {seed})");
                        *counts.entry(c).or_insert(0) += 1;
                    }
                }
            }
            assert_eq!(counts.get(&'B'), Some(&1));
            assert!(counts.get(&'I').copied().unwrap_or(0) >= 3, "itens insuficientes (seed {seed}): {counts:?}");
            assert!(counts.get(&'N').copied().unwrap_or(0) >= 1);
        }
    }

    #[test]
    fn fps_maze_is_connected() {
        for seed in 0..15u64 {
            let spec = GameSpec { genre: Genre::Fps, seed, ..Default::default() };
            let w = generate_world(&spec);
            let g = rows(&w["rows"]);
            let open: Vec<Vec<bool>> = g.iter().map(|r| r.iter().map(|&c| !c.is_ascii_digit()).collect()).collect();
            let d = bfs(&open, 1, 1);
            for (y, r) in g.iter().enumerate() {
                for (x, &c) in r.iter().enumerate() {
                    if "EHAIX".contains(c) {
                        assert!(d[y][x] >= 0, "{c} isolado");
                    }
                }
            }
            assert_eq!(g.iter().flatten().filter(|&&c| c == 'X').count(), 1);
        }
    }

    #[test]
    fn platformer3d_gaps_are_jumpable() {
        let spec = GameSpec { genre: Genre::Platformer3d, seed: 3, ..Default::default() };
        let w = generate_world(&spec);
        let p = w["platforms"].as_array().unwrap();
        for pair in p.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let gap_z = (a["z"].as_f64().unwrap() - b["z"].as_f64().unwrap()).abs() - (a["d"].as_f64().unwrap() + b["d"].as_f64().unwrap()) / 2.0;
            assert!(gap_z <= 3.0, "vão de {gap_z}");
            assert!(b["y"].as_f64().unwrap() - a["y"].as_f64().unwrap() <= 1.0);
        }
    }
}
