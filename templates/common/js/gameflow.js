/* GameFlow mini-engine — carrega dados/assets, entrada, áudio, laço principal, telas de início/fim.
 * Todos os gêneros usam este arquivo. Pode editar, mas prefira js/custom.js para seus ajustes. */
(() => {
  'use strict';
  const GF = (window.GF = {
    cfg: null, manifest: { images: {}, audio: {} }, palette: {}, ui: {},
    W: 960, H: 540, canvas: null, ctx: null, game: null,
    state: 'loading', time: 0, muted: false, paused: false, smooth: false,
  });
  const handlers = {};
  const imgs = {}, auds = {};
  let def = null, last = 0, musicEl = null, musicId = null, endInfo = null, toastTimer = 0;

  // ───────── eventos (ganchos para custom.js) ─────────
  GF.on = (name, fn) => { (handlers[name] ||= []).push({ fn, errors: 0 }); };
  GF.emit = (name, ...args) => {
    const list = handlers[name];
    if (!list) return;
    for (const h of list.slice()) {
      try { h.fn(...args); } catch (e) {
        h.errors++;
        console.error('[GameFlow] erro em um ajuste (' + name + '):', e);
        if (h.errors >= 3) { list.splice(list.indexOf(h), 1); GF.toast('Um ajuste de js/custom.js foi desativado por causa de erros.', 4); }
      }
    }
  };
  GF.event = (name, data) => GF.emit('event', name, data);

  // ───────── utilidades ─────────
  GF.clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  GF.lerp = (a, b, t) => a + (b - a) * t;
  GF.dist = (ax, ay, bx, by) => Math.hypot(ax - bx, ay - by);
  let seed = 1;
  GF.seed = (s) => { seed = (s >>> 0) || 1; };
  GF.rand = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  GF.irand = (a, b) => a + Math.floor(GF.rand() * (b - a + 1));
  GF.img = (id) => imgs[id] || null;
  GF.t = (key, fallback) => (GF.ui && GF.ui[key]) || fallback || key;
  GF.save = (k, v) => { try { localStorage.setItem('gf:' + (GF.cfg?.title || '') + ':' + k, JSON.stringify(v)); } catch (e) { /* sem armazenamento */ } };
  GF.load = (k, d) => { try { const v = localStorage.getItem('gf:' + (GF.cfg?.title || '') + ':' + k); return v == null ? d : JSON.parse(v); } catch (e) { return d; } };
  GF.toast = (msg, secs = 2.2) => {
    const el = document.getElementById('toast');
    if (!el) return;
    el.textContent = msg; el.hidden = false; el.style.opacity = 1;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { el.style.opacity = 0; setTimeout(() => (el.hidden = true), 300); }, secs * 1000);
  };

  // ───────── desenho ─────────
  GF.text = (ctx, str, x, y, o = {}) => {
    ctx.save();
    ctx.font = `${o.bold === false ? '' : 'bold '}${o.size || 20}px "Trebuchet MS", "Segoe UI", system-ui, sans-serif`;
    ctx.textAlign = o.align || 'left'; ctx.textBaseline = o.base || 'top';
    if (o.shadow !== false) { ctx.fillStyle = 'rgba(0,0,0,.65)'; ctx.fillText(str, x + 2, y + 2); }
    ctx.fillStyle = o.color || GF.palette.text || '#fff';
    ctx.fillText(str, x, y);
    ctx.restore();
  };
  GF.wrap = (ctx, str, maxW) => {
    const out = []; let line = '';
    for (const word of String(str).split(/\s+/)) {
      const test = line ? line + ' ' + word : word;
      if (ctx.measureText(test).width > maxW && line) { out.push(line); line = word; } else line = test;
    }
    if (line) out.push(line);
    return out;
  };
  GF.box = (ctx, x, y, w, h, o = {}) => {
    ctx.save();
    ctx.fillStyle = o.fill || 'rgba(8,10,16,.82)';
    ctx.strokeStyle = o.stroke || GF.palette.accent || '#fff';
    ctx.lineWidth = 3;
    const r = 10;
    ctx.beginPath(); ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r); ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
    ctx.fill(); ctx.stroke(); ctx.restore();
  };
  /** Desenha uma imagem cobrindo a área (como CSS background-size: cover). */
  GF.cover = (ctx, img, x, y, w, h) => {
    if (!img) return false;
    const s = Math.max(w / img.width, h / img.height), dw = img.width * s, dh = img.height * s;
    ctx.drawImage(img, x + (w - dw) / 2, y + (h - dh) / 2, dw, dh);
    return true;
  };
  GF.bar = (ctx, x, y, w, h, frac, color) => {
    ctx.fillStyle = 'rgba(0,0,0,.6)'; ctx.fillRect(x - 2, y - 2, w + 4, h + 4);
    ctx.fillStyle = color; ctx.fillRect(x, y, Math.max(0, w * GF.clamp(frac, 0, 1)), h);
  };
  GF.hud = (ctx, items) => {
    let x = 14;
    for (const [label, value] of items) {
      const s = `${label}: ${value}`;
      ctx.save(); ctx.font = 'bold 20px "Trebuchet MS", system-ui, sans-serif'; const w = ctx.measureText(s).width + 20; ctx.restore();
      ctx.fillStyle = 'rgba(8,10,16,.6)'; ctx.fillRect(x - 6, 10, w, 32);
      GF.text(ctx, s, x + 4, 15, { size: 20 });
      x += w + 8;
    }
  };

  // ───────── entrada ─────────
  const keys = new Set(), pressed = new Set();
  const mouse = { x: GF.W / 2, y: GF.H / 2, down: false, clicked: false, dx: 0, dy: 0, locked: false };
  GF.input = {
    mouse,
    down: (c) => keys.has(c),
    pressed: (c) => pressed.has(c),
    any: (...cs) => cs.some((c) => keys.has(c)),
    axis: (neg, pos) => (keys.has(pos) ? 1 : 0) - (keys.has(neg) ? 1 : 0),
  };
  const PREVENT = new Set(['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Tab']);
  window.addEventListener('keydown', (e) => {
    if (PREVENT.has(e.code)) e.preventDefault();
    if (!keys.has(e.code)) pressed.add(e.code);
    keys.add(e.code);
  });
  window.addEventListener('keyup', (e) => keys.delete(e.code));
  window.addEventListener('blur', () => keys.clear());
  function setMouse(e) {
    const r = GF.canvas.getBoundingClientRect();
    mouse.x = ((e.clientX - r.left) / r.width) * GF.W;
    mouse.y = ((e.clientY - r.top) / r.height) * GF.H;
  }
  function bindPointer() {
    const c = GF.canvas;
    c.addEventListener('mousemove', (e) => { setMouse(e); mouse.dx += e.movementX || 0; mouse.dy += e.movementY || 0; });
    c.addEventListener('mousedown', (e) => { setMouse(e); mouse.down = true; mouse.clicked = true; c.focus(); });
    window.addEventListener('mouseup', () => (mouse.down = false));
    c.addEventListener('touchstart', (e) => { e.preventDefault(); const t = e.touches[0]; setMouse(t); mouse.down = true; mouse.clicked = true; }, { passive: false });
    c.addEventListener('touchmove', (e) => { e.preventDefault(); setMouse(e.touches[0]); }, { passive: false });
    c.addEventListener('touchend', () => (mouse.down = false));
    document.addEventListener('pointerlockchange', () => (mouse.locked = document.pointerLockElement === c));
    c.addEventListener('contextmenu', (e) => e.preventDefault());
  }
  GF.lockPointer = () => { try { GF.canvas.requestPointerLock && GF.canvas.requestPointerLock(); } catch (e) { /* ignorado */ } };

  // ───────── áudio ─────────
  GF.play = (id, o = {}) => {
    const a = auds[id];
    if (!a || GF.muted) return;
    try {
      const n = a.cloneNode(); n.volume = GF.clamp(o.vol ?? 0.5, 0, 1);
      n.play().catch(() => {});
    } catch (e) { /* áudio indisponível */ }
  };
  GF.music = (id, vol = 0.3) => {
    if (musicEl) { musicEl.pause(); musicEl = null; }
    musicId = id;
    const a = auds[id];
    if (!a) return;
    musicEl = a.cloneNode(); musicEl.loop = true; musicEl.volume = vol;
    if (!GF.muted) musicEl.play().catch(() => {});
  };
  GF.mute = (m) => {
    GF.muted = m;
    if (musicEl) { if (m) musicEl.pause(); else musicEl.play().catch(() => {}); }
    GF.toast(m ? GF.t('muted', 'Som desligado') : '♪', 1.2);
  };

  // ───────── carregamento ─────────
  async function loadJson(globalName, url, fallback) {
    if (window[globalName]) return window[globalName];
    try { const r = await fetch(url, { cache: 'no-store' }); if (r.ok) return await r.json(); } catch (e) { /* tenta o fallback */ }
    return fallback;
  }
  function loadImage(src) {
    return new Promise((res) => { const im = new Image(); im.onload = () => res(im); im.onerror = () => res(null); im.src = src; });
  }
  function loadAudio(src) {
    return new Promise((res) => {
      const a = new Audio(); a.preload = 'auto';
      let done = false; const fin = (v) => { if (!done) { done = true; res(v); } };
      a.addEventListener('canplaythrough', () => fin(a), { once: true });
      a.addEventListener('error', () => fin(null), { once: true });
      a.src = src; a.load(); setTimeout(() => fin(a), 500);
    });
  }
  async function loadAll() {
    GF.cfg = await loadJson('GAME_DATA', 'game.json', {});
    GF.manifest = await loadJson('GAME_MANIFEST', 'assets/manifest.json', { images: {}, audio: {} });
    GF.palette = Object.assign({ bg: '#1d2433', bg2: '#2c3a52', ground: '#4a6b3f', primary: '#6fa8dc', secondary: '#c27ba0', accent: '#ffd966', danger: '#e06666', text: '#f3f3f3' }, GF.cfg.palette || {});
    GF.ui = GF.cfg.ui || {};
    GF.smooth = !/pixel/i.test(GF.cfg.art_style || 'pixel');
    GF.seed(GF.cfg.seed || 1);
    document.title = GF.cfg.title || document.title;
    draw_loading(0);
    const ie = Object.entries(GF.manifest.images || {}), ae = Object.entries(GF.manifest.audio || {});
    let n = 0; const total = ie.length + ae.length || 1;
    await Promise.all([
      ...ie.map(async ([id, p]) => { const im = await loadImage(p); if (im) imgs[id] = im; draw_loading(++n / total); }),
      ...ae.map(async ([id, a]) => { const el = await loadAudio(typeof a === 'string' ? a : a.path); if (el) auds[id] = el; draw_loading(++n / total); }),
    ]);
  }
  function draw_loading(f) {
    const c = GF.ctx; if (!c) return;
    c.fillStyle = GF.palette.bg || '#111'; c.fillRect(0, 0, GF.W, GF.H);
    GF.text(c, GF.t('loading', 'Carregando...'), GF.W / 2, GF.H / 2 - 30, { align: 'center', size: 26 });
    c.fillStyle = 'rgba(255,255,255,.2)'; c.fillRect(GF.W / 2 - 150, GF.H / 2 + 14, 300, 10);
    c.fillStyle = GF.palette.accent || '#fc6'; c.fillRect(GF.W / 2 - 150, GF.H / 2 + 14, 300 * f, 10);
  }

  // ───────── telas ─────────
  function drawTitle(ctx) {
    const bg = GF.img('bg') || GF.img('bg_a');
    if (!GF.cover(ctx, bg, 0, 0, GF.W, GF.H)) {
      const g = ctx.createLinearGradient(0, 0, 0, GF.H); g.addColorStop(0, GF.palette.bg); g.addColorStop(1, GF.palette.bg2);
      ctx.fillStyle = g; ctx.fillRect(0, 0, GF.W, GF.H);
    }
    ctx.fillStyle = 'rgba(0,0,0,.5)'; ctx.fillRect(0, 0, GF.W, GF.H);
    GF.text(ctx, GF.cfg.title || 'GameFlow', GF.W / 2, GF.H * 0.26, { align: 'center', size: 56, color: GF.palette.accent });
    if (GF.cfg.tagline) GF.text(ctx, GF.cfg.tagline, GF.W / 2, GF.H * 0.26 + 76, { align: 'center', size: 20, bold: false });
    const hero = GF.img('player') || GF.img('char_player');
    if (hero && !GF.img('char_player')) { ctx.imageSmoothingEnabled = GF.smooth; ctx.drawImage(hero, GF.W / 2 - 40, GF.H * 0.5, 80, 80); }
    if (Math.floor(performance.now() / 600) % 2 === 0) GF.text(ctx, GF.t('press_start'), GF.W / 2, GF.H * 0.76, { align: 'center', size: 24 });
    if (def && def.controls) GF.text(ctx, GF.t(def.controls), GF.W / 2, GF.H * 0.88, { align: 'center', size: 15, bold: false });
  }
  function drawEnd(ctx) {
    ctx.fillStyle = 'rgba(0,0,0,.62)'; ctx.fillRect(0, 0, GF.W, GF.H);
    const win = endInfo.win;
    GF.text(ctx, win ? GF.t('you_win') : GF.t('game_over'), GF.W / 2, GF.H * 0.3, { align: 'center', size: 54, color: win ? GF.palette.accent : GF.palette.danger });
    const q = GF.cfg.quest || {};
    const msg = endInfo.text || (win ? q.win_text : q.lose_text);
    if (msg) GF.text(ctx, msg, GF.W / 2, GF.H * 0.3 + 80, { align: 'center', size: 20, bold: false });
    if (endInfo.score != null) GF.text(ctx, `${GF.t('score')}: ${endInfo.score}`, GF.W / 2, GF.H * 0.3 + 120, { align: 'center', size: 24 });
    GF.text(ctx, GF.t('restart'), GF.W / 2, GF.H * 0.72, { align: 'center', size: 22 });
  }

  GF.finish = (win, extra = {}) => {
    if (GF.state !== 'playing') return;
    GF.state = 'end'; endInfo = Object.assign({ win }, extra);
    GF.play(win ? 'win' : 'lose', { vol: 0.6 });
    GF.event(win ? 'win' : 'lose', endInfo);
  };
  GF.restart = () => {
    if (document.pointerLockElement) document.exitPointerLock();
    GF.state = 'playing'; GF.paused = false; endInfo = null; GF.time = 0;
    def.init(); GF.emit('start');
    if (!musicEl || musicId !== 'music') GF.music('music');
  };

  // ───────── laço principal ─────────
  function frame(now) {
    requestAnimationFrame(frame);
    const dt = Math.min(0.05, (now - last) / 1000 || 0.016); last = now;
    const ctx = GF.ctx;
    ctx.imageSmoothingEnabled = GF.smooth;
    try {
      if (pressed.has('KeyM')) GF.mute(!GF.muted);
      if (pressed.has('KeyF')) { document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen?.().catch(() => {}); }
      if (GF.state === 'title') {
        drawTitle(ctx);
        if (pressed.has('Enter') || pressed.has('Space') || mouse.clicked) { GF.state = 'playing'; GF.restart(); }
      } else if (GF.state === 'playing') {
        if (pressed.has('KeyP') || pressed.has('Escape')) GF.paused = !GF.paused;
        if (!GF.paused) { GF.time += dt; def.update(dt); GF.emit('update', dt); }
        def.draw(ctx); GF.emit('draw', ctx);
        if (GF.paused) { ctx.fillStyle = 'rgba(0,0,0,.55)'; ctx.fillRect(0, 0, GF.W, GF.H); GF.text(ctx, GF.t('paused'), GF.W / 2, GF.H / 2 - 20, { align: 'center', size: 44 }); }
      } else if (GF.state === 'end') {
        def.draw(ctx); GF.emit('draw', ctx); drawEnd(ctx);
        if (pressed.has('KeyR') || pressed.has('Enter')) { GF.state = 'playing'; GF.restart(); }
      }
    } catch (e) {
      console.error('[GameFlow] erro no jogo:', e);
      reportError(e.message, 'js/game.js', 0);
      GF.state = 'crashed';
      ctx.fillStyle = '#200'; ctx.fillRect(0, 0, GF.W, GF.H);
      GF.text(ctx, 'Ops! Algo deu errado no jogo.', GF.W / 2, GF.H / 2 - 40, { align: 'center', size: 30, color: '#f88' });
      GF.text(ctx, String(e.message).slice(0, 90), GF.W / 2, GF.H / 2 + 10, { align: 'center', size: 16, bold: false });
    }
    pressed.clear(); mouse.clicked = false; mouse.dx = 0; mouse.dy = 0;
  }

  /** Inicia o jogo. def = { controls, setup(), init(), update(dt), draw(ctx) } */
  GF.start = async (d) => {
    def = d;
    GF.canvas = document.getElementById('game'); GF.ctx = GF.canvas.getContext('2d');
    GF.W = GF.canvas.width; GF.H = GF.canvas.height;
    bindPointer();
    await loadAll();
    if (def.setup) def.setup();
    GF.state = 'title';
    requestAnimationFrame((t) => { last = t; frame(t); });
  };

  // ───────── erros -> console da IDE ─────────
  function reportError(message, file, line) {
    try { parent !== window && parent.postMessage({ gameflow: 'error', message: String(message), file: String(file || ''), line: line || 0 }, '*'); } catch (e) { /* sem pai */ }
  }
  window.addEventListener('error', (e) => reportError(e.message, (e.filename || '').split('/').slice(-2).join('/'), e.lineno));
  window.addEventListener('unhandledrejection', (e) => reportError(e.reason && e.reason.message || e.reason, 'promise', 0));
})();
