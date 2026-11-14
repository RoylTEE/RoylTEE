#!/usr/bin/env node
// Copies docs/WHITEPAPER.md to src/data/whitepaper.md, the text the /whitepaper page renders.
// The site can be built from this folder alone, so the page reads this copy at build time.
// A test (tests/whitepaper.test.ts) fails when the copy differs from docs/WHITEPAPER.md.
//   node scripts/sync-whitepaper.mjs          writes the copy
//   node scripts/sync-whitepaper.mjs --check  exits 1 when the copy is out of date
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const SOURCE = new URL('../../docs/WHITEPAPER.md', import.meta.url);
export const TARGET = new URL('../src/data/whitepaper.md', import.meta.url);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const want = readFileSync(SOURCE, 'utf8');
  if (process.argv.includes('--check')) {
    let have = '';
    try { have = readFileSync(TARGET, 'utf8'); } catch { /* missing */ }
    if (have !== want) { console.error('src/data/whitepaper.md is out of date: run node scripts/sync-whitepaper.mjs'); process.exit(1); }
    console.log('whitepaper copy is current');
  } else {
    writeFileSync(TARGET, want);
    console.log('wrote src/data/whitepaper.md');
  }
}
