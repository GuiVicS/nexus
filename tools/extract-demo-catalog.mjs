// Extrai o catálogo fictício do protótipo (console-os.html) para assets/demo/catalog.json.
// Uso: node tools/extract-demo-catalog.mjs /root/retropad/console-os.html assets/demo/catalog.json
import fs from 'node:fs';
import vm from 'node:vm';
const [src, out] = process.argv.slice(2);
const lines = fs.readFileSync(src, 'utf8').split('\n');
const a = lines.findIndex(l => l.startsWith('const GAMES=['));
const b = lines.findIndex(l => l.startsWith('const brandHTML'));
const ctx = {};
vm.createContext(ctx);
vm.runInContext(lines.slice(a, b).join('\n') + '\nthis.D={GAMES,STORE,RETRO,APPS,STORE_APPS,OFF,LOGO,BRAND,FRIENDS};', ctx);
const D = ctx.D;
const unit = { GB: 1e9, MB: 1e6, KB: 1e3 };
const sizeOf = s => { const m = /^([\d,]+)\s*(GB|MB|KB)/.exec(s || ''); return m ? Math.round(parseFloat(m[1].replace(',', '.')) * unit[m[2]]) : null; };
const ageOf = r => r === 'L' ? 0 : parseInt(r, 10) || null;
const fileExists = f => fs.existsSync(new URL('../assets/demo/icons/' + f, import.meta.url));
const items = [];
function base(it, extra) {
  const o = {
    id: 'demo:' + it.id, provider: 'demo', kind: it.kind === 'app' ? 'app' : 'game', title: it.title,
    subtitle: it.cat || it.genre || '', description: it.desc || '', studio: it.studio || null, year: it.year || null,
    rating: it.rating !== undefined ? ageOf(it.rating) : null, state: 'installed', sizeBytes: sizeOf(it.size),
    art: { cover: it.kind !== 'app', hero: it.kind !== 'app', icon: false }, caps: { install: false, pause: false, cancel: false, uninstall: false },
    playtimeSec: Math.round((it.hours || 0) * 3600), lastPlayed: null, progress: it.progress || null, chapter: it.chapter || null,
    achievements: it.ach ? { got: it.ach[0], total: it.ach[1] } : null, price: null, offer: null, category: it.cat || null,
    tags: [], hue: [it.h ?? 180, it.h2 ?? 200], brand: null, logo: D.LOGO[it.id] || null, install: null,
  };
  return Object.assign(o, extra);
}
const tagOf = t => ({ 'Nuvem': 'cloud', 'Atualização': 'update', 'Clássicos': 'classic' })[t];
const offer = id => D.OFF[id] ? { old: D.OFF[id][0], new: D.OFF[id][1], pct: D.OFF[id][2] } : null;
D.GAMES.forEach((g, i) => items.push(base(g, { tags: [tagOf(g.tag), 'lib'].filter(Boolean), lastPlayed: 1_700_000_000 - i * 86400 })));
const store = (arr, cat) => arr.forEach(g => items.push(base(g, {
  state: 'not_installed', price: g.price, offer: offer(g.id), tags: ['store:' + cat, tagOf(g.tag)].filter(Boolean),
  caps: { install: true, pause: false, cancel: true, uninstall: true }, subtitle: g.genre, category: g.system || null,
})));
store(D.STORE, 'jogos'); store(D.RETRO, 'classicos');
const brandOf = it => {
  if (it.brand) { const [bg, f, t] = D.BRAND[it.brand]; return [{ bg, mode: t }, f]; }
  if (it.icon) return [{ bg: it.bg, mode: 'i' }, it.icon + '.svg'];
  return [null, null];
};
D.APPS.forEach(a => { const [br, f] = brandOf(a); items.push(base(a, { brand: br, art: { cover: false, hero: false, icon: !!(f && fileExists(f)) }, tags: ['lib'], _icon: f })); });
D.STORE_APPS.forEach(a => { const [br, f] = brandOf(a); items.push(base(a, {
  state: 'not_installed', price: 'Grátis', brand: br, art: { cover: false, hero: false, icon: !!(f && fileExists(f)) }, tags: ['store:apps'],
  caps: { install: true, pause: false, cancel: true, uninstall: true }, _icon: f, subtitle: a.cat })); });
for (const i of items) { if (i.art.icon) i.iconFile = i._icon; delete i._icon; }
const friends = D.FRIENDS.map(f => ({ name: f[0], status: f[1], hue: f[2], online: !!f[3] }));
fs.writeFileSync(out, JSON.stringify({ items, friends }, null, 1));
console.log(items.length, 'itens;', items.filter(i => i.art.icon).length, 'com ícone');
