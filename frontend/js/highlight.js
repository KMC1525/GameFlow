// Realce de sintaxe leve (JS, CSS, HTML, JSON, Markdown) — tokenizador por expressões regulares.
import { esc } from './ui.js';

const JS_KW = 'const|let|var|function|return|if|else|for|while|do|class|extends|new|import|export|from|async|await|try|catch|finally|throw|switch|case|break|continue|default|typeof|instanceof|of|in|this|super|null|undefined|true|false|delete|void|yield|static|get|set';

const RULES = {
  js: [
    ['c', /\/\/[^\n]*|\/\*[\s\S]*?\*\//y],
    ['s', /"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|`(?:\\[\s\S]|[^`\\])*`/y],
    ['n', /\b0x[\da-f]+\b|\b\d+(?:\.\d+)?(?:e[+-]?\d+)?\b/iy],
    ['k', new RegExp(`\\b(?:${JS_KW})\\b`, 'y')],
    ['f', /[A-Za-z_$][\w$]*(?=\s*\()/y],
    ['p', /[{}()[\];,.<>=+\-*/%!&|?:]+/y],
  ],
  css: [
    ['c', /\/\*[\s\S]*?\*\//y],
    ['s', /"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'/y],
    ['n', /#[\da-f]{3,8}\b|-?\b\d+(?:\.\d+)?(?:px|rem|em|%|vh|vw|s|ms|deg)?\b/iy],
    ['k', /--?[a-z][\w-]*(?=\s*:)|@[a-z-]+/y],
    ['f', /[a-z-]+(?=\()/y],
    ['p', /[{}()[\];:,>+~*]+/y],
  ],
  json: [
    ['k', /"(?:\\.|[^"\\\n])*"(?=\s*:)/y],
    ['s', /"(?:\\.|[^"\\\n])*"/y],
    ['n', /-?\b\d+(?:\.\d+)?(?:e[+-]?\d+)?\b/iy],
    ['k', /\b(?:true|false|null)\b/y],
    ['p', /[{}[\],:]+/y],
  ],
  md: [
    ['k', /^#{1,6} [^\n]*/my],
    ['s', /`[^`\n]+`/y],
    ['f', /\*\*[^*\n]+\*\*/y],
    ['p', /^\s*(?:[-*+]|\d+\.)\s/my],
  ],
};

function tokenize(code, rules) {
  let out = '', i = 0, plain = '';
  const flush = () => { if (plain) { out += esc(plain); plain = ''; } };
  while (i < code.length) {
    let hit = false;
    for (const [cls, re] of rules) {
      re.lastIndex = i;
      const m = re.exec(code);
      if (m && m.index === i && m[0].length) {
        flush(); out += `<span class="hl-${cls}">${esc(m[0])}</span>`; i += m[0].length; hit = true; break;
      }
    }
    if (!hit) {
      // consome identificadores inteiros para não quebrar palavras-chave no meio
      const w = /[A-Za-z_$][\w$]*|\s+|./y; w.lastIndex = i; const m = w.exec(code);
      plain += m[0]; i += m[0].length;
    }
  }
  flush();
  return out;
}

function html(code) {
  let out = '', i = 0;
  const re = /<!--[\s\S]*?-->|<\/?[A-Za-z][^>]*>?/g;
  let m;
  while ((m = re.exec(code))) {
    out += esc(code.slice(i, m.index));
    const t = m[0];
    if (t.startsWith('<!--')) out += `<span class="hl-c">${esc(t)}</span>`;
    else {
      const inner = esc(t).replace(/^(&lt;\/?)([\w-]+)/, '$1<span class="hl-t">$2</span>')
        .replace(/([\w-:]+)(=)(&quot;[^&]*?&quot;|'[^']*')/g, '<span class="hl-a">$1</span>$2<span class="hl-s">$3</span>');
      out += inner;
    }
    i = m.index + t.length;
  }
  out += esc(code.slice(i));
  return out;
}

export function langOf(path) {
  const e = (path.split('.').pop() || '').toLowerCase();
  return { js: 'js', mjs: 'js', css: 'css', html: 'html', htm: 'html', json: 'json', md: 'md', svg: 'html', xml: 'html' }[e] || 'text';
}

export function highlight(code, lang) {
  if (code.length > 400000) return esc(code);
  if (lang === 'html') return html(code);
  if (RULES[lang]) return tokenize(code, RULES[lang]);
  return esc(code);
}
