/* RPG (2D e 3D compartilham esta lógica; o desenho fica em js/render.js).
 * Mapa em game.json → world.rows:  # parede/árvore   ~ água   . chão   P início   N NPC   I relíquia   M/m monstro   B chefe
 * Controles: WASD move • ESPAÇO/J ataca (ESPAÇO perto de um NPC = falar/avançar diálogo) • Q/E giram a câmera (só no 3D). */
(() => {
  'use strict';
  let g, rows, MW, MH;
  const R = 0.3;

  const blocked = (x, y) => { const cx = Math.floor(x), cy = Math.floor(y); if (cx < 0 || cy < 0 || cx >= MW || cy >= MH) return true; const c = rows[cy][cx]; return c === '#' || c === '~'; };
  const free = (x, y, r = R) => !blocked(x - r, y - r) && !blocked(x + r, y - r) && !blocked(x - r, y + r) && !blocked(x + r, y + r);
  function move(e, dx, dy, r = R) { if (free(e.x + dx, e.y, r)) e.x += dx; if (free(e.x, e.y + dy, r)) e.y += dy; }

  function say(name, lines) { g.dialog = { name, lines: lines.slice(), i: 0 }; GF.play('blip', { vol: 0.3 }); }
  function hurtPlayer(dmg) {
    const p = g.p; if (p.inv > 0) return;
    p.hp -= dmg; p.inv = 0.7; GF.play('hit', { vol: 0.4 }); GF.event('hit', { hp: p.hp });
    if (p.hp <= 0) GF.finish(false, { score: g.score });
  }
  function gainXp(n) {
    const p = g.p; p.xp += n;
    while (p.xp >= p.level * 30) { p.xp -= p.level * 30; p.level++; p.max += 10; p.hp = p.max; GF.play('level_up', { vol: 0.5 }); GF.toast(`${GF.t('level_up')} ${p.level}`, 1.8); GF.event('level_up', { level: p.level }); }
  }
  const dmgOf = () => 1 + Math.floor((g.p.level - 1) / 2);

  function attack() {
    const p = g.p; if (p.cool > 0 || g.dialog) return;
    p.cool = 0.38; p.swing = 0.2; GF.play('attack', { vol: 0.3 });
    const targets = g.monsters.concat(g.boss ? [g.boss] : []);
    for (const m of targets) {
      if (m.dead) continue;
      const dx = m.x - p.x, dy = m.y - p.y, d = Math.hypot(dx, dy);
      if (d > 1.25 + (m.big ? 0.5 : 0)) continue;
      const fa = Math.atan2(p.fy, p.fx); let da = Math.atan2(dy, dx) - fa; da = Math.atan2(Math.sin(da), Math.cos(da));
      if (Math.abs(da) > 1.2 && d > 0.7) continue;
      if (m.big && !m.awake) { GF.toast(GF.t('find_relics'), 1.6); GF.play('hit', { vol: 0.2 }); continue; }
      m.hp -= dmgOf(); m.flash = 0.15; m.kx = (dx / (d || 1)) * 5; m.ky = (dy / (d || 1)) * 5; GF.play('hit', { vol: 0.3 });
      if (m.hp <= 0) { m.dead = 0.001; g.score += m.big ? 500 : 50; g.kills++; gainXp(m.big ? 100 : 12); GF.event('kill', { boss: !!m.big }); }
    }
  }

  function interact() {
    const p = g.p;
    if (g.dialog) { const d = g.dialog; d.i++; GF.play('blip', { vol: 0.2 }); if (d.i >= d.lines.length) g.dialog = null; return true; }
    let best = null;
    for (const n of g.npcs) { const d = Math.hypot(n.x - p.x, n.y - p.y); if (d < 1.5 && (!best || d < best.d)) best = { n, d }; }
    if (!best) return false;
    const n = best.n, lines = n.lines.slice();
    if (n.quest) lines.push(g.relics >= g.total ? (GF.cfg.boss?.name ? `${GF.cfg.boss.name}!` : '...') : `${GF.t('relics')}: ${g.relics}/${g.total}`);
    say(n.name, lines); return true;
  }

  GF.start({
    controls: (window.RPGRender && RPGRender.controls) || 'controls_rpg',
    setup() {},
    init() {
      rows = GF.cfg.world.rows; MH = rows.length; MW = rows[0].length;
      const useBoss = (GF.cfg.features || []).includes('boss');
      g = GF.game = { rows, MW, MH, p: { x: 2, y: 2, hp: 100, max: 100, level: 1, xp: 0, fx: 0, fy: 1, inv: 0, cool: 0, swing: 0, anim: 0, moving: false }, npcs: [], monsters: [], items: [], boss: null, relics: 0, total: 0, score: 0, kills: 0, dialog: null, useBoss };
      const npcDefs = GF.cfg.npcs || [], enemies = GF.cfg.enemies || [{ name: '?' }];
      let ni = 0;
      for (let y = 0; y < MH; y++) for (let x = 0; x < MW; x++) {
        const c = rows[y][x], cx = x + 0.5, cy = y + 0.5;
        if (c === 'P') { g.p.x = cx; g.p.y = cy; }
        else if (c === 'N') { const d = npcDefs[ni % Math.max(1, npcDefs.length)] || { name: 'NPC', lines: ['...'] }; g.npcs.push({ x: cx, y: cy, name: d.name, lines: d.lines || ['...'], quest: d.role === 'quest', ph: GF.rand() * 6 }); ni++; }
        else if (c === 'I') { g.items.push({ x: cx, y: cy, ph: GF.rand() * 6 }); g.total++; }
        else if (c === 'M' || c === 'm') g.monsters.push({ x: cx, y: cy, hx: cx, hy: cy, hp: 3, kind: c === 'm' ? 2 : 1, name: (enemies[c === 'm' ? Math.min(1, enemies.length - 1) : 0] || {}).name, cool: 0, flash: 0, dead: 0, kx: 0, ky: 0, ph: GF.rand() * 6, wander: GF.rand() * 3 });
        else if (c === 'B' && useBoss) g.boss = { x: cx, y: cy, hp: 22, max: 22, big: true, awake: false, cool: 0, flash: 0, dead: 0, kx: 0, ky: 0, name: GF.cfg.boss?.name || 'Boss' };
      }
      if (g.npcs[0]) say(g.npcs[0].name, [GF.cfg.quest?.intro || '', GF.cfg.quest?.goal || ''].filter(Boolean));
      if (window.RPGRender && RPGRender.init) RPGRender.init(g);
    },
    update(dt) {
      const p = g.p, I = GF.input;
      const near = g.npcs.some((n) => Math.hypot(n.x - p.x, n.y - p.y) < 1.5), rot = window.RPGRender && RPGRender.rotateKeys;
      if (g.dialog ? (I.pressed('Space') || I.pressed('Enter') || (!rot && I.pressed('KeyE'))) : (I.pressed('Enter') && near) || (I.pressed('Space') && near) || (!rot && I.pressed('KeyE'))) interact();
      if (window.RPGRender && RPGRender.update) RPGRender.update(dt, g);
      p.inv = Math.max(0, p.inv - dt); p.cool = Math.max(0, p.cool - dt); p.swing = Math.max(0, p.swing - dt);
      if (!g.dialog) {
        const yaw = (window.RPGRender && RPGRender.yaw) || 0;
        const iz = I.axis('KeyS', 'KeyW') + I.axis('ArrowDown', 'ArrowUp'), ix = I.axis('KeyA', 'KeyD') + I.axis('ArrowLeft', 'ArrowRight');
        const fx = Math.sin(yaw), fy = -Math.cos(yaw), rx = Math.cos(yaw), ry = Math.sin(yaw);
        let mx = fx * iz + rx * ix, my = fy * iz + ry * ix; const len = Math.hypot(mx, my);
        p.moving = len > 0;
        if (len > 0) { mx /= len; my /= len; p.fx = mx; p.fy = my; move(p, mx * 3.4 * dt, my * 3.4 * dt); p.anim += dt * 9; }
        if ((I.pressed('Space') && !near) || I.pressed('KeyJ')) attack();
      }
      const all = g.monsters.concat(g.boss ? [g.boss] : []);
      for (const m of all) {
        if (m.dead) { m.dead += dt; continue; }
        m.flash = Math.max(0, m.flash - dt); m.cool = Math.max(0, m.cool - dt);
        if (m.kx || m.ky) { move(m, m.kx * dt, m.ky * dt, 0.35); m.kx *= 0.85; m.ky *= 0.85; if (Math.abs(m.kx) < 0.1) m.kx = m.ky = 0; }
        const dx = p.x - m.x, dy = p.y - m.y, d = Math.hypot(dx, dy) || 1;
        const active = m.big ? m.awake : d < 5.5;
        if (active && !g.dialog) {
          const sp = m.big ? 1.9 : 1.5 + (m.kind === 2 ? 0.4 : 0);
          if (d > 0.7) move(m, (dx / d) * sp * dt, (dy / d) * sp * dt, m.big ? 0.5 : 0.3);
          else if (m.cool <= 0) { m.cool = 1; hurtPlayer(m.big ? 14 : 7); }
        } else if (!m.big) {
          m.wander -= dt; if (m.wander < 0) { m.wander = 1 + GF.rand() * 2; m.wx = (GF.rand() - 0.5) * 0.8; m.wy = (GF.rand() - 0.5) * 0.8; }
          if (m.wx) { move(m, m.wx * dt, m.wy * dt, 0.3); if (Math.hypot(m.x - m.hx, m.y - m.hy) > 3) { m.wx = -m.wx; m.wy = -m.wy; } }
        }
      }
      g.monsters = g.monsters.filter((m) => m.dead < 0.4);
      if (g.boss && g.boss.dead > 0.6) { GF.finish(true, { score: g.score + p.hp * 2 }); return; }
      for (const it of g.items.slice()) if (Math.hypot(it.x - p.x, it.y - p.y) < 0.7) {
        g.items.splice(g.items.indexOf(it), 1); g.relics++; g.score += 100; GF.play('coin', { vol: 0.5 }); GF.event('coin');
        GF.toast(`${GF.t('relics')}: ${g.relics}/${g.total}`, 1.4);
        if (g.relics >= g.total) {
          if (g.boss) { g.boss.awake = true; GF.toast(GF.t('boss_awake'), 2.5); GF.play('explosion', { vol: 0.4 }); }
          else { GF.finish(true, { score: g.score }); return; }
        }
      }
    },
    draw(ctx) {
      RPGRender.draw(ctx, g);
      const P = GF.palette, p = g.p;
      // HUD
      GF.hud(ctx, [[GF.t('level'), p.level], [GF.t('relics'), `${g.relics}/${g.total}`], [GF.t('score'), g.score]]);
      GF.bar(ctx, 20, GF.H - 34, 220, 16, p.hp / p.max, p.hp / p.max > 0.35 ? P.primary : P.danger);
      GF.text(ctx, `${GF.t('hp')} ${Math.max(0, Math.ceil(p.hp))}/${p.max}`, 24, GF.H - 60, { size: 16 });
      GF.bar(ctx, 20, GF.H - 14, 220, 5, p.xp / (p.level * 30), P.accent);
      if (g.boss && g.boss.awake && !g.boss.dead) { GF.bar(ctx, GF.W / 2 - 150, 54, 300, 12, g.boss.hp / g.boss.max, P.danger); GF.text(ctx, g.boss.name, GF.W / 2, 28, { align: 'center', size: 16 }); }
      if (!g.dialog) {
        const near = g.npcs.find((n) => Math.hypot(n.x - p.x, n.y - p.y) < 1.5);
        if (near) GF.text(ctx, `[ESPAÇO] ${GF.t('talk')} — ${near.name}`, GF.W / 2, GF.H - 90, { align: 'center', size: 20, color: P.accent });
      } else {
        const d = g.dialog, bx = 60, by = GF.H - 170, bw = GF.W - 120, bh = 140;
        GF.box(ctx, bx, by, bw, bh);
        GF.text(ctx, d.name, bx + 24, by + 14, { size: 20, color: P.accent });
        ctx.font = '22px "Trebuchet MS", system-ui, sans-serif';
        GF.wrap(ctx, d.lines[d.i], bw - 56).slice(0, 3).forEach((l, i) => GF.text(ctx, l, bx + 24, by + 46 + i * 28, { size: 22, bold: false }));
        GF.text(ctx, `${d.i + 1}/${d.lines.length}  ▼`, bx + bw - 24, by + bh - 28, { size: 16, align: 'right', color: P.accent });
      }
    },
  });
})();
