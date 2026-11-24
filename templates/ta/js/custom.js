// js/custom.js — SEUS AJUSTES. Este arquivo é carregado depois do jogo e nunca é sobrescrito sem aviso.
//
// Ganchos da aventura de texto:
//   GF.on('start', () => { ... });                 // quando a aventura (re)começa
//   GF.on('enter', (idDaSala, estado) => { ... }); // toda vez que o jogador entra numa sala
//   GF.on('event', (nome, dados) => { ... });      // 'take', 'drop', 'unlock', 'give', 'talk', 'move', 'win'
//   GF.on('command', (texto, estado) => { ... });  // veja o exemplo: devolva um texto para tratar o comando você mesmo
//
// Úteis: GF.game (estado: room, inv, flags, moves), GF.world (salas, itens, personagens — vem do game.json),
//        GF.print('texto', 'ok'|'warn'|'sys'|'npc'), GF.play('coin'), GF.lang ('pt' ou 'en').
//
// Exemplo (remova as barras para ativar): um comando secreto
//   GF.on('command', (texto) => { if (texto.trim().toLowerCase() === 'xyzzy') return 'Nada acontece... por enquanto.'; });
