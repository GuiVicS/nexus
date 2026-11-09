// Gera packages/protocol/src/generated/index.ts reexportando todos os tipos escritos pelo ts-rs.
import fs from 'node:fs';
const dir = new URL('../packages/protocol/src/generated/', import.meta.url);
const names = fs.readdirSync(dir).filter(f => f.endsWith('.ts') && f !== 'index.ts').map(f => f.slice(0, -3)).sort();
fs.writeFileSync(new URL('index.ts', dir), names.map(n => `export type { ${n} } from './${n}';`).join('\n') + '\n');
console.log(names.length, 'tipos');
