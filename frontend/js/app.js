// GameFlow — ponto de entrada da interface.
import { api } from './api.js';
import { S, bus, playUrl, GENRE_LABEL, agentSummary } from './state.js';
import { h, $, toast } from './ui.js';
import { applyAppearance, openSettings } from './settings.js';
import * as editor from './editor.js';
import * as explorer from './explorer.js';
import * as agents from './agents.js';

async function boot() {
  try {
    [S.info, S.settings] = await Promise.all([api('/info'), api('/settings')]);
  } catch (e) {
    document.body.replaceChildren(h('div', { style: { padding: '3rem', fontFamily: 'system-ui', color: '#ccc' } }, h('h2', {}, 'GameFlow não respondeu'), h('p', {}, e.message)));
    return;
  }
  applyAppearance();
  restoreLayout();
  editor.init(); explorer.init(); agents.init();
  wireTitlebar(); wireResizers();
  bus.on('render-welcome', renderWelcome);
  bus.on('project', updateTitle); bus.on('project-meta', updateTitle);
  bus.on('settings', updateStatus);
  updateStatus(); updateTitle();
  await explorer.loadProjects();
  updateTitle();
  document.addEventListener('keydown', (e) => { if ((e.ctrlKey || e.metaKey) && e.key === ',') { e.preventDefault(); openSettings(); } });
  window.addEventListener('beforeunload', (e) => { if (S.running) { e.preventDefault(); e.returnValue = ''; } });
}

// ───────── barra de título ─────────
function wireTitlebar() {
  $('#btnPlay').onclick = () => editor.openPreview(true);
  $('#btnReload').onclick = () => editor.openPreview(true);
  $('#btnPopout').onclick = () => S.project && window.open(playUrl(''), '_blank', 'noopener');
  $('#btnExport').onclick = () => { if (S.project) { const a = h('a', { href: `/api/projects/${S.project.id}/export`, download: `${S.project.id}.zip` }); document.body.append(a); a.click(); a.remove(); toast('Baixando o jogo como .zip — é só abrir o index.html em qualquer navegador.', 'ok', 5000); } };
  $('#btnFolder').onclick = async () => { if (!S.project) return; try { const r = await api(`/projects/${S.project.id}/open-folder`, { method: 'POST' }); if (!r.ok) toast('Pasta do projeto: ' + r.path, '', 8000); } catch (e) { toast(e.message, 'bad'); } };
  $('#btnSettings').onclick = () => openSettings();
}

function updateTitle() {
  const has = !!S.project;
  for (const id of ['btnPlay', 'btnReload', 'btnPopout', 'btnExport', 'btnFolder']) $('#' + id).disabled = !has;
  $('#projectTitle').textContent = has ? `${S.project.name}${S.project.genre ? ' — ' + GENRE_LABEL(S.project.genre) : ''}` : 'Nenhum projeto aberto';
  document.title = has ? `${S.project.name} — GameFlow` : 'GameFlow';
}

function updateStatus() {
  const parts = ['text', 'code', 'image', 'sound'].map((t) => `${{ text: 'Texto', code: 'Código', image: 'Imagem', sound: 'Som' }[t]}: ${agentSummary(t)}`);
  const left = $('#statusLeft');
  left.textContent = parts.join('  •  ');
  left.style.cursor = 'pointer'; left.title = 'Clique para escolher os modelos de cada agente';
  left.onclick = () => openSettings('agents');
  $('#statusRight').textContent = `GameFlow ${S.info.version}  •  ${location.host}`;
}

// ───────── boas-vindas ─────────
function renderWelcome(el) {
  const steps = h('div', { class: 'steps' },
    ...[['Crie um projeto', 'Cada jogo fica salvo num projeto, com todo o código e os assets.'], ['Descreva o jogo na barra à direita', 'Gênero, clima, personagens, regras. Você pode anexar 📎 desenhos e sons.'], ['Jogue, converse e ajuste', 'Peça mudanças em linguagem natural ou edite os arquivos à esquerda.']].map(([t, d], i) => h('div', { class: 'step' }, h('b', {}, i + 1), h('div', {}, h('strong', {}, t), h('div', { style: { color: 'var(--muted)' } }, d)))));
  if (!S.project) {
    const ex = agents.EXAMPLES.slice(0, 4).map((t) => h('span', { class: 'chip', onclick: () => quickStart(t) }, t));
    el.replaceChildren(h('div', { class: 'card' }, h('h1', {}, 'Bem-vindo ao GameFlow'), h('p', {}, 'Crie jogos completos — código, arte, sons e mapas — só descrevendo a ideia. Tudo roda no seu computador, com modelos locais do Hugging Face.'), steps,
      h('div', { class: 'row' }, h('button', { class: 'btn primary', onclick: () => explorer.newProjectDialog() }, '＋ Criar meu primeiro jogo'), h('button', { class: 'btn', onclick: () => openSettings('agents') }, '🤖 Escolher modelos de IA')),
      h('p', { style: { marginTop: '1.2rem', marginBottom: '.2rem' } }, 'Ou comece por uma ideia:'), h('div', { class: 'example-chips' }, ex)));
  } else {
    el.replaceChildren(h('div', { class: 'card' }, h('h1', {}, S.project.name), h('p', {}, S.project.generated ? 'Abra um arquivo na árvore à esquerda para editar, ou jogue o seu jogo.' : 'Este projeto ainda está vazio. Descreva o jogo na barra à direita e clique em Gerar.'), steps,
      h('div', { class: 'row' }, h('button', { class: 'btn primary', onclick: () => editor.openPreview(true) }, '▶ Jogar'), h('button', { class: 'btn', onclick: () => $('#prompt').focus() }, '✎ Descrever o jogo'))));
  }
}

async function quickStart(example) {
  try {
    const name = example.split(' ').slice(0, 4).join(' ');
    const p = await api('/projects', { method: 'POST', body: { name, genre: null } });
    await explorer.loadProjects(p.id);
    $('#prompt').value = example; $('#prompt').dispatchEvent(new Event('input')); $('#prompt').focus();
  } catch (e) { toast(e.message, 'bad'); }
}

// ───────── layout ─────────
function restoreLayout() {
  const r = document.documentElement.style;
  const l = localStorage.getItem('gf:left'), rr = localStorage.getItem('gf:right');
  if (l) r.setProperty('--left-w', l); if (rr) r.setProperty('--right-w', rr);
}

function wireResizers() {
  const drag = (el, varName, key, fromRight) => {
    el.addEventListener('mousedown', (e) => {
      e.preventDefault(); el.classList.add('drag'); document.body.classList.add('dragging');
      const root = document.documentElement, rem = parseFloat(getComputedStyle(root).fontSize);
      const move = (ev) => {
        const px = fromRight ? innerWidth - ev.clientX : ev.clientX;
        const v = Math.max(10, Math.min(46, px / rem)) + 'rem';
        root.style.setProperty(varName, v); localStorage.setItem(key, v);
      };
      const up = () => { el.classList.remove('drag'); document.body.classList.remove('dragging'); removeEventListener('mousemove', move); removeEventListener('mouseup', up); document.body.style.userSelect = ''; };
      document.body.style.userSelect = 'none';
      addEventListener('mousemove', move); addEventListener('mouseup', up);
    });
  };
  drag($('#resizeLeft'), '--left-w', 'gf:left', false);
  drag($('#resizeRight'), '--right-w', 'gf:right', true);
}

boot();
