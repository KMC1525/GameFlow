/* Renderizador 2D (visão de cima) para o RPG. Lê o estado `g` criado em js/game.js. */
(() => {
  'use strict';
  const TS = 44;
  let camX = 0, camY = 0;
  const tileImg = { '.': 'tile_floor', '#': 'tile_wall', '~': 'tile_water' };

  window.RPGRender = {
    yaw: 0,
    init(g) { camX = g.p.x * TS - GF.W / 2; camY = g.p.y * TS - GF.H / 2; },
    update(dt, g) {
      const tx = GF.clamp(g.p.x * TS - GF.W / 2, 0, Math.max(0, g.MW * TS - GF.W)), ty = GF.clamp(g.p.y * TS - GF.H / 2, 0, Math.max(0, g.MH * TS - GF.H));
      camX = GF.lerp(camX, tx, Math.min(1, dt * 7)); camY = GF.lerp(camY, ty, Math.min(1, dt * 7));
    },
    draw(ctx, g) {
      const P = GF.palette;
      ctx.fillStyle = P.bg; ctx.fillRect(0, 0, GF.W, GF.H);
      ctx.save(); ctx.translate(-Math.round(camX), -Math.round(camY));
      const x0 = Math.max(0, Math.floor(camX / TS)), x1 = Math.min(g.MW - 1, Math.ceil((camX + GF.W) / TS));
      const y0 = Math.max(0, Math.floor(camY / TS)), y1 = Math.min(g.MH - 1, Math.ceil((camY + GF.H) / TS));
      const fallback = { '.': P.ground, '#': P.bg2, '~': '#3b6fb6' };
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const c = g.rows[y][x], t = c === '#' || c === '~' ? c : '.';
        const im = GF.img(tileImg[t]);
        if (t === '#' && GF.img('tile_floor')) ctx.drawImage(GF.img('tile_floor'), x * TS, y * TS, TS, TS);
        if (im) ctx.drawImage(im, x * TS, y * TS, TS, TS); else { ctx.fillStyle = fallback[t]; ctx.fillRect(x * TS, y * TS, TS, TS); }
        if (t === '~') { ctx.fillStyle = 'rgba(255,255,255,.12)'; ctx.fillRect(x * TS + ((GF.time * 20 + y * 9) % TS), y * TS + 10, 8, 2); }
      }
      // entidades ordenadas por y
      const draws = [];
      const spr = (img, x, y, s, color, extra) => draws.push({ y, f: () => {
        const px = x * TS, py = y * TS;
        ctx.fillStyle = 'rgba(0,0,0,.28)'; ctx.beginPath(); ctx.ellipse(px, py + s * 0.38, s * 0.34, s * 0.12, 0, 0, 7); ctx.fill();
        const im = GF.img(img);
        if (im) ctx.drawImage(im, px - s / 2, py - s * 0.62, s, s); else { ctx.fillStyle = color; ctx.fillRect(px - s * 0.3, py - s * 0.5, s * 0.6, s * 0.85); }
        if (extra) extra(px, py);
      } });
      for (const it of g.items) { const b = Math.sin(GF.time * 3 + it.ph) * 3; draws.push({ y: it.y, f: () => { const im = GF.img('item'); if (im) ctx.drawImage(im, it.x * TS - 15, it.y * TS - 20 + b, 30, 30); else { ctx.fillStyle = P.accent; ctx.beginPath(); ctx.arc(it.x * TS, it.y * TS - 5 + b, 9, 0, 7); ctx.fill(); } } }); }
      for (const n of g.npcs) spr('npc', n.x, n.y, TS * 1.05, P.secondary, (px, py) => { if (Math.hypot(n.x - g.p.x, n.y - g.p.y) < 1.5) GF.text(ctx, '!', px, py - TS * 0.95, { align: 'center', size: 22, color: P.accent }); });
      for (const m of g.monsters) { if (m.dead) ctx.globalAlpha = Math.max(0, 1 - m.dead * 2.5); spr(m.kind === 2 ? 'enemy2' : 'enemy', m.x, m.y, TS, P.danger, (px, py) => { GF.bar(ctx, px - 14, py - TS * 0.72, 28, 3, m.hp / 3, P.danger); if (m.flash > 0) { ctx.fillStyle = 'rgba(255,255,255,.5)'; ctx.fillRect(px - 14, py - 20, 28, 34); } }); draws.push({ y: m.y + 0.01, f: () => (ctx.globalAlpha = 1) }); }
      if (g.boss && !g.boss.dead) spr('boss', g.boss.x, g.boss.y, TS * 2, P.danger, (px, py) => { if (!g.boss.awake) { ctx.strokeStyle = P.accent; ctx.globalAlpha = 0.5 + Math.sin(GF.time * 4) * 0.3; ctx.lineWidth = 3; ctx.beginPath(); ctx.arc(px, py - 10, TS * 1.1, 0, 7); ctx.stroke(); ctx.globalAlpha = 1; } if (g.boss.flash > 0) { ctx.fillStyle = 'rgba(255,255,255,.4)'; ctx.fillRect(px - 30, py - 60, 60, 80); } });
      const p = g.p;
      if (p.inv <= 0 || Math.floor(p.inv * 14) % 2 === 0) spr('player', p.x, p.y + (p.moving ? Math.sin(p.anim) * 0.02 : 0), TS * 1.05, P.primary);
      draws.sort((a, b) => a.y - b.y).forEach((d) => d.f());
      // arco do ataque
      if (p.swing > 0) {
        ctx.strokeStyle = P.accent; ctx.lineWidth = 6; ctx.globalAlpha = p.swing / 0.2; ctx.beginPath();
        const a = Math.atan2(p.fy, p.fx); ctx.arc(p.x * TS, p.y * TS - 6, TS * 1.0, a - 1.0, a + 1.0); ctx.stroke(); ctx.globalAlpha = 1;
      }
      ctx.restore();
    },
  };
})();
