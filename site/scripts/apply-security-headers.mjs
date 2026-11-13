/** Copy site/vercel.json headers into the Vercel Build Output config so they apply on CLI deploys. */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const vercel = JSON.parse(readFileSync(join(root, 'vercel.json'), 'utf8'));
const outPath = join(root, '.vercel/output/config.json');
const out = JSON.parse(readFileSync(outPath, 'utf8'));
const blocks = vercel.headers ?? [];
const routes = [];
for (const block of blocks) {
  const headers = {};
  for (const h of block.headers ?? []) headers[h.key] = h.value;
  const src = block.source === '/(.*)' ? '/(.*)' : block.source;
  routes.push({ src, headers, continue: true });
}
if (!routes.length) throw new Error('No headers in vercel.json');
out.routes = [...routes, ...(out.routes ?? [])];
writeFileSync(outPath, JSON.stringify(out, null, 2) + '\n');
