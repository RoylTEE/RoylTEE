#!/usr/bin/env node
// Read-only check of the live NEAR testnet deployment against
// deployments/testnet.json. It sends only RPC view queries: no keys,
// no signing, no transactions. Needs Node 18 or newer (built-in fetch).
//
// Checks:
//  1. every share vault has zero access keys (T1, T20), for every vault in the
//     record and every vault found through the tokens
//  2. code hashes of the registry, mock USDC and every vault equal the record (T13)
//  3. the access keys of the registry, mock USDC and payment agent equal the
//     record key by key: public key, full access or function call, and for a
//     function call key its receiver and method names (T13 current state)
//  4. tokens are read page by page until the end (not only the first 100).
//     Each token must be in token_owners in the record, name its own vault
//     s<N>.REG, and have the recorded owner (for example s2 after the catalog
//     sale). Each recorded token and vault must be found on chain.
//  5. the vault code the registry deploys at mint (get_vault_code_hash) equals
//     wasm.vault in the record, and the registry code hash in the record agrees
//     with wasm.registry (T13)
//  6. solvency: all five accounting counters must be present as nonnegative
//     integers, and their sum must be <= the registry's mock USDC balance (T31)
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

const accounts = record.accounts;
const byVar = (name) => Object.values(accounts).find((a) => a.variable === name);
const reg = byVar('REG');
const usdc = byVar('USDC');
const agent = byVar('AGENT');
const vaults = Object.values(accounts).filter((a) => /^S\d+$/.test(a.variable));
const owners = (record.token_owners && record.token_owners.owners) || {};
// A vault that is not in the record was minted by the current registry, so it
// should carry the vault code that registry embeds.
const wasmVault = (record.wasm && record.wasm.vault) || {};
const vaultCodeHash = wasmVault.on_chain_code_hash_base58 || (vaults.length > 0 ? vaults[0].code_hash_base58 : undefined);

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

// One comparable string per key: public key, kind, and for a function call
// key its receiver and sorted method names. The allowance changes with use,
// so it is not compared.
function liveKey(k) {
  const p = k.access_key.permission;
  if (p === 'FullAccess') return `${k.public_key} full_access`;
  const fc = p.FunctionCall || {};
  return `${k.public_key} function_call ${fc.receiver_id} [${[...(fc.method_names || [])].sort().join(',')}]`;
}
function recordedKey(k) {
  if (k.permission === 'full_access') return `${k.public_key} full_access`;
  return `${k.public_key} function_call ${k.contract} [${[...(k.methods || [])].sort().join(',')}]`;
}

async function checkVault(accountId, expectedHash, source) {
  const keys = await keyList(accountId);
  keys.length === 0 ? pass(`${accountId} has 0 access keys (${source})`) : fail(`${accountId} has ${keys.length} access keys (${source})`);
  const acc = await viewAccount(accountId);
  acc.code_hash === expectedHash
    ? pass(`${accountId} code hash ${acc.code_hash} matches the record`)
    : fail(`${accountId} code hash ${acc.code_hash}, record says ${expectedHash}`);
}

console.log(`RPC ${RPC}`);
console.log(`Registry ${reg.account_id}, mock USDC ${usdc.account_id}, ${vaults.length} vaults in the record\n`);

const checkedVaults = new Set();
for (const v of vaults) {
  await checkVault(v.account_id, v.code_hash_base58, 'in the record');
  checkedVaults.add(v.account_id);
}

for (const c of [reg, usdc]) {
  const acc = await viewAccount(c.account_id);
  acc.code_hash === c.code_hash_base58
    ? pass(`${c.account_id} code hash ${acc.code_hash} matches the record`)
    : fail(`${c.account_id} code hash ${acc.code_hash}, record says ${c.code_hash_base58}`);
}

const wasmReg = (record.wasm && record.wasm.registry) || {};
wasmReg.on_chain_code_hash_base58 === reg.code_hash_base58
  ? pass(`record agrees with itself: wasm.registry and ${reg.account_id} both say ${reg.code_hash_base58}`)
  : fail(`record disagrees: wasm.registry says ${wasmReg.on_chain_code_hash_base58}, accounts say ${reg.code_hash_base58}`);
const embedded = await view(reg.account_id, 'get_vault_code_hash');
embedded === wasmVault.on_chain_sha256
  ? pass(`${reg.account_id} get_vault_code_hash ${embedded} matches wasm.vault in the record`)
  : fail(`${reg.account_id} get_vault_code_hash ${embedded}, record wasm.vault says ${wasmVault.on_chain_sha256}`);

for (const c of [reg, usdc, agent].filter(Boolean)) {
  const live = (await keyList(c.account_id)).map(liveKey).sort();
  const expected = (c.keys || []).map(recordedKey).sort();
  const same = live.length === expected.length && live.every((k, i) => k === expected[i]);
  if (same) {
    pass(`${c.account_id} access keys match the record: ${live.map((k) => k.split(' ').slice(1).join(' ')).join('; ') || 'none'}`);
  } else {
    for (const k of live) if (!expected.includes(k)) fail(`${c.account_id} has key not in the record: ${k}`);
    for (const k of expected) if (!live.includes(k)) fail(`${c.account_id} is missing recorded key: ${k}`);
  }
}

const tokens = [];
for (let from = 0; ; from += PAGE) {
  const page = await view(reg.account_id, 'nft_tokens', { from_index: String(from), limit: PAGE });
  if (!Array.isArray(page)) { fail(`nft_tokens from ${from} did not return a list`); break; }
  tokens.push(...page);
  if (page.length < PAGE) break;
}
pass(`nft_tokens read to the end: ${tokens.length} token(s)`);

for (const t of tokens) {
  let extra = {};
  try { extra = JSON.parse((t.metadata && t.metadata.extra) || '{}'); } catch { fail(`token ${t.token_id} metadata.extra is not JSON`); }
  const expected = `${t.token_id}.${reg.account_id}`;
  extra.vault === expected
    ? pass(`token ${t.token_id} vault ${extra.vault}, status ${extra.status}, earned ${extra.earned}, paid out ${extra.paid_out}`)
    : fail(`token ${t.token_id} names vault ${extra.vault}, expected ${expected}`);
  if (!(t.token_id in owners)) fail(`token ${t.token_id} owner ${t.owner_id}: token is not in token_owners in the record`);
  else if (owners[t.token_id] === t.owner_id) pass(`token ${t.token_id} owner ${t.owner_id}, as recorded`);
  else fail(`token ${t.token_id} owner ${t.owner_id}, record says ${owners[t.token_id]}`);
  if (!checkedVaults.has(expected)) {
    fail(`token ${t.token_id} vault ${expected} is not in the record`);
    await checkVault(expected, vaultCodeHash, 'found through the token, not in the record');
    checkedVaults.add(expected);
  }
}
for (const id of Object.keys(owners)) {
  if (!tokens.some((t) => t.token_id === id)) fail(`token ${id} is in token_owners but not returned by nft_tokens`);
}
for (const v of vaults) {
  if (!tokens.some((t) => `${t.token_id}.${reg.account_id}` === v.account_id)) fail(`vault ${v.account_id} is in the record but no token names it`);
}

const acct = await view(reg.account_id, 'get_accounting');
const counters = ['allowance_total', 'unallocated_total', 'escrow_total', 'undelivered_total', 'forwards_in_flight_total'];
const isCount = (x) => (typeof x === 'string' && /^\d+$/.test(x)) || (typeof x === 'number' && Number.isSafeInteger(x) && x >= 0);
const bad = counters.filter((k) => !acct || !isCount(acct[k]));
for (const k of bad) fail(`get_accounting ${k} is missing or not a nonnegative integer: ${JSON.stringify(acct ? acct[k] : undefined)}`);
if (bad.length === 0) {
  const tracked = counters.reduce((s, k) => s + BigInt(acct[k]), 0n);
  const balance = BigInt(await view(usdc.account_id, 'ft_balance_of', { account_id: reg.account_id }));
  tracked <= balance
    ? pass(`solvency: tracked counters ${tracked} <= registry mock USDC balance ${balance} (base units, 6 decimals)`)
    : fail(`solvency: tracked counters ${tracked} > registry mock USDC balance ${balance}`);
} else {
  fail('solvency not checked: accounting is incomplete');
}

console.log(`\n${failures === 0 ? 'ALL CHECKS PASSED' : `${failures} CHECK(S) FAILED`}`);
process.exit(failures === 0 ? 0 : 1);
