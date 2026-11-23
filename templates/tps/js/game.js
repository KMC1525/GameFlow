/* Ação em 3ª pessoa — arena com ondas de inimigos (game.json → world.size/obstacles/waves).
 * Controles: WASD move • ←/→, Q/E ou mouse giram a câmera • ESPAÇO/clique atira • SHIFT corre. */
(() => {
  'use strict';
  const R = 0.45;
  let g, W3, S;

  const collide = (o, x, z, r) => {
    const hx = o.w / 2 + r, hz = o.d / 2 + r, dx = x - o.x, dz = z - o.z;
    if (Math.abs(dx) >= hx || Math.abs(dz) >= hz) return null;
    const px = hx - Math.abs(dx), pz = hz - Math.abs(dz);
    return px < pz ? [o.x + Math.sign(dx || 1) * hx, z] : [x, o.z + Math.sign(dz || 1) * hz];
  };
  function push(ent, r) {
    for (const o of W3.obstacles) { const c = collide(o, ent.x, ent.z, r); if (c) { ent.x = c[0]; ent.z = c[1]; } }
    ent.x = GF.clamp(ent.x, -S + r, S - r); ent.z = GF.clamp(ent.z, -S + r, S - r);
  }
  function spawnWave() {
    const n = W3.baseEnemies + (g.wave - 1) * 2;
    for (let i = 0; i < n; i++) {
      const a = GF.rand() * Math.PI * 2, d = S * 0.75;
      const e = { x: Math.cos(a) * d, z: Math.sin(a) * d, hp: 2 + Math.floor(g.wave / 3), speed: 2 + GF.rand() * 0.8 + g.wave * 0.12, cool: 0, flash: 0, dead: 0 };
      push(e, 0.6); g.enemies.push(e);
    }
    GF.toast(`${GF.t('wave')} ${g.wave}/${W3.waves}`, 1.6);
  }

  GF.start({
    controls: 'controls_tps',
    setup() { W3 = GF.cfg.world; S = W3.size; },
    init() {
      W3 = GF.cfg.world; S = W3.size;
      g = GF.game = { score: 0, wave: 1, kills: 0, p: { x: 0, z: 0, vx: 0, vz: 0, hp: 100, cool: 0, hurt: 0, face: 0 }, enemies: [], bullets: [], drops: [], camYaw: 0, wait: 0 };
      spawnWave();
    },
    update(dt) {
      const p = g.p, I = GF.input, M = I.mouse;
      if (M.clicked && !M.locked) GF.lockPointer();
      g.camYaw += (I.axis('ArrowLeft', 'ArrowRight') + I.axis('KeyQ', 'KeyE')) * 2.2 * dt + (M.locked ? M.dx * 0.0026 : 0);
      const fx = Math.sin(g.camYaw), fz = Math.cos(g.camYaw), rx = Math.cos(g.camYaw), rz = -Math.sin(g.camYaw);
      const iz = I.axis('KeyS', 'KeyW') + I.axis('ArrowDown', 'ArrowUp'), ix = I.axis('KeyA', 'KeyD');
      let mx = fx * iz + rx * ix, mz = fz * iz + rz * ix; const len = Math.hypot(mx, mz) || 1;
      const sp = (I.down('ShiftLeft') || I.down('ShiftRight') ? 9 : 6) * (iz || ix ? 1 : 0);
      p.x += (mx / len) * sp * dt; p.z += (mz / len) * sp * dt; push(p, R);
      p.face = g.camYaw; p.cool = Math.max(0, p.cool - dt); p.hurt = Math.max(0, p.hurt - dt);
      if ((I.down('Space') || M.down) && p.cool <= 0) {
        p.cool = 0.18; GF.play('shoot', { vol: 0.25 });
        g.bullets.push({ x: p.x + fx * 0.8, z: p.z + fz * 0.8, dx: fx, dz: fz, life: 1.4 });
      }
      for (const b of g.bullets) {
        b.x += b.dx * 34 * dt; b.z += b.dz * 34 * dt; b.life -= dt;
        if (Math.abs(b.x) > S || Math.abs(b.z) > S || W3.obstacles.some((o) => Math.abs(b.x - o.x) < o.w / 2 && Math.abs(b.z - o.z) < o.d / 2)) b.life = 0;
        for (const e of g.enemies) if (!e.dead && b.life > 0 && Math.hypot(e.x - b.x, e.z - b.z) < 0.75) {
          b.life = 0; e.hp--; e.flash = 0.12; GF.play('hit', { vol: 0.25 });
          if (e.hp <= 0) { e.dead = 0.001; g.score += 100; g.kills++; GF.play('explosion', { vol: 0.3 }); GF.event('kill'); if (GF.rand() < 0.25) g.drops.push({ x: e.x, z: e.z }); }
        }
      }
      g.bullets = g.bullets.filter((b) => b.life > 0);
      for (const e of g.enemies) {
        if (e.dead) { e.dead += dt; continue; }
        e.flash = Math.max(0, e.flash - dt); e.cool = Math.max(0, e.cool - dt);
        const dx = p.x - e.x, dz = p.z - e.z, d = Math.hypot(dx, dz) || 1;
        if (d > 1.1) { e.x += (dx / d) * e.speed * dt; e.z += (dz / d) * e.speed * dt; push(e, 0.6); }
        else if (e.cool <= 0) { e.cool = 0.8; p.hp -= 8; p.hurt = 0.3; GF.play('hit', { vol: 0.4 }); GF.event('hit'); }
        for (const o of g.enemies) if (o !== e && !o.dead) { const ox = e.x - o.x, oz = e.z - o.z, od = Math.hypot(ox, oz); if (od < 0.9 && od > 0) { e.x += (ox / od) * 0.9 * dt * 3; e.z += (oz / od) * 0.9 * dt * 3; } }
      }
      g.enemies = g.enemies.filter((e) => e.dead < 0.35);
      for (const d of g.drops.slice()) if (Math.hypot(d.x - p.x, d.z - p.z) < 1) { p.hp = Math.min(100, p.hp + 20); g.drops.splice(g.drops.indexOf(d), 1); GF.play('coin', { vol: 0.4 }); }
      if (p.hp <= 0) { GF.finish(false, { score: g.score }); return; }
      if (!g.enemies.length) {
        g.wait += dt;
        if (g.wait > 1.5) { g.wait = 0; if (g.wave >= W3.waves) { g.score += 300 + Math.round(p.hp); GF.finish(true, { score: g.score }); return; } g.wave++; spawnWave(); }
      }
      const cam = E3D.cam, dist = 6.2;
      cam.x = p.x - Math.sin(g.camYaw) * dist; cam.z = p.z - Math.cos(g.camYaw) * dist; cam.y = 3.3; cam.yaw = g.camYaw; cam.pitch = -0.3;
    },
    draw(ctx) {
      const P = GF.palette; E3D.begin(ctx); E3D.fogDist = 80; E3D.sky(P.bg, P.bg2);
      E3D.ground(0, P.ground, P.bg2, 300, 3);
      const wallC = P.bg2, t = 2.5;
      E3D.box(0, 0, -S - 1, S * 2 + 4, 3.2, 2, wallC); E3D.box(0, 0, S + 1, S * 2 + 4, 3.2, 2, wallC);
      E3D.box(-S - 1, 0, 0, 2, 3.2, S * 2, wallC); E3D.box(S + 1, 0, 0, 2, 3.2, S * 2, wallC);
      void t;
      for (const o of W3.obstacles) E3D.box(o.x, 0, o.z, o.w, o.h, o.d, P.secondary);
      for (const d of g.drops) { E3D.shadow(d.x, 0, d.z, 0.4); E3D.sprite(GF.img('item'), d.x, 0.3 + Math.sin(GF.time * 4) * 0.1, d.z, 0.8, 0.8, '#6f6'); }
      for (const e of g.enemies) {
        E3D.shadow(e.x, 0, e.z, 0.6);
        const sq = e.dead ? Math.max(0.1, 1 - e.dead * 3) : 1;
        E3D.sprite(GF.img('enemy'), e.x, 0, e.z, 1.5, 1.5 * sq, (c, x, y, w, h) => { c.fillStyle = e.flash > 0 ? '#fff' : P.danger; c.fillRect(x - w / 2, y - h / 2, w, h); });
      }
      const p = g.p; E3D.shadow(p.x, 0, p.z, 0.55);
      E3D.sprite(GF.img('player'), p.x, 0, p.z, 1.6, 1.6, P.primary);
      for (const b of g.bullets) E3D.sprite(null, b.x, 1.1, b.z, 0.22, 0.22, P.accent);
      E3D.flush();
      ctx.strokeStyle = 'rgba(255,255,255,.8)'; ctx.lineWidth = 2; ctx.beginPath(); ctx.arc(GF.W / 2, GF.H * 0.5 + 40, 10, 0, 7); ctx.stroke();
      if (p.hurt > 0) { ctx.fillStyle = `rgba(200,20,20,${p.hurt})`; ctx.fillRect(0, 0, GF.W, GF.H); }
      GF.hud(ctx, [[GF.t('score'), g.score], [GF.t('wave'), `${g.wave}/${W3.waves}`], [GF.t('enemies'), g.enemies.length]]);
      GF.bar(ctx, 20, GF.H - 34, 220, 16, p.hp / 100, p.hp > 35 ? P.primary : P.danger); GF.text(ctx, GF.t('hp') + ' ' + Math.max(0, Math.ceil(p.hp)), 24, GF.H - 60, { size: 16 });
    },
  });
})();
