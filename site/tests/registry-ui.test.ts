import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { units, musdc, near, pct, utc, short } from '../src/scripts/chain';
import { RPC_URLS, SCENES, REGISTRY, TOKEN, EXPLORER } from '../src/data/testnet';
import { strictPolicy, walletPolicy } from '../src/server/csp';

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
    expect(SCENES.length).toBe(13);
    for (const s of SCENES) for (const t of s.txs) expect(t).toMatch(/^[1-9A-HJ-NP-Za-km-z]{43,44}$/);
    expect(REGISTRY).toBe('reg.royltee-dev.testnet');
    expect(TOKEN).toBe('usdc.royltee-dev.testnet');
    expect(EXPLORER).toBe('https://testnet.nearblocks.io');
    expect(SCENES.filter(s => /payer pays/i.test(s.what)).every(s => s.simulated)).toBe(true);
  });
  it('only view calls are made outside the wallet module', () => {
    for (const f of readdirSync(new URL('../src/scripts/', import.meta.url))) {
      if (f === 'wallet.ts') continue;
      expect(src(`scripts/${f}`), f).not.toMatch(/signAndSendTransaction|broadcast_tx|send_tx/);
    }
    expect(src('scripts/chain.ts')).toMatch(/request_type: 'call_function'/);
  });
  it('every change call from Your shares attaches a deposit and goes to the registry', () => {
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
    expect(src('pages/dashboard.astro')).toMatch(/Simulated payer/);
    expect(src('pages/index.astro')).not.toMatch(/deployment to NEAR testnet is planned/i);
  });
  it('the project name lives in one constants file', () => {
    for (const f of ['pages/index.astro', 'pages/dashboard.astro', 'components/Nav.astro', 'components/Footer.astro', 'components/RegistryHeader.astro']) expect(src(f), f).not.toMatch(/RoylTEE/);
  });
});

describe('registry pages: strict CSP holds after build', () => {
  it('Astro never inlines a script into the HTML', () => {
    expect(readFileSync(new URL('../astro.config.mjs', import.meta.url), 'utf8')).toMatch(/assetsInlineLimit: 0/);
  });
});
