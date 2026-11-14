#!/usr/bin/env node
// Writes src/data/auction.json, the part of deployments/auction-testnet.json and
// deployments/auction-settle-verify.txt that the /auction page shows.
// The site can be built from this folder alone, so the page reads this copy at build time.
// A test (tests/auction.test.ts) fails when the copy differs from the records.
//   node scripts/sync-auction.mjs          writes the copy
//   node scripts/sync-auction.mjs --check  exits 1 when the copy is out of date
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const SOURCE = new URL('../../deployments/auction-testnet.json', import.meta.url);
export const VERIFY = new URL('../../deployments/auction-settle-verify.txt', import.meta.url);
export const TARGET = new URL('../src/data/auction.json', import.meta.url);

/** Pure: the five checks and the task facts, read from the outlayer-verify output text. */
export function parseVerify(text) {
  const checks = [...text.matchAll(/^\s*\[ (PASS|FAIL)\s*\] (\w+)\s+(.*)$/gm)].map((m) => ({ name: m[2], verdict: m[1], question: m[3].trim() }));
  const one = (re) => { const m = text.match(re); return m ? m[1].trim() : null; };
  return {
    checks,
    proven: /^\s*PROVEN\s/m.test(text),
    taskId: one(/^\s*task id\s+(\d+)/m),
    executedAt: one(/^\s*task type\s+execute\s*\n\s*executed at\s+(\S+)/m) ?? one(/^\s*executed at\s+(\S+)/m),
    wasmSha256: one(/^\s*wasm sha256\s+([0-9a-f]{64})/m),
    inputSha256: one(/^\s*input sha256\s+([0-9a-f]{64})/m),
    outputSha256: one(/^\s*output sha256\s+([0-9a-f]{64})/m),
    tcbStatus: one(/^\s*Intel TCB status\s+(\S+)/m),
    command: one(/^\s*reproduce\s+(outlayer-verify tx \S+)/m),
  };
}

/** Pure: the copy for one parsed auction record and the verifier output text. */
export function buildAuction(record, verifyText) {
  const byStep = (step) => record.transactions.filter((t) => t.step === step || t.step.startsWith(`${step} `));
  const tx = (step) => byStep(step).map((t) => ({ hash: t.transaction_hash, signer: t.signer, at: t.block_timestamp_utc, success: t.success }));
  const a = record.auction_0;
  return {
    writtenFrom: 'deployments/auction-testnet.json',
    recordWrittenAtUtc: record.record.written_at_utc,
    contract: { account: record.contract.account, codeSha256: record.contract.code_sha256, codeHashBase58: record.contract.code_hash_base58, secretsAccount: record.contract.config_read_back.secrets_account, keyCount: record.contract.key_count },
    wasi: { sha256: record.wasi_module.sha256, buildTarget: record.wasi_module.build_target },
    accessRule: record.outlayer.access_rule,
    verifier: { tool: record.record.outlayer_verify.tool, rev: record.record.outlayer_verify.rev, evidenceFile: record.record.outlayer_verify.evidence_file },
    auction: {
      id: 0, token: a.token_id, seller: a.seller, reserve: a.reserve, maxDeposit: a.max_deposit, endsAtUtc: a.ends_at_utc,
      status: a.status, winner: a.winner, price: a.price, tokenOwnerAfter: a.token_owner_after,
      listedTx: a.listed_in, settledTx: a.settled_in, withdrawnTx: a.withdrawn_in,
      bids: a.bids.map((b) => ({ bidder: b.bidder, tx: b.tx, deposit: b.deposit })),
      events: a.events,
    },
    times: Object.fromEntries(['17i', '17j', '17k', '17m', '17g'].map((s) => [s, tx(`auction ${s}`)])),
    outlayerRequests: record.outlayer.requests.map((r) => ({ step: r.step, requestId: r.request_id, task: r.task ?? null, chargedNear: r.charged_near, refundedNear: r.refunded_near ?? null })),
    balancesAfter: Object.fromEntries(Object.entries(record.musdc_balances_after_17m).filter(([k]) => k !== 'note')),
    accountingAfter: { owedTotal: record.accounting_after_17m.owed_total, pendingTotal: record.accounting_after_17m.pending_total, escrowTotal: record.accounting_after_17m.escrow_total, openAuctions: record.accounting_after_17m.open_auctions },
    verify: parseVerify(verifyText),
  };
}

const text = (o) => JSON.stringify(o, null, 2) + '\n';
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const next = text(buildAuction(JSON.parse(readFileSync(SOURCE, 'utf8')), readFileSync(VERIFY, 'utf8')));
  if (process.argv.includes('--check')) {
    const same = readFileSync(TARGET, 'utf8') === next;
    console.log(same ? 'auction.json is current' : 'auction.json is OUT OF DATE: run node scripts/sync-auction.mjs');
    process.exit(same ? 0 : 1);
  }
  writeFileSync(TARGET, next);
  console.log('wrote src/data/auction.json');
}
