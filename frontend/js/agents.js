// Painel da direita: barra de prompt, anexos, fluxo de agentes em tempo real e histórico.
import { api, watchRun } from './api.js';
import { S, bus, GENRE_LABEL, playUrl } from './state.js';
import { h, $, toast } from './ui.js';
import { Pipeline } from './pipeline.js';
import { upload as uploadToProject, loadTree } from './explorer.js';
import { openPreview, refreshOpenFiles, reloadPreview } from './editor.js';
import { importPresetText, refreshPresets, fillPresetSelect } from './presets.js';
import { refreshProject } from './explorer.js';

let attachments = []; // { path, name, mode: 'reference'|'use', thumb }
let modeSel = 'auto';  // auto | create | modify
let chat, promptEl, sendBtn, stopBtn, genreSel, presetSel;

export const EXAMPLES = [
  'Um RPG 2D de fantasia numa floresta mágica, com um dragão como chefe final',
  'Plataforma 2D no espaço com pulo duplo e muitas moedas',
  'Visual novel de mistério cyberpunk com 3 finais diferentes',
  'FPS em primeira pessoa num castelo assombrado',
  'Jogo de nave arcade com chefe e escudo',
  'Plataforma 3D num vulcão cheio de lava',
  'Aventura de texto num castelo assombrado, com um enigma da chave',
];

export function init() {
  chat = $('#chat'); promptEl = $('#prompt'); sendBtn = $('#btnSend'); stopBtn = $('#btnStop'); genreSel = $('#genreSelect'); presetSel = $('#presetSelect');
  genreSel.replaceChildren(h('option', { value: '' }, 'Gênero: automático'), ...S.info.genres.map((g) => h('option', { value: g.id }, g.label)));
  fillPresetSelect(presetSel);
  bus.on('presets-changed', () => fillPresetSelect(presetSel));
  bus.on('select-preset', (id) => { fillPresetSelect(presetSel, id); });
  refreshPresets();
  sendBtn.onclick = send;
  stopBtn.onclick = stop;
  promptEl.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } });
  promptEl.addEventListener('input', updateButtons);
  $('#btnAttach').onclick = () => $('#attachInput').click();
  $('#attachInput').onchange = async (e) => { await addFiles([...e.target.files]); e.target.value = ''; };
  const comp = $('#composer');
  comp.addEventListener('dragover', (e) => { if (e.dataTransfer.types.includes('Files')) { e.preventDefault(); comp.classList.add('drag'); } });
  comp.addEventListener('dragleave', () => comp.classList.remove('drag'));
  comp.addEventListener('drop', async (e) => { if (!e.dataTransfer.files.length) return; e.preventDefault(); comp.classList.remove('drag'); await addFiles([...e.dataTransfer.files]); });
  promptEl.addEventListener('paste', async (e) => { const fs = [...(e.clipboardData?.files || [])]; if (fs.length) { e.preventDefault(); await addFiles(fs); } });
  bus.on('project', onProject);
  bus.on('set-genre', (g) => { genreSel.value = g; });
  bus.on('project-meta', renderMode);
}

function updateButtons() {
  sendBtn.disabled = !S.project || !!S.running || !promptEl.value.trim();
  promptEl.disabled = !S.project;
  $('#btnAttach').disabled = !S.project;
  stopBtn.hidden = !S.running;
  sendBtn.hidden = !!S.running;
}

async function onProject() {
  attachments = []; renderChips(); chat.replaceChildren();
  genreSel.value = S.project?.genre || '';
  modeSel = 'auto';
  renderMode();
  updateButtons();
  if (!S.project) { chat.append(h('div', { class: 'empty-hint' }, 'Abra ou crie um projeto para conversar com os agentes.')); return; }
  const id = S.project.id;
  let hist = [];
  try { hist = await api(`/projects/${id}/chat`); } catch { /* sem histórico */ }
  if (S.project?.id !== id) return;
  if (!hist.length) renderIntro();
  for (const m of hist) addMessage(m.role === 'user' ? 'user' : 'bot', m.text, { attachments: m.attachments, ok: m.ok });
  chat.scrollTop = chat.scrollHeight;
  // retoma uma geração em andamento
  try { const r = await api(`/projects/${id}/active-run`); if (r.run_id && !S.running) follow(r.run_id, S.project.id, true); } catch { /* ignora */ }
}

function renderIntro() {
  const chips = EXAMPLES.map((t) => h('span', { class: 'chip', onclick: () => { promptEl.value = t; promptEl.focus(); updateButtons(); } }, t));
  chat.append(h('div', { class: 'msg bot' },
    h('div', {}, 'Olá! Conte o jogo que você quer ver pronto: gênero, clima, personagens, regras… Pode anexar 📎 desenhos e sons de referência (ou para usar no jogo).'),
    h('div', { class: 'example-chips' }, chips)));
}

/** Controle "Criar do zero | Editar o jogo atual" quando o projeto já foi gerado. */
function renderMode() {
  const box = $('#modeSwitch');
  if (!S.project?.generated) { box.hidden = true; return; }
  box.hidden = false;
  const cur = modeSel === 'auto' ? 'modify' : modeSel;
  box.replaceChildren(
    h('button', { class: cur === 'modify' ? 'on' : '', title: 'Aplica o que você pedir ao jogo atual', onclick: () => { modeSel = 'modify'; renderMode(); } }, 'Editar'),
    h('button', { class: cur === 'create' ? 'on' : '', title: 'Recria o jogo do zero (substitui os arquivos de base)', onclick: () => { modeSel = 'create'; renderMode(); } }, 'Recriar'));
  promptEl.placeholder = cur === 'modify' ? 'O que mudar no jogo? Ex.: deixe mais difícil, troque o tema para neon, adicione um chefe…' : 'Descreva o novo jogo do zero…';
}

// ───────── anexos ─────────
async function addFiles(files) {
  const presetFiles = files.filter((f) => /\.json$/i.test(f.name));
  for (const f of presetFiles) await importPresetText(await f.text(), true);
  files = files.filter((f) => !presetFiles.includes(f));
  if (!files.length) return;
  if (!S.project) return toast('Abra um projeto primeiro.', 'bad');
  const ok = files.filter((f) => /^(image|audio)\//.test(f.type) || /\.(png|jpe?g|gif|webp|svg|wav|mp3|ogg|m4a)$/i.test(f.name));
  if (ok.length < files.length) toast('Só imagens e sons podem ser anexados.', 'bad');
  if (!ok.length) return;
  const saved = await uploadToProject(ok, 'references');
  saved.forEach((p, i) => {
    const f = ok[i], isImg = /^image\//.test(f?.type || '') || /\.(png|jpe?g|gif|webp|svg)$/i.test(p);
    attachments.push({ path: p, name: p.split('/').pop(), mode: 'reference', thumb: isImg ? playUrl(p, true) : null });
  });
  renderChips();
}

function renderChips() {
  $('#attachChips').replaceChildren(...attachments.map((a, i) => h('span', { class: 'achip', title: a.path },
    a.thumb ? h('img', { src: a.thumb, alt: '' }) : h('span', {}, '♪'),
    h('span', { class: 'nm' }, a.name),
    h('span', { class: 'mode' + (a.mode === 'use' ? ' use' : ''), title: a.mode === 'use' ? 'Será usado no jogo como está (clique para virar só referência)' : 'Só inspiração para a IA (clique para usar no jogo)', onclick: () => { a.mode = a.mode === 'use' ? 'reference' : 'use'; renderChips(); } }, a.mode === 'use' ? 'usar no jogo' : 'referência'),
    h('span', { class: 'x', onclick: () => { attachments.splice(i, 1); renderChips(); } }, '✕'))));
}

// ───────── mensagens e execução ─────────
function addMessage(kind, text, extra = {}) {
  const el = h('div', { class: `msg ${kind}${extra.ok === false ? ' bad' : ''}` }, text, extra.attachments?.length ? h('span', { class: 'att' }, '📎 ' + extra.attachments.map((p) => p.split('/').pop()).join(', ')) : null);
  chat.append(el); chat.scrollTop = chat.scrollHeight;
  return el;
}

async function send() {
  const prompt = promptEl.value.trim();
  if (!prompt || !S.project || S.running) return;
  const mode = S.project.generated ? (modeSel === 'create' ? 'create' : 'modify') : 'create';
  const body = { prompt, genre: genreSel.value || 'auto', mode, preset: presetSel.value || null, attachments: attachments.map((a) => ({ path: a.path, mode: a.mode })) };
  if (await startRun(body, prompt, attachments.map((a) => a.path))) {
    promptEl.value = ''; attachments = []; renderChips();
    presetSel.value = ''; // the style is now saved in the project; do not re-apply it on every edit
  }
}

async function startRun(body, shownText, attachedPaths = []) {
  let r;
  try { r = await api(`/projects/${S.project.id}/generate`, { method: 'POST', body }); } catch (e) { toast(e.message, 'bad'); return false; }
  if (chat.querySelector('.example-chips')) chat.replaceChildren();
  addMessage('user', shownText, { attachments: attachedPaths });
  follow(r.run_id, S.project.id, false);
  return true;
}

/** Applies a library style to the open project (edit mode: restyles palette, shell and generated art). */
export async function applyStyleToProject(preset) {
  if (!S.project) return toast('Abra um projeto primeiro.', 'bad');
  if (S.running) return toast('Aguarde a geração atual terminar.', 'bad');
  if (!S.project.generated) { bus.emit('select-preset', preset.id); return toast(`Estilo “${preset.name}” escolhido para a próxima geração. Descreva o jogo e clique em Gerar.`, 'ok', 5000); }
  return startRun({ prompt: `Aplicar estilo visual: ${preset.name}`, genre: 'auto', mode: 'modify', preset: preset.id, attachments: [] }, `🎨 Aplicar estilo: ${preset.name}`);
}

function follow(runId, projectId, resumed) {
  const pipe = new Pipeline();
  const logs = h('div', { class: 'logs' });
  const head = h('div', { class: 'run-head' }, h('span', { class: 'spin' }), h('span', { class: 'title' }, resumed ? 'Retomando geração…' : 'Agentes trabalhando…'), h('span', { class: 'spacer' }), h('span', { class: 'timer' }, '0s'));
  const card = h('div', { class: 'run-card' }, head, pipe.el, logs);
  chat.append(card); chat.scrollTop = chat.scrollHeight;
  const t0 = Date.now();
  const timer = setInterval(() => { head.querySelector('.timer').textContent = Math.round((Date.now() - t0) / 1000) + 's'; }, 1000);
  const line = (cls, html) => { logs.append(h('div', { class: 'l ' + cls, html })); logs.scrollTop = logs.scrollHeight; chat.scrollTop = chat.scrollHeight; };
  const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  const files = [];
  const stopWatch = watchRun(runId, (ev) => {
    switch (ev.type) {
      case 'node_added': pipe.upsert(ev); break;
      case 'node_status': pipe.setStatus(ev.id, ev.status, ev.detail); break;
      case 'log': line(ev.level, `<b>${esc(pipe.label(ev.node))}</b> ${esc(ev.text)}`); break;
      case 'message': line('msg', `💬 <b>${esc(pipe.label(ev.from))}</b> → ${ev.to === 'all' ? 'todos' : esc(pipe.label(ev.to))}: ${esc(ev.text)}`); break;
      case 'file': files.push(ev.path); if (!ev.path.startsWith('.gameflow')) line('file', `${ev.action === 'created' ? '＋' : '✎'} ${esc(ev.path)}`); if (files.length % 6 === 0) loadTree(); break;
      default: break;
    }
  }, async (done) => {
    clearInterval(timer);
    head.classList.add(done.ok ? 'done' : 'fail');
    head.querySelector('.title').textContent = done.ok ? 'Pronto!' : 'Não concluído';
    addMessage('bot', done.summary, { ok: done.ok });
    S.running = null; updateButtons();
    if (S.project?.id === projectId) {
      await refreshProject(); await loadTree(); await refreshOpenFiles();
      if (done.ok) { openPreview(true); toast('Jogo atualizado! Clique na tela do jogo e jogue.', 'ok', 5000); }
      else reloadPreview();
    }
  });
  S.running = { runId, stop: stopWatch, projectId };
  updateButtons();
}

async function stop() {
  if (!S.running) return;
  try { await api(`/runs/${S.running.runId}/cancel`, { method: 'POST' }); toast('Cancelando…'); } catch (e) { toast(e.message, 'bad'); }
}
