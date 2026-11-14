import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { units, musdc, near, pct, utc, short, base58ToHex } from '../src/scripts/chain';
import { DEPLOY_RECORD } from '../src/data/testnet';
import { RPC_URLS, SCENES, REGISTRY, TOKEN, EXPLORER } from '../src/data/testnet';
import { strictPolicy, walletPolicy } from '../src/server/csp';
import { AREAS, GROUPS } from '../src/data/ia';
import { existsSync } from 'node:fs';

const src = (rel: string) => readFileSync(new URL(`../src/${rel}`, import.meta.url), 'utf8');
const directive = (policy: string, name: string) => policy.split(';').map(s => s.trim()).find(s => s.startsWith(name + ' '))!.slice(name.length + 1).split(' ');

describe('registry pages: formatting', () => {
  it('formats Mock USDC with 6 decimals and NEAR with 24, exactly', () => {
    expect(musdc('0')).toBe('0.00');
    expect(musdc('500000')).toBe('0.50');
    expect(musdc('260000')).toBe('0.26');
    expect(musdc('1234567')).toBe('1.234567');
    expect(musdc(-500000n)).toBe('-0.50');
    expect(near('99999999999999999999999')).toBe('0.0999');
    expect(near(7520n * 10n ** 19n)).toBe('0.0752');
    expect(units('1000000000', 6)).toBe('1,000.00');
  });
  it('shows basis points as percent and times in UTC', () => {
    expect(pct(5000)).toBe('50%');
    expect(pct(2050)).toBe('20.50%');
    expect(utc('1794548975776921777')).toMatch(/^2026-11-\d\d \d\d:\d\d UTC$/);
    expect(short('writer-a.royltee-dev.testnet')).toBe('writer-a');
  });
});

describe('registry pages: no new hosts, no secrets, honest data', () => {
  it('reads only RPC hosts the strict CSP already allows', () => {
    const allowed = directive(strictPolicy(), 'connect-src');
    for (const url of RPC_URLS) expect(allowed).toContain(url);
    expect(directive(walletPolicy('a'.repeat(24)), 'script-src')).not.toContain("'unsafe-inline'");
  });
  it('the recorded scenes are real explorer transaction ids for the testnet accounts', () => {
    expect(SCENES.length).toBe(22);
    for (const s of SCENES) for (const t of s.txs) expect(t).toMatch(/^[1-9A-HJ-NP-Za-km-z]{43,44}$/);
    expect(REGISTRY).toBe('reg.royltee-dev.testnet');
    expect(TOKEN).toBe('usdc.royltee-dev.testnet');
    expect(EXPLORER).toBe('https://testnet.nearblocks.io');
    expect(SCENES.filter(s => /payer pays/i.test(s.what)).every(s => s.simulated)).toBe(true);
  });
  it('no script outside the wallet module calls a send or broadcast method', () => {
    for (const f of readdirSync(new URL('../src/scripts/', import.meta.url))) {
      if (f === 'wallet.ts') continue;
      expect(src(`scripts/${f}`), f).not.toMatch(/signAndSendTransaction|broadcast_tx|send_tx/);
    }
    expect(src('scripts/chain.ts')).toMatch(/request_type: 'call_function'/);
  });
  it('Your shares sends its change calls to the registry and uses a 1 yoctoNEAR deposit', () => {
    const m = src('scripts/member.ts');
    for (const method of ['claim', 'approve_share', 'reject_share', 'register_share']) expect(m).toContain(`'${method}'`);
    expect(m.match(/sendCalls\(/g)!.length).toBe(m.match(/sendCalls\(REGISTRY,/g)!.length);
    expect(m.match(/sendCalls\(REGISTRY,/g)!.length).toBeGreaterThanOrEqual(3);
    expect(m).toMatch(/deposit: '1'/);
  });
  it('unverified and simulated parts stay labeled in the UI', () => {
    expect(src('pages/member.astro')).toMatch(/UNVERIFIED/);
    expect(src('components/Footer.astro')).toMatch(/Simulated/);
    expect(src('pages/index.astro')).toMatch(/Simulated/);
    expect(src('components/ActivityTable.astro')).toMatch(/Simulated payer/);
    expect(src('pages/dashboard.astro')).toMatch(/<ActivityTable/);
    expect(src('pages/index.astro')).not.toMatch(/deployment to NEAR testnet is planned/i);
  });
  it('the project name lives in one constants file', () => {
    for (const f of ['pages/index.astro', 'pages/dashboard.astro', 'components/Nav.astro', 'components/Footer.astro', 'components/RegistryHeader.astro', 'layouts/App.astro', 'data/ia.ts']) expect(src(f), f).not.toMatch(/RoylTEE/);
  });
});

describe('registry pages: strict CSP holds after build', () => {
  it('Astro never inlines a script into the HTML', () => {
    expect(readFileSync(new URL('../astro.config.mjs', import.meta.url), 'utf8')).toMatch(/assetsInlineLimit: 0/);
  });
});

describe('app shell: menu by purpose', () => {
  const pageFor = (href: string) => {
    const path = href.split('#')[0];
    if (path === '/') return 'pages/index.astro';
    if (path === '/dashboard/') return 'pages/dashboard.astro';
    if (path.endsWith('/')) return existsSync(new URL(`../src/pages${path}index.astro`, import.meta.url)) ? `pages${path}index.astro` : `pages${path.slice(0, -1)}.astro`;
    return `pages${path}.astro`;
  };
  it('every area and tab in the menu opens a real page, and hash tabs have a panel', () => {
    for (const a of AREAS) {
      expect(GROUPS as readonly string[]).toContain(a.group);
      expect(existsSync(new URL(`../src/${pageFor(a.href)}`, import.meta.url)), a.href).toBe(true);
      for (const t of a.tabs) {
        const file = pageFor(t.href);
        expect(existsSync(new URL(`../src/${file}`, import.meta.url)), t.href).toBe(true);
        const hash = t.href.split('#')[1];
        if (hash) expect(src(file), t.href).toMatch(new RegExp(`id="${hash}"[^>]*data-tab-panel|data-tab-panel[^>]*id="${hash}"|ActivityTable`));
      }
    }
  });
  it('every app page names a known area, and admin stays out of the menu', () => {
    const keys = AREAS.map((a) => a.key);
    for (const f of ['dashboard', 'catalog/index', 'catalog/splits', 'catalog/lookup', 'shares/index', 'shares/accounts', 'shares/claims', 'share', 'activity', 'agents', 'disputes', 'registrar', 'verify', 'member', 'help', 'docs', 'whitepaper', 'admin']) {
      const m = src(`pages/${f}.astro`).match(/<App area="([a-z]+)"/);
      expect(m, f).not.toBeNull();
      expect(keys, f).toContain(m![1]);
    }
    expect(AREAS.some((a) => a.href.includes('admin') || a.tabs.some((t) => t.href.includes('admin')))).toBe(false);
  });
  it('the shell runs from an external module and has no inline script but the wallet nonce carrier', () => {
    const app = src('layouts/App.astro');
    expect(app).toMatch(/<script>import '..\/scripts\/shell.ts';<\/script>/);
    expect(app.match(/<script is:inline/g)?.length).toBe(1);
    expect(app).toMatch(/\{cspNonce && <script is:inline nonce=\{cspNonce\} data-csp-nonce><\/script>\}/);
    expect(src('scripts/shell.ts')).not.toMatch(/innerHTML|eval\(/);
  });
});

describe('verify page', () => {
  it('base58 code hashes convert to the sha256 hex in the deploy record', () => {
    expect(base58ToHex(DEPLOY_RECORD.registry.codeHash)).toBe(DEPLOY_RECORD.registry.sha256);
    expect(base58ToHex(DEPLOY_RECORD.mockUsdc.codeHash)).toBe(DEPLOY_RECORD.mockUsdc.sha256);
    expect(base58ToHex(DEPLOY_RECORD.vault.codeHash)).toBe(DEPLOY_RECORD.vault.sha256);
  });
  it('shows the verify command and labels key deletion as planned, not done', () => {
    const v = src('pages/verify.astro');
    expect(v).toMatch(/npm run verify:testnet/);
    expect(v).toMatch(/Deleting them is planned after that update \(roadmap\)/);
    expect(v).toMatch(/UNVERIFIED/);
  });
});
