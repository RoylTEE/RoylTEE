import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { forbidden } from '../scripts/wording.mjs';
import { UPDATES } from '../src/data/updates';
import { TXS } from '../src/data/claims';
import { AUCTION } from '../src/data/auction';

const root = join(__dirname, '..');
const read = (p: string) => readFileSync(join(root, p), 'utf8');
const dist = join(root, 'dist/client');

/** page route, source file, header art (null for the framed card pages), a phrase its title must carry */
const PAGES: { route: string; file: string; art: string; title: string; band: boolean }[] = [
  { route: '/how-it-works', file: 'how-it-works', art: 'hdr-how', title: 'How it works', band: true },
  { route: '/artists', file: 'artists', art: 'card-artists', title: 'Artists', band: false },
  { route: '/publishers-and-labels', file: 'publishers-and-labels', art: 'card-publishers', title: 'Publishers and labels', band: false },
  { route: '/catalog-buyers', file: 'catalog-buyers', art: 'card-buyers', title: 'Catalog buyers', band: false },
  { route: '/security', file: 'security', art: 'hdr-security', title: 'Security', band: true },
  { route: '/faq', file: 'faq', art: 'hdr-faq', title: 'FAQ and glossary', band: true },
  { route: '/updates', file: 'updates', art: 'hdr-updates', title: 'Updates', band: true },
  { route: '/about', file: 'about', art: 'hdr-about', title: 'About', band: true },
];

/** Every route the app serves: static files, server pages and the public folder. */
function routeExists(path: string): boolean {
  if (path === '/') return true;
  const p = path.split('?')[0].replace(/\/$/, '');
  return [`src/pages${p}.astro`, `src/pages${p}/index.astro`, `public${p}`].some((f) => existsSync(join(root, f)) && !statSync(join(root, f)).isDirectory());
}
/** Source files that can hold an anchor id for a route. */
function sourcesOf(path: string): string {
  const p = path.replace(/\/$/, '') || '/index';
  const files = [`src/pages${p}.astro`, `src/pages${p}/index.astro`].filter((f) => existsSync(join(root, f)));
  let text = files.map(read).join('\n');
  if (p === '/index') text += readdirSync(join(root, 'src/components/landing')).map((f) => read(`src/components/landing/${f}`)).join('\n');
  return text;
}

describe('new pages', () => {
  for (const pg of PAGES) {
    describe(pg.route, () => {
      const src = read(`src/pages/${pg.file}.astro`);
      it('has a title and a description, and uses the shared layout', () => {
        expect(src).toMatch(new RegExp(`<Base [^>]*title="${pg.title} \\. RoylTEE"`));
        expect(src).toMatch(/description="[^"]{40,}"/);
        expect(src).toMatch(/<PageHero /);
      });
      it('uses its header art, which exists, with alt text', () => {
        expect(src).toContain(`art="${pg.art}"`);
        expect(existsSync(join(root, 'public/art', `${pg.art}.webp`))).toBe(true);
        expect(src).toMatch(/alt="Flat retro illustration: [^"]{20,}"/);
        if (pg.band) expect(pg.art).toMatch(/^hdr-/);
      });
      it('never says placeholder and passes the wording check', () => {
        expect(src).not.toMatch(/placeholder/i);
        expect(forbidden(src)).toEqual([]);
      });
      it('links only to routes and anchors that exist', () => {
        const hrefs = [...src.matchAll(/href="(\/[^"{]*)"/g)].map((m) => m[1]);
        if (pg.route !== '/updates') expect(hrefs.length).toBeGreaterThan(0);
        for (const h of hrefs) {
          const [path, anchor] = h.split('#');
          if (path && path !== '/') expect(routeExists(path), h).toBe(true);
          if (anchor) expect(sourcesOf(path || '/'), h).toContain(`id="${anchor}"`);
        }
      });
    });
  }

  it('the audience pages link to How it works and Verify and say what is Simulated', () => {
    for (const f of ['artists', 'publishers-and-labels', 'catalog-buyers']) {
      const src = read(`src/pages/${f}.astro`);
      expect(src, f).toContain('href="/how-it-works"');
      expect(src, f).toContain('href="/verify"');
      expect(src, f).toMatch(/Simulated/);
      expect(src, f).toMatch(/The problem/);
      expect(src, f).toMatch(/Try it in the demo dashboard/);
    }
    expect(read('src/pages/publishers-and-labels.astro')).toMatch(/id="publishers"[\s\S]*id="labels"/);
  });

  it('How it works carries the lifecycle, the sale transactions and the real method names', () => {
    const src = read('src/pages/how-it-works.astro');
    expect(src).toContain('id="lifecycle"');
    for (const m of ['register_work', 'register_share', 'approve_share', 'ft_transfer_call', 'nft_transfer', 'claim']) expect(src, m).toContain(m);
    for (const id of ['steps', 'disputes', 'auction', 'layers', 'splitsheet']) expect(src, id).toContain(`id="${id}"`);
    for (const h of [...src.matchAll(/'([1-9A-HJ-NP-Za-km-z]{43,44})'/g)].map((m) => m[1])) expect(TXS[h], h).toBeTruthy();
  });

  it('Security has the vault, agent, dispute, TEE, threat model, Real and Simulated, plan and check-it-yourself parts', () => {
    const src = read('src/pages/security.astro');
    for (const id of ['vaults', 'agents', 'disputes', 'tee', 'threats', 'trust', 'plan', 'verify']) expect(src, id).toContain(`id="${id}"`);
    expect(src).toContain("REPO_FILE_BASE + 'THREAT_MODEL.md'");
    expect(src).toContain("REPO_FILE_BASE + 'AUDIT.md'");
    expect(src).toMatch(/not an independent review/);
    expect(src).toMatch(/Plan, not done/);
    expect(src).toMatch(/view_access_key_list/);
    expect(src).toMatch(/npm run verify:testnet/);
    expect(src).toMatch(/Change a split alone \(except the registrar/);
    expect(AUCTION.verify.checks.length).toBeGreaterThan(0);
  });

  it('the FAQ answers from the docs and the glossary has every required term', () => {
    const src = read('src/pages/faq.astro');
    for (const t of ['NFT share', 'vault', 'split sheet', 'escrow', 'NEP-171', 'NEP-177', 'NEP-181', 'NEP-141', 'TEE', 'attestation', 'testnet', 'Simulated', 'mUSDC']) expect(src, t).toContain(`['${t}',`);
    expect(src).toMatch(/UNVERIFIED/);
    expect(src).toMatch(/<details>/);
  });

  it('About states the origin limits, the license and the publisher, and names no people', () => {
    const src = read('src/pages/about.astro');
    expect(src).toContain('Built at NEAR HACKATHON');
    expect(src).toMatch(/No endorsement by NEAR/);
    expect(src).toMatch(/MIT license/);
    expect(src).toMatch(/RoylTEE Labs/);
    expect(src).toContain('SOURCE_URL');
    expect(src).not.toMatch(/\b(won|winner|winning|placed|finalist|sponsor|customers?|offices?|founded|partners?)\b/i);
  });

  it('the updates are dated in UTC, newest first, tied to a real record, and invent nothing', () => {
    expect(UPDATES.length).toBeGreaterThanOrEqual(15);
    const ats = UPDATES.map((u) => u.at);
    expect([...ats].sort().reverse()).toEqual(ats);
    for (const u of UPDATES) {
      expect(u.at, u.title).toMatch(u.dateOnly ? /^\d{4}-\d\d-\d\d$/ : /^\d{4}-\d\d-\d\dT\d\d:\d\dZ$/);
      expect(u.links.length, u.title).toBeGreaterThan(0);
      expect(`${u.title} ${u.body}`, u.title).not.toMatch(/partner|press|award|launch|announc|customer|raised|funding|winner/i);
      for (const l of u.links) {
        if (l.external) expect(l.href, l.label).toMatch(/^https:\/\/(testnet\.nearblocks\.io\/txns\/|github\.com\/RoylTEE\/RoylTEE\/)/);
        else expect(routeExists(l.href.split('#')[0]), l.href).toBe(true);
        const m = l.href.match(/\/txns\/([A-Za-z0-9]+)$/);
        if (m) expect(TXS[m[1]] ?? Object.values(AUCTION.times).flat().find((t: { hash: string }) => t.hash === m[1]), l.label).toBeTruthy();
      }
    }
    for (const day of ['2026-11-12', '2026-11-13', '2026-11-14']) expect(UPDATES.some((u) => u.at.startsWith(day))).toBe(true);
    for (const f of ['src/data/updates.ts', 'src/pages/updates.astro']) expect(forbidden(read(f)), f).toEqual([]);
  });

  it('the nav stays short and the footer groups the new pages', () => {
    const nav = read('src/components/Nav.astro');
    expect([...nav.matchAll(/\{ key: '/g)].length).toBeLessThanOrEqual(6);
    const foot = read('src/components/Footer.astro');
    for (const h of ['/artists', '/publishers-and-labels', '/catalog-buyers', '/faq', '/updates', '/about', '/security', '/how-it-works']) expect(foot, h).toContain(`'${h}'`);
    for (const g of ['Product', 'For you', 'Learn', 'Company']) expect(foot, g).toContain(`h: '${g}'`);
  });

  it('the auction page shows its header art with alt text', () => {
    const src = read('src/pages/auction.astro');
    expect(src).toMatch(/<img src="\/art\/hdr-auction\.webp"[^>]*alt="[^"]{20,}"/);
    expect(existsSync(join(root, 'public/art/hdr-auction.webp'))).toBe(true);
  });
});

/** Rendered output: runs against the build when one exists (npm run build first). */
describe.skipIf(!existsSync(join(dist, 'how-it-works/index.html')))('built pages', () => {
  for (const pg of PAGES) {
    it(`${pg.route} renders with a title, its art, one h1 and no placeholder text`, () => {
      const html = readFileSync(join(dist, pg.route, 'index.html'), 'utf8');
      expect(html).toMatch(new RegExp(`<title>${pg.title} \\. RoylTEE</title>`));
      expect(html).toContain(`/art/${pg.art}.webp`);
      expect([...html.matchAll(/<h1[ >]/g)]).toHaveLength(1);
      const text = html.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, '');
      expect(text).not.toMatch(/placeholder/i);
      expect(text).not.toMatch(/\u2013|\u2014/);
    });
  }
  it('every internal link on every built page resolves to a built page or a known route, and every anchor exists', () => {
    const files: string[] = [];
    const walk = (d: string) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = join(d, e.name); if (e.isDirectory()) walk(p); else if (p.endsWith('.html')) files.push(p); } };
    walk(dist);
    for (const f of files) {
      const html = readFileSync(f, 'utf8');
      for (const m of html.matchAll(/href="(\/[^"#?]*)(#[^"]*)?"/g)) {
        const [, path, hash] = m;
        if (path.startsWith('//') || path.startsWith('/_astro/') || existsSync(join(dist, path)) && statSync(join(dist, path)).isFile()) continue;
        const built = join(dist, path, path.endsWith('/') || path === '' ? 'index.html' : 'index.html');
        const isBuilt = existsSync(built);
        expect(isBuilt || routeExists(path === '' ? '/' : path), `${f.slice(dist.length)} -> ${path}`).toBe(true);
        if (hash && hash.length > 1 && isBuilt && (path === '/' || PAGES.some((pg) => pg.route === path.replace(/\/$/, '')))) expect(readFileSync(built, 'utf8'), `${f.slice(dist.length)} -> ${path}${hash}`).toContain(`id="${hash.slice(1)}"`);
      }
    }
  });
});
