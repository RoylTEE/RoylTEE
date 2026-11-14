import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { forbidden } from '../scripts/wording.mjs';
import { PROOF, LATEST } from '../src/data/landing';
import { CLAIMS } from '../src/data/claims';
import evidence from '../src/data/evidence.json';
const root = join(__dirname, '..');
const read = (p: string) => readFileSync(join(root, p), 'utf8');
const landingFiles = ['src/pages/index.astro', 'src/styles/landing.css', 'src/data/landing.ts', 'src/components/AnnounceBar.astro', 'src/components/Nav.astro', 'src/components/Footer.astro', 'src/consts.ts', ...readdirSync(join(root, 'src/components/landing')).map((f) => `src/components/landing/${f}`)];

describe('landing page', () => {
  it('tells the story in order', () => {
    const page = read('src/pages/index.astro');
    const order = ['<section class="hero">', '<Problem />', '<Audience />', '<Flow />', '<Product />', '<Tech />', '<WhyNear />', '<Tracks />', '<Compare />', '<Proof />', '<Latest />', '<Origin />', '<Closing />'].map((m) => page.indexOf(m));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });
  it('keeps the anchors the help notes and footer link to', () => {
    const all = landingFiles.filter((f) => f.endsWith('.astro')).map(read).join('\n');
    for (const id of ['problem', 'how', 'near', 'tracks']) expect(all, id).toContain(`id="${id}"`);
  });
  it('the detail that left the landing lives on its own page, under the same anchor', () => {
    const where: Record<string, string> = { lifecycle: 'how-it-works', agents: 'security', trust: 'security', verify: 'security' };
    for (const [id, page] of Object.entries(where)) expect(read(`src/pages/${page}.astro`), `${page}#${id}`).toContain(`id="${id}"`);
    expect(read('src/pages/index.astro')).not.toMatch(/id="(lifecycle|agents|trust)"/);
  });
  it('the landing keeps the story: no method names, account ids, key details or test counts in its copy', () => {
    const copy = ['src/pages/index.astro', ...readdirSync(join(root, 'src/components/landing')).map((f) => `src/components/landing/${f}`)].map(read).join('\n');
    expect(copy).not.toMatch(/\.royltee-dev\.testnet|full access key|yoctoNEAR|ft_transfer_call|nft_transfer|pay_from_allowance|view_access_key_list|outlayer-verify|\d+ tests/i);
  });
  it('nav and footer link only to pages that exist, and nothing is called a placeholder', () => {
    const files = [read('src/components/Nav.astro'), read('src/components/Footer.astro')].join('\n');
    const hrefs = [...files.matchAll(/'(\/[a-z/#-]*)'/g)].map((m) => m[1]).filter((h) => h !== '/');
    expect(hrefs.length).toBeGreaterThan(10);
    for (const h of hrefs) {
      const path = h.split('#')[0];
      if (!path || path === '/') continue;
      const page = path.endsWith('/') ? [`src/pages${path}index.astro`, `src/pages${path.slice(0, -1)}.astro`] : [`src/pages${path}.astro`];
      expect(page.some((p) => existsSync(join(root, p))), h).toBe(true);
    }
    expect(files).not.toMatch(/placeholder/i);
    expect(read('src/consts.ts')).toContain("SOURCE_URL = 'https://github.com/RoylTEE/RoylTEE'");
  });
  it('the top bars name the NFT standard and the testnet demo, and the hero opens the testnet app', () => {
    const bar = read('src/components/AnnounceBar.astro');
    expect(bar).not.toContain('announce-tag');
    expect(bar).toContain('Every rights share is an NEP-171 NFT that owns its own royalty vault.');
    expect(bar).toContain('See how it works &rarr;');
    expect(bar).toContain('href="/how-it-works"');
    const base = read('src/layouts/Base.astro');
    expect(base).toContain('class="badge ok"');
    expect(base).toContain('NEAR testnet');
    expect(base).toContain('Live demo. Payments use Mock USDC.');
    expect(base).not.toMatch(/badge sim/);
    const hero = read('src/pages/index.astro');
    expect(hero).toContain('Open the testnet app');
    expect(hero).toContain('Live on NEAR testnet. Money in the demo is Simulated.');
    expect(hero).toContain("A writer's share of a song can be sold, split or disputed, but the place royalties are sent should never break.");
    expect(hero).not.toContain('Open the demo dashboard');
    expect(read('src/consts.ts')).toContain("PITCH = 'RoylTEE turns each share of a song into an NFT with its own verified payout address, for human and AI music alike.'");
    const nav = read('src/components/Nav.astro');
    expect(nav).toContain('Open the testnet app');
    expect(nav).not.toContain('badge ok net');
    expect(nav).not.toContain('Open the demo dashboard');
    expect(read('src/components/landing/Proof.astro')).toContain('href="/auction"');
    expect(read('src/components/landing/Closing.astro')).toContain('Open the testnet app');
    expect(read('src/components/landing/Closing.astro')).not.toContain('Open the demo dashboard');
    const app = read('src/layouts/App.astro');
    expect(app).toContain('class="badge ok app-net"');
    expect(app).toContain('Live demo. Payments use Mock USDC.');
    expect(app).toContain('Identity checks, payers and payer statements are Simulated. Nothing here is an investment product or legal advice.');
    expect(app).not.toMatch(/app-foot[\s\S]*badge sim/);
    expect(app).toContain('.side-link[aria-current="page"] { background: var(--surface); }');
    expect(app).not.toMatch(/inset 3px 0 0 var\(--amber\)/);
  });
  it('passes the wording check (no dashes, no banned names, no local paths)', () => {
    for (const f of landingFiles) expect(forbidden(read(f)), f).toEqual([]);
  });
  it('every art file the landing uses exists and has alt text', () => {
    const used = new Set<string>();
    for (const f of landingFiles.filter((x) => x.endsWith('.astro'))) {
      const text = read(f);
      for (const m of text.matchAll(/\/art\/([a-z0-9-]+\.webp)/g)) used.add(m[1]);
      for (const m of text.matchAll(/img: '(card-[a-z]+)'/g)) used.add(`${m[1]}.webp`);
      for (const img of text.matchAll(/<img[^>]*>/g)) expect(img[0], `${f} ${img[0].slice(0, 60)}`).toMatch(/alt=/);
    }
    expect(used.size).toBeGreaterThan(8);
    for (const u of used) expect(existsSync(join(root, 'public/art', u)), u).toBe(true);
    expect(read('src/pages/index.astro')).toMatch(/fetchpriority="high"/);
  });
  it('the proof numbers are counted from the evidence, not typed in', () => {
    expect(PROOF[0].value).toBe(String(Object.keys(evidence.txs).length));
    expect(PROOF[1].value).toBe(String(evidence.vaults.length));
    expect(PROOF[2].value).toBe(String(CLAIMS.length));
    expect(PROOF[3].value).toMatch(/^\d+ of \d+$/);
  });
  it('the latest cards are dated, newest first, and link to the explorer', () => {
    expect(LATEST).toHaveLength(4);
    for (const l of LATEST) { expect(l.when).toMatch(/^\d{4}-\d\d-\d\d \d\d:\d\d UTC$/); expect(l.txHref).toContain('nearblocks.io/txns/'); }
    const whens = LATEST.map((l) => l.when);
    expect([...whens].sort().reverse()).toEqual(whens);
  });
  it('the tracks carry their published names, and the origin section claims no result', () => {
    const tracks = read('src/components/landing/Tracks.astro');
    for (const n of ['NFTs That Do Something', 'Privacy & Private AI', 'AI Security & Defense']) expect(tracks).toContain(n);
    const origin = read('src/components/landing/Origin.astro');
    expect(origin).toContain('Built at');
    expect(origin).toMatch(/No endorsement by NEAR/);
    expect(origin).not.toMatch(/\b(won|winner|winning|placed|finalist|sponsor)/i);
  });
});
