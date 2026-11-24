#!/usr/bin/env node
/* Joga aventuras de texto geradas SEM navegador: carrega js/data.js + js/game.js num contexto isolado, reproduz o
 * "walkthrough" do mundo no motor real e confere que o jogo é vencido (e só no último comando).
 * Uso: node scripts/test-textadventure.js <pasta-do-projeto> [outra-pasta...] */
const vm = require('vm'), fs = require('fs'), path = require('path');

function load(dir) {
  const ctx = { console, setTimeout, clearTimeout };
  ctx.window = ctx; ctx.globalThis = ctx;
  vm.createContext(ctx);
  for (const f of ['js/data.js', 'js/game.js']) vm.runInContext(fs.readFileSync(path.join(dir, f), 'utf8'), ctx, { filename: f });
  return ctx;
}

let failures = 0;
const fail = (dir, m) => { failures++; console.error(`  ✗ ${path.basename(dir)}: ${m}`); };

for (const dir of process.argv.slice(2)) {
  const ctx = load(dir), TA = ctx.TA, w = ctx.GF.world;
  if (!TA || !w) { fail(dir, 'motor ou mundo não carregou'); continue; }
  const s = TA.createState(w);
  const steps = w.walkthrough || [];
  if (steps.length < 5) fail(dir, 'walkthrough curto demais');
  let won = false;
  steps.forEach((cmd, i) => {
    const r = TA.exec(s, w, cmd);
    const bad = r.out.find((o) => o.k === 'warn');
    if (bad) fail(dir, `comando ${i + 1} “${cmd}” falhou: ${bad.t}`);
    if (r.won && i < steps.length - 1) fail(dir, `venceu cedo demais no comando ${i + 1}`);
    won = r.won;
  });
  if (!won) fail(dir, 'o walkthrough não vence o jogo');
  // sanidade do parser
  const t = TA.createState(w);
  const check = (cmd, test, what) => { const r = TA.exec(t, w, cmd); if (!test(r.out)) fail(dir, `${what} (“${cmd}”): ${JSON.stringify(r.out.map((o) => o.t))}`); };
  check('olhar', (o) => o.some((x) => x.k === 'title'), 'olhar deve descrever a sala');
  check('look', (o) => o.some((x) => x.k === 'title'), 'look deve descrever a sala');
  check('inventario', (o) => o.some((x) => x.k === 'text'), 'inventário');
  check('i', (o) => o.some((x) => x.k === 'text'), 'atalho i');
  check('ajuda', (o) => o.some((x) => x.k === 'sys'), 'ajuda');
  check('help', (o) => o.some((x) => x.k === 'sys'), 'help');
  check('dica', (o) => o.some((x) => x.k === 'ok'), 'dica no início');
  check('blablabla', (o) => o.some((x) => x.k === 'warn'), 'comando desconhecido');
  check('pegar coisa-inexistente', (o) => o.some((x) => x.k === 'warn'), 'pegar algo que não existe');
  check('ir para o norte', (o) => o.length > 1, 'ir para o norte responde');
  // uma saída trancada nunca abre sem a chave
  const locked = w.rooms.find((r) => Object.keys(r.locks || {}).length);
  if (locked) {
    const u = TA.createState(w); u.room = locked.id;
    const d = Object.keys(locked.locks)[0];
    TA.exec(u, w, 'ir ' + d);
    if (u.room !== locked.id) fail(dir, 'atravessou a porta trancada sem a chave');
  }
  console.log(`  ${failures ? '·' : '✓'} ${path.basename(dir)} — ${steps.length} comandos, ${w.rooms.length} salas, venceu em ${s.moves} movimentos`);
}
if (failures) { console.error(`\n${failures} problema(s)`); process.exit(1); }
