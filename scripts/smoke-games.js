#!/usr/bin/env node
/* Teste de fumaça: gera um jogo de cada gênero com o GameFlow, abre cada um no Chromium, "joga" alguns segundos
 * e falha se houver qualquer erro de JavaScript. Também abre a interface (IDE) e confere que não há erros.
 *
 * Uso:  GAMEFLOW_BIN=target/debug/gameflow node scripts/smoke-games.js
 * Requer: npm i playwright && npx playwright install chromium   (ou NODE_PATH apontando para um playwright existente)
 */
const { spawn, spawnSync } = require('child_process');
const fs = require('fs'), os = require('os'), path = require('path');
const { chromium } = require('playwright');

const BIN = process.env.GAMEFLOW_BIN || path.join(__dirname, '..', 'target', 'debug', 'gameflow');
const PORT = process.env.SMOKE_PORT || '17878';
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'gf-smoke-'));
const env = { ...process.env, GAMEFLOW_HOME: home, NO_PROXY: '127.0.0.1,localhost', no_proxy: '127.0.0.1,localhost' };

const GENRES = {
  rpg2d: { prompt: 'RPG 2D de fantasia', keys: ['KeyD', 'KeyS', 'KeyA', 'KeyW'], hold: 500 },
  rpg3d: { prompt: 'RPG 3D no deserto', keys: ['KeyD', 'KeyS', 'KeyA', 'KeyW'], hold: 500 },
  visual_novel: { prompt: 'Visual novel de mistério', keys: [], hold: 0, vn: true },
  platformer2d: { prompt: 'Plataforma 2D na floresta', keys: ['KeyD'], hold: 1500, jump: true, preset: 'neon-noir' },
  platformer3d: { prompt: 'Plataforma 3D de lava', keys: ['KeyW', 'KeyD'], hold: 600 },
  fps: { prompt: 'FPS em primeira pessoa', keys: ['KeyW', 'KeyD', 'KeyW'], hold: 500 },
  tps: { prompt: 'Tiro em terceira pessoa no espaço', keys: ['KeyW', 'KeyD'], hold: 600 },
  arcade: { prompt: 'Jogo de nave arcade', keys: ['KeyA', 'KeyD'], hold: 500 },
  text_adventure: { prompt: 'Aventura de texto no deserto', ta: true },
};

let failures = 0;
const fail = (m) => { failures++; console.error('  ✗ ' + m); };

(async () => {
  for (const [g, c] of Object.entries(GENRES)) {
    const r = spawnSync(BIN, ['generate', c.prompt, '--name', 'smoke-' + g, '--genre', g, ...(c.preset ? ['--preset', c.preset] : [])], { env, encoding: 'utf8' });
    if (r.status !== 0) { fail(`generate ${g} falhou:\n${r.stdout}\n${r.stderr}`); }
  }
  const server = spawn(BIN, ['start', '--no-open', '--port', PORT], { env, stdio: 'ignore' });
  const base = `http://127.0.0.1:${PORT}`;
  let info;
  for (let i = 0; i < 50 && !info; i++) {
    try { info = await (await fetch(base + '/api/info')).json(); } catch { await new Promise((r) => setTimeout(r, 200)); }
  }
  if (!info) { console.error('servidor não subiu'); server.kill(); process.exit(1); }

  const browser = await chromium.launch({ args: ['--no-sandbox', '--autoplay-policy=no-user-gesture-required'] });
  // IDE
  {
    const page = await browser.newPage({ viewport: { width: 1360, height: 820 } });
    const errs = [];
    page.on('pageerror', (e) => errs.push(e.message));
    await page.goto(base + '/');
    await page.waitForSelector('.proj', { timeout: 10000 }).catch(() => errs.push('lista de projetos não apareceu'));
    await page.click('.proj >> nth=0').catch(() => {});
    await page.waitForTimeout(800);
    console.log(errs.length ? '✗ IDE' : '✓ IDE');
    errs.forEach(fail);
    await page.close();
  }
  for (const [g, c] of Object.entries(GENRES)) {
    const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
    const errs = [];
    page.on('pageerror', (e) => errs.push('pageerror: ' + e.message));
    page.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
    await page.goto(`${info.play_base}/smoke-${g.replace(/_/g, '-')}/`);
    if (c.ta) {
      // aventura de texto: digita o passo a passo e confere a vitória (primeiro clicando em Começar)
      await page.waitForSelector('#btnStart', { timeout: 15000 }).catch(() => errs.push('tela inicial não apareceu'));
      await page.click('#btnStart');
      const steps = await page.evaluate(() => GF.world.walkthrough);
      for (const cmd of steps) { await page.fill('#input', cmd); await page.keyboard.press('Enter'); await page.waitForTimeout(40); }
      const won = await page.evaluate(() => !!(GF.game && GF.game.won));
      if (!won) errs.push('o passo a passo não venceu a aventura no navegador');
      const shown = await page.locator('#terminal .win').count();
      if (!shown) errs.push('mensagem de vitória não apareceu');
      console.log(errs.length ? `✗ ${g}` : `✓ ${g}  (venceu em ${steps.length} comandos)`);
      errs.forEach(fail);
      await page.close();
      continue;
    }
    await page.waitForFunction(() => window.GF && GF.state === 'title', null, { timeout: 15000 }).catch(() => errs.push('o jogo não chegou à tela inicial'));
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);
    if (c.vn) { for (let i = 0; i < 8; i++) { await page.keyboard.press('Enter'); await page.waitForTimeout(250); } }
    for (const k of c.keys) { await page.keyboard.down(k); await page.waitForTimeout(c.hold); await page.keyboard.up(k); }
    if (c.jump) { for (let i = 0; i < 6; i++) { await page.keyboard.press('Space'); await page.waitForTimeout(250); } }
    await page.keyboard.press('Space');
    await page.waitForTimeout(500);
    const st = await page.evaluate(() => (window.GF ? GF.state : 'sem GF'));
    if (!['playing', 'end'].includes(st)) errs.push('estado final inesperado: ' + st);
    console.log(errs.length ? `✗ ${g}` : `✓ ${g}  (estado: ${st})`);
    errs.forEach(fail);
    await page.close();
  }
  await browser.close();
  server.kill();
  fs.rmSync(home, { recursive: true, force: true });
  if (failures) { console.error(`\n${failures} problema(s)`); process.exit(1); }
  console.log('\nTodos os jogos rodaram sem erros.');
})().catch((e) => { console.error(e); process.exit(1); });
