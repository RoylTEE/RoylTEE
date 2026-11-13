#!/usr/bin/env node
// Read-only check of the live NEAR testnet deployment against
// deployments/testnet.json. It sends only RPC view queries: no keys,
// no signing, no transactions. Needs Node 18 or newer (built-in fetch).
//
// Checks:
//  1. every share vault s<N>.REG has zero access keys (T1, T20)
//  2. code hashes of the registry, mock USDC and every vault equal the record (T13)
//  3. the registry and mock USDC key counts equal the record (T13 current state)
//  4. every share token names its own vault account
//  5. solvency: the registry's tracked counters <= its mock USDC balance (T31)
//
// RPC endpoint: NEAR_RPC_URL, default https://rpc.testnet.near.org
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const record = JSON.parse(readFileSync(join(root, 'deployments', 'testnet.json'), 'utf8'));
const RPC = process.env.NEAR_RPC_URL || 'https://rpc.testnet.near.org';

const accounts = record.accounts;
const reg = Object.values(accounts).find((a) => a.variable === 'REG');
const usdc = Object.values(accounts).find((a) => a.variable === 'USDC');
const vaults = Object.values(accounts).filter((a) => /^S\d+$/.test(a.variable));

let failures = 0;
const pass = (msg) => console.log(`PASS  ${msg}`);
const fail = (msg) => { failures += 1; console.log(`FAIL  ${msg}`); };

async function rpc(params) {
  for (let attempt = 1; attempt <= 4; attempt += 1) {
    try {
      const res = await fetch(RPC, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 'verify', method: 'query', params: { finality: 'final', ...params } }),
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

const viewAccount = (account_id) => rpc({ request_type: 'view_account', account_id });
const keyList = async (account_id) => (await rpc({ request_type: 'view_access_key_list', account_id })).keys;
async function view(account_id, method_name, args = {}) {
  const r = await rpc({
    request_type: 'call_function', account_id, method_name,
    args_base64: Buffer.from(JSON.stringify(args)).toString('base64'),
  });
  return JSON.parse(Buffer.from(r.result).toString('utf8'));
}

console.log(`RPC ${RPC}`);
console.log(`Registry ${reg.account_id}, mock USDC ${usdc.account_id}, ${vaults.length} vaults in the record\n`);

for (const v of vaults) {
  const keys = await keyList(v.account_id);
  keys.length === 0 ? pass(`${v.account_id} has 0 access keys`) : fail(`${v.account_id} has ${keys.length} access keys`);
  const acc = await viewAccount(v.account_id);
  acc.code_hash === v.code_hash_base58
    ? pass(`${v.account_id} code hash ${acc.code_hash} matches the record`)
    : fail(`${v.account_id} code hash ${acc.code_hash}, record says ${v.code_hash_base58}`);
}

for (const c of [reg, usdc]) {
  const acc = await viewAccount(c.account_id);
  acc.code_hash === c.code_hash_base58
    ? pass(`${c.account_id} code hash ${acc.code_hash} matches the record`)
    : fail(`${c.account_id} code hash ${acc.code_hash}, record says ${c.code_hash_base58}`);
  const keys = await keyList(c.account_id);
  const full = keys.filter((k) => k.access_key.permission === 'FullAccess').length;
  keys.length === c.key_count
    ? pass(`${c.account_id} has ${keys.length} access key(s), ${full} full access, as recorded`)
    : fail(`${c.account_id} has ${keys.length} access key(s), record says ${c.key_count}`);
}

const tokens = await view(reg.account_id, 'nft_tokens', { from_index: '0', limit: 100 });
for (const t of tokens) {
  const extra = JSON.parse(t.metadata.extra || '{}');
  const expected = `${t.token_id}.${reg.account_id}`;
  extra.vault === expected
    ? pass(`token ${t.token_id} owner ${t.owner_id}, vault ${extra.vault}, status ${extra.status}, earned ${extra.earned}, paid out ${extra.paid_out}`)
    : fail(`token ${t.token_id} names vault ${extra.vault}, expected ${expected}`);
}

const acct = await view(reg.account_id, 'get_accounting');
const tracked = ['allowance_total', 'unallocated_total', 'escrow_total', 'undelivered_total', 'forwards_in_flight_total']
  .reduce((s, k) => s + BigInt(acct[k] ?? '0'), 0n);
const balance = BigInt(await view(usdc.account_id, 'ft_balance_of', { account_id: reg.account_id }));
tracked <= balance
  ? pass(`solvency: tracked counters ${tracked} <= registry mock USDC balance ${balance} (base units, 6 decimals)`)
  : fail(`solvency: tracked counters ${tracked} > registry mock USDC balance ${balance}`);

console.log(`\n${failures === 0 ? 'ALL CHECKS PASSED' : `${failures} CHECK(S) FAILED`}`);
process.exit(failures === 0 ? 0 : 1);
