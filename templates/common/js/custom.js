// js/custom.js — SEUS AJUSTES. Este arquivo é carregado depois do jogo e nunca é sobrescrito sem aviso.
//
// Ganchos disponíveis:
//   GF.on('start',  () => { ... });            // quando o jogo (re)começa
//   GF.on('update', (dt) => { ... });          // todo quadro (dt em segundos)
//   GF.on('draw',   (ctx) => { ... });         // depois que o jogo desenhou o quadro
//   GF.on('event',  (nome, dados) => { ... }); // 'coin', 'hit', 'kill', 'win', 'lose'...
//
// Úteis: GF.game (estado atual), GF.cfg (conteúdo do game.json), GF.input.down('KeyW'),
//        GF.play('coin'), GF.img('player'), GF.palette, GF.W, GF.H, GF.toast('texto').
//
// Exemplo (remova as barras para ativar):
//   GF.on('event', (nome) => { if (nome === 'coin') GF.toast('Boa!'); });
