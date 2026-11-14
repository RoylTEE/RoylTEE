#!/usr/bin/env node
// Mechanical check of the README tables "Every claim, checked" and "Try to break it".
// Offline: reads README.md, contracts/ and deployments/testnet.json. Fails when
//  - a backticked test name in the Test column is not a #[test] or #[tokio::test] function in contracts/
//  - a linked code file does not exist, or the function named next to it is not in that file
//  - a transaction link is not in deployments/testnet.json
//  - an account link is not an account in deployments/testnet.json
// Rows that say "pending deploy" in the Test column are skipped for tests (nothing exists yet).
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const SECTIONS = ['Every claim, checked', 'Try to break it'];

export function section(readme, title) {
  const m = readme.match(new RegExp(`^## ${title.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\n([\\s\\S]*?)(?=^## )`, 'm'));
  return m ? m[1] : '';
}
export function rows(sectionText) {
  return sectionText.split('\n').filter((l) => l.startsWith('|') && !/^\|[-| ]+\|$/.test(l)).slice(1)
    .map((l) => l.slice(1, -1).split('|').map((c) => c.trim()));
}

/** Pure: returns a list of problems. `readSource(path)` returns file text or null. */
export function checkClaims({ readme, testSource, record, readSource }) {
  const problems = [];
  const hashes = new Set(record.transactions.map((t) => t.transaction_hash).filter(Boolean));
  const accounts = new Set(Object.values(record.accounts).map((a) => a.account_id));
  let tests = 0, txs = 0, codeRefs = 0, acct = 0;
  for (const title of SECTIONS) {
    const text = section(readme, title);
    if (!text) { problems.push(`section "${title}" not found`); continue; }
    for (const cells of rows(text)) {
      const [, a, b, c] = cells;
      const code = cells.length === 4 && title === SECTIONS[0] ? a : '';
      const testCell = title === SECTIONS[0] ? b : cells[2];
      const proofCell = title === SECTIONS[0] ? c : cells[3];
      if (!/pending deploy/i.test(testCell)) {
        for (const [, name] of testCell.matchAll(/`([a-z0-9_]+)`/g)) {
          tests += 1;
          if (!new RegExp(`#\\[(tokio::)?test\\][^\\n]*\\n(\\s*#\\[[^\\n]*\\]\\n)*\\s*(pub )?(async )?fn ${name}\\b`).test(testSource)) problems.push(`test not found: ${name}`);
        }
      }
      for (const [, file, fn] of code.matchAll(/\[`[^`]+`\]\(([^)]+\.rs)\) `([a-z0-9_]+)`/g)) {
        codeRefs += 1;
        const src = readSource(file);
        if (src === null) problems.push(`code file not found: ${file}`);
        else if (!new RegExp(`fn ${fn}\\b`).test(src)) problems.push(`function ${fn} not in ${file}`);
      }
      for (const [, h] of proofCell.matchAll(/nearblocks\.io\/txns\/([A-Za-z0-9]+)\)/g)) { txs += 1; if (!hashes.has(h)) problems.push(`transaction not in record: ${h}`); }
      for (const [, id] of proofCell.matchAll(/nearblocks\.io\/address\/([a-z0-9._-]+)\)/g)) { acct += 1; if (!accounts.has(id)) problems.push(`account not in record: ${id}`); }
    }
  }
  return { problems, counts: { tests, txs, codeRefs, accounts: acct } };
}

const walk = (d) => readdirSync(d).flatMap((f) => (f === 'target' || f === 'node_modules') ? [] : statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), '..');
  const readme = readFileSync(join(root, 'README.md'), 'utf8');
  const record = JSON.parse(readFileSync(join(root, 'deployments', 'testnet.json'), 'utf8'));
  const testSource = walk(join(root, 'contracts')).filter((f) => f.endsWith('.rs')).map((f) => readFileSync(f, 'utf8')).join('\n');
  const readSource = (p) => existsSync(join(root, p)) ? readFileSync(join(root, p), 'utf8') : null;
  const { problems, counts } = checkClaims({ readme, testSource, record, readSource });
  for (const p of problems) console.log(`FAIL  ${p}`);
  console.log(`checked ${counts.tests} test names, ${counts.codeRefs} code references, ${counts.txs} transaction links, ${counts.accounts} account links`);
  console.log(problems.length ? `${problems.length} PROBLEM(S)` : 'ALL CLAIM REFERENCES EXIST');
  process.exit(problems.length ? 1 : 0);
}
