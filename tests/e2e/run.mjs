// Executa os cenários de interface. Uso: node tests/e2e/run.mjs [filtro]
import { failed } from './lib.mjs';
import { scenarios } from './scenarios.mjs';

const only = process.argv[2];
let ran = 0;
let crashed = 0;
for (const [name, fn] of Object.entries(scenarios)) {
  if (only && !name.includes(only)) continue;
  console.log(`\n# ${name}`);
  ran++;
  const t = Date.now();
  try {
    await fn();
  } catch (e) {
    console.log('  FAIL (exceção)', e.stack || e);
    crashed++;
    process.exitCode = 1;
  }
  console.log(`  (${((Date.now() - t) / 1000).toFixed(1)} s)`);
}
console.log(`\n${ran} cenários, ${failed() + crashed} falhas`);
if (failed()) process.exitCode = 1;
process.exit(process.exitCode ?? 0);
