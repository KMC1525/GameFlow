/* Plataforma 2D — fases em texto (game.json → world.levels[].rows):
 *   # bloco   C moeda   E inimigo   ^ espinhos   P início   G bandeira */
(() => {
  'use strict';
  const T = 32;
  const PW = 22, PH = 28;          // hitbox do jogador
  const ACC = 1900, MAXV = 230, FRIC = 1700, GRAV = 1900, JUMPV = 640, COYOTE = 0.1, BUFFER = 0.12;
  let g, level, rows, LW, LH, camX;

  const solid = (tx, ty) => (ty < 0 ? false : ty >= LH ? false : tx < 0 || tx >= LW ? true : rows[ty][tx] === '#');

  function loadLevel(i) {
    const lv = GF.cfg.world.levels[i];
    rows = lv.rows; LH = rows.length; LW = rows[0].length;
    g.coins = []; g.enemies = []; g.spikes = []; g.goal = null; g.level = i;
    for (let y = 0; y < LH; y++) for (let x = 0; x < LW; x++) {
      const c = rows[y][x], px = x * T, py = y * T;
      if (c === 'C') g.coins.push({ x: px + T / 2, y: py + T / 2, got: false, ph: GF.rand() * 6 });
      else if (c === 'E') g.enemies.push({ x: px + 4, y: py + 6, w: 24, h: 26, vx: -50 - GF.rand() * 20, dead: 0, kind: GF.rand() < 0.4 ? 2 : 1 });
      else if (c === '^') g.spikes.push({ x: px, y: py + T / 2, w: T, h: T / 2 });
      else if (c === 'G') g.goal = { x: px + 6, y: py - T, w: 20, h: 64 };
      else if (c === 'P') { g.start = { x: px + 5, y: py + 4 }; }
    }
    respawn(true);
  }

  function respawn(full) {
    const p = g.player;
    const cp = g.checkpoint && !full ? g.checkpoint : g.start;
    p.x = cp.x; p.y = cp.y; p.vx = p.vy = 0; p.inv = full ? 0 : 1.6; p.onGround = false; p.coyote = 0; p.buffer = 0; p.jumps = 0;
    camX = Math.max(0, p.x - GF.W / 2);
  }

  function hurt() {
    const p = g.player;
    if (p.inv > 0) return;
    g.lives--; GF.play('hit'); GF.event('hit', { lives: g.lives });
    if (g.lives <= 0) { GF.finish(false, { score: g.score }); return; }
    respawn(false);
  }

  function moveX(p, dx) {
    p.x += dx;
    const dir = Math.sign(dx);
    if (!dir) return;
    const edge = dir > 0 ? p.x + PW : p.x;
    const tx = Math.floor(edge / T);
    for (let ty = Math.floor(p.y / T); ty <= Math.floor((p.y + PH - 1) / T); ty++) {
      if (solid(tx, ty)) { p.x = dir > 0 ? tx * T - PW : (tx + 1) * T; p.vx = 0; break; }
    }
  }
  function moveY(p, dy) {
    p.y += dy; p.onGround = false;
    const dir = Math.sign(dy);
    if (!dir) return;
    const edge = dir > 0 ? p.y + PH : p.y;
    const ty = Math.floor(edge / T);
    for (let tx = Math.floor(p.x / T); tx <= Math.floor((p.x + PW - 1) / T); tx++) {
      if (solid(tx, ty)) {
        if (dir > 0) { p.y = ty * T - PH; p.onGround = true; } else p.y = (ty + 1) * T;
        p.vy = 0; break;
      }
    }
  }
  const hit = (a, b) => a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;

  GF.start({
    controls: 'controls_platformer',
    setup() {},
    init() {
      g = GF.game = { score: 0, coins: [], lives: 3, player: { x: 0, y: 0, vx: 0, vy: 0, face: 1, w: PW, h: PH }, checkpoint: null, tcoin: 0, msg: 0 };
      GF.seed(GF.cfg.seed || 1);
      loadLevel(0);
    },
    update(dt) {
      const p = g.player, input = GF.input;
      const dir = input.axis('KeyA', 'KeyD') + input.axis('ArrowLeft', 'ArrowRight');
      const want = GF.clamp(dir, -1, 1);
      if (want) { p.vx += want * ACC * dt; p.face = want; } else { const f = FRIC * dt; p.vx = Math.abs(p.vx) <= f ? 0 : p.vx - Math.sign(p.vx) * f; }
      p.vx = GF.clamp(p.vx, -MAXV, MAXV);
      p.vy = Math.min(p.vy + GRAV * dt, 900);
      p.coyote = p.onGround ? COYOTE : p.coyote - dt;
      if (p.onGround) p.jumps = 0;
      if (input.pressed('Space') || input.pressed('ArrowUp') || input.pressed('KeyW')) p.buffer = BUFFER; else p.buffer -= dt;
      if (p.buffer > 0) {
        const can2 = GF.cfg.features?.includes('double_jump') && p.jumps < 1 && !p.onGround && p.coyote <= 0;
        if (p.coyote > 0 || can2) {
          if (!(p.coyote > 0)) p.jumps++;
          p.vy = -JUMPV; p.coyote = 0; p.buffer = 0; GF.play('jump', { vol: 0.35 });
        }
      }
      if (p.vy < -200 && !(input.down('Space') || input.down('ArrowUp') || input.down('KeyW'))) p.vy += GRAV * 1.6 * dt; // pulo variável
      moveX(p, p.vx * dt); moveY(p, p.vy * dt);
      p.x = Math.max(0, p.x);
      p.inv = Math.max(0, p.inv - dt);
      if (p.y > LH * T + 100) hurt();

      const box = { x: p.x, y: p.y, w: PW, h: PH };
      for (const c of g.coins) if (!c.got && Math.abs(c.x - (p.x + PW / 2)) < 20 && Math.abs(c.y - (p.y + PH / 2)) < 22) { c.got = true; g.score += 10; g.tcoin++; GF.play('coin', { vol: 0.4 }); GF.event('coin'); }
      for (const s of g.spikes) if (hit(box, s)) hurt();
      for (const e of g.enemies) {
        if (e.dead) { e.dead += dt; continue; }
        const ahead = e.vx > 0 ? e.x + e.w + 2 : e.x - 2;
        const wallAhead = solid(Math.floor(ahead / T), Math.floor((e.y + e.h / 2) / T));
        const floorAhead = solid(Math.floor(ahead / T), Math.floor((e.y + e.h + 4) / T));
        if (wallAhead || !floorAhead) e.vx = -e.vx;
        e.x += e.vx * dt;
        if (hit(box, e)) {
          if (p.vy > 80 && p.y + PH - 10 < e.y + e.h / 2) { e.dead = 0.001; p.vy = -420; g.score += 100; GF.play('stomp'); GF.event('kill'); }
          else hurt();
        }
      }
      g.enemies = g.enemies.filter((e) => e.dead < 0.4);
      if (g.goal && hit(box, g.goal)) {
        g.score += 200;
        if (g.level + 1 >= GF.cfg.world.levels.length) { GF.finish(true, { score: g.score }); }
        else { GF.play('win', { vol: 0.5 }); g.msg = 2; const keep = { score: g.score, lives: g.lives, tcoin: g.tcoin }; loadLevel(g.level + 1); Object.assign(g, keep); }
      }
      g.msg = Math.max(0, g.msg - dt);
      camX = GF.lerp(camX, GF.clamp(p.x + PW / 2 - GF.W / 2, 0, LW * T - GF.W), Math.min(1, dt * 6));
    },
    draw(ctx) {
      const P = GF.palette, oy = GF.H - LH * T;
      // fundo com parallax
      const bg = GF.img('bg');
      const sky = ctx.createLinearGradient(0, 0, 0, GF.H); sky.addColorStop(0, P.bg); sky.addColorStop(1, P.bg2);
      ctx.fillStyle = sky; ctx.fillRect(0, 0, GF.W, GF.H);
      if (bg) {
        const s = GF.H / bg.height, w = bg.width * s, off = -((camX * 0.25) % w);
        for (let x = off; x < GF.W; x += w) ctx.drawImage(bg, x, 0, w, GF.H);
      }
      ctx.save(); ctx.translate(-Math.round(camX), oy);
      // tiles
      const x0 = Math.max(0, Math.floor(camX / T)), x1 = Math.min(LW - 1, Math.ceil((camX + GF.W) / T));
      const top = GF.img('tile_top'), body = GF.img('tile');
      for (let y = 0; y < LH; y++) for (let x = x0; x <= x1; x++) {
        if (rows[y][x] !== '#') continue;
        const isTop = !solid(x, y - 1), im = isTop ? top || body : body;
        if (im) ctx.drawImage(im, x * T, y * T, T, T);
        else { ctx.fillStyle = isTop ? P.secondary : P.ground; ctx.fillRect(x * T, y * T, T, T); }
      }
      // espinhos
      for (const s of g.spikes) { ctx.fillStyle = '#d8d8e0'; for (let k = 0; k < 4; k++) { ctx.beginPath(); ctx.moveTo(s.x + k * 8, s.y + s.h); ctx.lineTo(s.x + k * 8 + 4, s.y); ctx.lineTo(s.x + k * 8 + 8, s.y + s.h); ctx.fill(); } }
      // bandeira
      if (g.goal) { const im = GF.img('goal'); if (im) ctx.drawImage(im, g.goal.x - 6, g.goal.y, 32, 64); else { ctx.fillStyle = '#ddd'; ctx.fillRect(g.goal.x + 8, g.goal.y, 4, 64); ctx.fillStyle = P.danger; ctx.fillRect(g.goal.x + 12, g.goal.y + 4, 22, 16); } }
      // moedas
      const item = GF.img('item');
      for (const c of g.coins) {
        if (c.got) continue;
        const bob = Math.sin(GF.time * 4 + c.ph) * 3;
        if (item) ctx.drawImage(item, c.x - 11, c.y - 11 + bob, 22, 22);
        else { ctx.fillStyle = P.accent; ctx.beginPath(); ctx.arc(c.x, c.y + bob, 8, 0, 7); ctx.fill(); }
      }
      // inimigos
      for (const e of g.enemies) {
        const im = GF.img(e.kind === 2 ? 'enemy2' : 'enemy') || GF.img('enemy');
        ctx.save(); ctx.translate(e.x + e.w / 2, e.y + e.h / 2); if (e.vx > 0) ctx.scale(-1, 1);
        if (e.dead) ctx.scale(1, Math.max(0.1, 1 - e.dead * 2.4));
        if (im) ctx.drawImage(im, -16, -16 + (e.dead ? 8 : 0), 32, 32); else { ctx.fillStyle = P.danger; ctx.fillRect(-12, -13, 24, 26); }
        ctx.restore();
      }
      // jogador
      const p = g.player;
      if (p.inv <= 0 || Math.floor(p.inv * 12) % 2 === 0) {
        const im = GF.img('player');
        ctx.save(); ctx.translate(p.x + PW / 2, p.y + PH / 2 + 2); if (p.face < 0) ctx.scale(-1, 1);
        if (im) ctx.drawImage(im, -16, -17, 32, 32); else { ctx.fillStyle = P.primary; ctx.fillRect(-11, -14, 22, 28); }
        ctx.restore();
      }
      ctx.restore();
      GF.hud(ctx, [[GF.t('score'), g.score], [GF.t('lives'), '♥'.repeat(Math.max(0, g.lives))], [GF.t('level'), g.level + 1 + '/' + GF.cfg.world.levels.length]]);
      if (g.msg > 0) GF.text(ctx, GF.t('next_level'), GF.W / 2, 90, { align: 'center', size: 40, color: P.accent });
    },
  });
})();
