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
//  4. no stored forward watch (raw storage key prefix "fwd:") exists
// Every token is read page by page until the end. Exit code 0 only if all
// of this holds. If it fails, wait for the callbacks and run it again.
//
// All reads use ONE pinned block: the script first asks for the latest final
// block, then sends every query with that block hash, so the checks describe
// a single state and not a mix of states. A pass prints "SNAPSHOT QUIET".
// This is a precheck for people. The atomic guarantee is the on-chain
// assert_quiescent call in the same transaction as the deploy (DEPLOY_PLAN
// step 15c). Run this script once more after the deploy: the registry should
// still be quiet. Run it then with AFTER_DEPLOY=1: the new code may hold a
// settled watch entry, so the check reads each token's get_forward_watch
// instead of counting fwd: keys.
//
// Check 4 exists because the watch layout changed between builds (36, 60 and
// now 68 bytes). The new code must never meet an older entry, and the live
// registry has none because the deployed code never writes them.
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

async function rpc(method, params) {
  for (let attempt = 1; attempt <= 4; attempt += 1) {
    try {
      const res = await fetch(RPC, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 'quiet', method, params }),
      });
      const body = await res.json();
      if (body.error) throw new Error(JSON.stringify(body.error));
      if (body.result && body.result.error) throw new Error(body.result.error);
      return body.result;
    } catch (err) {
      if (attempt === 4) throw err;
      await new Promise((r) => setTimeout(r, 1500 * attempt));
    }
  }
  throw new Error('unreachable');
}

// One block for every read.
const head = await rpc('block', { finality: 'final' });
const block_id = head.header.hash;
console.log(`Pinned block ${head.header.height} (${block_id})`);

async function view(account_id, method_name, args = {}) {
  const result = await rpc('query', {
    block_id, request_type: 'call_function', account_id, method_name,
    args_base64: Buffer.from(JSON.stringify(args)).toString('base64'),
  });
  return JSON.parse(Buffer.from(result.result).toString('utf8'));
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

if (process.env.AFTER_DEPLOY === '1') {
  // New code is live: a settled forward watch (a tombstone of an earlier
  // quarantine keeps its settlement sequence) is normal. Every token's watch
  // must be absent or hold nothing in flight or quarantined.
  for (const t of tokens) {
    const w = await view(reg.account_id, 'get_forward_watch', { token_id: t.token_id });
    const settled = w === null || (w.count === 0 && w.amount === '0' && w.written_off === '0');
    settled
      ? pass(`token ${t.token_id} forward watch is settled`)
      : fail(`token ${t.token_id} forward watch is not settled: ${JSON.stringify(w)}`);
  }
} else {
  const watched = await rpc('query', {
    block_id, request_type: 'view_state', account_id: reg.account_id,
    prefix_base64: Buffer.from('fwd:').toString('base64'),
  });
  const nWatch = Array.isArray(watched.values) ? watched.values.length : -1;
  nWatch === 0
    ? pass('no fwd: watch entries in storage')
    : fail(`fwd: watch entries in storage: ${nWatch}, expected 0`);
}

console.log(`\n${failures === 0 ? 'SNAPSHOT QUIET at block ' + head.header.height + ': SAFE TO REDEPLOY' : `${failures} CHECK(S) FAILED: DO NOT REDEPLOY YET`}`);
process.exit(failures === 0 ? 0 : 1);
