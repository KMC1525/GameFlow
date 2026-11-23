/* Visual Novel — roteiro ramificado em game.json → story[].
 * Cada nó: { id, bg, character, speaker, text, next | choices[{text,goto,effect}] | branch[{cond,goto}] | ending } */
(() => {
  'use strict';
  let g, byId;
  const ENDING = { pt: { good: 'Final feliz', neutral: 'Final neutro', bad: 'Final triste' }, en: { good: 'Good ending', neutral: 'Neutral ending', bad: 'Bad ending' } };

  function cond(c) {
    if (!c) return true;
    const m = /^\s*([\wÀ-ſ]+)\s*(>=|<=|==|>|<)\s*(-?\d+)\s*$/.exec(c);
    if (!m) return false;
    const v = g.vars[m[1]] || 0, n = +m[3];
    return { '>=': v >= n, '<=': v <= n, '==': v === n, '>': v > n, '<': v < n }[m[2]];
  }
  function enter(id) {
    const n = byId[id];
    if (!n) { GF.finish(true, { text: '…' }); return; }
    if (g.node && n.bg !== g.node.bg) { g.prevBg = g.node.bg; g.fade = 0; }
    g.node = n; g.chars = 0; g.sel = 0; g.t = 0;
    if (n.music) GF.music(n.music);
  }
  function go(target) {
    // `branch` é resolvido ao avançar
    const n = byId[target];
    if (n && n.branch && n.branch.length && !n.text) { for (const b of n.branch) if (cond(b.cond)) return enter(b.goto); }
    enter(target);
  }
  function advance() {
    const n = g.node;
    if (g.chars < n.text.length) { g.chars = n.text.length; return; }
    if (n.choices && n.choices.length) return;
    if (n.branch && n.branch.length) { for (const b of n.branch) if (cond(b.cond)) return enter(b.goto); }
    if (n.next) return go(n.next);
    if (n.ending) { const lang = GF.cfg.lang === 'en' ? 'en' : 'pt'; GF.finish(n.ending !== 'bad', { text: `${GF.t('ending')}: ${ENDING[lang][n.ending] || n.ending}` }); }
  }
  function choose(i) {
    const c = g.node.choices[i]; if (!c) return;
    for (const [k, v] of Object.entries(c.effect || {})) g.vars[k] = (g.vars[k] || 0) + v;
    GF.play('choice', { vol: 0.4 }); GF.event('choice', { index: i, text: c.text, vars: g.vars });
    go(c.goto);
  }
  const charX = (id) => (id === 'char_player' ? 0.27 : id === 'char_rival' ? 0.74 : 0.7);

  GF.start({
    controls: 'controls_vn',
    setup() { byId = Object.fromEntries((GF.cfg.story || []).map((n) => [n.id, n])); },
    init() {
      g = GF.game = { vars: {}, node: null, chars: 0, sel: 0, t: 0, fade: 1, prevBg: null, rects: [] };
      enter(byId.start ? 'start' : GF.cfg.story[0].id);
      g.fade = 1;
    },
    update(dt) {
      const n = g.node, I = GF.input, M = I.mouse;
      g.t += dt; g.fade = Math.min(1, g.fade + dt * 2.2);
      const prev = Math.floor(g.chars);
      if (g.chars < n.text.length) g.chars += dt * 48;
      if (Math.floor(g.chars) > prev && Math.floor(g.chars) % 3 === 0 && n.speaker) GF.play('blip', { vol: 0.12 });
      const hasChoices = n.choices && n.choices.length && g.chars >= n.text.length;
      if (hasChoices) {
        const cs = n.choices;
        if (I.pressed('ArrowUp') || I.pressed('KeyW')) g.sel = (g.sel + cs.length - 1) % cs.length;
        if (I.pressed('ArrowDown') || I.pressed('KeyS')) g.sel = (g.sel + 1) % cs.length;
        for (let i = 0; i < cs.length; i++) if (I.pressed('Digit' + (i + 1)) || I.pressed('Numpad' + (i + 1))) return choose(i);
        g.rects.forEach((r, i) => { if (M.x >= r.x && M.x <= r.x + r.w && M.y >= r.y && M.y <= r.y + r.h) { g.sel = i; if (M.clicked) { choose(i); } } });
        if (I.pressed('Enter') || I.pressed('Space')) choose(g.sel);
      } else if (I.pressed('Enter') || I.pressed('Space') || M.clicked) advance();
    },
    draw(ctx) {
      const P = GF.palette, W = GF.W, H = GF.H, n = g.node;
      const grad = ctx.createLinearGradient(0, 0, 0, H); grad.addColorStop(0, P.bg); grad.addColorStop(1, P.bg2); ctx.fillStyle = grad; ctx.fillRect(0, 0, W, H);
      if (g.prevBg && g.fade < 1) { GF.cover(ctx, GF.img(g.prevBg), 0, 0, W, H); ctx.globalAlpha = g.fade; }
      GF.cover(ctx, GF.img(n.bg), 0, 0, W, H); ctx.globalAlpha = 1;
      // personagem
      if (n.character) {
        const im = GF.img(n.character);
        const cx = W * charX(n.character), ch = H * 0.86, bob = Math.sin(g.t * 2) * 2;
        if (im) { const cw = (im.width / im.height) * ch; ctx.drawImage(im, cx - cw / 2, H - ch - 70 + bob, cw, ch); }
        else { ctx.fillStyle = P.primary; ctx.beginPath(); ctx.ellipse(cx, H * 0.55, 80, 170, 0, 0, 7); ctx.fill(); }
      }
      // caixa de diálogo
      const bx = 40, by = H - 190, bw = W - 80, bh = 160;
      GF.box(ctx, bx, by, bw, bh);
      if (n.speaker) { GF.box(ctx, bx + 24, by - 22, 24 + ctx.measureText(n.speaker).width + n.speaker.length * 11, 38, { fill: P.bg2 }); GF.text(ctx, n.speaker, bx + 40, by - 15, { size: 22, color: P.accent }); }
      ctx.font = '24px "Trebuchet MS", system-ui, sans-serif';
      const lines = GF.wrap(ctx, n.text.slice(0, Math.floor(g.chars)), bw - 60);
      lines.slice(0, 4).forEach((l, i) => GF.text(ctx, l, bx + 30, by + 24 + i * 32, { size: 24, bold: false }));
      const done = g.chars >= n.text.length;
      if (done && !(n.choices && n.choices.length) && Math.floor(g.t * 2) % 2 === 0) GF.text(ctx, '▼', bx + bw - 40, by + bh - 34, { size: 22, color: P.accent });
      // escolhas
      g.rects = [];
      if (done && n.choices && n.choices.length) {
        const cs = n.choices, ch = 54, total = cs.length * (ch + 12), y0 = by - total - 24;
        cs.forEach((c, i) => {
          const r = { x: W * 0.18, y: y0 + i * (ch + 12), w: W * 0.64, h: ch }; g.rects.push(r);
          GF.box(ctx, r.x, r.y, r.w, r.h, { fill: i === g.sel ? P.primary : 'rgba(8,10,16,.85)', stroke: i === g.sel ? P.text : P.accent });
          ctx.font = 'bold 22px "Trebuchet MS", system-ui, sans-serif';
          GF.text(ctx, `${i + 1}. ${c.text}`, r.x + 22, r.y + 14, { size: 22, color: i === g.sel ? P.bg : P.text, shadow: i !== g.sel });
        });
      }
    },
  });
})();
