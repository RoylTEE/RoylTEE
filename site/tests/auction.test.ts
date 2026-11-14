import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { AUCTION, STAGES, TIMELINE, WHAT_TEE, musdc, utc, EVIDENCE_FILE } from '../src/data/auction';
import { AREAS } from '../src/data/ia';
import { forbidden } from '../scripts/wording.mjs';
// @ts-ignore the script is plain JavaScript
import { buildAuction, parseVerify } from '../scripts/sync-auction.mjs';

const root = join(__dirname, '..');
const repo = join(root, '..');
const haveRepo = existsSync(join(repo, 'deployments', 'auction-testnet.json'));
const page = readFileSync(join(root, 'src/pages/auction.astro'), 'utf8');

describe('auction page data', () => {
  it('the build time copy equals the records', () => {
    if (!haveRepo) return;
    const rec = JSON.parse(readFileSync(join(repo, 'deployments/auction-testnet.json'), 'utf8'));
    const txt = readFileSync(join(repo, 'deployments/auction-settle-verify.txt'), 'utf8');
    expect(buildAuction(rec, txt)).toEqual(AUCTION);
  });
  it('the recorded sale is the one the README tells: s10, writer-a, buyer-pub, 70 mUSDC', () => {
    const a = AUCTION.auction;
    expect(a.token).toBe('s10');
    expect(a.seller).toBe('writer-a.royltee-dev.testnet');
    expect(a.winner).toBe('buyer-pub.royltee-dev.testnet');
    expect(a.tokenOwnerAfter).toBe(a.winner);
    expect(musdc(a.price)).toBe('70');
    expect(musdc(a.reserve)).toBe('20');
    expect(musdc(a.maxDeposit)).toBe('100');
    expect(a.endsAtUtc).toBe('2026-11-14T10:38:50Z');
    expect(a.status).toBe('sold');
    expect(a.bids).toHaveLength(2);
  });
  it('shows the five attestation checks as PASS, the module hash and the task from the evidence file', () => {
    const v = AUCTION.verify;
    expect(v.checks.map((c: { name: string }) => c.name)).toEqual(['Authenticity', 'Identity', 'Binding', 'Input', 'Output']);
    expect(v.checks.every((c: { verdict: string }) => c.verdict === 'PASS')).toBe(true);
    expect(v.proven).toBe(true);
    expect(v.wasmSha256).toBe('c4a29e55ca61da2e069bdcfebd4d4e6690820620582ed2635d1dd6bc17c2594e');
    expect(v.taskId).toBe('106286');
    expect(AUCTION.wasi.sha256).toBe(v.wasmSha256);
    expect(AUCTION.accessRule).toContain(`WasmHash(${v.wasmSha256})`);
    expect(EVIDENCE_FILE).toBe('deployments/auction-settle-verify.txt');
  });
  it('the verifier parser does not report a pass for a failed or missing check', () => {
    expect(parseVerify('  [ FAIL     ] Identity  is this code approved on chain?\n').checks[0].verdict).toBe('FAIL');
    expect(parseVerify('nothing here').checks).toEqual([]);
    expect(parseVerify('nothing here').proven).toBe(false);
  });
  it('every stage has a transaction that is in the evidence copy of the deploy record, and the refusals are the two outsider calls', () => {
    const evidence = JSON.parse(readFileSync(join(root, 'src/data/evidence.json'), 'utf8'));
    expect(TIMELINE).toEqual(['refused', 'listed', 'sealed', 'settled', 'withdrawn']);
    expect(new Set(STAGES.map((s) => s.key))).toEqual(new Set(TIMELINE));
    for (const s of STAGES) {
      expect(s.txs.length, s.key).toBeGreaterThan(0);
      for (const t of s.txs) expect(evidence.txs[t.hash], `${s.key} ${t.hash}`).toBeTruthy();
    }
    expect(STAGES.find((s) => s.key === 'refused')!.txs.map((t) => t.hash)).toEqual(['EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM', '57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP']);
    expect(STAGES.find((s) => s.key === 'settled')!.txs[0].hash).toBe('CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW');
  });
  it('no bid amount is shown on the page, only the price', () => {
    const text = STAGES.flatMap((s) => s.lines).join(' ') + page;
    expect(text).not.toMatch(/\b90\b/);
    expect(STAGES.find((s) => s.key === 'sealed')!.lines.join(' ')).toContain('ciphertext');
  });
  it('withdraw numbers add up: seller 70, loser 100 back, winner 30 change', () => {
    const text = STAGES.find((s) => s.key === 'withdrawn')!.lines.join(' ');
    expect(text).toContain('was paid 70 mUSDC');
    expect(text).toContain('full 100 mUSDC deposit back');
    expect(text).toContain('30 mUSDC change');
    expect(AUCTION.accountingAfter).toEqual({ owedTotal: '0', pendingTotal: '0', escrowTotal: '0', openAuctions: 0 });
  });
  it('time and amount formatting', () => {
    expect(utc('2026-11-14T10:18:51.696Z')).toBe('2026-11-14 10:18 UTC');
    expect(musdc('70500000')).toBe('70.5');
    expect(musdc('1')).toBe('0.000001');
    expect(musdc('0')).toBe('0');
  });
  it('the page says Simulated, the TEE box lists what is inside and outside, and the page links its evidence', () => {
    expect(page).toContain('Simulated');
    expect(page).toContain('What runs in the TEE and what does not');
    expect(page).toContain('Attestation proof');
    expect(WHAT_TEE.inside.length).toBeGreaterThan(1);
    expect(WHAT_TEE.outside.join(' ')).toContain('Fairness');
    expect(page).toContain('EVIDENCE_FILE');
  });
  it('the Registry menu has the Auction area and the pages link to it', () => {
    const a = AREAS.find((x) => x.key === 'auction')!;
    expect(a.group).toBe('Registry');
    expect(a.href).toBe('/auction');
    for (const f of ['src/components/landing/Proof.astro', 'src/pages/dashboard.astro', 'src/pages/verify.astro']) expect(readFileSync(join(root, f), 'utf8'), f).toContain('href="/auction"');
  });
  it('page and data pass the wording check', () => {
    for (const f of ['src/pages/auction.astro', 'src/data/auction.ts', 'src/data/auction.json']) expect(forbidden(readFileSync(join(root, f), 'utf8')), f).toEqual([]);
  });
});
