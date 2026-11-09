// Teste de pickNeighbor (sem DOM): grade 3x2 e fileira.
import { pickNeighbor } from './src/index.ts';
const R = (l, t, w = 100, h = 100) => ({ left: l, top: t, right: l + w, bottom: t + h, width: w, height: h });
const mk = (id, r) => ({ el: { id }, rect: r });
const grid = [mk('a', R(0, 0)), mk('b', R(120, 0)), mk('c', R(240, 0)), mk('d', R(0, 120)), mk('e', R(120, 120)), mk('f', R(240, 120))];
const t = (d, from, want) => { const f = grid.find(g => g.el.id === from); const got = pickNeighbor(d, f.rect, grid, f.el)?.id ?? null; if (got !== want) { console.error(`FALHOU ${from} ${d}: ${got} != ${want}`); process.exit(1); } };
t('r', 'a', 'b'); t('r', 'b', 'c'); t('r', 'c', null); t('l', 'a', null); t('d', 'a', 'd'); t('d', 'e', null); t('u', 'f', 'c'); t('u', 'c', null); t('l', 'f', 'e');
// grade irregular: ao descer de uma coluna estreita, escolhe o mais alinhado
const g2 = [mk('x', R(0, 0, 400, 100)), mk('y', R(0, 150, 100, 100)), mk('z', R(300, 150, 100, 100))];
const got = pickNeighbor('d', g2[0].rect, g2, g2[0].el)?.id;
if (!['y', 'z'].includes(got)) { console.error('descida irregular', got); process.exit(1); }
console.log('focus: ok');
