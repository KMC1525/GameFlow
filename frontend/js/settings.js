// Configurações: aparência (tema/escala), agentes (modelo por tarefa), modelos do Hugging Face e avançado.
import { api, qs } from './api.js';
import { S, bus, taskName } from './state.js';
import { h, $, toast, modal, fmtSize, debounce } from './ui.js';

export const THEMES = [
  { id: 'grafite', name: 'Grafite', desc: 'Cinza escuro e azul suave (padrão)', c: ['#23262b', '#2a2e34', '#6f9ed8'] },
  { id: 'nevoa', name: 'Névoa', desc: 'Cinza médio, menos contraste', c: ['#3a3f47', '#424852', '#82b3ea'] },
  { id: 'ardosia', name: 'Ardósia', desc: 'Bem escuro, ótimo à noite', c: ['#1a1c20', '#202328', '#6a9bd6'] },
  { id: 'crepusculo', name: 'Crepúsculo', desc: 'Cinza-azulado', c: ['#262b36', '#2d3340', '#7aa7de'] },
  { id: 'papel', name: 'Papel', desc: 'Cinza claro, para ambientes iluminados', c: ['#e3e5e9', '#eceef1', '#3a74bd'] },
];

export function applyAppearance() {
  const s = S.settings; if (!s) return;
  const root = document.documentElement;
  root.dataset.theme = THEMES.some((t) => t.id === s.theme) ? s.theme : 'grafite';
  root.style.setProperty('--scale', String(s.ui_scale || 1));
  root.style.setProperty('--editor-size', String(s.editor_font_size || 13));
}

const save = debounce(async () => {
  try { await api('/settings', { method: 'PUT', body: S.settings }); bus.emit('settings'); } catch (e) { toast('Não consegui salvar as configurações: ' + e.message, 'bad'); }
}, 450);
const commit = () => { applyAppearance(); save(); };

const TASKS = [
  { id: 'text', title: 'Texto', sub: 'diretor e roteirista — ideias, história, diálogos', runtimes: ['builtin', 'llama_cpp', 'openai'] },
  { id: 'code', title: 'Código', sub: 'arquiteto, engenheiro, integrador e testador — programa o jogo', runtimes: ['builtin', 'llama_cpp', 'openai'] },
  { id: 'image', title: 'Imagem', sub: 'artista — sprites, fundos, texturas, retratos', runtimes: ['builtin', 'sd_cpp', 'command', 'openai'] },
  { id: 'sound', title: 'Som', sub: 'compositor — efeitos e música', runtimes: ['builtin', 'command'] },
];
const RT_LABEL = {
  builtin: 'Embutido (sem IA — funciona em qualquer computador)',
  llama_cpp: 'llama.cpp — modelo GGUF baixado do Hugging Face',
  openai: 'Servidor local compatível com OpenAI (Ollama, LM Studio…)',
  sd_cpp: 'stable-diffusion.cpp — modelo de imagem local',
  command: 'Comando próprio (Piper, scripts…)',
};
const BUILTIN_LABEL = { text: 'Embutido (modelos prontos de roteiro)', code: 'Embutido (templates de jogo)', image: 'Embutido (arte pixel-art procedural)', sound: 'Embutido (sons e músicas procedurais)' };

let tabId = 'appearance';
let body, pollTimer = null;

export function openSettings(tab) {
  tabId = tab || tabId;
  body = h('div', { class: 's-body' });
  const nav = h('div', { class: 's-nav' });
  const wrap = h('div', { class: 'settings' }, nav, body);
  const draw = () => {
    nav.replaceChildren(...[['appearance', '🎨 Aparência'], ['agents', '🤖 Agentes'], ['models', '📦 Modelos (Hugging Face)'], ['advanced', '🔧 Avançado']].map(([id, label]) => h('button', { class: tabId === id ? 'on' : '', onclick: () => { tabId = id; draw(); } }, label)));
    clearInterval(pollTimer); pollTimer = null;
    ({ appearance, agents, models, advanced })[tabId]();
  };
  const m = modal({ title: 'Configurações', size: 'big', body: wrap, onClose: () => { clearInterval(pollTimer); save(); } });
  m.body.style.padding = '0'; m.body.style.display = 'flex';
  draw();
}

const row = (label, el, help) => [h('label', {}, label), h('div', {}, el, help ? h('div', { class: 'help', style: { margin: '.25rem 0 0' } }, help) : null)];

// ───────── Aparência ─────────
function appearance() {
  const s = S.settings;
  const cards = THEMES.map((t) => h('div', { class: 'theme-card' + (s.theme === t.id ? ' on' : ''), onclick: () => { s.theme = t.id; commit(); appearance(); } },
    h('div', { class: 'sw', style: { background: t.c[0] } }, h('i', { style: { background: t.c[1] } }), h('div', { class: 'r' }, h('b', { style: { background: t.c[2] } }), h('b', { style: { background: t.c[1], opacity: '.7' } }), h('b', { style: { background: t.c[1], opacity: '.45' } }))),
    h('div', { style: { fontWeight: 600 } }, t.name), h('div', { class: 'help', style: { margin: 0 } }, t.desc)));
  const pct = h('b', {}, Math.round(s.ui_scale * 100) + '%');
  const scale = h('input', { type: 'range', min: '70', max: '180', step: '5', value: String(Math.round(s.ui_scale * 100)), oninput: (e) => { s.ui_scale = +e.target.value / 100; pct.textContent = e.target.value + '%'; commit(); } });
  const fpct = h('b', {}, s.editor_font_size + ' px');
  const fsize = h('input', { type: 'range', min: '10', max: '26', step: '1', value: String(s.editor_font_size), oninput: (e) => { s.editor_font_size = +e.target.value; fpct.textContent = e.target.value + ' px'; commit(); } });
  const presets = [['Pequeno', 90], ['Normal', 100], ['Grande', 125], ['Extra grande', 150]].map(([n, v]) => h('button', { class: 'btn', onclick: () => { s.ui_scale = v / 100; commit(); appearance(); } }, n));
  body.replaceChildren(
    h('h3', {}, 'Tema'), h('p', { class: 'help' }, 'Todos os temas usam cinzas suaves e azul dessaturado, pensados para sessões longas sem cansar a vista.'), h('div', { class: 'themes' }, cards),
    h('h3', {}, 'Tamanho da interface'), h('div', { class: 'range-row' }, scale, pct), h('div', { class: 'row-flex', style: { marginTop: '.6rem' } }, presets),
    h('h3', {}, 'Tamanho da letra do editor'), h('div', { class: 'range-row' }, fsize, fpct));
}

// ───────── Agentes ─────────
async function localModels() { try { return await api('/models'); } catch { return []; } }

async function agents() {
  const s = S.settings, locals = await localModels(), tools = (await api('/info')).tools;
  S.info.tools = tools;
  const cards = TASKS.map((t) => {
    const a = s.agents[t.id];
    const result = h('div', { class: 'test-res' });
    const fields = h('div', { class: 'form' });
    const rtSel = h('select', { onchange: (e) => { a.runtime = e.target.value; commit(); drawFields(); } }, t.runtimes.map((r) => h('option', { value: r, selected: a.runtime === r }, r === 'builtin' ? BUILTIN_LABEL[t.id] : RT_LABEL[r])));
    const mine = locals.filter((m) => m.suits.includes(t.id));
    const dl = h('datalist', { id: 'dl-' + t.id }, mine.map((m) => h('option', { value: m.path })));
    const inp = (key, props = {}) => h('input', { type: props.type || 'text', value: a[key] ?? '', placeholder: props.ph || '', list: props.list, oninput: (e) => { a[key] = props.num ? +e.target.value : e.target.value; commit(); } });
    function drawFields() {
      const rows = [...row('Programa de IA', rtSel)];
      if (['llama_cpp', 'sd_cpp'].includes(a.runtime)) {
        rows.push(...row('Arquivo do modelo', [inp('model', { list: 'dl-' + t.id, ph: mine.length ? 'escolha um modelo baixado' : 'baixe um modelo na aba Modelos' }), dl],
          mine.length ? `${mine.length} modelo(s) baixado(s) servem para ${taskName(t.id)}.` : 'Você ainda não baixou nenhum modelo compatível — use a aba “Modelos (Hugging Face)”.'));
        rows.push(...row('Argumentos extras', inp('command', { ph: a.runtime === 'llama_cpp' ? 'ex.: -ngl 99  (usar a GPU)' : 'ex.: --steps 20' })));
        if (a.runtime === 'llama_cpp') rows.push(...row('Contexto (tokens)', inp('context', { type: 'number', num: true })));
      } else if (a.runtime === 'openai') {
        rows.push(...row('Endereço do servidor', inp('endpoint', { ph: 'http://127.0.0.1:11434/v1' }), 'Ollama: http://127.0.0.1:11434/v1 • LM Studio: http://127.0.0.1:1234/v1'));
        rows.push(...row('Nome do modelo', inp('model', { ph: 'ex.: qwen2.5-coder:7b' })));
        rows.push(...row('Chave (opcional)', inp('api_key', { type: 'password' })));
      } else if (a.runtime === 'command') {
        rows.push(...row('Comando', inp('command', { ph: t.id === 'sound' ? 'piper --model {model} --output_file {out}' : 'python gerar.py "{prompt}" {out}' }),
          'Use {prompt}, {out} e {model}. Sem {prompt}, o texto vai pela entrada padrão (como o Piper espera).'));
        rows.push(...row('Arquivo do modelo (opcional)', [inp('model', { list: 'dl-' + t.id }), dl]));
      }
      if (['text', 'code'].includes(t.id) && a.runtime !== 'builtin') rows.push(...row('Criatividade', h('div', { class: 'range-row' }, h('input', { type: 'range', min: '0', max: '1.5', step: '0.05', value: String(a.temperature), oninput: (e) => { a.temperature = +e.target.value; commit(); } }))));
      fields.replaceChildren(...rows);
    }
    drawFields();
    const test = h('button', { class: 'btn', onclick: async (e) => { e.target.disabled = true; result.className = 'test-res'; result.textContent = 'Testando… (a primeira vez pode demorar enquanto o modelo carrega)'; await api('/settings', { method: 'PUT', body: s }); const r = await api('/agents/test', { method: 'POST', body: { task: t.id } }).catch((er) => ({ ok: false, message: er.message })); result.textContent = (r.ok ? '✓ ' : '✕ ') + r.message; result.className = 'test-res ' + (r.ok ? 'ok' : 'bad'); e.target.disabled = false; } }, 'Testar');
    return h('div', { class: 'agent-card' }, h('h4', {}, t.title, h('span', { class: 'tag' }, t.sub)), fields, h('div', {}, test, result));
  });
  const tool = (label, p, hint) => h('div', { class: 'help', style: { margin: '.15rem 0' } }, p ? `✓ ${label}: ${p}` : `✕ ${label}: não encontrado. ${hint}`);
  body.replaceChildren(
    h('p', { class: 'help' }, 'Escolha qual modelo cada agente usa. Tudo roda no seu computador. Sem configurar nada, o GameFlow usa geradores embutidos e já cria jogos completos — modelos de IA deixam textos, código e arte muito mais livres e originais.'),
    ...cards,
    h('h3', {}, 'Programas de IA encontrados'),
    tool('llama-server (llama.cpp)', tools.llama_server, 'Instale o llama.cpp (ex.: brew install llama.cpp) ou informe o caminho na aba Avançado.'),
    tool('stable-diffusion.cpp (sd)', tools.sd, 'Opcional, só para gerar imagens por IA.'),
    tool('Node.js', tools.node, 'Opcional: permite validar a sintaxe do código gerado com mais precisão.'));
}

// ───────── Modelos ─────────
async function models() {
  const s = S.settings;
  const token = h('input', { type: 'password', value: s.hf_token || '', placeholder: 'hf_… (opcional)', oninput: (e) => { s.hf_token = e.target.value; commit(); } });
  const list = h('div', {}), results = h('div', {}), dls = h('div', {}), local = h('div', {});
  const q = h('input', { type: 'search', placeholder: 'Buscar no Hugging Face (ex.: qwen coder gguf)' });
  const taskSel = h('select', {}, ['text|Texto (GGUF)', 'code|Código (GGUF)', 'image|Imagem', 'sound|Som / voz', '|Tudo'].map((x) => h('option', { value: x.split('|')[0] }, x.split('|')[1])));
  const doSearch = async () => {
    if (!q.value.trim()) return;
    results.replaceChildren(h('div', { class: 'help' }, 'Buscando…'));
    try { const r = await api(`/hf/search?${qs({ q: q.value, task: taskSel.value })}`); results.replaceChildren(...(r.length ? r.map((m) => repoRow({ repo: m.id, title: m.id, note: `${(m.downloads || 0).toLocaleString('pt-BR')} downloads · ♥ ${m.likes || 0}${m.gated ? ' · exige aceitar a licença' : ''}`, task: taskSel.value || '' })) : [h('div', { class: 'help' }, 'Nada encontrado.')])); }
    catch (e) { results.replaceChildren(h('div', { class: 'help', style: { color: 'var(--bad)' } }, e.message)); }
  };
  q.addEventListener('keydown', (e) => e.key === 'Enter' && doSearch());

  function repoRow(r) {
    const files = h('div', { style: { marginTop: '.5rem' }, hidden: true });
    const btn = h('button', { class: 'btn', onclick: async () => {
      if (!files.hidden) { files.hidden = true; return; }
      files.hidden = false; files.replaceChildren(h('div', { class: 'help' }, 'Carregando arquivos…'));
      try {
        const fs = await api(`/hf/files?${qs({ repo: r.repo })}`);
        files.replaceChildren(...(fs.length ? fs.map((f) => h('div', { class: 'model-row', style: { marginBottom: '.25rem' } }, h('div', { class: 'info' }, h('div', { class: 't' }, f.path, /q4_k_m/i.test(f.path) ? h('span', { class: 'tag', style: { marginLeft: '.4rem' } }, 'recomendado') : null), h('div', { class: 'd' }, fmtSize(f.size))), h('button', { class: 'btn primary', onclick: () => download(r.repo, f.path) }, '⬇ Baixar'))) : [h('div', { class: 'help' }, 'Nenhum arquivo de modelo encontrado neste repositório.')]));
      } catch (e) { files.replaceChildren(h('div', { class: 'help', style: { color: 'var(--bad)' } }, e.message)); }
    } }, 'Ver arquivos');
    return h('div', { class: 'model-row', style: { flexWrap: 'wrap' } }, h('div', { class: 'info' }, h('div', { class: 't' }, r.title), h('div', { class: 'd' }, r.task ? h('span', { class: 'tag' }, taskName(r.task)) : null, r.note)), btn, h('div', { style: { flexBasis: '100%' } }, files));
  }
  async function download(repo, file) {
    try { await api('/hf/download', { method: 'POST', body: { repo, file } }); toast('Download iniciado.', 'ok'); pollDownloads(); } catch (e) { toast(e.message, 'bad'); }
  }
  async function pollDownloads() {
    let ds = []; try { ds = await api('/downloads'); } catch { return; }
    dls.replaceChildren(...(ds.length ? [h('h3', {}, 'Downloads'), ...ds.map((d) => h('div', { class: 'model-row' }, h('div', { class: 'info' }, h('div', { class: 't' }, d.repo + ' / ' + d.file), h('div', { class: 'd' }, d.status === 'error' ? '✕ ' + d.error : d.status === 'done' ? '✓ concluído' : d.status === 'cancelled' ? 'cancelado' : `${fmtSize(d.done)} / ${d.total ? fmtSize(d.total) : '?'}`), d.status === 'downloading' ? h('div', { class: 'progress' }, h('i', { style: { width: d.total ? (100 * d.done / d.total).toFixed(1) + '%' : '5%' } })) : null), d.status === 'downloading' || d.status === 'queued' ? h('button', { class: 'btn', onclick: () => api('/downloads/cancel', { method: 'POST', body: { id: d.id } }) }, 'Cancelar') : null)), h('button', { class: 'btn ghost', onclick: async () => { await api('/downloads', { method: 'DELETE' }); pollDownloads(); } }, 'Limpar lista')] : []));
    if (ds.some((d) => d.status === 'done') && !pollDownloads.refreshed?.has(ds.filter((d) => d.status === 'done').map((d) => d.id).join())) { (pollDownloads.refreshed ||= new Set()).add(ds.filter((d) => d.status === 'done').map((d) => d.id).join()); drawLocal(); }
  }
  async function drawLocal() {
    const ms = await localModels();
    const use = (m, task) => {
      const a = s.agents[task];
      a.model = m.path;
      if (a.runtime === 'builtin') a.runtime = task === 'image' ? 'sd_cpp' : task === 'sound' ? 'command' : 'llama_cpp';
      commit(); toast(`“${m.path.split('/').pop()}” agora é o modelo de ${taskName(task)}. Veja os detalhes na aba Agentes.`, 'ok', 5000);
    };
    local.replaceChildren(h('h3', {}, `Modelos baixados (${ms.length})`), ...(ms.length ? ms.map((m) => h('div', { class: 'model-row' }, h('div', { class: 'info' }, h('div', { class: 't', title: m.path }, m.path), h('div', { class: 'd' }, fmtSize(m.size), ' · serve para: ', m.suits.map(taskName).join(', '))),
      ...m.suits.slice(0, 2).map((t) => h('button', { class: 'btn', onclick: () => use(m, t) }, 'Usar p/ ' + taskName(t))),
      h('button', { class: 'icon-btn', title: 'Excluir', onclick: async () => { if (confirm(`Excluir ${m.path}?`)) { await api(`/models?${qs({ path: m.path })}`, { method: 'DELETE' }); drawLocal(); } } }, '🗑'))) : [h('div', { class: 'help' }, 'Nenhum modelo baixado ainda. Escolha um dos recomendados abaixo ou busque no Hugging Face.')]));
  }
  body.replaceChildren(
    h('p', { class: 'help' }, 'Baixe modelos do Hugging Face para a pasta local do GameFlow. Eles rodam 100% no seu computador. Modelos GGUF são usados pelo llama.cpp (texto/código).'),
    dls, local,
    h('h3', {}, 'Recomendados'), list,
    h('h3', {}, 'Buscar no Hugging Face'), h('div', { class: 'row-flex' }, q, taskSel, h('button', { class: 'btn primary', onclick: doSearch }, 'Buscar')), results,
    h('h3', {}, 'Token do Hugging Face'), h('p', { class: 'help' }, 'Só é necessário para modelos que exigem aceitar uma licença. Crie em huggingface.co/settings/tokens (permissão de leitura).'), h('div', { class: 'row-flex' }, token));
  await drawLocal();
  try { const rec = await api('/hf/recommended'); list.replaceChildren(...rec.map((r) => repoRow({ repo: r.repo, title: r.title, note: r.note, task: r.task }))); } catch { list.replaceChildren(h('div', { class: 'help' }, 'Não consegui carregar as recomendações.')); }
  pollDownloads(); pollTimer = setInterval(pollDownloads, 1200);
}

// ───────── Avançado ─────────
function advanced() {
  const s = S.settings;
  const num = (key, min, max) => h('input', { type: 'number', min, max, value: s[key], oninput: (e) => { s[key] = +e.target.value; commit(); } });
  const txt = (key, ph) => h('input', { type: 'text', value: s[key] || '', placeholder: ph, oninput: (e) => { s[key] = e.target.value; commit(); } });
  body.replaceChildren(
    h('h3', {}, 'Agentes'),
    h('div', { class: 'form' }, ...row('Rodadas de correção', num('max_review_rounds', 0, 5), 'Quantas vezes o Testador pode devolver o jogo ao Engenheiro para consertar problemas.'), ...row('Tempo limite por pedido (s)', num('request_timeout_secs', 10, 7200), 'Modelos grandes em CPU podem demorar; aumente se necessário.')),
    h('h3', {}, 'Programas de IA (caminhos manuais)'),
    h('div', { class: 'form' }, ...row('llama-server', txt('llama_server_path', 'vazio = procurar no PATH')), ...row('stable-diffusion.cpp', txt('sd_cli_path', 'vazio = procurar sd-cli / sd')), ...row('Node.js', txt('node_path', 'vazio = procurar node'))),
    h('h3', {}, 'Pastas'),
    h('div', { class: 'help' }, 'Projetos: ', h('span', { class: 'kbd' }, S.info.paths.projects)), h('div', { class: 'help' }, 'Modelos e configurações: ', h('span', { class: 'kbd' }, S.info.paths.root)),
    h('h3', {}, 'Sobre'),
    h('div', { class: 'help' }, `GameFlow ${S.info.version} · ${S.info.os}/${S.info.arch}`),
    h('div', { class: 'help' }, 'Para atualizar, feche o GameFlow e rode no terminal: ', h('span', { class: 'kbd' }, 'gameflow update')));
}
