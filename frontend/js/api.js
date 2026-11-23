// Cliente da API local do GameFlow.

export async function api(path, { method = 'GET', body, form } = {}) {
  const opt = { method, headers: {} };
  if (body !== undefined) { opt.headers['Content-Type'] = 'application/json'; opt.body = JSON.stringify(body); }
  if (form) opt.body = form;
  let r;
  try { r = await fetch('/api' + path, opt); } catch (e) { throw new Error('Não consegui falar com o GameFlow (o programa ainda está rodando no terminal?)'); }
  let data = null;
  try { data = await r.json(); } catch { /* resposta sem JSON */ }
  if (!r.ok) { const e = new Error((data && data.error) || r.statusText); e.status = r.status; throw e; }
  return data;
}

export const qs = (o) => Object.entries(o).filter(([, v]) => v != null && v !== '').map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`).join('&');

/** Assina os eventos de uma execução (SSE) com deduplicação por seq e reconexão. */
export function watchRun(runId, onEvent, onEnd) {
  let last = -1, closed = false, es;
  const open = () => {
    es = new EventSource(`/api/runs/${runId}/events?from=${last + 1}`);
    es.onmessage = (m) => {
      let ev; try { ev = JSON.parse(m.data); } catch { return; }
      if (ev.seq <= last) return;
      last = ev.seq; onEvent(ev);
      if (ev.type === 'done') { closed = true; es.close(); onEnd && onEnd(ev); }
    };
    es.onerror = () => { if (closed) return; es.close(); setTimeout(() => !closed && open(), 1200); };
  };
  open();
  return () => { closed = true; es && es.close(); };
}
