// Tests for verify-testnet.mjs against a local fake RPC built from the record.
// Run: npm run test:verify (no network, no keys).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const record = JSON.parse(readFileSync(join(here, '..', 'deployments', 'testnet.json'), 'utf8'));
const accounts = Object.values(record.accounts);
const reg = accounts.find((a) => a.variable === 'REG');
const owners = record.token_owners.owners;

function liveKeys(a) {
  return (a.keys || []).map((k) => ({
    public_key: k.public_key,
    access_key: { nonce: 1, permission: k.permission === 'full_access' ? 'FullAccess' : { FunctionCall: { allowance: '1', receiver_id: k.contract, method_names: k.methods } } },
  }));
}

function chain(change = {}) {
  const tokenIds = change.tokens || Object.keys(owners);
  const tokens = tokenIds.map((id) => ({ token_id: id, owner_id: owners[id] || 'someone.testnet', metadata: { extra: JSON.stringify({ vault: `${id}.${reg.account_id}`, status: 'active', earned: '0', paid_out: '0' }) } }));
  const vaultHash = accounts.find((a) => a.variable === 'S1').code_hash_base58;
  return {
    keys: (id) => (change.keys && change.keys[id]) || liveKeys(record.accounts[id] || { keys: [] }),
    account: (id) => ({ code_hash: (record.accounts[id] && record.accounts[id].code_hash_base58) || vaultHash }),
    nft_tokens: ({ from_index, limit }) => tokens.slice(Number(from_index), Number(from_index) + limit),
    get_accounting: () => change.accounting || { allowance_total: '0', unallocated_total: '0', escrow_total: '400000', undelivered_total: '0', forwards_in_flight_total: '0' },
    ft_balance_of: () => '400000',
  };
}

async function run(state) {
  const server = createServer((req, res) => {
    let body = '';
    req.on('data', (c) => { body += c; });
    req.on('end', () => {
      const p = JSON.parse(body).params;
      let result;
      if (p.request_type === 'view_access_key_list') result = { keys: state.keys(p.account_id) };
      else if (p.request_type === 'view_account') result = state.account(p.account_id);
      else {
        const args = JSON.parse(Buffer.from(p.args_base64, 'base64').toString('utf8'));
        result = { result: [...Buffer.from(JSON.stringify(state[p.method_name](args)))] };
      }
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ jsonrpc: '2.0', id: 'verify', result }));
    });
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const url = `http://127.0.0.1:${server.address().port}`;
  const out = await new Promise((resolve) => {
    const child = spawn(process.execPath, [join(here, 'verify-testnet.mjs')], { env: { ...process.env, NEAR_RPC_URL: url } });
    let text = '';
    child.stdout.on('data', (c) => { text += c; });
    child.stderr.on('data', (c) => { text += c; });
    child.on('close', (code) => resolve({ code, text }));
  });
  server.close();
  return out;
}

test('passes on a chain that matches the record', async () => {
  const r = await run(chain());
  assert.equal(r.code, 0, r.text);
  assert.match(r.text, /ALL CHECKS PASSED/);
});

test('fails on a token that is not in the record, and checks its vault', async () => {
  const r = await run(chain({ tokens: [...Object.keys(owners), 's8'] }));
  assert.equal(r.code, 1);
  assert.match(r.text, /token s8 .*not in token_owners/);
  assert.match(r.text, /s8\.reg\.royltee-dev\.testnet has 0 access keys \(found through the token/);
});

test('reads tokens past the first page of 100', async () => {
  const many = [...Object.keys(owners), ...Array.from({ length: 120 }, (_, i) => `x${i}`)];
  const r = await run(chain({ tokens: many }));
  assert.equal(r.code, 1);
  assert.match(r.text, /nft_tokens read to the end: 127 token/);
  assert.match(r.text, /token x119 /);
});

test('fails when a recorded token is missing', async () => {
  const r = await run(chain({ tokens: Object.keys(owners).filter((t) => t !== 's7') }));
  assert.equal(r.code, 1);
  assert.match(r.text, /token s7 is in token_owners but not returned/);
});

test('fails when accounting is empty instead of treating it as zero', async () => {
  const r = await run(chain({ accounting: {} }));
  assert.equal(r.code, 1);
  assert.match(r.text, /allowance_total is missing/);
  assert.match(r.text, /solvency not checked/);
});

test('fails on a negative or non integer counter', async () => {
  const r = await run(chain({ accounting: { allowance_total: '-1', unallocated_total: '1.5', escrow_total: '0', undelivered_total: '0', forwards_in_flight_total: '0' } }));
  assert.equal(r.code, 1);
  assert.match(r.text, /allowance_total is missing or not a nonnegative integer/);
  assert.match(r.text, /unallocated_total is missing or not a nonnegative integer/);
});

test('fails when a full access key is replaced by a function call key with the same public key', async () => {
  const k = reg.keys[0].public_key;
  const r = await run(chain({ keys: { [reg.account_id]: [{ public_key: k, access_key: { nonce: 1, permission: { FunctionCall: { allowance: '1', receiver_id: reg.account_id, method_names: [] } } } }] } }));
  assert.equal(r.code, 1);
  assert.match(r.text, /reg\.royltee-dev\.testnet is missing recorded key: .* full_access/);
});

test('fails when the agent key gains a method', async () => {
  const a = accounts.find((x) => x.variable === 'AGENT');
  const k = a.keys[0];
  const r = await run(chain({ keys: { [a.account_id]: [{ public_key: k.public_key, access_key: { nonce: 1, permission: { FunctionCall: { allowance: '1', receiver_id: k.contract, method_names: [...k.methods, 'claim'] } } } }] } }));
  assert.equal(r.code, 1);
  assert.match(r.text, /pay-agent\.buyer-pub\.royltee-dev\.testnet has key not in the record/);
});
