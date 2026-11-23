// Abas, editor de código, visualizador de arquivos e prévia do jogo.
import { api, qs } from './api.js';
import { S, bus, playUrl } from './state.js';
import { h, $, toast, confirmDialog, debounce, fmtSize } from './ui.js';
import { highlight, langOf } from './highlight.js';

const tabs = [];          // { id, title, kind, path, content, orig, dirty, error, el }
let active = null;
let panePreview, iframe, previewEmpty, paneEditorHost, paneViewer, paneWelcome;
let consoleErrors = 0;

export const hasTab = (id) => tabs.some((t) => t.id === id);
export const activeId = () => active;
export const openPaths = () => tabs.filter((t) => t.path).map((t) => t.path);

export function init() {
  const stage = $('#stage');
  iframe = h('iframe', { allow: 'autoplay; fullscreen; gamepad; clipboard-write', sandbox: 'allow-scripts allow-same-origin allow-pointer-lock allow-popups allow-forms allow-modals', title: 'Prévia do jogo' });
  previewEmpty = h('div', { class: 'welcome', hidden: true }, h('div', { class: 'card' }, h('h1', {}, 'Ainda não há um jogo aqui'), h('p', {}, 'Descreva o jogo na barra à direita (➤ Gerar) e os agentes criam tudo para você — código, arte, sons e mapas.')));
  panePreview = h('div', { class: 'preview', hidden: true },
    h('div', { class: 'preview-bar' },
      h('span', {}, 'Prévia'), h('span', { class: 'spacer' }),
      h('button', { class: 'btn ghost', onclick: () => reloadPreview(), title: 'Recarregar (F5)' }, '⟳ Recarregar'),
      h('button', { class: 'btn ghost', onclick: () => iframe.requestFullscreen && iframe.requestFullscreen(), title: 'Tela cheia' }, '⛶ Tela cheia'),
      h('button', { class: 'btn ghost', onclick: () => toggleConsole(), title: 'Mostrar erros do jogo' }, '⚠ Console')),
    h('div', { style: { position: 'relative', flex: 1, display: 'flex', minHeight: 0 } }, iframe, previewEmpty));
  paneEditorHost = h('div', { style: { display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }, hidden: true });
  paneViewer = h('div', { class: 'viewer', hidden: true });
  paneWelcome = h('div', { class: 'welcome', hidden: true });
  stage.append(panePreview, paneEditorHost, paneViewer, paneWelcome);

  $('#btnClearConsole').onclick = () => { $('#consoleBody').replaceChildren(); consoleErrors = 0; renderTabs(); };
  $('#btnCloseConsole').onclick = () => ($('#console').hidden = true);
  window.addEventListener('message', (e) => {
    if (!S.info || e.origin !== new URL(S.info.play_base).origin) return;
    const d = e.data;
    if (d && d.gameflow === 'error') logConsole(`${d.message}${d.file ? `  (${d.file}${d.line ? ':' + d.line : ''})` : ''}`);
  });
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') { e.preventDefault(); saveActive(); }
    if (e.key === 'F5') { e.preventDefault(); openPreview(true); }
  });
  bus.on('project', () => onProject());
  bus.on('saved', debounce(() => { if (previewLoadedFor() === S.project?.id) reloadPreview(); }, 400));
  renderTabs(); renderStage();
}

const previewLoadedFor = () => iframe.dataset.pid;

function logConsole(text, info = false) {
  const body = $('#consoleBody');
  body.append(h('div', { class: 'line' + (info ? ' info' : '') }, text));
  body.scrollTop = body.scrollHeight;
  if (!info) { consoleErrors++; $('#console').hidden = false; renderTabs(); }
}
export function toggleConsole() { const c = $('#console'); c.hidden = !c.hidden; }

// ───────── projeto ─────────
function onProject() {
  // fecha tudo ao trocar de projeto
  tabs.splice(0).forEach((t) => t.el && t.el.remove());
  active = null; consoleErrors = 0; $('#consoleBody').replaceChildren();
  iframe.removeAttribute('src'); delete iframe.dataset.pid;
  if (S.project) openPreview(true, true);
  else { renderTabs(); renderStage(); }
}

// ───────── abas ─────────
export function renderTabs() {
  const bar = $('#tabs'); bar.replaceChildren();
  for (const t of tabs) {
    bar.append(h('div', { class: 'tab' + (t.id === active ? ' active' : ''), onclick: () => activate(t.id), onauxclick: (e) => { if (e.button === 1) closeTab(t.id); }, title: t.path || 'Jogo' },
      t.id === 'preview' ? '▶ Jogo' : t.title,
      t.id === 'preview' && consoleErrors ? h('span', { class: 'badge' }, consoleErrors) : null,
      t.dirty ? h('span', { class: 'dot', title: 'Alterações não salvas' }) : null,
      t.id !== 'preview' || tabs.length > 1 ? h('span', { class: 'x', onclick: (e) => { e.stopPropagation(); closeTab(t.id); } }, '✕') : null));
  }
}

function activate(id) { active = id; renderTabs(); renderStage(); bus.emit('active-tab', id); }

export async function closeTab(id) {
  const i = tabs.findIndex((t) => t.id === id);
  if (i < 0) return;
  const t = tabs[i];
  if (t.dirty && !(await confirmDialog('Fechar sem salvar?', `“${t.title}” tem alterações não salvas. Elas serão perdidas.`, 'Descartar', true))) return;
  t.el && t.el.remove();
  tabs.splice(i, 1);
  if (active === id) active = (tabs[i] || tabs[i - 1] || {}).id || null;
  renderTabs(); renderStage(); bus.emit('active-tab', active);
}

export function closeByPath(path) {
  const t = tabs.find((x) => x.path === path || (x.path && x.path.startsWith(path + '/')));
  if (t) { t.dirty = false; closeTab(t.id); }
}

function renderStage() {
  const t = tabs.find((x) => x.id === active);
  panePreview.hidden = !(t && t.kind === 'preview');
  paneEditorHost.hidden = !(t && t.kind === 'text');
  paneViewer.hidden = !(t && (t.kind === 'image' || t.kind === 'audio' || t.kind === 'binary'));
  paneWelcome.hidden = !!t;
  for (const x of tabs) if (x.el) x.el.hidden = x.id !== active;
  if (!t) renderWelcome();
  else if (t.kind === 'preview') { updatePreviewEmpty(); setTimeout(() => iframe.focus(), 50); }
  else if (t.kind !== 'text') renderViewer(t);
  else setTimeout(() => t.focus && t.focus(), 30);
}

function renderWelcome() {
  bus.emit('render-welcome', paneWelcome);
}

// ───────── prévia ─────────
function updatePreviewEmpty() {
  const has = S.project && (S.project.generated || S.tree.some((e) => e.path === 'index.html'));
  previewEmpty.hidden = !!has;
  iframe.style.visibility = has ? 'visible' : 'hidden';
}

export function reloadPreview() {
  if (!S.project) return;
  updatePreviewEmpty();
  iframe.dataset.pid = S.project.id;
  const has = S.project.generated || S.tree.some((e) => e.path === 'index.html');
  iframe.src = has ? playUrl('', true) : 'about:blank';
}

export function openPreview(reload = false, silent = false) {
  if (!S.project) return;
  if (!hasTab('preview')) tabs.unshift({ id: 'preview', title: 'Jogo', kind: 'preview' });
  if (reload || previewLoadedFor() !== S.project.id) reloadPreview();
  if (!silent || true) { active = 'preview'; renderTabs(); renderStage(); }
}

// ───────── arquivos ─────────
const imgExt = /\.(png|jpe?g|gif|webp|svg|bmp)$/i, audExt = /\.(wav|mp3|ogg|m4a)$/i;

export async function openFile(path) {
  if (!S.project) return;
  const existing = tabs.find((t) => t.path === path);
  if (existing) return activate(existing.id);
  const title = path.split('/').pop();
  if (imgExt.test(path) || audExt.test(path)) {
    tabs.push({ id: path, path, title, kind: imgExt.test(path) ? 'image' : 'audio' });
    return activate(path);
  }
  try {
    const r = await api(`/projects/${S.project.id}/file?${qs({ path })}`);
    const tab = { id: path, path, title, kind: 'text', content: r.content, orig: r.content, dirty: false, error: '' };
    buildEditor(tab);
    tabs.push(tab);
    activate(path);
  } catch (e) {
    if (e.status === 415) { tabs.push({ id: path, path, title, kind: 'binary' }); activate(path); }
    else toast(e.message, 'bad');
  }
}

function renderViewer(t) {
  const size = S.tree.find((e) => e.path === t.path)?.size;
  const url = playUrl(t.path, true);
  paneViewer.replaceChildren(
    t.kind === 'image' ? h('img', { src: url, alt: t.title }) : t.kind === 'audio' ? h('audio', { controls: true, src: url }) : h('div', { class: 'meta' }, 'Arquivo binário — não pode ser editado aqui.'),
    h('div', { class: 'meta' }, `${t.path}${size != null ? ' · ' + fmtSize(size) : ''}`),
    t.kind !== 'binary' ? h('div', { class: 'meta' }, 'Para trocar este arquivo, envie outro com o mesmo nome (⤒ Enviar) — o jogo passa a usá-lo.') : null);
}

function buildEditor(tab) {
  const lang = langOf(tab.path);
  const ta = h('textarea', { spellcheck: 'false', autocapitalize: 'off', autocomplete: 'off', wrap: 'off', 'aria-label': tab.title });
  ta.value = tab.content;
  const pre = h('pre', { 'aria-hidden': 'true' });
  const gutter = h('div', { class: 'gutter' });
  const err = h('span', { class: 'err' });
  const info = h('span', {});
  let lines = 0;
  const paint = () => {
    const v = ta.value;
    pre.innerHTML = highlight(v, lang) + '\n';
    const n = v.split('\n').length;
    if (n !== lines) { lines = n; gutter.textContent = Array.from({ length: n }, (_, i) => i + 1).join('\n'); }
    info.textContent = `${n} linhas`;
  };
  const sync = () => { pre.scrollTop = ta.scrollTop; pre.scrollLeft = ta.scrollLeft; gutter.scrollTop = ta.scrollTop; };
  let raf = 0;
  ta.addEventListener('input', () => {
    tab.content = ta.value; const d = tab.content !== tab.orig;
    if (d !== tab.dirty) { tab.dirty = d; renderTabs(); }
    cancelAnimationFrame(raf); raf = requestAnimationFrame(() => { paint(); sync(); });
  });
  ta.addEventListener('scroll', sync);
  ta.addEventListener('keydown', (e) => {
    if (e.key === 'Tab' && !e.ctrlKey && !e.altKey) { e.preventDefault(); document.execCommand('insertText', false, '  '); }
    else if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
      const v = ta.value, pos = ta.selectionStart, ls = v.lastIndexOf('\n', pos - 1) + 1;
      const indent = /^[ \t]*/.exec(v.slice(ls, pos))[0], before = v[pos - 1];
      if (ta.selectionStart === ta.selectionEnd) { e.preventDefault(); document.execCommand('insertText', false, '\n' + indent + (before && '{([:'.includes(before) && lang !== 'md' ? '  ' : '')); }
    }
  });
  const save = h('button', { class: 'btn', onclick: () => saveTab(tab) }, '💾 Salvar');
  const bar = h('div', { class: 'editor-bar' }, h('span', {}, tab.path), info, err, h('span', { class: 'spacer' }), h('span', {}, 'Ctrl+S'), save);
  tab.showError = (m) => { err.textContent = m || ''; };
  tab.focus = () => ta.focus();
  tab.setContent = (c) => { tab.content = tab.orig = c; tab.dirty = false; ta.value = c; paint(); renderTabs(); };
  const ed = h('div', { class: 'editor' }, gutter, h('div', { class: 'codewrap' }, pre, ta));
  tab.el = h('div', { style: { display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }, hidden: true }, bar, ed);
  paneEditorHost.append(tab.el);
  paint();
}

export async function saveTab(tab) {
  if (!tab || tab.kind !== 'text' || !S.project) return;
  try {
    await api(`/projects/${S.project.id}/file`, { method: 'PUT', body: { path: tab.path, content: tab.content } });
    tab.orig = tab.content; tab.dirty = false; tab.showError(''); renderTabs();
    toast(`Salvo: ${tab.path}`, 'ok', 1800);
    bus.emit('saved', tab.path);
  } catch (e) {
    tab.showError('⚠ ' + e.message);
    toast(e.message, 'bad', 6000);
  }
}
export const saveActive = () => saveTab(tabs.find((t) => t.id === active));

/** Recarrega do disco as abas abertas (depois que os agentes mexem nos arquivos). */
export async function refreshOpenFiles() {
  for (const t of tabs.filter((x) => x.kind === 'text').slice()) {
    try {
      const r = await api(`/projects/${S.project.id}/file?${qs({ path: t.path })}`);
      if (r.content !== t.orig) {
        if (t.dirty) toast(`“${t.title}” mudou no disco, mas você tem alterações não salvas.`, 'bad', 6000);
        else t.setContent(r.content);
      }
    } catch (e) { if (e.status === 400 || e.status === 404) { t.dirty = false; closeTab(t.id); } }
  }
  if (active && tabs.find((t) => t.id === active)?.kind === 'image') renderViewer(tabs.find((t) => t.id === active));
}
