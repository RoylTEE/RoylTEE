// Tests for check-quiescent.mjs against a local fake RPC built from the record.
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
const ids = Object.keys(record.token_owners.owners);
const lock = { account: 'a.testnet', amount: '1', since_block: '1' };
const quiet = { inbound_pending: 0, payout_in_flight: null, transfer_pending: null };

async function run({ ledgers = {}, fif = '0', tokens = ids } = {}) {
  const server = createServer((req, res) => {
    let body = '';
    req.on('data', (c) => { body += c; });
    req.on('end', () => {
      const p = JSON.parse(body).params;
      const args = JSON.parse(Buffer.from(p.args_base64, 'base64').toString('utf8'));
      let out;
      if (p.method_name === 'nft_tokens') out = tokens.slice(Number(args.from_index), Number(args.from_index) + args.limit).map((token_id) => ({ token_id }));
      else if (p.method_name === 'get_ledger') out = { ...quiet, ...(ledgers[args.token_id] || {}) };
      else if (p.method_name === 'get_accounting') out = { forwards_in_flight_total: fif };
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ jsonrpc: '2.0', id: 'quiet', result: { result: [...Buffer.from(JSON.stringify(out))] } }));
    });
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const url = `http://127.0.0.1:${server.address().port}`;
  const out = await new Promise((resolve) => {
    const child = spawn(process.execPath, [join(here, 'check-quiescent.mjs')], { env: { ...process.env, NEAR_RPC_URL: url } });
    let text = '';
    child.stdout.on('data', (c) => { text += c; });
    child.stderr.on('data', (c) => { text += c; });
    child.on('close', (code) => resolve({ code, text }));
  });
  server.close();
  return out;
}

test('quiet registry passes', async () => {
  const r = await run();
  assert.equal(r.code, 0, r.text);
  assert.match(r.text, /SAFE TO REDEPLOY/);
});

test('a pending forward blocks the deploy', async () => {
  const r = await run({ ledgers: { [ids[0]]: { inbound_pending: 1 } } });
  assert.equal(r.code, 1);
  assert.match(r.text, new RegExp(`token ${ids[0]} is not quiet: inbound_pending 1`));
});

test('a payout lock or a transfer lock blocks the deploy', async () => {
  const r = await run({ ledgers: { [ids[0]]: { payout_in_flight: lock }, [ids[1]]: { transfer_pending: lock } } });
  assert.equal(r.code, 1);
  assert.match(r.text, /payout lock set/);
  assert.match(r.text, /transfer lock set/);
});

test('forwards_in_flight_total above zero blocks the deploy', async () => {
  const r = await run({ fif: '80000' });
  assert.equal(r.code, 1);
  assert.match(r.text, /forwards_in_flight_total is "80000"/);
});

test('reads tokens past the first page of 100', async () => {
  const many = [...ids, ...Array.from({ length: 120 }, (_, i) => `x${i}`)];
  const r = await run({ tokens: many, ledgers: { x119: { inbound_pending: 2 } } });
  assert.equal(r.code, 1);
  assert.match(r.text, /token x119 is not quiet/);
});
