import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { forbidden } from './wording.mjs';
const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).split('\n').filter(f => f && !/package-lock\.json$|\.(png|jpe?g|webm|woff2?|ico)$/.test(f));
let bad = 0;
for (const f of files) {
  let text; try { text = readFileSync(f, 'utf8'); } catch { continue; }
  for (const hit of forbidden(text)) { console.log(`${f}: ${hit}`); bad++; }
}
console.log(bad ? `${bad} findings` : `wording check clean (${files.length} files)`);
process.exit(bad ? 1 : 0);
