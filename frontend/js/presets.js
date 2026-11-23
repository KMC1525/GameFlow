// Style presets: library UI (import / export / apply / save project style) and the prompt-bar selector.
import { api } from './api.js';
import { S, bus } from './state.js';
import { h, toast, promptDialog, confirmDialog } from './ui.js';

export let presets = [];

export async function refreshPresets() {
  try { presets = await api('/presets'); } catch { presets = []; }
  bus.emit('presets-changed', presets);
  return presets;
}

/** Fills the prompt-bar <select>. `value` selects an id afterwards. */
export function fillPresetSelect(sel, value) {
  const keep = value ?? sel.value;
  sel.replaceChildren(h('option', { value: '' }, 'Estilo: automático'), ...presets.map((p) => h('option', { value: p.id }, `🎨 ${p.name}`)));
  sel.value = presets.some((p) => p.id === keep) ? keep : '';
}

/** Imports the text of a .gfstyle.json file. `select` also picks it for the next generation. */
export async function importPresetText(text, select = false) {
  try {
    const p = await api('/presets', { method: 'POST', raw: text });
    await refreshPresets();
    toast(`Estilo “${p.name}” importado.`, 'ok');
    if (select) bus.emit('select-preset', p.id);
    return p;
  } catch (e) { toast('Não consegui importar o estilo: ' + e.message, 'bad', 7000); return null; }
}

const download = (id) => { const a = h('a', { href: `/api/presets/${encodeURIComponent(id)}/export` }); document.body.append(a); a.click(); a.remove(); };

function swatches(p) {
  return h('div', { style: { display: 'flex', borderRadius: '.4rem', overflow: 'hidden', border: '1px solid var(--border)', height: '1.5rem' } },
    ...['bg', 'bg2', 'ground', 'primary', 'secondary', 'accent', 'danger', 'text'].map((k) => h('i', { title: `${k}: ${p.palette[k]}`, style: { flex: 1, background: p.palette[k] } })));
}

/** Renders the "Estilos" tab of the settings dialog into `body`. `closeSettings` closes the dialog. */
export async function renderTab(body, closeSettings, applyToProject) {
  await refreshPresets();
  const fileInput = h('input', { type: 'file', accept: '.json,application/json', multiple: true, hidden: true, onchange: async (e) => { for (const f of e.target.files) await importPresetText(await f.text()); e.target.value = ''; draw(); } });
  const drop = h('div', { class: 'drop-zone', ondragover: (e) => { e.preventDefault(); drop.classList.add('over'); }, ondragleave: () => drop.classList.remove('over'),
    ondrop: async (e) => { e.preventDefault(); drop.classList.remove('over'); for (const f of e.dataTransfer.files) await importPresetText(await f.text()); draw(); } },
    '⤒ Solte aqui um arquivo ', h('span', { class: 'kbd' }, '.gfstyle.json'), ' ou ', h('button', { class: 'btn', onclick: () => fileInput.click() }, 'Importar estilo…'));
  const saveBtn = h('button', { class: 'btn', disabled: !S.project?.generated, title: S.project?.generated ? '' : 'Gere um jogo neste projeto primeiro', onclick: async () => {
    const name = await promptDialog('Salvar estilo do projeto', 'Dê um nome para o visual deste jogo (paleta + estilo de arte). Ele vai para a sua biblioteca e pode ser exportado.', `${S.project.name} (estilo)`, 'Salvar');
    if (!name) return;
    try { const p = await api(`/projects/${S.project.id}/preset-save`, { method: 'POST', body: { name, description: `Visual do projeto “${S.project.name}”.` } }); await refreshPresets(); toast(`Estilo “${p.name}” salvo na biblioteca.`, 'ok'); draw(); } catch (e) { toast(e.message, 'bad'); }
  } }, '＋ Salvar o estilo do projeto atual');
  const list = h('div', { class: 'preset-grid' });

  function card(p) {
    return h('div', { class: 'preset-card' },
      h('div', { class: 'preset-head' }, h('strong', {}, p.name), p.builtin ? h('span', { class: 'tag' }, 'embutido') : h('span', { class: 'tag' }, 'importado'), p.author && !p.builtin ? h('span', { class: 'tag' }, 'por ' + p.author) : null),
      swatches(p),
      h('div', { class: 'help', style: { margin: '.4rem 0 .2rem' } }, p.description || '—'),
      h('div', { class: 'help', style: { margin: 0 } }, h('b', {}, 'Arte: '), p.art_style),
      h('div', { class: 'preset-actions' },
        h('button', { class: 'btn', title: 'Usa este estilo na próxima geração (seletor da barra de prompt)', onclick: () => { bus.emit('select-preset', p.id); toast(`Estilo “${p.name}” escolhido para a próxima geração.`, 'ok'); closeSettings(); } }, 'Usar na próxima geração'),
        h('button', { class: 'btn primary', disabled: !S.project, onclick: () => { closeSettings(); applyToProject(p); } }, 'Aplicar ao projeto'),
        h('button', { class: 'btn', onclick: () => download(p.id) }, '⬇ Exportar'),
        p.builtin ? null : h('button', { class: 'btn danger', onclick: async () => { if (await confirmDialog('Excluir estilo', `Remover “${p.name}” da biblioteca? Projetos que já usam este estilo não mudam.`, 'Excluir', true)) { try { await api(`/presets/${encodeURIComponent(p.id)}`, { method: 'DELETE' }); await refreshPresets(); draw(); } catch (e) { toast(e.message, 'bad'); } } } }, 'Excluir')));
  }
  function draw() { list.replaceChildren(...presets.map(card)); }
  body.replaceChildren(
    h('p', { class: 'help' }, 'Um estilo é um “visual” reutilizável: paleta de 8 cores, descrição do estilo de arte e dicas para a IA de imagem. Exporte para um arquivo ', h('span', { class: 'kbd' }, '.gfstyle.json'), ' para usar em outros projetos, outros computadores ou compartilhar com outras pessoas.'),
    h('div', { class: 'row-flex' }, saveBtn), drop, fileInput, h('h3', {}, `Biblioteca (${presets.length})`), list,
    h('p', { class: 'help' }, 'Dica: se o estilo descreve “pixel art”, o jogo desenha com pixels nítidos; qualquer outra descrição usa suavização. A arte gerada pelo GameFlow sem modelo de imagem sempre usa a paleta do estilo.'));
  draw();
}
