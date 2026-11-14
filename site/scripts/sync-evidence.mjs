#!/usr/bin/env node
// Writes src/data/evidence.json, the part of deployments/testnet.json that the /verify page shows
// (transaction outcomes, recorded token owners, the payment agent key, code hashes).
// The site can be built from this folder alone, so the page reads this copy at build time.
// A test (tests/claims.test.ts) fails when the copy differs from deployments/testnet.json.
//   node scripts/sync-evidence.mjs          writes the copy
//   node scripts/sync-evidence.mjs --check  exits 1 when the copy is out of date
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const SOURCE = new URL('../../deployments/testnet.json', import.meta.url);
export const TARGET = new URL('../src/data/evidence.json', import.meta.url);

/** Pure: the evidence copy for one parsed deployments/testnet.json. */
export function buildEvidence(record) {
  const txs = {};
  for (const t of record.transactions) {
    if (!t.transaction_hash) continue;
    txs[t.transaction_hash] = {
      step: String(t.step), method: t.method, success: t.success,
      ...(t.success ? {} : { error: t.error ?? null }),
      at: t.block_timestamp_utc ?? null,
    };
  }
  const agent = record.accounts['pay-agent.buyer-pub.royltee-dev.testnet'];
  const vaults = Object.values(record.accounts).filter((a) => /^S\d+$/.test(a.variable))
    .map((a) => ({ account: a.account_id, codeHash: a.code_hash_base58 }));
  return {
    writtenFrom: 'deployments/testnet.json',
    recordWrittenAtUtc: record.record.written_at_utc,
    txs,
    tokenOwners: record.token_owners.owners,
    vaults,
    agentKeys: agent.keys.map((k) => ({ permission: k.permission, contract: k.contract ?? null, methods: k.methods ?? [] })),
  };
}

const text = (o) => JSON.stringify(o, null, 2) + '\n';
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const next = text(buildEvidence(JSON.parse(readFileSync(SOURCE, 'utf8'))));
  if (process.argv.includes('--check')) {
    const same = readFileSync(TARGET, 'utf8') === next;
    console.log(same ? 'evidence.json is current' : 'evidence.json is OUT OF DATE: run node scripts/sync-evidence.mjs');
    process.exit(same ? 0 : 1);
  }
  writeFileSync(TARGET, next);
  console.log('wrote src/data/evidence.json');
}
