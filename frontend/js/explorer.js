// Barra lateral esquerda: projetos e árvore de arquivos.
import { api, qs } from './api.js';
import { S, bus, GENRE_LABEL } from './state.js';
import { h, $, toast, modal, promptDialog, confirmDialog, contextMenu, fmtSize } from './ui.js';
import { openFile, closeByPath, activeId } from './editor.js';

const expanded = new Set(['js', 'assets']);
let uploadTarget = '';

export function init() {
  $('#btnNewProject').onclick = newProjectDialog;
  $('#btnNewFile').onclick = () => createEntry('', 'file');
  $('#btnNewFolder').onclick = () => createEntry('', 'dir');
  $('#btnUpload').onclick = () => pickUpload('');
  $('#btnRefreshTree').onclick = () => loadTree();
  $('#fileInput').onchange = async (e) => { await upload([...e.target.files], uploadTarget); e.target.value = ''; };
  const tree = $('#tree');
  tree.addEventListener('contextmenu', (e) => { if (e.target === tree) { e.preventDefault(); menuFor(e, null); } });
  tree.addEventListener('dragover', (e) => { if (e.dataTransfer.types.includes('Files')) { e.preventDefault(); } });
  tree.addEventListener('drop', async (e) => { if (!e.dataTransfer.files.length) return; e.preventDefault(); clearDrop(); await upload([...e.dataTransfer.files], dropDir(e.target.closest('.node'))); });
  bus.on('active-tab', renderTree);
  bus.on('files-changed', loadTree);
}

// ───────── projetos ─────────
export async function loadProjects(selectId) {
  S.projects = await api('/projects');
  renderProjects();
  const want = selectId || localStorage.getItem('gf:last') || (S.projects[0] && S.projects[0].id);
  if (want && S.projects.some((p) => p.id === want)) await selectProject(want);
  else if (!S.projects.length) { S.project = null; S.tree = []; renderTree(); bus.emit('project', null); }
  else await selectProject(S.projects[0].id);
}

export function renderProjects() {
  const box = $('#projectList');
  if (!S.projects.length) { box.replaceChildren(h('div', { class: 'empty-hint' }, 'Nenhum projeto ainda. Clique em ＋ para criar o seu primeiro jogo.')); return; }
  box.replaceChildren(...S.projects.map((p) => h('div', { class: 'proj' + (S.project?.id === p.id ? ' active' : ''), onclick: () => selectProject(p.id), oncontextmenu: (e) => { e.preventDefault(); projectMenu(e.clientX, e.clientY, p); }, title: p.prompt || p.name },
    h('span', { class: 'nm' }, p.name), p.genre ? h('span', { class: 'gn' }, GENRE_LABEL(p.genre).replace(/ \(.*\)/, '')) : null,
    h('button', { class: 'icon-btn more', onclick: (e) => { e.stopPropagation(); projectMenu(e.clientX, e.clientY, p); } }, '⋯'))));
}

export async function selectProject(id) {
  if (S.running) { toast('Aguarde a geração terminar (ou clique em Parar) antes de trocar de projeto.', 'bad'); return; }
  try {
    S.project = await api(`/projects/${id}`);
  } catch (e) { toast(e.message, 'bad'); return; }
  localStorage.setItem('gf:last', id);
  await loadTree();
  renderProjects();
  bus.emit('project', S.project);
}

export async function refreshProject() {
  if (!S.project) return;
  try { S.project = await api(`/projects/${S.project.id}`); S.projects = await api('/projects'); } catch { /* ignora */ }
  renderProjects(); bus.emit('project-meta', S.project);
}

function projectMenu(x, y, p) {
  contextMenu(x, y, [
    { label: 'Renomear…', onClick: async () => { const n = await promptDialog('Renomear projeto', '', p.name, 'Renomear'); if (n) { try { await api(`/projects/${p.id}`, { method: 'PATCH', body: { name: n } }); await loadProjects(S.project?.id); } catch (e) { toast(e.message, 'bad'); } } } },
    { label: 'Abrir pasta no computador', onClick: () => api(`/projects/${p.id}/open-folder`, { method: 'POST' }).then((r) => !r.ok && toast('Pasta: ' + r.path)).catch((e) => toast(e.message, 'bad')) },
    '-',
    { label: 'Excluir projeto…', kind: 'bad', onClick: async () => { if (await confirmDialog('Excluir projeto', `Excluir “${p.name}” e todos os arquivos dele? Isso não pode ser desfeito.`, 'Excluir', true)) { try { await api(`/projects/${p.id}`, { method: 'DELETE' }); if (S.project?.id === p.id) { S.project = null; localStorage.removeItem('gf:last'); } await loadProjects(); toast('Projeto excluído.'); } catch (e) { toast(e.message, 'bad'); } } } },
  ]);
}

export function newProjectDialog(preset = {}) {
  return new Promise((resolve) => {
    const name = h('input', { type: 'text', placeholder: 'Ex.: Aventura na Floresta', value: preset.name || '' });
    const genre = h('select', {}, h('option', { value: '' }, 'Automático (a IA escolhe pelo seu texto)'), ...S.info.genres.map((g) => h('option', { value: g.id }, g.label)));
    if (preset.genre) genre.value = preset.genre;
    const go = async () => {
      try {
        const p = await api('/projects', { method: 'POST', body: { name: name.value.trim() || 'Novo jogo', genre: genre.value || null } });
        m.close();
        await loadProjects(p.id);
        if (genre.value) bus.emit('set-genre', genre.value);
        resolve(p);
      } catch (e) { toast(e.message, 'bad'); }
    };
    const m = modal({ title: 'Novo projeto', body: [h('p', {}, 'Cada jogo fica salvo em um projeto, com todo o código e os assets.'), h('div', { class: 'form' }, h('label', {}, 'Nome'), name, h('label', {}, 'Gênero'), genre)], buttons: [{ label: 'Cancelar', onClick: () => resolve(null) }, { label: 'Criar projeto', kind: 'primary', close: false, onClick: go }], onClose: () => resolve(null) });
    name.addEventListener('keydown', (e) => e.key === 'Enter' && go());
    setTimeout(() => name.focus(), 30);
  });
}

// ───────── árvore ─────────
export async function loadTree() {
  if (!S.project) { S.tree = []; renderTree(); return; }
  try { S.tree = await api(`/projects/${S.project.id}/tree`); } catch { S.tree = []; }
  renderTree();
}

function buildNested() {
  const root = { children: {}, path: '' };
  for (const e of S.tree) {
    const parts = e.path.split('/'); let cur = root;
    parts.forEach((part, i) => {
      cur.children[part] ||= { name: part, children: {}, path: parts.slice(0, i + 1).join('/'), kind: 'dir', size: 0 };
      cur = cur.children[part];
      if (i === parts.length - 1) { cur.kind = e.kind; cur.size = e.size; }
    });
  }
  return root;
}

const iconFor = (n) => {
  if (n.kind === 'dir') return ['dir', expanded.has(n.path) ? '▾' : '▸'];
  const ext = n.name.split('.').pop().toLowerCase();
  if (['js', 'mjs'].includes(ext)) return ['js', 'JS'];
  if (ext === 'css') return ['css', '#'];
  if (['html', 'htm'].includes(ext)) return ['html', '<>'];
  if (ext === 'json') return ['json', '{}'];
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(ext)) return ['img', '▣'];
  if (['wav', 'mp3', 'ogg', 'm4a'].includes(ext)) return ['aud', '♪'];
  if (ext === 'md') return ['md', 'M'];
  return ['md', '·'];
};

export function renderTree() {
  const box = $('#tree');
  if (!S.project) { box.replaceChildren(h('div', { class: 'empty-hint' }, 'Abra ou crie um projeto para ver os arquivos.')); return; }
  const rows = [];
  const act = activeId();
  const walk = (node, depth) => {
    const kids = Object.values(node.children).sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === 'dir' ? -1 : 1));
    for (const n of kids) {
      const [cls, glyph] = iconFor(n);
      rows.push(h('div', {
        class: 'node' + (act === n.path ? ' active' : ''), style: { paddingLeft: `${0.4 + depth * 0.95}rem` }, 'data-path': n.path, 'data-kind': n.kind, title: n.path,
        onclick: () => { if (n.kind === 'dir') { expanded.has(n.path) ? expanded.delete(n.path) : expanded.add(n.path); renderTree(); } else openFile(n.path); },
        oncontextmenu: (e) => { e.preventDefault(); menuFor(e, n); },
        ondragover: (e) => { if (e.dataTransfer.types.includes('Files')) { e.preventDefault(); clearDrop(); (n.kind === 'dir' ? e.currentTarget : e.currentTarget.parentNode.querySelector('.node')).classList.add('drop'); } },
        ondragleave: clearDrop,
      }, h('span', { class: 'ic ' + cls }, glyph), h('span', {}, n.name), n.kind === 'file' ? h('span', { class: 'sz' }, fmtSize(n.size)) : null));
      if (n.kind === 'dir' && expanded.has(n.path)) walk(n, depth + 1);
    }
  };
  walk(buildNested(), 0);
  box.replaceChildren(...(rows.length ? rows : [h('div', { class: 'empty-hint' }, 'Projeto vazio. Descreva o jogo no painel da direita ou envie arquivos com ⤒.')]));
}

const clearDrop = () => $('#tree').querySelectorAll('.drop').forEach((n) => n.classList.remove('drop'));
function dropDir(nodeEl) {
  if (!nodeEl) return '';
  const p = nodeEl.dataset.path;
  return nodeEl.dataset.kind === 'dir' ? p : p.split('/').slice(0, -1).join('/');
}

function menuFor(e, n) {
  const dir = !n ? '' : n.kind === 'dir' ? n.path : n.path.split('/').slice(0, -1).join('/');
  const items = [
    { label: 'Novo arquivo…', onClick: () => createEntry(dir, 'file') },
    { label: 'Nova pasta…', onClick: () => createEntry(dir, 'dir') },
    { label: 'Enviar arquivos aqui…', onClick: () => pickUpload(dir) },
  ];
  if (n) {
    items.unshift(...(n.kind === 'file' ? [{ label: 'Abrir', onClick: () => openFile(n.path) }, '-'] : []));
    items.push('-', { label: 'Renomear / mover…', onClick: () => renameEntry(n) }, { label: 'Excluir…', kind: 'bad', onClick: () => deleteEntry(n) });
  }
  contextMenu(e.clientX, e.clientY, items);
}

async function createEntry(dir, kind) {
  if (!S.project) return toast('Abra um projeto primeiro.', 'bad');
  const name = await promptDialog(kind === 'dir' ? 'Nova pasta' : 'Novo arquivo', dir ? `Dentro de “${dir}”` : 'Na raiz do projeto', kind === 'dir' ? 'nova-pasta' : 'novo.js', 'Criar');
  if (!name) return;
  const path = (dir ? dir + '/' : '') + name.replace(/^\/+/, '');
  try {
    await api(`/projects/${S.project.id}/file`, { method: 'POST', body: { path, kind } });
    if (dir) expanded.add(dir);
    await loadTree();
    if (kind === 'file') openFile(path);
  } catch (e) { toast(e.message, 'bad'); }
}

async function renameEntry(n) {
  const to = await promptDialog('Renomear / mover', 'Novo caminho dentro do projeto:', n.path, 'Renomear');
  if (!to || to === n.path) return;
  try {
    closeByPath(n.path);
    await api(`/projects/${S.project.id}/move`, { method: 'POST', body: { from: n.path, to } });
    await loadTree(); bus.emit('saved', to);
  } catch (e) { toast(e.message, 'bad'); }
}

async function deleteEntry(n) {
  if (!(await confirmDialog('Excluir', `Excluir “${n.path}”${n.kind === 'dir' ? ' e tudo dentro dela' : ''}?`, 'Excluir', true))) return;
  try {
    closeByPath(n.path);
    await api(`/projects/${S.project.id}/file?${qs({ path: n.path })}`, { method: 'DELETE' });
    await loadTree(); bus.emit('saved', n.path);
  } catch (e) { toast(e.message, 'bad'); }
}

function pickUpload(dir) { if (!S.project) return toast('Abra um projeto primeiro.', 'bad'); uploadTarget = dir; $('#fileInput').click(); }

/** Envia arquivos. Sem pasta escolhida: imagens → assets/images, sons → assets/audio, o resto → raiz. */
export async function upload(files, dir) {
  if (!S.project || !files.length) return [];
  const saved = [];
  const groups = {};
  for (const f of files) {
    const d = dir || (/^image\//.test(f.type) || /\.(png|jpe?g|gif|webp|svg)$/i.test(f.name) ? 'assets/images' : /^audio\//.test(f.type) || /\.(wav|mp3|ogg|m4a)$/i.test(f.name) ? 'assets/audio' : '');
    (groups[d] ||= []).push(f);
  }
  try {
    for (const [d, fs] of Object.entries(groups)) {
      const fd = new FormData(); fs.forEach((f) => fd.append('file', f, f.name));
      const r = await api(`/projects/${S.project.id}/upload?${qs({ dir: d || '.' })}`, { method: 'POST', form: fd });
      saved.push(...r.saved);
    }
    toast(`${saved.length} arquivo(s) enviado(s).`, 'ok');
    await loadTree(); bus.emit('saved', saved[0]);
  } catch (e) { toast(e.message, 'bad'); }
  return saved;
}
