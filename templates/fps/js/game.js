/* Tiro em 1ª pessoa — labirinto com raycasting (game.json → world.rows).
 *   1 2 3 paredes   P início   E inimigo   H vida   A munição   I relíquia   X saída
 * Controles: WASD move • mouse (clique para capturar) ou ←/→ giram • clique/ESPAÇO atira. */
(() => {
  'use strict';
  const COLW = 2, FOV = Math.PI / 3, R = 0.24;
  let g, map, MW, MH, zbuf;

  const wallAt = (x, y) => { const cx = Math.floor(x), cy = Math.floor(y); return cx < 0 || cy < 0 || cx >= MW || cy >= MH || (map[cy][cx] >= '1' && map[cy][cx] <= '9'); };
  const canStand = (x, y) => !wallAt(x - R, y - R) && !wallAt(x + R, y - R) && !wallAt(x - R, y + R) && !wallAt(x + R, y + R);

  function castRay(px, py, ang) {
    const dx = Math.cos(ang), dy = Math.sin(ang);
    let mx = Math.floor(px), my = Math.floor(py);
    const ddx = Math.abs(1 / (dx || 1e-9)), ddy = Math.abs(1 / (dy || 1e-9));
    let sx, sy, sdx, sdy;
    if (dx < 0) { sx = -1; sdx = (px - mx) * ddx; } else { sx = 1; sdx = (mx + 1 - px) * ddx; }
    if (dy < 0) { sy = -1; sdy = (py - my) * ddy; } else { sy = 1; sdy = (my + 1 - py) * ddy; }
    let side = 0, guard = 0;
    while (guard++ < 80) {
      if (sdx < sdy) { sdx += ddx; mx += sx; side = 0; } else { sdy += ddy; my += sy; side = 1; }
      if (mx < 0 || my < 0 || mx >= MW || my >= MH) break;
      const c = map[my][mx]; if (c >= '1' && c <= '9') {
        const dist = side === 0 ? sdx - ddx : sdy - ddy;
        const wx = side === 0 ? py + dist * dy : px + dist * dx;
        return { dist, side, tex: c, u: wx - Math.floor(wx) };
      }
    }
    return { dist: 99, side: 0, tex: '1', u: 0 };
  }

  function shoot() {
    const p = g.p;
    if (p.cool > 0 || g.ammo <= 0) { if (g.ammo <= 0 && p.cool <= 0) { GF.toast('…', 0.4); p.cool = 0.3; } return; }
    g.ammo--; p.cool = 0.28; p.recoil = 1; GF.play('shoot', { vol: 0.35 });
    let best = null;
    for (const e of g.enemies) {
      if (e.dead) continue;
      const dx = e.x - p.x, dy = e.y - p.y, d = Math.hypot(dx, dy);
      let da = Math.atan2(dy, dx) - p.a; da = Math.atan2(Math.sin(da), Math.cos(da));
      if (Math.abs(da) < Math.atan(0.4 / d) && castRay(p.x, p.y, p.a + da).dist > d - 0.2 && (!best || d < best.d)) best = { e, d };
    }
    if (best) {
      best.e.hp--; best.e.flash = 0.15; GF.play('hit', { vol: 0.3 });
      if (best.e.hp <= 0) { best.e.dead = 0.001; g.score += 100; g.kills++; GF.play('explosion', { vol: 0.35 }); GF.event('kill'); }
    }
  }

  GF.start({
    controls: 'controls_fps',
    setup() {
      map = GF.cfg.world.rows; MH = map.length; MW = map[0].length; zbuf = new Float32Array(GF.W / COLW);
    },
    init() {
      map = GF.cfg.world.rows; MH = map.length; MW = map[0].length;
      g = GF.game = { score: 0, ammo: 30, kills: 0, p: { x: 1.5, y: 1.5, a: 0, hp: 100, cool: 0, recoil: 0, hurt: 0, bob: 0 }, enemies: [], pickups: [], exit: null, relics: 0, relicsTotal: 0 };
      for (let y = 0; y < MH; y++) for (let x = 0; x < MW; x++) {
        const c = map[y][x], cx = x + 0.5, cy = y + 0.5;
        if (c === 'P') { g.p.x = cx; g.p.y = cy; }
        else if (c === 'E') g.enemies.push({ x: cx, y: cy, hp: 3, dead: 0, cool: 0, flash: 0, seen: false });
        else if ('HAI'.includes(c)) { g.pickups.push({ x: cx, y: cy, t: c }); if (c === 'I') g.relicsTotal++; }
        else if (c === 'X') g.exit = { x: cx, y: cy };
      }
      // começa olhando para um corredor livre
      for (let k = 0; k < 4; k++) { g.p.a = (k * Math.PI) / 2; if (castRay(g.p.x, g.p.y, g.p.a).dist > 2) break; }
    },
    update(dt) {
      const p = g.p, I = GF.input, M = I.mouse;
      if (M.clicked && !M.locked) GF.lockPointer();
      p.a += (I.axis('ArrowLeft', 'ArrowRight')) * 2.4 * dt + (M.locked ? M.dx * 0.0026 : 0);
      const fwd = I.axis('KeyS', 'KeyW') + I.axis('ArrowDown', 'ArrowUp'), str = I.axis('KeyA', 'KeyD');
      const sp = 3.2 * dt;
      let mx = (Math.cos(p.a) * fwd + Math.cos(p.a + Math.PI / 2) * str), my = (Math.sin(p.a) * fwd + Math.sin(p.a + Math.PI / 2) * str);
      const len = Math.hypot(mx, my) || 1; mx = (mx / len) * sp * (fwd || str ? 1 : 0); my = (my / len) * sp * (fwd || str ? 1 : 0);
      if (canStand(p.x + mx, p.y)) p.x += mx;
      if (canStand(p.x, p.y + my)) p.y += my;
      p.bob += (Math.abs(mx) + Math.abs(my)) * 9;
      p.cool = Math.max(0, p.cool - dt); p.recoil = Math.max(0, p.recoil - dt * 6); p.hurt = Math.max(0, p.hurt - dt);
      if (I.pressed('Space') || (M.down && (M.locked || M.clicked))) shoot();
      if (M.down && M.locked && p.cool <= 0) shoot();

      for (const e of g.enemies) {
        if (e.dead) { e.dead += dt; continue; }
        e.flash = Math.max(0, e.flash - dt); e.cool = Math.max(0, e.cool - dt);
        const dx = p.x - e.x, dy = p.y - e.y, d = Math.hypot(dx, dy);
        if (!e.seen && d < 9 && castRay(e.x, e.y, Math.atan2(dy, dx)).dist > d) e.seen = true;
        if (e.seen && d > 0.8) {
          const s = 1.5 * dt, nx = e.x + (dx / d) * s, ny = e.y + (dy / d) * s;
          if (!wallAt(nx, e.y)) e.x = nx; if (!wallAt(e.x, ny)) e.y = ny;
        } else if (e.seen && e.cool <= 0) { e.cool = 0.9; p.hp -= 9; p.hurt = 0.3; GF.play('hit', { vol: 0.4 }); GF.event('hit'); }
      }
      g.enemies = g.enemies.filter((e) => e.dead < 0.4);
      for (const k of g.pickups.slice()) {
        if (Math.hypot(k.x - p.x, k.y - p.y) < 0.6) {
          if (k.t === 'H' && p.hp < 100) { p.hp = Math.min(100, p.hp + 35); } else if (k.t === 'A') { g.ammo += 12; } else if (k.t === 'I') { g.score += 50; g.relics++; } else continue;
          g.pickups.splice(g.pickups.indexOf(k), 1); GF.play('coin', { vol: 0.4 });
        }
      }
      if (p.hp <= 0) GF.finish(false, { score: g.score });
      if (g.exit && Math.hypot(g.exit.x - p.x, g.exit.y - p.y) < 0.7) { g.score += 300 + g.p.hp; GF.finish(true, { score: g.score }); }
    },
    draw(ctx) {
      const P = GF.palette, W = GF.W, H = GF.H, p = g.p, bob = Math.sin(p.bob) * 3;
      const sky = ctx.createLinearGradient(0, 0, 0, H / 2); sky.addColorStop(0, P.bg); sky.addColorStop(1, P.bg2);
      ctx.fillStyle = sky; ctx.fillRect(0, 0, W, H / 2);
      const fl = ctx.createLinearGradient(0, H / 2, 0, H); fl.addColorStop(0, P.bg2); fl.addColorStop(1, P.ground);
      ctx.fillStyle = fl; ctx.fillRect(0, H / 2, W, H / 2);
      const cols = W / COLW, fogRGB = E3Dless(P.bg);
      for (let i = 0; i < cols; i++) {
        const ray = castRay(p.x, p.y, p.a + Math.atan((i / cols - 0.5) * 2 * Math.tan(FOV / 2)));
        const perp = Math.max(0.05, ray.dist * Math.cos(Math.atan((i / cols - 0.5) * 2 * Math.tan(FOV / 2))));
        zbuf[i] = perp;
        const lh = H / perp, top = (H - lh) / 2 + bob, tex = GF.img('wall' + ray.tex) || GF.img('wall1');
        if (tex) ctx.drawImage(tex, Math.min(tex.width - 1, Math.floor(ray.u * tex.width)), 0, 1, tex.height, i * COLW, top, COLW, lh);
        else { ctx.fillStyle = ray.tex === '2' ? P.secondary : ray.tex === '3' ? P.primary : P.ground; ctx.fillRect(i * COLW, top, COLW, lh); }
        const dark = Math.min(0.85, perp / 14 + (ray.side ? 0.22 : 0));
        ctx.fillStyle = `rgba(${fogRGB},${dark})`; ctx.fillRect(i * COLW, top, COLW, lh);
      }
      // sprites (ordenados do mais longe ao mais perto)
      const sprites = [];
      for (const e of g.enemies) sprites.push({ x: e.x, y: e.y, img: GF.img('enemy'), s: 0.85, lift: 0, e, color: P.danger });
      for (const k of g.pickups) sprites.push({ x: k.x, y: k.y, img: GF.img(k.t === 'H' ? 'health' : k.t === 'A' ? 'ammo' : 'item'), s: 0.32, lift: 0.28 + Math.sin(GF.time * 3 + k.x) * 0.03, color: k.t === 'H' ? '#fff' : k.t === 'A' ? P.ground : P.accent });
      if (g.exit) sprites.push({ x: g.exit.x, y: g.exit.y, img: null, s: 0.9, lift: 0, color: P.accent, exit: true });
      for (const s of sprites) { s.d = Math.hypot(s.x - p.x, s.y - p.y); }
      sprites.sort((a, b) => b.d - a.d);
      for (const s of sprites) {
        const dx = s.x - p.x, dy = s.y - p.y, ca = Math.cos(-p.a), sa = Math.sin(-p.a);
        const ty = dx * ca - dy * sa, tx = dx * sa + dy * ca;       // ty = profundidade, tx = lateral
        if (ty < 0.15) continue;
        const scale = (W / 2) / Math.tan(FOV / 2) / ty, size = scale * s.s, sx = W / 2 + (tx / ty) * (W / 2) / Math.tan(FOV / 2);
        const dead = s.e && s.e.dead, fall = dead ? Math.min(1, s.e.dead * 3) : 0;
        const bottom = H / 2 + scale * 0.5 + bob - s.lift * scale + fall * size * 0.5, hh = size * (1 - fall * 0.8);
        const x0 = Math.floor(sx - size / 2);
        for (let x = Math.max(0, x0); x < Math.min(W, x0 + size); x += COLW) {
          if (zbuf[Math.floor(x / COLW)] < ty) continue;
          if (s.img) ctx.drawImage(s.img, ((x - x0) / size) * s.img.width, 0, Math.max(1, (COLW / size) * s.img.width), s.img.height, x, bottom - hh, COLW, hh);
          else { ctx.fillStyle = s.color; ctx.globalAlpha = s.exit ? 0.35 + Math.sin(GF.time * 4) * 0.15 : 1; ctx.fillRect(x, bottom - hh * (s.exit ? 1 : 0.8), COLW, hh * (s.exit ? 1 : 0.8)); ctx.globalAlpha = 1; }
        }
        if (s.e && s.e.flash > 0) { ctx.fillStyle = 'rgba(255,60,60,.35)'; ctx.fillRect(x0, bottom - hh, size, hh); }
      }
      // arma
      const rc = p.recoil, gx = W / 2 + Math.sin(p.bob * 0.5) * 8, gy = H - 110 + rc * 22 + Math.abs(Math.cos(p.bob * 0.5)) * 5;
      ctx.fillStyle = '#2a2e38'; ctx.fillRect(gx - 22, gy, 44, 110); ctx.fillStyle = '#3d4352'; ctx.fillRect(gx - 14, gy - 26, 28, 40); ctx.fillStyle = P.accent; ctx.fillRect(gx - 4, gy - 30, 8, 8);
      if (rc > 0.6) { ctx.fillStyle = 'rgba(255,230,140,.9)'; ctx.beginPath(); ctx.arc(gx, gy - 36, 18 * rc, 0, 7); ctx.fill(); }
      // mira
      ctx.strokeStyle = 'rgba(255,255,255,.8)'; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(W / 2 - 8, H / 2); ctx.lineTo(W / 2 + 8, H / 2); ctx.moveTo(W / 2, H / 2 - 8); ctx.lineTo(W / 2, H / 2 + 8); ctx.stroke();
      if (p.hurt > 0) { ctx.fillStyle = `rgba(200,20,20,${p.hurt})`; ctx.fillRect(0, 0, W, H); }
      // HUD + minimapa
      GF.hud(ctx, [[GF.t('score'), g.score], [GF.t('ammo'), g.ammo], [GF.t('enemies'), g.enemies.length]]);
      GF.bar(ctx, 20, H - 34, 220, 16, p.hp / 100, p.hp > 35 ? P.primary : P.danger); GF.text(ctx, GF.t('hp') + ' ' + Math.max(0, Math.ceil(p.hp)), 24, H - 60, { size: 16 });
      const s = Math.min(4, 150 / Math.max(MW, MH)), ox = W - MW * s - 14, oy = 14;
      ctx.globalAlpha = 0.75; ctx.fillStyle = 'rgba(0,0,0,.5)'; ctx.fillRect(ox - 3, oy - 3, MW * s + 6, MH * s + 6);
      for (let y = 0; y < MH; y++) for (let x = 0; x < MW; x++) if (map[y][x] >= '1' && map[y][x] <= '9') { ctx.fillStyle = P.ground; ctx.fillRect(ox + x * s, oy + y * s, s, s); }
      ctx.fillStyle = P.accent; if (g.exit) ctx.fillRect(ox + g.exit.x * s - 2, oy + g.exit.y * s - 2, 4, 4);
      ctx.fillStyle = P.danger; for (const e of g.enemies) ctx.fillRect(ox + e.x * s - 1, oy + e.y * s - 1, 3, 3);
      ctx.fillStyle = '#fff'; ctx.fillRect(ox + p.x * s - 2, oy + p.y * s - 2, 4, 4); ctx.globalAlpha = 1;
      if (g.relicsTotal) GF.text(ctx, `${GF.t('relics')}: ${g.relics}/${g.relicsTotal}`, W / 2, 14, { align: 'center', size: 18 });
      if (g.p.x < 3 && g.p.y < 3 && GF.time < 8) GF.text(ctx, GF.t('exit_found'), W / 2, H * 0.2, { align: 'center', size: 30, color: P.accent });
    },
  });

  function E3Dless(hex) { const h = hex.replace('#', ''); return `${parseInt(h.slice(0, 2), 16)},${parseInt(h.slice(2, 4), 16)},${parseInt(h.slice(4, 6), 16)}`; }
})();
