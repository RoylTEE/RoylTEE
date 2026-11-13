import { randomBytes } from 'node:crypto';
/**
 * One source for both Content Security Policies.
 * Every page gets the strict policy from vercel.json (script-src 'self', no inline script).
 * Only the wallet pages (WALLET_ROUTES: sign in, and the Your shares page that asks a wallet to sign transactions) get a per request nonce instead, because the wallet
 * selector runs each wallet in a sandboxed srcdoc iframe with inline scripts, and that iframe inherits
 * the page policy. The selector takes the nonce through its cspNonce option.
 */
const SHARED: [string, string][] = [
  ['default-src', "'self'"],
  ['style-src', "'self' 'unsafe-inline'"],
  ['img-src', "'self' data: https://raw.githubusercontent.com https://app.hot-labs.org https://storage.herewallet.app https://www.mynearwallet.com https://res.cloudinary.com https://evm-on-near.dev"],
  ['connect-src', "'self' https://test.rpc.fastnear.com https://rpc.testnet.near.org https://raw.githubusercontent.com https://cdn.jsdelivr.net https://wallet.intear.tech https://near-mobile-production.aws.peersyst.tech https://trezu.app https://evm-on-near.dev"],
  ['frame-src', "'self' https://wallet.meteorwallet.app https://app.mynearwallet.com https://wallet.intear.tech https://staging.wallet.intear.tech https://app.hot-labs.org https://hot-labs.org https://download.hot-labs.org https://www.okx.com https://trezu.app https://nearmobile.app https://www.hana.money https://evm-on-near.dev https://wallet.near.org https://t.me"],
  ['frame-ancestors', "'none'"],
  ['object-src', "'none'"],
  ['base-uri', "'self'"],
  ['form-action', "'self'"],
];
const ORDER = ['default-src', 'script-src', 'style-src', 'img-src', 'font-src', 'connect-src', 'frame-src', 'frame-ancestors', 'object-src', 'base-uri', 'form-action'];
function render(extra: Record<string, string>) {
  const all = new Map<string, string>([...SHARED, ...Object.entries(extra)]);
  return ORDER.map(k => `${k} ${all.get(k)}`).join('; ');
}
/** Pages that open the wallet selector. Keep in sync with the vercel.json CSP source pattern. */
export const WALLET_ROUTES = ['/admin', '/help', '/member'] as const;
export const isWalletRoute = (pathname: string) => (WALLET_ROUTES as readonly string[]).includes(pathname.replace(/\/+$/, '') || '/');
/** The policy every non wallet page gets from vercel.json. */
export const strictPolicy = () => render({ 'script-src': "'self'", 'font-src': "'self'" });
/**
 * The wallet page policy. Script stays locked: only same origin files and inline script carrying this
 * response's nonce run, so injected markup still cannot execute. 'wasm-unsafe-eval' allows WebAssembly
 * compile for wallet signing code (MyNearWallet), not JS eval. Wallets load their own images, styles,
 * fonts and backends from hosts that change without notice (Meteor uses meteorwallet.app, rsms.me and
 * fonts.cdnfonts.com), so those four directives accept any https host on these two pages only.
 */
export const walletPolicy = (nonce: string) => {
  if (!/^[A-Za-z0-9+/]{22,}={0,2}$/.test(nonce)) throw new Error('bad nonce');
  return render({
    'script-src': `'self' 'nonce-${nonce}' 'wasm-unsafe-eval'`,
    'style-src': "'self' 'unsafe-inline' https:",
    'img-src': "'self' data: https:",
    'font-src': "'self' data: https:",
    'connect-src': "'self' https: wss:",
  });
};
export const newNonce = () => randomBytes(18).toString('base64');
/** Middleware body, kept here so tests can run it without the Astro runtime. */
export async function withWalletCsp(pathname: string, prerendered: boolean, locals: { cspNonce?: string }, next: () => Promise<Response>) {
  if (prerendered || !isWalletRoute(pathname)) return next();
  const nonce = newNonce();
  locals.cspNonce = nonce;
  const response = await next();
  response.headers.set('Content-Security-Policy', walletPolicy(nonce));
  response.headers.set('Cache-Control', 'private, no-store');
  return response;
}
