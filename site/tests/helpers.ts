import { KeyPair, KeyPairSigner, Account, JsonRpcProvider } from 'near-api-js';
import { signMessage } from 'near-api-js/nep413';
import { handle, type Deps } from '../src/server/api';
import { loadConfig } from '../src/server/config';
import { MemoryStore } from '../src/server/store';
import type { Turn } from '../src/server/chat';
export const ORIGIN = 'https://royltee.example';
export const ADMIN = 'admin.royltee-dev.testnet';
export function setup(overrides: Record<string, string> = {}) {
  const clock = { now: Date.UTC(2026, 10, 13, 6) };
  const store = new MemoryStore(() => clock.now);
  const config = loadConfig({ CHAT_HMAC_SECRET: 'c'.repeat(40), SESSION_SECRET: 's'.repeat(40), ALLOWED_ORIGINS: ORIGIN, ADMIN_WALLET: ADMIN, NEAR_RPC_URL: 'https://rpc.test.invalid', NEARAI_API_KEY: 'test-only', CRON_SECRET: 'k'.repeat(40), ...overrides });
  const keys = new Map<string, Set<string>>();
  const fullAccess = (account: string, key: string) => { if (!keys.has(account)) keys.set(account, new Set()); keys.get(account)!.add(key); };
  const rpcCalls: unknown[] = [];
  const fetcher = (async (_url: string, init: RequestInit) => {
    const p = JSON.parse(String(init.body)).params; rpcCalls.push(p);
    return new Response(JSON.stringify(keys.get(p.account_id)?.has(p.public_key) ? { result: { nonce: 1, permission: 'FullAccess' } } : { result: { nonce: 1, permission: { FunctionCall: { receiver_id: 'x', method_names: [] } } } }));
  }) as typeof fetch;
  const modelCalls: Turn[][] = [];
  const reply = { text: 'Unclaimed money stays in the share vault, so it moves with the share. See [Lifecycle and catalog sale](/#lifecycle).' };
  const deps: Deps = { config, store, now: () => clock.now, fetcher, model: async turns => { modelCalls.push(turns); return reply.text; }, address: '203.0.113.7' };
  let jar = '';
  async function call(route: string, body?: unknown, opts: { method?: string; origin?: string | null; type?: string; cookie?: string; address?: string; auth?: string } = {}) {
    if (route === 'tickets' && body && typeof body === 'object' && !('submitNonce' in body) && opts.origin !== null && !opts.type) {
      const nonce = await call('tickets', undefined, opts);
      body = { ...body, submitNonce: nonce.body.submitNonce };
    }
    const headers: Record<string, string> = {};
    if (opts.origin !== null) headers.origin = opts.origin ?? ORIGIN;
    if (body !== undefined) headers['content-type'] = opts.type ?? 'application/json';
    const c = opts.cookie ?? jar; if (c) headers.cookie = c;
    if (opts.auth) headers.authorization = opts.auth;
    const r = await handle(new Request(`${ORIGIN}/api/${route}`, { method: opts.method ?? (body === undefined ? 'GET' : 'POST'), headers, body: body === undefined ? undefined : typeof body === 'string' ? body : JSON.stringify(body) }), route, { ...deps, address: opts.address ?? deps.address });
    const set = r.headers.get('set-cookie'); if (set && opts.cookie === undefined) jar = set.split(';')[0];
    return { status: r.status, body: await r.json(), cookie: set };
  }
  async function wallet(accountId: string, register = true) {
    const kp = KeyPair.fromRandom('ed25519');
    if (register) fullAccess(accountId, kp.getPublicKey().toString());
    const account = new Account(accountId, new JsonRpcProvider({ url: 'https://rpc.test.invalid' }), new KeyPairSigner(kp));
    return {
      kp,
      async sign(ch: { message: string; recipient: string; nonce: string }) {
        const s = await signMessage({ signerAccount: account, payload: { message: ch.message, recipient: ch.recipient, nonce: Buffer.from(ch.nonce, 'base64') } });
        return { accountId, publicKey: s.publicKey.toString(), signature: Buffer.from(s.signature).toString('base64'), nonce: ch.nonce };
      },
    };
  }
  async function signIn(accountId: string) {
    const w = await wallet(accountId); jar = '';
    const ch = await call('auth/challenge', {});
    const r = await call('auth/verify', await w.sign(ch.body));
    if (r.status !== 200) throw new Error(JSON.stringify(r.body));
    return jar;
  }
  return { clock, store, config, deps, call, wallet, signIn, fullAccess, modelCalls, rpcCalls, reply, setJar: (v: string) => { jar = v; } };
}
