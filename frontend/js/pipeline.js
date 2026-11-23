// Visualização do fluxo NÃO LINEAR de agentes (grafo em SVG, atualizado em tempo real).
import { h } from './ui.js';
import { agentSummary, taskName } from './state.js';

const NS = 'http://www.w3.org/2000/svg';
const svg = (tag, attrs = {}, ...kids) => {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  kids.flat().forEach((k) => k != null && el.append(k.nodeType ? k : document.createTextNode(k)));
  return el;
};

const GLYPH = { pending: '○', running: '◔', done: '✓', failed: '✕', skipped: '–' };
const W = 360, NW = 84, NH = 40, GAP = 20, PAD = 8;

export class Pipeline {
  constructor() {
    this.nodes = new Map();
    this.el = h('div', {});
    this.render();
  }

  upsert(n) {
    const cur = this.nodes.get(n.id) || { status: 'pending', detail: '' };
    this.nodes.set(n.id, { ...cur, ...n });
    this.render();
  }

  setStatus(id, status, detail) {
    const n = this.nodes.get(id);
    if (!n) return;
    n.status = status; if (detail != null) n.detail = detail;
    this.render();
  }

  label(id) { return this.nodes.get(id)?.label || id; }

  depthOf(id, memo = {}, seen = new Set()) {
    if (memo[id] != null) return memo[id];
    if (seen.has(id)) return 0;
    seen.add(id);
    const n = this.nodes.get(id);
    const d = !n || !n.deps.length ? 0 : 1 + Math.max(...n.deps.filter((x) => this.nodes.has(x)).map((x) => this.depthOf(x, memo, seen)), -1);
    return (memo[id] = d);
  }

  render() {
    const nodes = [...this.nodes.values()];
    const memo = {}; const rows = [];
    for (const n of nodes) (rows[this.depthOf(n.id, memo)] ||= []).push(n);
    const pos = {};
    rows.forEach((row, ri) => row?.forEach((n, i) => { pos[n.id] = { x: ((i + 0.5) * W) / row.length, y: PAD + ri * (NH + GAP) }; }));
    const height = PAD * 2 + rows.length * (NH + GAP) - GAP;
    const root = svg('svg', { class: 'graph', viewBox: `0 0 ${W} ${Math.max(60, height)}`, role: 'img', 'aria-label': 'Fluxo de agentes' });
    // arestas
    for (const n of nodes) for (const d of n.deps) {
      const a = pos[d], b = pos[n.id], p = this.nodes.get(d);
      if (!a || !b || !p) continue;
      const rework = n.round > p.round;
      const my = (a.y + NH + b.y) / 2;
      root.append(svg('path', { class: 'gedge' + (rework ? ' rework' : '') + (p.status === 'done' && n.status === 'running' ? ' active' : ''), d: `M${a.x},${a.y + NH} C${a.x},${my} ${b.x},${my} ${b.x},${b.y}` }));
    }
    // nós
    for (const n of nodes) {
      const p = pos[n.id];
      const tip = `${n.label}${n.round > 1 ? ` (rodada ${n.round})` : ''}\nmodelo de ${taskName(n.task)}: ${agentSummary(n.task)}${n.detail ? '\n' + n.detail : ''}${n.status === 'skipped' ? '\n(pulado)' : ''}`;
      const sub = n.detail ? String(n.detail).split(' · ')[0].slice(0, 17) : taskName(n.task);
      root.append(svg('g', { class: `gnode ${n.status}${n.round > 1 ? ' rework' : ''}`, transform: `translate(${p.x - NW / 2},${p.y})` },
        svg('title', {}, tip),
        svg('rect', { width: NW, height: NH }),
        svg('text', { x: 8, y: 16 }, `${GLYPH[n.status] || '○'} ${n.round > 1 || n.id.includes('#') ? '↻ ' : ''}${n.label}`),
        svg('text', { x: 8, y: 31, class: 'sub' }, sub)));
    }
    this.el.replaceChildren(root);
  }
}
