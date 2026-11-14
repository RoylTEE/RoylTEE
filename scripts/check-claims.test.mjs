import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkClaims, rows, section } from './check-claims.mjs';

const record = { transactions: [{ transaction_hash: 'TXOK1' }], accounts: { a: { account_id: 's1.reg.example.testnet' } } };
const src = '#[test]\nfn good_test() {}\n#[tokio::test]\nasync fn sandbox_test() {}\nfn helper() {}\n';
const readme = (claimRow, attackRow) => `## Every claim, checked\n\nintro\n\n| Claim | Code | Test | On-chain evidence |\n|---|---|---|---|\n${claimRow}\n\n## Try to break it\n\n| Attack | What the contract does | Test | Proof |\n|---|---|---|---|\n${attackRow}\n\n## Next\n`;
const run = (claimRow, attackRow) => checkClaims({ readme: readme(claimRow, attackRow), testSource: src, record, readSource: (p) => (p === 'a.rs' ? 'fn real_fn() {}' : null) });
const goodClaim = '| c | [`a.rs`](a.rs) `real_fn` | `good_test` | [`TXOK1...`](https://testnet.nearblocks.io/txns/TXOK1) x<br>[`s1.reg.example.testnet`](https://testnet.nearblocks.io/address/s1.reg.example.testnet) y |';
const goodAttack = '| a | r | `sandbox_test` | test only |';

test('a correct table passes and counts what it checked', () => {
  const r = run(goodClaim, goodAttack);
  assert.deepEqual(r.problems, []);
  assert.deepEqual(r.counts, { tests: 2, txs: 1, codeRefs: 1, accounts: 1 });
});
test('a made-up test name, a plain helper function and a missing function all fail', () => {
  assert.match(run(goodClaim.replace('good_test', 'made_up'), goodAttack).problems[0], /test not found: made_up/);
  assert.match(run(goodClaim, goodAttack.replace('sandbox_test', 'helper')).problems[0], /test not found: helper/);
  assert.match(run(goodClaim.replace('real_fn', 'nope'), goodAttack).problems[0], /function nope not in a.rs/);
  assert.match(run(goodClaim.replace('a.rs`](a.rs)', 'b.rs`](b.rs)'), goodAttack).problems[0], /code file not found: b.rs/);
});
test('a transaction or account that is not in the record fails', () => {
  assert.match(run(goodClaim.replaceAll('TXOK1', 'TXBAD'), goodAttack).problems[0], /transaction not in record: TXBAD/);
  assert.match(run(goodClaim.replaceAll('s1.reg.example.testnet', 's9.reg.example.testnet'), goodAttack).problems[0], /account not in record/);
  assert.match(run(goodClaim, '| a | r | `sandbox_test` | [`TXBAD...`](https://testnet.nearblocks.io/txns/TXBAD) refused |').problems[0], /TXBAD/);
});
test('pending rows are skipped for tests and a missing section is reported', () => {
  assert.deepEqual(run('| c | code | pending deploy | pending deploy |', '| a | r | pending deploy | pending deploy |').problems, []);
  assert.match(checkClaims({ readme: '# x\n', testSource: src, record, readSource: () => null }).problems[0], /section/);
  assert.equal(rows(section(readme(goodClaim, goodAttack), 'Every claim, checked')).length, 1);
});
