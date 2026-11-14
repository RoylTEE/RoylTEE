#!/usr/bin/env node
// Makes public/royltee-whitepaper.pdf by printing the /whitepaper page with headless Chrome.
// The page shows docs/WHITEPAPER.md, so the PDF has the same text. It needs a running site and a Chrome binary:
//   cd site && npm run dev        (in one terminal)
//   node scripts/build-whitepaper-pdf.mjs [url]   (default http://127.0.0.1:4321/whitepaper)
// Set CHROME to a browser binary if google-chrome is not on the path.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const url = process.argv[2] ?? 'http://127.0.0.1:4321/whitepaper';
const out = fileURLToPath(new URL('../public/royltee-whitepaper.pdf', import.meta.url));
const chrome = process.env.CHROME ?? 'google-chrome';
const r = spawnSync(chrome, ['--headless=new', '--no-sandbox', '--disable-gpu', '--no-pdf-header-footer', '--virtual-time-budget=8000', `--print-to-pdf=${out}`, url], { stdio: 'inherit', env: { ...process.env, TZ: 'UTC' } });
if (r.status !== 0) { console.error('Chrome did not make the PDF'); process.exit(1); }
console.log(`wrote ${out}`);
