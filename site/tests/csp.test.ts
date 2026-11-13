import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { WALLET_ROUTES, isWalletRoute, newNonce, strictPolicy, walletPolicy, withWalletCsp } from '../src/server/csp';

const src = (rel: string) => readFileSync(new URL(`../src/${rel}`, import.meta.url), 'utf8');
type Block = { source: string; headers: { key: string; value: string }[] };
const conf = JSON.parse(readFileSync(new URL('../vercel.json', import.meta.url), 'utf8')) as { headers: Block[] };
const cspBlocks = conf.headers.filter(b => b.headers.some(h => h.key === 'Content-Security-Policy'));
const directive = (policy: string, name: string) => policy.split(';').map(s => s.trim()).find(s => s.startsWith(name + ' '))?.slice(name.length + 1);
/** Same anchoring as scripts/apply-security-headers.mjs. */
const matches = (source: string, path: string) => new RegExp(source === '/(.*)' ? '^/(.*)$' : `^${source}$`).test(path);

describe('CSP per route', () => {
  it('vercel.json has exactly one CSP block and it is the strict policy', () => {
    expect(cspBlocks).toHaveLength(1);
    expect(cspBlocks[0].headers.find(h => h.key === 'Content-Security-Policy')!.value).toBe(strictPolicy());
    expect(directive(strictPolicy(), 'script-src')).toBe("'self'");
    expect(directive(strictPolicy(), 'font-src')).toBe("'self'");
  });

  it('the strict policy covers every path except the wallet sign in pages', () => {
    const source = cspBlocks[0].source;
    for (const p of ['/', '/dashboard', '/dashboard/', '/whitepaper', '/api/auth/challenge', '/api/chat', '/administer', '/helpdesk', '/admin/x', '/nope', '/members', '/member/x', '/share', '/agents', '/disputes']) expect(matches(source, p), p).toBe(true);
    for (const p of ['/admin', '/admin/', '/help', '/help/', '/member', '/member/']) expect(matches(source, p), p).toBe(false);
    for (const r of WALLET_ROUTES) expect(isWalletRoute(r) && isWalletRoute(r + '/')).toBe(true);
    expect(isWalletRoute('/')).toBe(false);
    expect(isWalletRoute('/administer')).toBe(false);
  });

  it('the other security headers still apply to every path', () => {
    const rest = conf.headers.find(b => b.source === '/(.*)')!;
    expect(rest.headers.map(h => h.key)).toEqual(['X-Frame-Options', 'X-Content-Type-Options', 'Referrer-Policy', 'Permissions-Policy', 'Strict-Transport-Security']);
  });

  it('the wallet policy allows inline script only with the nonce and keeps the other locks', () => {
    const nonce = newNonce();
    const p = walletPolicy(nonce);
    expect(directive(p, 'script-src')).toBe(`'self' 'nonce-${nonce}' 'wasm-unsafe-eval'`);
    expect(p).not.toMatch(/'unsafe-eval'|'strict-dynamic'/);
    expect(directive(p, 'script-src')).not.toMatch(/unsafe-inline|https:|\*/);
    expect(directive(p, 'default-src')).toBe("'self'");
    expect(directive(p, 'object-src')).toBe("'none'");
    expect(directive(p, 'frame-ancestors')).toBe("'none'");
    expect(directive(p, 'base-uri')).toBe("'self'");
    expect(directive(p, 'form-action')).toBe("'self'");
    expect(directive(p, 'frame-src')).toBe(directive(strictPolicy(), 'frame-src'));
    expect(directive(p, 'font-src')).toMatch(/data:/);
    expect(directive(p, 'connect-src')).toBe("'self' https: wss:");
  });

  it('nonces are fresh and well formed, and a bad nonce is refused', () => {
    const a = newNonce(); const b = newNonce();
    expect(a).not.toBe(b);
    expect(a).toMatch(/^[A-Za-z0-9+/]{24}$/);
    expect(() => walletPolicy("x' 'unsafe-inline")).toThrow();
  });

  it('the middleware sets a nonce policy and no-store on wallet pages only', async () => {
    const locals: { cspNonce?: string } = {};
    const res = await withWalletCsp('/help', false, locals, async () => new Response('ok'));
    expect(locals.cspNonce).toBeTruthy();
    expect(res.headers.get('content-security-policy')).toBe(walletPolicy(locals.cspNonce!));
    expect(res.headers.get('cache-control')).toBe('private, no-store');
    const other: { cspNonce?: string } = {};
    const plain = await withWalletCsp('/api/chat', false, other, async () => new Response('ok'));
    expect(other.cspNonce).toBeUndefined();
    expect(plain.headers.get('content-security-policy')).toBeNull();
    expect((await withWalletCsp('/admin', true, {}, async () => new Response('ok'))).headers.get('content-security-policy')).toBeNull();
  });

  it('every page that opens the wallet selector is a wallet route rendered per request', () => {
    const signInScripts = readdirSync(new URL('../src/scripts/', import.meta.url)).filter(f => /import \{[^}]*\b(?:signIn|connectWallet)\b[^}]*\} from '\.\/wallet'/.test(src(`scripts/${f}`)));
    expect(signInScripts.length).toBeGreaterThan(0);
    const pages = readdirSync(new URL('../src/pages/', import.meta.url)).filter(f => f.endsWith('.astro'));
    const walletPages = pages.filter(f => signInScripts.some(s => src(`pages/${f}`).includes(`../scripts/${s}`)));
    expect(walletPages.map(f => '/' + f.replace(/\.astro$/, '')).sort()).toEqual([...WALLET_ROUTES].sort());
    for (const f of walletPages) expect(src(`pages/${f}`)).toMatch(/export const prerender = false;/);
  });

  it('the layout renders the nonce script and the connector receives the nonce', () => {
    expect(src('layouts/Base.astro')).toMatch(/<script is:inline nonce=\{cspNonce\} data-csp-nonce><\/script>/);
    expect(src('scripts/wallet.ts')).toMatch(/cspNonce: cspNonce\(\)/);
    expect(src('scripts/wallet.ts')).toMatch(/script\[data-csp-nonce\]'\)\?\.nonce/);
    expect(src('middleware.ts')).toMatch(/withWalletCsp/);
  });
});
