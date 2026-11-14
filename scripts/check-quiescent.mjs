#!/usr/bin/env node
// Deploy precondition for the final registry code (THREAT_MODEL T19b).
// Read-only: it sends only RPC view queries, no keys, no signing, no
// transactions. Needs Node 18 or newer (built-in fetch).
//
// The new code can recover a lost payment forward only if the forward began
// after the upgrade (it keeps a record for each forward). A forward or a
// payout that is in flight at upgrade time has no record, and its recovery
// is not supported. So redeploy only when the registry is quiet:
//  1. every ledger has inbound_pending 0
//  2. no ledger has a payout lock or a transfer lock
//  3. forwards_in_flight_total is 0
// Every token is read page by page until the end. Exit code 0 only if all
// of this holds. If it fails, wait for the callbacks and run it again.
//
// RPC endpoint: NEAR_RPC_URL, default https://rpc.testnet.near.org.
// VERIFY_RECORD overrides the record path (used by the script's own tests).
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const recordPath = process.env.VERIFY_RECORD || join(root, 'deployments', 'testnet.json');
const record = JSON.parse(readFileSync(recordPath, 'utf8'));
const RPC = process.env.NEAR_RPC_URL || 'https://rpc.testnet.near.org';
const PAGE = 100;
const reg = Object.values(record.accounts).find((a) => a.variable === 'REG');

let failures = 0;
const pass = (msg) => console.log(`PASS  ${msg}`);
const fail = (msg) => { failures += 1; console.log(`FAIL  ${msg}`); };

async function view(account_id, method_name, args = {}) {
  for (let attempt = 1; attempt <= 4; attempt += 1) {
    try {
      const res = await fetch(RPC, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0', id: 'quiet', method: 'query',
          params: {
            finality: 'final', request_type: 'call_function', account_id, method_name,
            args_base64: Buffer.from(JSON.stringify(args)).toString('base64'),
          },
        }),
      });
      const body = await res.json();
      if (body.error) throw new Error(JSON.stringify(body.error));
      if (body.result && body.result.error) throw new Error(body.result.error);
      return JSON.parse(Buffer.from(body.result.result).toString('utf8'));
    } catch (err) {
      if (attempt === 4) throw err;
      await new Promise((r) => setTimeout(r, 1500 * attempt));
    }
  }
  throw new Error('unreachable');
}

console.log(`RPC ${RPC}`);
console.log(`Registry ${reg.account_id}\n`);

const tokens = [];
for (let from = 0; ; from += PAGE) {
  const page = await view(reg.account_id, 'nft_tokens', { from_index: String(from), limit: PAGE });
  if (!Array.isArray(page)) { fail(`nft_tokens from ${from} did not return a list`); break; }
  tokens.push(...page);
  if (page.length < PAGE) break;
}
pass(`nft_tokens read to the end: ${tokens.length} token(s)`);

for (const t of tokens) {
  const l = await view(reg.account_id, 'get_ledger', { token_id: t.token_id });
  if (!l || typeof l !== 'object') { fail(`token ${t.token_id} has no ledger`); continue; }
  const problems = [];
  if (l.inbound_pending !== 0) problems.push(`inbound_pending ${l.inbound_pending}`);
  if (l.payout_in_flight) problems.push('payout lock set');
  if (l.transfer_pending) problems.push('transfer lock set');
  problems.length === 0
    ? pass(`token ${t.token_id} is quiet`)
    : fail(`token ${t.token_id} is not quiet: ${problems.join(', ')}`);
}

const acct = await view(reg.account_id, 'get_accounting');
const fif = acct && acct.forwards_in_flight_total;
fif === '0' || fif === 0
  ? pass('forwards_in_flight_total is 0')
  : fail(`forwards_in_flight_total is ${JSON.stringify(fif)}, expected 0`);

console.log(`\n${failures === 0 ? 'QUIET: SAFE TO REDEPLOY' : `${failures} CHECK(S) FAILED: DO NOT REDEPLOY YET`}`);
process.exit(failures === 0 ? 0 : 1);
