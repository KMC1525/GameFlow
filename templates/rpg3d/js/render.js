/* Renderizador 3D (câmera elevada em 3ª pessoa) para o RPG, usando a mini-engine E3D.
 * Mapeamento: x do mapa → x do mundo; y do mapa → -z do mundo (linha 0 fica ao fundo). Q/E giram a câmera. */
(() => {
  'use strict';
  let yaw = 0;
  const RANGE = 15;

  window.RPGRender = {
    get yaw() { return yaw; },
    rotateKeys: true,
    controls: 'controls_rpg3d',
    init() { yaw = 0; },
    update(dt) {
      const I = GF.input; yaw += (I.axis('KeyQ', 'KeyE')) * 1.8 * dt;
      const g = GF.game, cam = E3D.cam, dist = 6.2, h = 5.8;
      const px = g.p.x, pz = -g.p.y;
      const tx = px - Math.sin(yaw) * dist, tz = pz - Math.cos(yaw) * dist;
      cam.x = GF.lerp(cam.x, tx, Math.min(1, dt * 8)); cam.z = GF.lerp(cam.z, tz, Math.min(1, dt * 8)); cam.y = h; cam.yaw = yaw; cam.pitch = -0.62; cam.fov = 66;
    },
    draw(ctx, g) {
      const P = GF.palette; E3D.begin(ctx); E3D.fogDist = 26; E3D.sky(P.bg, P.bg2);
      E3D.ground(0, P.ground, P.bg2, 200, 2);
      const p = g.p, x0 = Math.max(0, Math.floor(p.x - RANGE)), x1 = Math.min(g.MW - 1, Math.ceil(p.x + RANGE)), y0 = Math.max(0, Math.floor(p.y - RANGE)), y1 = Math.min(g.MH - 1, Math.ceil(p.y + RANGE));
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const c = g.rows[y][x];
        if (c === '#') E3D.box(x + 0.5, 0, -(y + 0.5), 1, 1.3 + ((x * 7 + y * 13) % 5) * 0.12, 1, P.bg2);
        else if (c === '~') E3D.plane(x + 0.5, 0.02, -(y + 0.5), 1, 1, '#3b6fb6');
      }
      for (const it of g.items) { E3D.shadow(it.x, 0, -it.y, 0.3); E3D.sprite(GF.img('item'), it.x, 0.4 + Math.sin(GF.time * 3 + it.ph) * 0.1, -it.y, 0.7, 0.7, P.accent); }
      for (const n of g.npcs) { E3D.shadow(n.x, 0, -n.y, 0.4); E3D.sprite(GF.img('npc'), n.x, 0, -n.y, 1.2, 1.2, P.secondary); }
      for (const m of g.monsters) { E3D.shadow(m.x, 0, -m.y, 0.4); E3D.sprite(GF.img(m.kind === 2 ? 'enemy2' : 'enemy'), m.x, 0, -m.y, 1.2, m.dead ? Math.max(0.1, 1.2 - m.dead * 3) : 1.2, (c, x, y, w, h) => { c.fillStyle = m.flash > 0 ? '#fff' : P.danger; c.fillRect(x - w / 2, y - h / 2, w, h); }); }
      if (g.boss && !g.boss.dead) { E3D.shadow(g.boss.x, 0, -g.boss.y, 0.9); E3D.sprite(GF.img('boss'), g.boss.x, 0, -g.boss.y, 2.6, 2.6, P.danger); }
      E3D.shadow(p.x, 0, -p.y, 0.4);
      if (p.inv <= 0 || Math.floor(p.inv * 14) % 2 === 0) E3D.sprite(GF.img('player'), p.x, 0, -p.y, 1.25, 1.25, P.primary);
      if (p.swing > 0) { const a = p.swing / 0.2; E3D.sprite(null, p.x + p.fx * 0.9, 0.6, -(p.y + p.fy * 0.9), 1.4, 1.4, (c, x, y, w) => { c.strokeStyle = P.accent; c.lineWidth = 6; c.globalAlpha = a; c.beginPath(); c.arc(x, y, w / 2, 0, 7); c.stroke(); c.globalAlpha = 1; }); }
      E3D.flush();
    },
  };
})();
