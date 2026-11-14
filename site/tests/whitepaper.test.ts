import { readFileSync, existsSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { DEPLOY_RECORD } from '../src/data/testnet';
import { AUCTION } from '../src/data/auction';
import { AREAS } from '../src/data/ia';
import { forbidden } from '../scripts/wording.mjs';

const root = join(__dirname, '..');
const repo = join(root, '..');
const read = (f: string) => readFileSync(join(root, f), 'utf8');
const paper = read('src/data/whitepaper.md');
const haveDocs = existsSync(join(repo, 'docs', 'WHITEPAPER.md'));

describe('whitepaper 1.0', () => {
  it('the site copy equals docs/WHITEPAPER.md', () => {
    if (!haveDocs) return;
    expect(paper).toBe(readFileSync(join(repo, 'docs', 'WHITEPAPER.md'), 'utf8'));
  });
  it('is version 1.0 dated 14 November 2026, in UTC terms, and says testnet only', () => {
    expect(paper).toContain('**Version 1.0, 14 November 2026.**');
    expect(paper).toContain('Testnet only');
    expect(paper).not.toMatch(/Draft 0\.1|13 November 2026/);
  });
  it('has no dash characters, no banned words and passes the wording check', () => {
    expect(paper).not.toMatch(/[\u2013\u2014]/);
    expect(forbidden(paper)).toEqual([]);
    expect(paper).not.toMatch(/\/home\/|\/tmp\/|\/workspace|royltee-near-credentials/);
    expect(paper).not.toMatch(new RegExp(['Bang', 'kok'].join('') + '|UTC' + '\\+7'));
  });
  it('covers the sections the brief asks for', () => {
    for (const h of ['## 4 Lifecycle', '## 6 The sealed-bid auction and the TEE', '## 8 Trust model, keys and the path to mainnet', '## 10 What is Simulated, what is UNVERIFIED', '## 11 Limitations and open questions', '## 12 Roadmap', '### 8.2 Keys on testnet, by decision', '### 8.6 The path to mainnet']) expect(paper, h).toContain(h);
  });
  it('its numbers are the recorded ones', () => {
    expect(paper).toContain(DEPLOY_RECORD.registry.codeHash);
    expect(paper).toContain(DEPLOY_RECORD.registry.sha256);
    expect(paper).toContain(AUCTION.verify.wasmSha256);
    expect(paper).toContain('task 106286');
    expect(paper).toContain('request 44422');
    expect(paper).toContain('15 share vaults');
    expect(paper).toContain('232 Rust unit tests (155 registry, 5 vault, 4 Mock USDC, 68 auction)');
    expect(paper).toContain('54 sandbox tests, 14 WASI host tests and 18 Node tests');
    expect(paper).toContain('72 checks');
    expect(paper).toContain('5 of its checks');
    expect(paper).toContain('70 mUSDC');
    expect(paper).toContain('10:38:50 UTC');
    expect(paper).toContain('git revision `90c27d9`');
  });
  it('keeps the honest labels: keys kept on testnet, TEE only in the auction, Simulated and UNVERIFIED', () => {
    expect(paper).toContain('One full-access key, kept on purpose.');
    expect(paper).toContain('Delete the registry and token full-access keys after the final deploy');
    expect(paper.toLowerCase()).toContain('multisig or a dao');
    expect(paper).toContain('do not use a TEE. Only the auction settle step does.');
    expect(paper).toContain('Not built: TEE attested payer statements');
    expect(paper).toMatch(/UNVERIFIED/);
    expect(paper).toContain('Whether the custom domain royltee.app serves the site.');
  });
  it('the page renders the copy, offers the PDF, and no menu label calls it a placeholder', () => {
    const page = read('src/pages/whitepaper.astro');
    expect(page).toContain("from '../data/whitepaper.md'");
    expect(page).toContain('href="/royltee-whitepaper.pdf"');
    expect(page).toContain('<App area="docs" tab="whitepaper"');
    expect(page).not.toMatch(/placeholder/i);
    expect(JSON.stringify(AREAS)).not.toMatch(/Whitepaper \(placeholder\)/);
    expect(read('src/components/Footer.astro')).not.toContain('Whitepaper (placeholder)');
    expect(read('src/content/help-notes.md')).not.toMatch(/Whitepaper placeholder/);
  });
  it('the PDF is committed in the public folder', () => {
    const f = join(root, 'public/royltee-whitepaper.pdf');
    expect(existsSync(f)).toBe(true);
    expect(statSync(f).size).toBeGreaterThan(100_000);
    expect(readFileSync(f).subarray(0, 5).toString()).toBe('%PDF-');
    expect(readFileSync(f).toString('latin1')).toContain("D:20261114");
  });
});
