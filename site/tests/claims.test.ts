import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { CLAIMS, ATTACKS, TXS, TOKEN_OWNERS, AGENT_KEYS, txOutcome } from '../src/data/claims';
import evidence from '../src/data/evidence.json';
import { evaluate, type Facts } from '../src/scripts/verify-claims';

const siteRoot = new URL('..', import.meta.url).pathname;
const repoRoot = join(siteRoot, '..');
const recordUrl = new URL('../../deployments/testnet.json', import.meta.url);
const haveRepo = existsSync(recordUrl) && existsSync(join(repoRoot, 'contracts'));
const walk = (d: string): string[] => readdirSync(d).flatMap((f) => (f === 'target' || f === 'node_modules' || f === '.git') ? [] : statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]);

describe('claims data: tests, code and transactions are real', () => {
  const sources = haveRepo ? [...walk(join(repoRoot, 'contracts')).filter((f) => f.endsWith('.rs')), ...walk(join(siteRoot, 'tests')).filter((f) => f.endsWith('.ts'))] : [];
  const text = new Map(sources.map((f) => [f, readFileSync(f, 'utf8')]));
  const allText = [...text.values()].join('\n');
  const names = [...new Set([...CLAIMS.flatMap((c) => c.tests), ...ATTACKS.flatMap((a) => a.tests)])];

  it('every named test exists as a Rust test function or a site test', () => {
    if (!haveRepo) return;
    expect(names.length).toBeGreaterThan(20);
    for (const n of names) expect(new RegExp(`fn ${n}\\b`).test(allText) || allText.includes(`'${n}'`), n).toBe(true);
  });
  it('every named test is a test: a #[test] or #[tokio::test] function', () => {
    if (!haveRepo) return;
    for (const n of names) expect(new RegExp(`#\\[(tokio::)?test\\][^\\n]*\\n(\\s*#\\[[^\\n]*\\]\\n)*\\s*(pub )?(async )?fn ${n}\\b`).test(allText), n).toBe(true);
  });
  it('every code reference names a real file and a function in it', () => {
    if (!haveRepo) return;
    for (const c of CLAIMS) for (const r of c.code) {
      const f = join(repoRoot, r.file);
      if (!existsSync(f) && r.file.startsWith('contracts/auction/')) continue; // the auction contract lives on branch feat/tee-auction until merged
      expect(existsSync(f), r.file).toBe(true);
      expect(new RegExp(`fn ${r.fn}\\b`).test(readFileSync(f, 'utf8')), `${r.file} ${r.fn}`).toBe(true);
    }
  });
  it('every transaction and account shown is in deployments/testnet.json', () => {
    if (!haveRepo) return;
    const rec = JSON.parse(readFileSync(recordUrl, 'utf8'));
    const hashes = new Set<string>(rec.transactions.map((t: { transaction_hash?: string }) => t.transaction_hash).filter(Boolean));
    const accounts = new Set<string>(Object.values(rec.accounts).map((a) => (a as { account_id: string }).account_id));
    const shown = [...CLAIMS.flatMap((c) => c.evidence), ...ATTACKS.flatMap((a) => [...(a.tx ? [a.tx] : []), ...(a.more ?? [])]).map((tx) => ({ tx }))];
    expect(shown.length).toBeGreaterThan(25);
    for (const e of shown) {
      if ('tx' in e) { expect(hashes.has(e.tx), e.tx).toBe(true); expect(TXS[e.tx], e.tx).toBeDefined(); }
      else expect(accounts.has(e.account), e.account).toBe(true);
    }
  });
  it('the build-time copy equals what deployments/testnet.json gives now', async () => {
    if (!haveRepo) return;
    // @ts-ignore the script is plain JavaScript
    const { buildEvidence } = await import('../scripts/sync-evidence.mjs');
    expect(buildEvidence(JSON.parse(readFileSync(recordUrl, 'utf8')))).toEqual(evidence);
  });
  it('a refused row points at a failed transaction and a succeeded row at a successful one', () => {
    for (const a of ATTACKS) for (const t of [...(a.tx ? [a.tx] : []), ...(a.more ?? [])]) expect(TXS[t].success, a.id).toBe(a.status !== 'refused');
    for (const c of CLAIMS) for (const e of c.evidence) if ('tx' in e) {
      const refused = /refus/.test(e.what);
      expect(TXS[e.tx].success, `${c.id} ${e.tx}`).toBe(!refused);
    }
    expect(txOutcome('7wxZ7Bmstzu9qTUayrvzbGP97tcD6GRLMaZo7pTVAqzh')).toBe('register_share, succeeded');
    expect(txOutcome('GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB')).toBe('claim, refused: only the current holder can claim');
  });
  it('no row waits for a deploy now; recorded rows hold transactions and no live check, live rows hold tests, evidence and a check', () => {
    expect(CLAIMS.filter((c) => c.status === 'pending')).toEqual([]);
    const recorded = CLAIMS.filter((c) => c.status === 'recorded');
    expect(recorded.map((c) => c.id)).toEqual(['registry-redeploy', 'split-sheet', 'tee-auction']);
    for (const c of recorded) { expect(c.evidence.some((e) => 'tx' in e), c.id).toBe(true); expect(c.check).toBeUndefined(); }
    for (const c of CLAIMS.filter((x) => x.status === 'live')) { expect(c.tests.length, c.id).toBeGreaterThan(0); expect(c.evidence.length, c.id).toBeGreaterThan(0); expect(c.check, c.id).toBeTruthy(); }
    expect(ATTACKS.filter((a) => a.status === 'pending')).toEqual([]);
    const outsider = ATTACKS.find((a) => a.id === 'auction-outsider')!;
    expect([outsider.tx, ...(outsider.more ?? [])]).toEqual(['EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM', '57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP']);
  });
  it('the README tables show every test name and transaction the page shows', () => {
    const readme = join(repoRoot, 'README.md');
    if (!haveRepo || !existsSync(readme)) return;
    const text = readFileSync(readme, 'utf8');
    for (const n of names) expect(text.includes(`\`${n}\``), n).toBe(true);
    for (const e of [...CLAIMS.flatMap((c) => c.evidence), ...ATTACKS.flatMap((a) => [...(a.tx ? [a.tx] : []), ...(a.more ?? [])]).map((tx) => ({ tx }))]) if ('tx' in e) expect(text.includes(`txns/${e.tx}`), e.tx).toBe(true);
  });
  it('uses plain words: no em or en dashes in any text', () => {
    const all = JSON.stringify([CLAIMS, ATTACKS]);
    expect(all).not.toMatch(/[\u2013\u2014]/);
  });
  it('the record keeps the recorded owner of s2 and the single function call key of the agent', () => {
    expect(TOKEN_OWNERS.s2).toBe('buyer-pub.royltee-dev.testnet');
    expect(TOKEN_OWNERS.s10).toBe('buyer-pub.royltee-dev.testnet');
    expect(AGENT_KEYS).toEqual([{ permission: 'function_call', contract: 'reg.royltee-dev.testnet', methods: ['pay_from_allowance', 'report'] }]);
  });
});

describe('verify page live checks', () => {
  const REG = 'reg.royltee-dev.testnet';
  const good = (): Facts => ({
    vaultKeyCounts: Array(15).fill(0),
    ledgerS1: { total_routed_in: '1000000' }, ledgerS2: { total_routed_in: '1000000' },
    accounting: { allowance_total: '200000', unallocated_total: '0', escrow_total: '0', undelivered_total: '0', forwards_in_flight_total: '0' },
    registryMusdc: '200000', s2Owner: 'buyer-pub.royltee-dev.testnet', recordedS2Owner: 'buyer-pub.royltee-dev.testnet',
    agentKeys: [{ permission: { FunctionCall: { receiver_id: REG, method_names: ['report', 'pay_from_allowance'] } } }], registry: REG,
    recordedAgentMethods: ['pay_from_allowance', 'report'],
    role: { methods: ['pay_from_allowance', 'report'], per_tx_limit: '100000', revoked: false },
    dispute0: { status: { resolved: { outcome: 'uphold_existing' } } }, outsiderActive: false, memberActive: true,
    usdcMeta: { name: 'Mock USDC (testnet)', symbol: 'mUSDC', decimals: 6 },
  });
  it('passes on the recorded state', () => {
    for (const [id, r] of Object.entries(evaluate(good()))) expect(r.ok, `${id}: ${r.detail}`).toBe(true);
  });
  it('fails each claim when the chain differs', () => {
    const f = good();
    expect(evaluate({ ...f, vaultKeyCounts: [0, 1, 0] })['vault-keys'].ok).toBe(false);
    expect(evaluate({ ...f, vaultKeyCounts: [] })['vault-keys'].ok).toBe(false);
    expect(evaluate({ ...f, ledgerS2: { total_routed_in: '400000' } }).split.ok).toBe(false);
    expect(evaluate({ ...f, registryMusdc: '199999' }).split.ok).toBe(false);
    expect(evaluate({ ...f, s2Owner: 'writer-b.royltee-dev.testnet' })['owner-s2'].ok).toBe(false);
    expect(evaluate({ ...f, agentKeys: [...f.agentKeys, { permission: 'FullAccess' }] })['agent-key'].ok).toBe(false);
    expect(evaluate({ ...f, agentKeys: [{ permission: 'FullAccess' }] })['agent-key'].ok).toBe(false);
    expect(evaluate({ ...f, agentKeys: [{ permission: { FunctionCall: { receiver_id: REG, method_names: ['pay_from_allowance', 'report', 'claim'] } } }] })['agent-key'].ok).toBe(false);
    expect(evaluate({ ...f, role: null })['agent-limits'].ok).toBe(false);
    expect(evaluate({ ...f, role: { ...f.role!, per_tx_limit: '1000000' } })['agent-limits'].ok).toBe(false);
    expect(evaluate({ ...f, dispute0: { status: 'open' } }).dispute.ok).toBe(false);
    expect(evaluate({ ...f, accounting: { ...f.accounting, escrow_total: '5' } }).dispute.ok).toBe(false);
    expect(evaluate({ ...f, outsiderActive: true }).credential.ok).toBe(false);
    expect(evaluate({ ...f, usdcMeta: { name: 'USD Coin', symbol: 'USDC', decimals: 6 } }).usdc.ok).toBe(false);
  });
});
