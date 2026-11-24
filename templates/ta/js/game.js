/* Aventura de texto — motor (parser em português e inglês) + interface em DOM.
 * O mundo vem de game.json → world (salas, itens, personagens, objetivo, dicas): edite à mão à vontade.
 * A lógica (TA.createState / TA.exec) não toca no DOM, então pode ser testada fora do navegador. */
(() => {
  'use strict';
  const root = typeof window !== 'undefined' ? window : globalThis;
  const GF = (root.GF = { cfg: null, world: null, game: null, lang: 'pt', palette: {}, ui: {}, muted: false });

  // ───────── ganchos (custom.js) ─────────
  const handlers = {};
  GF.on = (name, fn) => { (handlers[name] ||= []).push({ fn, errors: 0 }); };
  GF.emit = (name, ...args) => {
    const out = [];
    for (const h of (handlers[name] || []).slice()) {
      try { out.push(h.fn(...args)); } catch (e) {
        h.errors++;
        if (typeof console !== 'undefined') console.error('[GameFlow] erro em um ajuste (' + name + '):', e);
        if (h.errors >= 3) handlers[name].splice(handlers[name].indexOf(h), 1);
      }
    }
    return out;
  };
  GF.emitFirst = (name, ...args) => GF.emit(name, ...args).find((r) => r !== undefined && r !== null && r !== false);
  GF.event = (name, data) => GF.emit('event', name, data);

  // ───────── texto ─────────
  const norm = (s) => String(s).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, ' ').trim();

  const MSG = {
    pt: {
      unknown: 'Não entendi “{0}”. Digite “ajuda” para ver os comandos.',
      help: 'Comandos: olhar • examinar X • ler X • pegar X • largar X • inventário • ir <direção> (norte, sul, leste, oeste, cima, baixo) • usar X • abrir • falar com X • dar X para Y • dica • salvar • carregar • reiniciar.\nAtalhos: n, s, e, o, c (cima), b (baixo) • l = olhar • i = inventário • x = examinar.\nVocê também pode clicar nas palavras destacadas e nos botões.',
      exits: 'Saídas:', sees: 'Você vê:', people: 'Aqui está:',
      cant_go: 'Você não pode ir por aí.', go_where: 'Ir para onde?', locked_default: 'Está trancado.', unlocked: 'Você destranca a passagem com {0}.',
      taken: 'Você pegou {0}.', already: 'Você já está com {0}.', cant_take: '{0} não pode ser carregado.', take_what: 'Pegar o quê?', nothing_to_take: 'Não há nada aqui para pegar.',
      dropped: 'Você largou {0}.', drop_what: 'Largar o quê?', not_have: 'Você não tem isso.',
      inv_empty: 'Você não está carregando nada.', inv: 'Você carrega:',
      examine_what: 'Examinar o quê?', not_here: 'Não vejo isso aqui.', nothing_special: 'Você não vê nada de especial em {0}.', readable: '(Dá para ler.)', read_nothing: 'Não há nada para ler.',
      talk_who: 'Falar com quem?', talk_nobody: 'Não há ninguém com esse nome aqui.', npc_silent: '{0} não tem nada a dizer.', npc_done: '{0} sorri: “Já fizemos nosso negócio.”',
      give_what: 'Dar o quê, e para quem? Exemplo: dar maçã para fulano.', give_noitem: 'Você não tem {0}.', give_nonpc: 'Não há ninguém assim aqui.', give_refuse: '{0} não parece interessado.', received: 'Você recebeu {0}.',
      use_what: 'Usar o quê?', use_nothing: 'Nada acontece.', open_nothing: 'Não há nada para abrir aqui.', open_nokey: 'Está trancado e você não tem a chave certa.',
      hint: 'Dica:', hint_none: 'Sem dicas por agora — continue explorando!',
      saved: 'Jogo salvo.', loaded: 'Jogo carregado.', nosave: 'Nenhum jogo salvo.', restarted: 'Aventura reiniciada.',
      start: 'Começar', cont: 'Continuar', typehint: 'Digite “ajuda” a qualquer momento.', confirm_restart: 'Reiniciar a aventura do começo?', again: 'Jogar de novo',
      look: 'olhar', inventory: 'inventário', help_btn: 'ajuda', hint_btn: 'dica', go: 'ir', examine: 'examinar', talk: 'falar com', moves: 'movimentos',
    },
    en: {
      unknown: 'I did not understand “{0}”. Type “help” to see the commands.',
      help: 'Commands: look • examine X • read X • take X • drop X • inventory • go <direction> (north, south, east, west, up, down) • use X • open • talk to X • give X to Y • hint • save • load • restart.\nShortcuts: n, s, e, w, u, d • l = look • i = inventory • x = examine.\nYou can also click the highlighted words and the buttons.',
      exits: 'Exits:', sees: 'You see:', people: 'Here is:',
      cant_go: 'You cannot go that way.', go_where: 'Go where?', locked_default: 'It is locked.', unlocked: 'You unlock the way with {0}.',
      taken: 'You took {0}.', already: 'You already have {0}.', cant_take: '{0} cannot be carried.', take_what: 'Take what?', nothing_to_take: 'There is nothing here to take.',
      dropped: 'You dropped {0}.', drop_what: 'Drop what?', not_have: 'You do not have that.',
      inv_empty: 'You are not carrying anything.', inv: 'You are carrying:',
      examine_what: 'Examine what?', not_here: 'I do not see that here.', nothing_special: 'You see nothing special about {0}.', readable: '(It can be read.)', read_nothing: 'There is nothing to read.',
      talk_who: 'Talk to whom?', talk_nobody: 'There is nobody by that name here.', npc_silent: '{0} has nothing to say.', npc_done: '{0} smiles: “We already made our deal.”',
      give_what: 'Give what, and to whom? Example: give apple to someone.', give_noitem: 'You do not have {0}.', give_nonpc: 'There is nobody like that here.', give_refuse: '{0} does not seem interested.', received: 'You received {0}.',
      use_what: 'Use what?', use_nothing: 'Nothing happens.', open_nothing: 'There is nothing to open here.', open_nokey: 'It is locked and you do not have the right key.',
      hint: 'Hint:', hint_none: 'No hints for now — keep exploring!',
      saved: 'Game saved.', loaded: 'Game loaded.', nosave: 'No saved game.', restarted: 'Adventure restarted.',
      start: 'Start', cont: 'Continue', typehint: 'Type “help” at any time.', confirm_restart: 'Restart the adventure from the beginning?', again: 'Play again',
      look: 'look', inventory: 'inventory', help_btn: 'help', hint_btn: 'hint', go: 'go', examine: 'examine', talk: 'talk to', moves: 'moves',
    },
  };
  const DIRNAME = {
    pt: { north: 'norte', south: 'sul', east: 'leste', west: 'oeste', up: 'cima', down: 'baixo' },
    en: { north: 'north', south: 'south', east: 'east', west: 'west', up: 'up', down: 'down' },
  };
  const ARROW = { north: '↑', south: '↓', east: '→', west: '←', up: '⇑', down: '⇓' };
  const DIRWORDS = {
    north: ['n', 'norte', 'north'], south: ['s', 'sul', 'south'], east: ['e', 'leste', 'east'], west: ['w', 'o', 'oeste', 'west'],
    up: ['u', 'c', 'cima', 'subir', 'up', 'acima'], down: ['d', 'b', 'baixo', 'descer', 'down', 'abaixo'],
  };
  const VERBS = {
    look: ['olhar', 'ver', 'l', 'look', 'observar', 'olha'], examine: ['examinar', 'exa', 'x', 'examine', 'inspecionar', 'analisar'], read: ['ler', 'read'],
    take: ['pegar', 'pega', 'pegue', 'apanhar', 'take', 'get', 'grab', 'coletar'], drop: ['largar', 'solte', 'soltar', 'drop', 'deixar'],
    inv: ['inventario', 'inv', 'i', 'itens', 'inventory', 'mochila'], go: ['ir', 'va', 'vai', 'andar', 'go', 'walk', 'mover'], use: ['usar', 'use', 'utilizar'],
    open: ['abrir', 'abra', 'open', 'destrancar', 'unlock', 'destranque'], talk: ['falar', 'fale', 'conversar', 'talk', 'speak', 'perguntar'],
    give: ['dar', 'de', 'entregar', 'give', 'oferecer', 'offer'], help: ['ajuda', 'help', 'comandos'], hint: ['dica', 'hint', 'pista'],
    save: ['salvar', 'save'], load: ['carregar', 'load'], restart: ['reiniciar', 'restart'],
  };
  const verbOf = (w) => Object.keys(VERBS).find((k) => VERBS[k].includes(w));
  const dirOf = (w) => Object.keys(DIRWORDS).find((k) => DIRWORDS[k].includes(w));
  const LEAD = /^(?:(?:o|a|os|as|um|uma|para|pra|pro|com|to|the|at|with|em|no|na|from|in|on|do|da)\s+)+/;
  const PREP = / (?:para|pra|pro|ao|aos|to) /;

  const lang = () => (GF.lang === 'en' ? 'en' : 'pt');
  const T = (key, ...args) => {
    let s = MSG[lang()][key] ?? MSG.pt[key] ?? key;
    args.forEach((a, i) => { s = s.split('{' + i + '}').join(a); });
    return s;
  };
  const dname = (d) => DIRNAME[lang()][d] || d;

  // ───────── estado e consultas ─────────
  function createState(world) {
    const roomItems = {};
    for (const r of world.rooms) roomItems[r.id] = (r.items || []).slice();
    return { room: world.start, inv: [], flags: {}, roomItems, unlocked: {}, moves: 0, won: false, talked: {} };
  }
  const roomOf = (w, id) => w.rooms.find((r) => r.id === id);
  const itemOf = (w, id) => w.items.find((i) => i.id === id);
  const npcOf = (w, id) => w.npcs.find((n) => n.id === id);
  const aliasesOf = (e) => [norm(e.name), ...(e.aliases || []).map(norm)].filter(Boolean);

  /** Procura, entre `list`, a entidade citada em `text` (casa por nome/apelido inteiro ou prefixo). */
  function find(list, text) {
    text = norm(text).replace(LEAD, '').trim();
    if (!text) return null;
    let best = null, bestScore = 0;
    for (const e of list) {
      for (const a of aliasesOf(e)) {
        let s = 0;
        if (text === a) s = 100 + a.length;
        else if ((' ' + text + ' ').includes(' ' + a + ' ')) s = 50 + a.length;
        else if (a.length >= 3 && text.length >= 3 && a.startsWith(text)) s = 20 + text.length;
        if (s > bestScore) { best = e; bestScore = s; }
      }
    }
    return best;
  }
  const scopeItems = (w, s) => (s.roomItems[s.room] || []).map((id) => itemOf(w, id)).filter(Boolean);
  const invItems = (w, s) => s.inv.map((id) => itemOf(w, id)).filter(Boolean);
  const scopeNpcs = (w, s) => (roomOf(w, s.room).npcs || []).map((id) => npcOf(w, id)).filter(Boolean);
  const listNames = (arr) => arr.map((e) => e.name).join(', ');
  const cond = (c, s) => {
    const neg = c.startsWith('!');
    const [k, v] = (neg ? c.slice(1) : c).split(':');
    const r = k === 'has' ? s.inv.includes(v) : k === 'flag' ? !!s.flags[v] : k === 'at' ? s.room === v : false;
    return neg ? !r : r;
  };

  /** Entradas de saída que descrevem uma sala (título, texto, coisas, personagens, saídas). */
  function roomEntries(world, state, r) {
    const out = [];
    const its = (state.roomItems[r.id] || []).map((id) => itemOf(world, id)).filter(Boolean);
    const ppl = (r.npcs || []).map((id) => npcOf(world, id)).filter(Boolean);
    out.push({ k: 'title', t: r.name, data: { room: r.id } }, { k: 'text', t: r.desc });
    if (its.length) out.push({ k: 'things', t: T('sees') + ' ' + listNames(its), data: { label: T('sees'), items: its.map((i) => ({ id: i.id, name: i.name })) } });
    if (ppl.length) out.push({ k: 'things', t: T('people') + ' ' + listNames(ppl), data: { label: T('people'), people: ppl.map((n) => ({ id: n.id, name: n.name })) } });
    const exits = Object.keys(r.exits || {}).map((d) => ({ dir: d, label: dname(d), locked: !!(r.locks && r.locks[d] && !state.unlocked[r.id + ':' + d]) }));
    if (exits.length) out.push({ k: 'exits', t: T('exits') + ' ' + exits.map((e) => e.label + (e.locked ? ' 🔒' : '')).join(', '), data: { exits } });
    return out;
  }

  // ───────── execução de um comando ─────────
  /** Executa `input` e devolve { out: [{k, t, data?}], won }. `k`: echo|title|text|sys|ok|warn|npc|things|exits|win. */
  function exec(state, world, input) {
    const out = [];
    const say = (k, t, data) => out.push(data ? { k, t, data } : { k, t });
    const raw = String(input || '').trim();
    if (!raw) return { out, won: state.won };
    say('echo', '> ' + raw);

    const custom = GF.emitFirst('command', raw, state);
    if (typeof custom === 'string') { say('ok', custom); return finish(); }

    const words = raw === '?' ? ['ajuda'] : norm(raw).split(' ').filter(Boolean);
    if (!words.length) { say('warn', T('unknown', raw)); return { out, won: state.won }; }
    let verb = verbOf(words[0]);
    let rest = words.slice(1).join(' ').replace(LEAD, '').trim();
    // direção sozinha ("norte", "n")
    if ((!verb || ['e', 'o', 'l', 'i', 'x', 'd', 's', 'n', 'c', 'b', 'u', 'w'].includes(words[0])) && words.length === 1 && dirOf(words[0])) { verb = 'go'; rest = words[0]; }
    if (!verb) { say('warn', T('unknown', raw)); return finish(); }

    const room = roomOf(world, state.room);

    const showRoom = () => out.push(...roomEntries(world, state, room));
    const unlock = (dir) => {
      const lock = room.locks[dir];
      state.unlocked[room.id + ':' + dir] = true;
      say('ok', T('unlocked', itemOf(world, lock.key)?.name || lock.key));
      GF.event('unlock', { room: room.id, dir });
    };
    const lockedDirs = () => Object.keys(room.locks || {}).filter((d) => room.exits[d] && !state.unlocked[room.id + ':' + d]);

    switch (verb) {
      case 'look':
        if (rest) { const r = describe(rest); if (r) break; } else showRoom();
        break;
      case 'examine': case 'read': {
        if (!rest) { say('warn', T(verb === 'read' ? 'read_nothing' : 'examine_what')); break; }
        describe(rest, verb === 'read');
        break;
      }
      case 'inv': {
        const inv = invItems(world, state);
        say('text', inv.length ? T('inv') + ' ' + listNames(inv) + '.' : T('inv_empty'), inv.length ? { inv: inv.map((i) => ({ id: i.id, name: i.name })) } : undefined);
        break;
      }
      case 'go': {
        const d = dirOf(rest);
        if (!rest) { say('warn', T('go_where')); break; }
        if (!d) { say('warn', T('cant_go')); break; }
        const to = room.exits && room.exits[d];
        if (!to) { say('warn', T('cant_go')); break; }
        const lock = room.locks && room.locks[d];
        if (lock && !state.unlocked[room.id + ':' + d]) {
          if (state.inv.includes(lock.key)) unlock(d);
          else { say('warn', lock.msg || T('locked_default')); break; }
        }
        state.room = to; state.moves++;
        GF.event('move', { from: room.id, to });
        const nroom = roomOf(world, to);
        out.push(...roomEntries(world, state, nroom));
        GF.emit('enter', to, state);
        return finish();
      }
      case 'take': {
        if (!rest) { say('warn', T('take_what')); break; }
        if (['tudo', 'all', 'everything', 'todos'].includes(rest)) {
          const t = scopeItems(world, state).filter((i) => i.take !== false);
          if (!t.length) say('warn', T('nothing_to_take'));
          for (const i of t) takeItem(i);
          break;
        }
        const it = find(scopeItems(world, state), rest);
        if (!it) { const mine = find(invItems(world, state), rest); say('warn', mine ? T('already', mine.name) : T('not_here')); break; }
        takeItem(it);
        break;
      }
      case 'drop': {
        if (!rest) { say('warn', T('drop_what')); break; }
        const it = find(invItems(world, state), rest);
        if (!it) { say('warn', T('not_have')); break; }
        state.inv.splice(state.inv.indexOf(it.id), 1);
        state.roomItems[state.room].push(it.id);
        say('text', T('dropped', it.name));
        GF.event('drop', { item: it.id });
        break;
      }
      case 'use': {
        if (!rest) { say('warn', T('use_what')); break; }
        const it = find(invItems(world, state).concat(scopeItems(world, state)), rest);
        if (!it) { say('warn', T('not_have')); break; }
        const d = lockedDirs().find((x) => room.locks[x].key === it.id);
        if (d && state.inv.includes(it.id)) unlock(d); else say('text', T('use_nothing'));
        break;
      }
      case 'open': {
        const ld = lockedDirs();
        if (!ld.length) { say('warn', T('open_nothing')); break; }
        const d = ld.find((x) => state.inv.includes(room.locks[x].key));
        if (d) unlock(d); else say('warn', room.locks[ld[0]].msg || T('open_nokey'));
        break;
      }
      case 'talk': {
        if (!rest) { say('warn', T('talk_who')); break; }
        const n = find(scopeNpcs(world, state), rest);
        if (!n) { say('warn', T('talk_nobody')); break; }
        if (n.flag && state.flags[n.flag]) { say('npc', T('npc_done', n.name)); break; }
        const lines = n.talk || [];
        if (!lines.length) say('text', T('npc_silent', n.name));
        for (const l of lines) say('npc', n.name + ': “' + l + '”');
        GF.event('talk', { npc: n.id });
        break;
      }
      case 'give': {
        if (!rest) { say('warn', T('give_what')); break; }
        const parts = (' ' + rest + ' ').split(PREP);
        let it, n;
        if (parts.length >= 2) { it = find(invItems(world, state), parts[0]); n = find(scopeNpcs(world, state), parts.slice(1).join(' ')); }
        else {
          // sem preposição: "dar maca guardiao" — procura os dois no texto
          it = find(invItems(world, state), rest); n = find(scopeNpcs(world, state), rest);
        }
        if (!it) { const guess = find(world.items, parts[0]); say('warn', guess ? T('give_noitem', guess.name) : T('give_what')); break; }
        if (!n) { say('warn', T('give_nonpc')); break; }
        if (n.wants && n.wants === it.id && !(n.flag && state.flags[n.flag])) {
          state.inv.splice(state.inv.indexOf(it.id), 1);
          if (n.flag) state.flags[n.flag] = true;
          say('npc', n.name + ': “' + (n.thanks || '…') + '”');
          if (n.gives && !state.inv.includes(n.gives)) {
            state.inv.push(n.gives);
            say('ok', T('received', itemOf(world, n.gives)?.name || n.gives));
          }
          GF.event('give', { npc: n.id, item: it.id });
        } else say('npc', n.name + ': “' + (n.refuse || T('give_refuse', n.name)) + '”');
        break;
      }
      case 'hint': {
        const h = (world.hints || []).find((x) => (x.when || []).every((c) => cond(c, state)));
        say('ok', h ? T('hint') + ' ' + h.text : T('hint_none'));
        break;
      }
      case 'help': say('sys', T('help')); break;
      case 'save': say('sys', GF.save() ? T('saved') : T('nosave')); break;
      case 'load': { const ok = GF.load(); say('sys', ok ? T('loaded') : T('nosave')); if (ok) { Object.assign(state, GF.game); out.push(...roomEntries(world, state, roomOf(world, state.room))); } break; }
      case 'restart': say('sys', '__restart__'); break;
      default: say('warn', T('unknown', raw));
    }
    return finish();

    function takeItem(it) {
      if (it.take === false) { say('warn', T('cant_take', it.name)); return; }
      const arr = state.roomItems[state.room];
      arr.splice(arr.indexOf(it.id), 1);
      state.inv.push(it.id);
      say('ok', T('taken', it.name));
      GF.event('take', { item: it.id });
    }
    function describe(text, reading) {
      const everything = scopeItems(world, state).concat(invItems(world, state), scopeNpcs(world, state));
      const e = find(everything, text);
      if (!e) { say('warn', T('not_here')); return false; }
      if (reading) { say('text', e.read || T('read_nothing')); return true; }
      say('text', e.desc || T('nothing_special', e.name));
      if (e.read) say('sys', T('readable'));
      return true;
    }
    function finish() {
      if (!state.won && world.goal && state.inv.includes(world.goal.item) && state.room === world.goal.room) {
        state.won = true;
        say('win', String(world.win_text || '').split('{moves}').join(state.moves));
        GF.event('win', { moves: state.moves });
      }
      return { out, won: state.won };
    }
  }

  const TA = (root.TA = { createState, exec, norm, find, MSG });
  if (typeof module !== 'undefined' && module.exports) module.exports = TA;

  // ───────── persistência ─────────
  const key = () => 'gf-ta:' + (GF.cfg && GF.cfg.title || '');
  const store = {
    get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); return true; } catch (e) { return false; } },
    del(k) { try { localStorage.removeItem(k); } catch (e) { /* sem armazenamento */ } },
  };
  GF.save = () => (GF.game ? store.set(key(), JSON.stringify(GF.game)) : false);
  GF.load = () => {
    const raw = store.get(key());
    if (!raw) return false;
    try { const s = JSON.parse(raw); if (!s || !GF.world.rooms.some((r) => r.id === s.room)) return false; GF.game = s; return true; } catch (e) { return false; }
  };

  // ───────── interface (só no navegador) ─────────
  const loadData = () => {
    GF.cfg = root.GAME_DATA || null;
    GF.manifest = root.GAME_MANIFEST || { images: {}, audio: {} };
    GF.world = GF.cfg && GF.cfg.world;
    GF.lang = (GF.cfg && GF.cfg.lang) === 'en' ? 'en' : 'pt';
    GF.palette = (GF.cfg && GF.cfg.palette) || {};
  };
  GF.start = (fromSave) => {
    if (fromSave && GF.load()) { /* estado restaurado */ } else GF.game = createState(GF.world);
    GF.emit('start');
  };
  loadData();
  if (typeof document === 'undefined' || !document.getElementById) return;

  const $ = (id) => document.getElementById(id);
  const term = $('terminal'), input = $('input');
  const auds = {};
  let music = null, history = [], hpos = 0;

  GF.print = (text, cls = 'text') => append([{ k: cls, t: text }]);
  GF.play = (id, vol = 0.5) => {
    if (GF.muted) return;
    const a = auds[id];
    if (!a) return;
    try { const n = a.cloneNode(); n.volume = vol; n.play().catch(() => {}); } catch (e) { /* áudio indisponível */ }
  };
  const setMute = (m) => {
    GF.muted = m; store.set('gf-ta:mute', m ? '1' : '0'); $('btnMute').style.opacity = m ? 0.45 : 1;
    if (music) { if (m) music.pause(); else music.play().catch(() => {}); }
  };
  const setFont = (d) => {
    const cur = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--fs')) || 18;
    const v = Math.max(13, Math.min(30, cur + d));
    document.documentElement.style.setProperty('--fs', v + 'px'); store.set('gf-ta:fs', String(v));
  };

  function el(tag, cls, text) { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; }
  function thingButton(name, cmd, cls) {
    const b = el('button', 'thing' + (cls ? ' ' + cls : ''), name);
    b.type = 'button'; b.addEventListener('click', () => run(cmd)); return b;
  }
  function append(entries) {
    for (const e of entries) {
      const row = el('div', 'l ' + e.k);
      if (e.k === 'things' && e.data) {
        row.append(el('span', 'lbl', e.data.label));
        (e.data.items || []).forEach((i) => row.append(thingButton(i.name, T('examine') + ' ' + i.name)));
        (e.data.people || []).forEach((n) => row.append(thingButton(n.name, T('talk') + ' ' + n.name, 'person')));
      } else if (e.k === 'exits') {
        row.className = 'l sys'; row.textContent = e.t;
      } else row.textContent = e.t;
      term.append(row);
    }
    term.scrollTop = term.scrollHeight;
  }
  function refreshNav() {
    const g = GF.game, room = roomOf(GF.world, g.room);
    const nav = $('exits'); nav.replaceChildren();
    for (const [d] of Object.entries(room.exits || {})) {
      const locked = room.locks && room.locks[d] && !g.unlocked[room.id + ':' + d];
      const b = el('button', locked ? 'locked' : '', (ARROW[d] || '') + ' ' + dname(d) + (locked ? ' 🔒' : ''));
      b.type = 'button'; b.addEventListener('click', () => run(T('go') + ' ' + dname(d))); nav.append(b);
    }
    const img = GF.manifest.images && GF.manifest.images['room_' + room.id];
    const ban = $('banner');
    if (img) { $('bannerImg').src = img; ban.hidden = false; } else ban.hidden = true;
  }
  function run(cmd) {
    if (!GF.game || (GF.game.won && !cmd.startsWith('__'))) return;
    const r = exec(GF.game, GF.world, cmd);
    const restart = r.out.some((o) => o.t === '__restart__');
    if (restart) { if (confirm(T('confirm_restart'))) startGame(false, true); return; }
    append(r.out);
    if (r.out.some((o) => o.k === 'title')) GF.play('blip', 0.25);
    if (r.out.some((o) => o.k === 'ok')) GF.play('coin', 0.35);
    refreshNav();
    if (r.won) {
      GF.play('win', 0.6); store.del(key()); input.disabled = true;
      const again = el('button', 'thing', T('again')); again.type = 'button'; again.addEventListener('click', () => startGame(false, true));
      const row = el('div', 'l'); row.append(again); term.append(row); term.scrollTop = term.scrollHeight;
    } else GF.save();
  }
  function startGame(fromSave, clear) {
    GF.start(fromSave);
    if (clear) { term.replaceChildren(); append([{ k: 'sys', t: T('restarted') }]); }
    input.disabled = false;
    const intro = GF.world.intro || '';
    if (intro && !(fromSave && GF.game.moves > 0)) append([{ k: 'text', t: intro }]);
    append(roomEntries(GF.world, GF.game, roomOf(GF.world, GF.game.room))); refreshNav();
    if (!GF.muted && !music && auds.music) { music = auds.music.cloneNode(); music.loop = true; music.volume = 0.22; music.play().catch(() => {}); }
    input.focus();
  }
  function boot() {
    if (!GF.world || !GF.world.rooms) { term.textContent = 'game.json sem mundo (world).'; return; }
    document.title = (GF.cfg && GF.cfg.title) || document.title;
    document.documentElement.lang = GF.lang === 'en' ? 'en' : 'pt-BR';
    if (/pixel/i.test((GF.cfg && GF.cfg.art_style) || '')) $('banner').classList.add('pixel');
    const fs = parseFloat(store.get('gf-ta:fs')); if (fs) document.documentElement.style.setProperty('--fs', fs + 'px');
    GF.muted = store.get('gf-ta:mute') === '1'; $('btnMute').style.opacity = GF.muted ? 0.45 : 1;
    for (const [id, a] of Object.entries((GF.manifest && GF.manifest.audio) || {})) { try { const e = new Audio(typeof a === 'string' ? a : a.path); e.preload = 'auto'; auds[id] = e; } catch (e) { /* sem áudio */ } }
    $('btnStart').textContent = T('start'); $('btnContinue').textContent = T('cont'); $('coverHint').textContent = T('typehint');
    $('btnContinue').hidden = !store.get(key());
    $('btnStart').onclick = () => { $('cover').hidden = true; startGame(false, false); };
    $('btnContinue').onclick = () => { $('cover').hidden = true; startGame(true, false); };
    $('btnMute').onclick = () => setMute(!GF.muted);
    $('btnFontUp').onclick = () => setFont(1.5); $('btnFontDown').onclick = () => setFont(-1.5);
    $('btnRestart').onclick = () => { if (GF.game && confirm(T('confirm_restart'))) startGame(false, true); };
    const quick = $('quick');
    [[T('look'), T('look')], [T('inventory'), T('inventory')], [T('hint_btn'), T('hint_btn')], [T('help_btn'), T('help_btn')]].forEach(([label, cmd]) => {
      const b = el('button', '', label); b.type = 'button'; b.addEventListener('click', () => run(cmd)); quick.append(b);
    });
    $('form').addEventListener('submit', (e) => {
      e.preventDefault();
      const v = input.value.trim(); if (!v) return;
      history.push(v); hpos = history.length; input.value = ''; run(v);
    });
    input.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowUp' && history.length) { e.preventDefault(); hpos = Math.max(0, hpos - 1); input.value = history[hpos] || ''; }
      else if (e.key === 'ArrowDown' && history.length) { e.preventDefault(); hpos = Math.min(history.length, hpos + 1); input.value = history[hpos] || ''; }
    });
    document.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !$('cover').hidden) $('btnStart').click(); });
    $('btnStart').focus();
  }
  // erros -> console da IDE
  const report = (message, file, line) => { try { parent !== window && parent.postMessage({ gameflow: 'error', message: String(message), file: String(file || ''), line: line || 0 }, '*'); } catch (e) { /* sem pai */ } };
  window.addEventListener('error', (e) => report(e.message, (e.filename || '').split('/').slice(-2).join('/'), e.lineno));
  boot();
})();
