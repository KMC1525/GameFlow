/* Arcade (nave) — ondas de inimigos, power-ups e chefe final (game.json → world.waves, features).
 * Controles: setas/WASD movem • ESPAÇO atira. */
(() => {
  'use strict';
  let g, stars;
  const F = () => GF.cfg.features || [];

  function spawnWave() {
    const w = g.wave, total = GF.cfg.world.waves, boss = F().includes('boss') && w === total;
    if (boss) { g.enemies.push({ boss: true, x: GF.W / 2, y: -80, hp: 40 + total * 2, max: 40 + total * 2, t: 0, cool: 1, r: 56 }); GF.toast(GF.t('boss_awake'), 2); return; }
    const n = 6 + w * 2, cols = Math.min(n, 8);
    for (let i = 0; i < n; i++) {
      const fast = w > 2 && i % 3 === 2;
      g.enemies.push({ x: 120 + ((i % cols) / Math.max(1, cols - 1)) * (GF.W - 240), y: -40 - Math.floor(i / cols) * 70, hp: fast ? 1 : 2, kind: fast ? 2 : 1, ph: GF.rand() * 6, speed: 40 + w * 6 + (fast ? 30 : 0), r: 20, cool: 1 + GF.rand() * 3, t: 0 });
    }
    GF.toast(`${GF.t('wave')} ${w}/${total}`, 1.5);
  }

  function hitPlayer() {
    const p = g.p; if (p.inv > 0) return;
    if (p.shield > 0) { p.shield--; p.inv = 1; GF.play('hit'); return; }
    g.lives--; p.inv = 2; GF.play('explosion', { vol: 0.5 }); GF.event('hit', { lives: g.lives });
    p.spread = 0;
    if (g.lives <= 0) GF.finish(false, { score: g.score });
  }

  GF.start({
    controls: 'controls_arcade',
    setup() { stars = Array.from({ length: 90 }, () => ({ x: GF.rand() * GF.W, y: GF.rand() * GF.H, s: 0.3 + GF.rand() * 1.2 })); },
    init() {
      g = GF.game = { score: 0, lives: 3, wave: 1, enemies: [], shots: [], ebullets: [], pickups: [], p: { x: GF.W / 2, y: GF.H - 70, cool: 0, inv: 0, shield: F().includes('shield') ? 1 : 0, spread: F().includes('spread_shot') ? 99 : 0 }, wait: 0 };
      spawnWave();
    },
    update(dt) {
      const p = g.p, I = GF.input;
      p.x = GF.clamp(p.x + (I.axis('KeyA', 'KeyD') + I.axis('ArrowLeft', 'ArrowRight')) * 360 * dt, 28, GF.W - 28);
      p.y = GF.clamp(p.y + (I.axis('KeyW', 'KeyS') + I.axis('ArrowUp', 'ArrowDown')) * 300 * dt, GF.H * 0.45, GF.H - 36);
      p.cool -= dt; p.inv = Math.max(0, p.inv - dt); p.spread = p.spread > 0 && p.spread < 90 ? p.spread - dt : p.spread;
      if (I.down('Space') && p.cool <= 0) {
        p.cool = 0.2; GF.play('shoot', { vol: 0.25 });
        const angles = p.spread > 0 ? [-0.18, 0, 0.18] : [0];
        for (const a of angles) g.shots.push({ x: p.x, y: p.y - 22, vx: Math.sin(a) * 600, vy: -Math.cos(a) * 600 });
      }
      for (const s of g.shots) { s.x += s.vx * dt; s.y += s.vy * dt; }
      g.shots = g.shots.filter((s) => s.y > -20 && s.x > -20 && s.x < GF.W + 20);
      for (const e of g.enemies) {
        e.t += dt;
        if (e.boss) {
          e.y = Math.min(110, e.y + 60 * dt); e.x = GF.W / 2 + Math.sin(e.t * 0.9) * (GF.W / 2 - 140);
          e.cool -= dt; if (e.cool <= 0 && e.y > 90) { e.cool = 0.9; for (let k = -2; k <= 2; k++) g.ebullets.push({ x: e.x, y: e.y + 50, vx: k * 90, vy: 210 }); }
        } else {
          e.y += e.speed * dt; e.x += Math.sin(e.t * 2 + e.ph) * (e.kind === 2 ? 170 : 70) * dt;
          e.cool -= dt; if (e.cool <= 0 && e.y > 0 && e.y < GF.H * 0.6) { e.cool = 2.5 + GF.rand() * 2.5; const a = Math.atan2(p.y - e.y, p.x - e.x); g.ebullets.push({ x: e.x, y: e.y, vx: Math.cos(a) * 190, vy: Math.sin(a) * 190 }); }
        }
        for (const s of g.shots) if (s.y > -50 && Math.hypot(s.x - e.x, s.y - e.y) < e.r + 6) { s.y = -999; e.hp--; e.flash = 0.08; GF.play('hit', { vol: 0.2 }); }
        if (e.hp <= 0 && !e.dead) {
          e.dead = true; g.score += e.boss ? 1000 : 50 * e.kind; GF.play('explosion', { vol: 0.35 }); GF.event('kill');
          if (!e.boss && GF.rand() < 0.14) g.pickups.push({ x: e.x, y: e.y, t: GF.rand() < 0.5 ? 's' : 'p' });
        }
        if (Math.hypot(p.x - e.x, p.y - e.y) < e.r + 14 && !e.dead) { if (!e.boss) { e.dead = true; e.hp = 0; } hitPlayer(); }
        if (e.y > GF.H + 40) e.dead = true;
        if (e.flash) e.flash = Math.max(0, e.flash - dt);
      }
      g.enemies = g.enemies.filter((e) => !e.dead);
      for (const b of g.ebullets) { b.x += b.vx * dt; b.y += b.vy * dt; if (Math.hypot(b.x - p.x, b.y - p.y) < 14) { b.y = 9999; hitPlayer(); } }
      g.ebullets = g.ebullets.filter((b) => b.y < GF.H + 20 && b.y > -20 && b.x > -20 && b.x < GF.W + 20);
      for (const k of g.pickups) { k.y += 90 * dt; if (Math.hypot(k.x - p.x, k.y - p.y) < 28) { k.y = 9999; GF.play('coin'); g.score += 25; if (k.t === 's') p.shield = 1; else p.spread = 10; } }
      g.pickups = g.pickups.filter((k) => k.y < GF.H + 20);
      if (!g.enemies.length) { g.wait += dt; if (g.wait > 1.2) { g.wait = 0; if (g.wave >= GF.cfg.world.waves) { g.score += 500; GF.finish(true, { score: g.score }); } else { g.wave++; spawnWave(); } } }
    },
    draw(ctx) {
      const P = GF.palette, bg = GF.img('bg');
      const sky = ctx.createLinearGradient(0, 0, 0, GF.H); sky.addColorStop(0, P.bg); sky.addColorStop(1, P.bg2); ctx.fillStyle = sky; ctx.fillRect(0, 0, GF.W, GF.H);
      if (bg) { const off = (GF.time * 30) % GF.H; ctx.globalAlpha = 0.55; ctx.drawImage(bg, 0, off - GF.H, GF.W, GF.H); ctx.drawImage(bg, 0, off, GF.W, GF.H); ctx.globalAlpha = 1; }
      ctx.fillStyle = P.text; for (const s of stars) { s.y = (s.y + s.s * 40 * 0.016) % GF.H; ctx.globalAlpha = 0.4 + s.s * 0.3; ctx.fillRect(s.x, s.y, 2, 2 * s.s); } ctx.globalAlpha = 1;
      const img = (id, x, y, w, h, color, flip) => { const im = GF.img(id); ctx.save(); ctx.translate(x, y); if (flip) ctx.scale(1, -1); if (im) ctx.drawImage(im, -w / 2, -h / 2, w, h); else { ctx.fillStyle = color; ctx.beginPath(); ctx.moveTo(0, -h / 2); ctx.lineTo(w / 2, h / 2); ctx.lineTo(-w / 2, h / 2); ctx.fill(); } ctx.restore(); };
      for (const k of g.pickups) { img('item', k.x, k.y, 30, 30, P.accent); GF.text(ctx, k.t === 's' ? 'S' : '3', k.x, k.y - 8, { align: 'center', size: 16 }); }
      for (const e of g.enemies) {
        const s = e.boss ? 128 : 48; if (e.flash) ctx.globalAlpha = 0.6;
        img(e.boss ? 'boss' : e.kind === 2 ? 'enemy2' : 'enemy', e.x, e.y, s, s, P.danger, false); ctx.globalAlpha = 1;
        if (e.boss) GF.bar(ctx, GF.W / 2 - 150, GF.H - 26, 300, 10, e.hp / e.max, P.danger);
      }
      ctx.fillStyle = P.accent; for (const s of g.shots) ctx.fillRect(s.x - 2, s.y - 8, 4, 14);
      ctx.fillStyle = P.danger; for (const b of g.ebullets) { ctx.beginPath(); ctx.arc(b.x, b.y, 5, 0, 7); ctx.fill(); }
      const p = g.p;
      if (p.inv <= 0 || Math.floor(p.inv * 12) % 2 === 0) { img('player', p.x, p.y, 48, 48, P.primary); if (p.shield > 0) { ctx.strokeStyle = P.accent; ctx.lineWidth = 3; ctx.globalAlpha = 0.7; ctx.beginPath(); ctx.arc(p.x, p.y, 34, 0, 7); ctx.stroke(); ctx.globalAlpha = 1; } }
      GF.hud(ctx, [[GF.t('score'), g.score], [GF.t('lives'), '♥'.repeat(Math.max(0, g.lives))], [GF.t('wave'), `${g.wave}/${GF.cfg.world.waves}`]]);
    },
  });
})();
