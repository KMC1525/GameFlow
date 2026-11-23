// Estado global compartilhado e barramento de eventos simples.

export const S = {
  info: null,
  settings: null,
  projects: [],
  project: null, // metadados do projeto aberto
  tree: [],
  running: null, // { runId, stop }
};

const listeners = {};
export const bus = {
  on(ev, fn) { (listeners[ev] ||= []).push(fn); },
  emit(ev, data) { for (const fn of listeners[ev] || []) { try { fn(data); } catch (e) { console.error(e); } } },
};

/** URL (origem isolada) de um arquivo do projeto aberto. */
export function playUrl(path = '', bust = false) {
  if (!S.info || !S.project) return 'about:blank';
  const p = path.split('/').map(encodeURIComponent).join('/');
  return `${S.info.play_base}/${S.project.id}/${p}${bust ? (p.includes('?') ? '&' : '?') + 't=' + Date.now() : ''}`;
}

export const GENRE_LABEL = (id) => (S.info?.genres.find((g) => g.id === id)?.label) || id || '';

const MODEL_NAMES = { text: 'texto', code: 'código', image: 'imagem', sound: 'som' };
export const taskName = (t) => MODEL_NAMES[t] || t;

/** Resumo curto do runtime de cada agente (para a barra de status e dicas). */
export function agentSummary(task) {
  const a = S.settings?.agents?.[task];
  if (!a || a.runtime === 'builtin') return 'embutido';
  const m = (a.model || '').split('/').pop().replace(/\.(gguf|safetensors|onnx)$/i, '');
  return m || a.runtime;
}
