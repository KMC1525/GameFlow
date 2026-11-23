/* Plataforma 3D — percurso de plataformas (game.json → world.platforms/coins/enemies). Câmera em 3ª pessoa.
 * Controles: WASD move (relativo à câmera) • ESPAÇO pula • ←/→ ou Q/E giram a câmera. */
(() => {
  'use strict';
  const GRAV = 28, JUMPV = 10.2, SPEED = 6.2, R = 0.32, THICK = 1.2;
  let g, W3;

  const platColor = (k) => ({ start: GF.palette.primary, checkpoint: GF.palette.accent, goal: '#f5d76e' }[k] || GF.palette.ground);

  function respawn() {
    const c = g.checkpoint, p = g.p;
    p.x = c.x; p.y = c.y + 0.5; p.z = c.z; p.vx = p.vy = p.vz = 0; p.inv = 1.5;
  }
  function hurt() {
    if (g.p.inv > 0) return;
    g.lives--; GF.play('hit'); GF.event('hit', { lives: g.lives });
    if (g.lives <= 0) { GF.finish(false, { score: g.score }); return; }
    respawn();
  }

  GF.start({
    controls: 'controls_platformer',
    setup() { W3 = GF.cfg.world; },
    init() {
      W3 = GF.cfg.world;
      const s = W3.platforms[0];
      g = GF.game = {
        score: 0, lives: 3, coinsGot: 0, p: { x: s.x, y: s.y, z: s.z, vx: 0, vy: 0, vz: 0, onGround: false, inv: 0, jumps: 0, face: Math.PI },
        camYaw: Math.PI, checkpoint: { x: s.x, y: s.y, z: s.z }, coins: W3.coins.map((c) => ({ ...c, got: false })),
        enemies: W3.enemies.map((e) => ({ ...e, ox: e.x, dir: 1, dead: 0 })), startZ: s.z, endZ: W3.platforms[W3.platforms.length - 1].z,
      };
      g.p.y = s.y;
    },
    update(dt) {
      const p = g.p, I = GF.input;
      g.camYaw += (I.axis('ArrowLeft', 'ArrowRight') + I.axis('KeyQ', 'KeyE')) * 2.2 * dt;
      if (GF.input.mouse.down) g.camYaw += GF.input.mouse.dx * 0.006;
      const fx = Math.sin(g.camYaw), fz = Math.cos(g.camYaw), rx = Math.cos(g.camYaw), rz = -Math.sin(g.camYaw);
      const iz = I.axis('KeyS', 'KeyW') + I.axis('ArrowDown', 'ArrowUp'), ix = I.axis('KeyA', 'KeyD');
      let mx = fx * iz + rx * ix, mz = fz * iz + rz * ix;
      const len = Math.hypot(mx, mz);
      if (len > 0) { mx /= len; mz /= len; p.face = Math.atan2(mx, mz); }
      const accel = p.onGround ? 40 : 18, tvx = mx * SPEED, tvz = mz * SPEED;
      p.vx += GF.clamp(tvx - p.vx, -accel * dt, accel * dt); p.vz += GF.clamp(tvz - p.vz, -accel * dt, accel * dt);
      p.vy -= GRAV * dt;
      if (I.pressed('Space') && p.onGround) { p.vy = JUMPV; p.onGround = false; GF.play('jump', { vol: 0.35 }); }
      else if (I.pressed('Space') && GF.cfg.features?.includes('double_jump') && p.jumps < 1) { p.vy = JUMPV; p.jumps++; GF.play('jump', { vol: 0.35 }); }

      const prevY = p.y;
      p.x += p.vx * dt; p.z += p.vz * dt; p.y += p.vy * dt;
      p.onGround = false;
      for (const pl of W3.platforms) {
        const hx = pl.w / 2 + R, hz = pl.d / 2 + R, top = pl.y, bot = pl.y - THICK;
        const dx = p.x - pl.x, dz = p.z - pl.z;
        if (Math.abs(dx) >= hx || Math.abs(dz) >= hz) continue;
        if (p.vy <= 0 && prevY >= top - 0.12 && p.y <= top) {                // pousou em cima
          p.y = top; p.vy = 0; p.onGround = true; p.jumps = 0;
          if (pl.kind === 'checkpoint' && g.checkpoint.z !== pl.z) { g.checkpoint = { x: pl.x, y: pl.y, z: pl.z }; GF.toast(GF.t('checkpoint'), 1.2); GF.play('coin'); }
          if (pl.kind === 'goal') { g.score += 500; GF.finish(true, { score: g.score }); }
        } else if (p.y < top - 0.05 && p.y + 0.9 > bot) {                    // bateu de lado
          const px = hx - Math.abs(dx), pz = hz - Math.abs(dz);
          if (px < pz) { p.x = pl.x + Math.sign(dx) * hx; p.vx = 0; } else { p.z = pl.z + Math.sign(dz) * hz; p.vz = 0; }
        } else if (p.vy > 0 && p.y + 0.9 > bot && prevY + 0.9 <= bot + 0.05) { p.vy = 0; } // cabeçada por baixo
      }
      p.inv = Math.max(0, p.inv - dt);
      if (p.y < W3.killY) hurt();

      for (const c of g.coins) if (!c.got && Math.hypot(c.x - p.x, c.z - p.z) < 0.9 && Math.abs(c.y - (p.y + 0.6)) < 1.1) { c.got = true; g.score += 10; g.coinsGot++; GF.play('coin', { vol: 0.4 }); GF.event('coin'); }
      for (const e of g.enemies) {
        if (e.dead) { e.dead += dt; continue; }
        e.x += e.dir * e.speed * dt; if (Math.abs(e.x - e.ox) > e.range) { e.dir = -e.dir; e.x = e.ox + Math.sign(e.x - e.ox) * e.range; }
        const d = Math.hypot(e.x - p.x, e.z - p.z);
        if (d < 0.85 && Math.abs(p.y - e.y) < 1.1) {
          if (p.vy < -1 && p.y > e.y + 0.45) { e.dead = 0.001; p.vy = 8.5; g.score += 100; GF.play('stomp'); GF.event('kill'); } else hurt();
        }
      }
      g.enemies = g.enemies.filter((e) => e.dead < 0.35);

      // câmera
      const cam = E3D.cam, dist = 7.5, h = 4.4;
      const tx = p.x - Math.sin(g.camYaw) * dist, tz = p.z - Math.cos(g.camYaw) * dist, ty = p.y + h;
      const k = Math.min(1, dt * 8);
      cam.x = GF.lerp(cam.x, tx, k); cam.y = GF.lerp(cam.y, ty, k); cam.z = GF.lerp(cam.z, tz, k);
      cam.yaw = g.camYaw; cam.pitch = -0.34; cam.fov = 70;
    },
    draw(ctx) {
      const P = GF.palette;
      E3D.begin(ctx);
      E3D.fogDist = 70;
      E3D.sky(P.bg, P.bg2);
      const bgc = E3D.rgb(P.bg), dgc = E3D.rgb(P.danger);
      E3D.ground(W3.killY + 0.5, dgc.map((v, i) => v * 0.62 + bgc[i] * 0.38), dgc.map((v) => Math.min(255, v * 1.15 + 25)), 300, 4);
      for (const pl of W3.platforms) {
        E3D.box(pl.x, pl.y - THICK, pl.z, pl.w, THICK, pl.d, P.ground);
        E3D.plane(pl.x, pl.y + 0.01, pl.z, pl.w - 0.1, pl.d - 0.1, platColor(pl.kind));
        if (pl.kind === 'goal') E3D.box(pl.x, pl.y, pl.z, 0.25, 3.2, 0.25, '#e8e8f0');
        if (pl.kind === 'goal') E3D.box(pl.x + 0.65, pl.y + 2.2, pl.z, 1.2, 0.9, 0.08, P.danger);
      }
      const item = GF.img('item');
      for (const c of g.coins) if (!c.got) { const bob = Math.sin(GF.time * 4 + c.x) * 0.12; E3D.sprite(item, c.x, c.y + bob - 0.3, c.z, 0.7, 0.7, P.accent); }
      for (const e of g.enemies) {
        E3D.shadow(e.x, e.y, e.z, 0.5);
        const sq = e.dead ? Math.max(0.15, 1 - e.dead * 3) : 1;
        E3D.sprite(GF.img('enemy'), e.x, e.y, e.z, 1.2, 1.2 * sq, P.danger);
      }
      const p = g.p;
      E3D.shadow(p.x, g.p.onGround ? p.y : groundBelow(p), p.z, 0.45);
      if (p.inv <= 0 || Math.floor(p.inv * 12) % 2 === 0) E3D.sprite(GF.img('player'), p.x, p.y, p.z, 1.15, 1.15, P.primary);
      E3D.flush();
      const prog = GF.clamp((g.startZ - p.z) / (g.startZ - g.endZ), 0, 1);
      GF.hud(ctx, [[GF.t('score'), g.score], [GF.t('lives'), '♥'.repeat(Math.max(0, g.lives))], [GF.t('coins'), g.coinsGot]]);
      GF.bar(ctx, GF.W - 224, 18, 200, 10, prog, P.accent);
    },
  });

  function groundBelow(p) {
    let best = W3.killY + 0.5;
    for (const pl of W3.platforms) if (Math.abs(p.x - pl.x) < pl.w / 2 && Math.abs(p.z - pl.z) < pl.d / 2 && pl.y <= p.y + 0.2) best = Math.max(best, pl.y);
    return best;
  }
})();
