// Utilitários de interface: criação de elementos, toasts, modais, menu de contexto.

/** h('div', {class:'x', onclick: fn}, 'texto', outroNo) */
export function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k === 'html') el.innerHTML = v;
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'value') el.value = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c.nodeType ? c : document.createTextNode(String(c)));
  }
  return el;
}

export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

export function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

export function fmtSize(n) {
  if (n < 1024) return n + ' B';
  if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
  if (n < 1073741824) return (n / 1048576).toFixed(1) + ' MB';
  return (n / 1073741824).toFixed(2) + ' GB';
}

export function toast(msg, kind = '', ms = 3500) {
  const t = h('div', { class: 'toast ' + kind }, msg);
  $('#toastRoot').append(t);
  setTimeout(() => t.remove(), ms);
}

/** Abre um modal. Retorna { close, el, body }. */
export function modal({ title, body, size = 'small', buttons = [], onClose } = {}) {
  const root = $('#modalRoot');
  const close = () => { bg.remove(); document.removeEventListener('keydown', onKey, true); onClose && onClose(); };
  const bodyEl = h('div', { class: 'modal-b' }, body);
  const foot = buttons.length ? h('div', { class: 'modal-f' }, buttons.map((b) => h('button', { class: 'btn ' + (b.kind || ''), onclick: async () => { const r = b.onClick ? await b.onClick(api) : undefined; if (r !== false && b.close !== false) close(); } }, b.label))) : null;
  const m = h('div', { class: 'modal ' + size, role: 'dialog' }, h('div', { class: 'modal-h' }, h('span', {}, title || ''), h('span', { class: 'spacer' }), h('button', { class: 'icon-btn', onclick: close, title: 'Fechar' }, '✕')), bodyEl, foot);
  const bg = h('div', { class: 'modal-bg', onmousedown: (e) => { if (e.target === bg) close(); } }, m);
  const onKey = (e) => { if (e.key === 'Escape') { e.stopPropagation(); close(); } };
  document.addEventListener('keydown', onKey, true);
  root.append(bg);
  const api = { close, el: m, body: bodyEl };
  return api;
}

export function promptDialog(title, label, value = '', okLabel = 'OK') {
  return new Promise((resolve) => {
    const input = h('input', { type: 'text', value, spellcheck: 'false' });
    let done = false;
    const finish = (v) => { if (!done) { done = true; resolve(v); } };
    const m = modal({
      title, body: [label ? h('p', {}, label) : null, input], buttons: [
        { label: 'Cancelar', onClick: () => finish(null) },
        { label: okLabel, kind: 'primary', onClick: () => finish(input.value.trim() || null) },
      ], onClose: () => finish(null),
    });
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { finish(input.value.trim() || null); m.close(); } });
    setTimeout(() => { input.focus(); input.select(); }, 30);
  });
}

export function confirmDialog(title, text, okLabel = 'Confirmar', danger = false) {
  return new Promise((resolve) => {
    let done = false;
    const finish = (v) => { if (!done) { done = true; resolve(v); } };
    modal({ title, body: h('p', {}, text), buttons: [{ label: 'Cancelar', onClick: () => finish(false) }, { label: okLabel, kind: danger ? 'danger' : 'primary', onClick: () => finish(true) }], onClose: () => finish(false) });
  });
}

/** Menu de contexto. items: [{label, onClick, kind}] ou '-' como separador. */
export function contextMenu(x, y, items) {
  const el = $('#ctxMenu');
  el.replaceChildren(...items.map((it) => (it === '-' ? h('hr') : h('button', { class: it.kind || '', onclick: () => { hide(); it.onClick(); } }, it.label))));
  el.hidden = false;
  const r = el.getBoundingClientRect();
  el.style.left = Math.min(x, innerWidth - r.width - 6) + 'px';
  el.style.top = Math.min(y, innerHeight - r.height - 6) + 'px';
  function hide() { el.hidden = true; document.removeEventListener('mousedown', away, true); document.removeEventListener('keydown', esc2, true); }
  const away = (e) => { if (!el.contains(e.target)) hide(); };
  const esc2 = (e) => { if (e.key === 'Escape') hide(); };
  setTimeout(() => { document.addEventListener('mousedown', away, true); document.addEventListener('keydown', esc2, true); });
}

export function debounce(fn, ms) {
  let t;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
