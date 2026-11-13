import { randomBytes } from 'node:crypto';
import { storeFor, type Config } from './config';
import type { Store } from './store';
import { ACCOUNT_ID, ED25519_KEY, HttpError, body, challengeCookie, challengeSetCookie, clientKey, cookieSecure, digest, equal, fail, json, originCheck, randomId, rate, sessionCookie, sessionSetCookie } from './security';
import { fullAccessKey, readSession, recordWallet, sessionToken, verifySignature, type Challenge, type Session } from './auth';
import { chat, startChat, type Model } from './chat';
import { adminUpdate, assertOpenSlot, backfillOpenTickets, createTicket, listTickets, sweep, ticketText, LIMITS } from './tickets';
export type Deps = { config: Config; store?: Store | null; model: Model; now?: () => number; fetcher?: typeof fetch; address?: string };
export const SESSION_COOKIE = 'royltee_support';
export const SESSION_SECONDS = 15 * 60;
export const SIGN_IN_MESSAGE = 'Sign in to RoylTEE support on NEAR testnet. This does not create a transaction.';
const sessionId = /^[a-f0-9]{48}$/;
function pinnedAdminKey(config: Config) {
  const key = config.adminPublicKey;
  if (!key) return '';
  return ED25519_KEY.test(key) ? key : fail(503, 'Support sign in is not configured yet.');
}
function isAdmin(config: Config, wallet: string, publicKey?: string) {
  if (!config.adminWallet || wallet !== config.adminWallet) return false;
  const pin = config.adminPublicKey;
  if (!pin) return true;
  if (!ED25519_KEY.test(pin)) return false;
  return publicKey === pin;
}
async function storedSession(store: Store, session: Session) {
  const record = recordWallet(await store.get(`session:${session.sid}`));
  if (!record || record.wallet !== session.wallet) fail(401, 'Please sign in with your wallet again.');
  return record;
}
/** The signed-in wallet for this request, or null when there is no valid session. */
async function optionalWallet(store: Store, config: Config, request: Request, now: number): Promise<string | null> {
  const token = sessionCookie(request);
  if (!token || !config.sessionSecret) return null;
  try {
    const session = readSession(config.sessionSecret, token, now);
    return (await storedSession(store, session)).wallet;
  } catch (e) {
    if (e instanceof HttpError && e.status === 401) return null;
    throw e;
  }
}
export async function handle(request: Request, route: string, deps: Deps): Promise<Response> {
  const now = deps.now?.() ?? Date.now();
  try {
    const { config } = deps;
    const store = ('store' in deps ? deps.store : storeFor(config)) ?? fail(503, 'Support storage is not configured yet.');
    if (!config.allowedOrigins.length) fail(503, 'This feature is not configured yet.');
    const method = request.method;
    const secure = cookieSecure(request);
    if (route === 'cron/sweep') {
      const expected = `Bearer ${config.cronSecret}`, got = request.headers.get('authorization') ?? '';
      if (!config.cronSecret || !equal(digest(got), digest(expected))) fail(401, 'Not allowed.');
      return json({ removed: await sweep(store, now) });
    }
    if (method !== 'GET' && method !== 'POST') fail(405, 'Method not allowed.');
    originCheck(request, config.allowedOrigins, method === 'POST');
    const ip = clientKey(deps.address ?? 'local', config.sessionSecret || config.chatSecret);
    if (route === 'chat/start' && method === 'POST') {
      if (!config.chatSecret) fail(503, 'Chat is not configured yet.');
      await rate(store, `chat-start:${ip}`, 30, 86400);
      const id = randomId(); await startChat(store, id); return json({ sessionId: id });
    }
    if (route === 'chat' && method === 'POST') {
      if (!config.chatSecret) fail(503, 'Chat is not configured yet.');
      const input = await body(request);
      if (typeof input.sessionId !== 'string' || !sessionId.test(input.sessionId)) fail(400, 'That chat history is not valid. Please start again.');
      const wallet = await optionalWallet(store, config, request, now);
      return json(await chat(store, config, input.sessionId as string, input, ip, deps.model, now, wallet));
    }
    if (!config.sessionSecret || !config.rpcUrl) fail(503, 'Support sign in is not configured yet.');
    if (route === 'auth/challenge' && method === 'POST') {
      await rate(store, `challenge:${ip}`, 30, 600);
      const recipient = new URL(request.headers.get('origin')!).host;
      const binding = randomId();
      const challenge: Challenge = { message: SIGN_IN_MESSAGE, recipient, nonce: randomBytes(32).toString('base64'), expires: now + 300000, binding };
      await store.put(`challenge:${Buffer.from(challenge.nonce, 'base64').toString('hex')}`, challenge, 300);
      return json({ message: challenge.message, recipient, nonce: challenge.nonce, expiresAt: challenge.expires }, 200, { 'set-cookie': challengeSetCookie(binding, 300, secure) });
    }
    if (route === 'auth/verify' && method === 'POST') {
      await rate(store, `verify:${ip}`, 20, 600);
      const input = await body(request);
      const { accountId, publicKey, signature, nonce } = input;
      if (typeof accountId !== 'string' || !ACCOUNT_ID.test(accountId) || typeof publicKey !== 'string' || typeof signature !== 'string' || typeof nonce !== 'string' || !/^[A-Za-z0-9+/]{43}=$/.test(nonce)) fail(400, 'Sign in data is not valid.');
      const binding = challengeCookie(request);
      const key = `challenge:${Buffer.from(nonce as string, 'base64').toString('hex')}`;
      const held = await store.get<Challenge>(key);
      if (!held || held.expires <= now || held.nonce !== nonce || !sessionId.test(binding) || held.binding !== binding || held.recipient !== new URL(request.headers.get('origin')!).host) fail(400, 'That sign in request was used or expired. Please try again.');
      const challenge = await store.take<Challenge>(key);
      if (!challenge || challenge.expires <= now || challenge.nonce !== nonce || challenge.binding !== binding || challenge.recipient !== new URL(request.headers.get('origin')!).host) fail(400, 'That sign in request was used or expired. Please try again.');
      if (!verifySignature(challenge!, publicKey as string, signature as string)) fail(401, 'The wallet signature did not check out.');
      if (!await fullAccessKey(config.rpcUrl, accountId as string, publicKey as string, deps.fetcher)) fail(401, 'That key is not a full access key for this account.');
      if (accountId === config.adminWallet && pinnedAdminKey(config) && publicKey !== pinnedAdminKey(config)) fail(401, 'That key is not the pinned admin key.');
      const session: Session = { sid: randomId(), wallet: accountId as string, exp: now + SESSION_SECONDS * 1000, publicKey: publicKey as string };
      await store.put(`session:${session.sid}`, { wallet: session.wallet, publicKey: session.publicKey }, SESSION_SECONDS);
      return json({ accountId, expiresAt: session.exp, admin: isAdmin(config, session.wallet, session.publicKey) }, 200, { 'set-cookie': [sessionSetCookie(sessionToken(config.sessionSecret, session), SESSION_SECONDS, secure), challengeSetCookie('', 0, secure)] });
    }
    const session = readSession(config.sessionSecret, sessionCookie(request), now);
    const stored = await storedSession(store, session);
    if (route === 'auth/logout' && method === 'POST') {
      await store.del(`session:${session.sid}`);
      return json({ ok: true }, 200, { 'set-cookie': sessionSetCookie('', 0, secure) });
    }
    await rate(store, `support-read:${session.wallet}`, 60, 60);
    const cursor = new URL(request.url).searchParams.get('cursor');
    if (route === 'tickets' && method === 'GET') {
      const submitNonce = randomId();
      await store.put(`ticket-submit:${submitNonce}`, `${session.sid}:${session.wallet}`, 300);
      const page = await listTickets(store, session.wallet, now, cursor);
      return json({ accountId: session.wallet, tickets: page.tickets.map(({ wallet: _w, ...t }) => t), cursor: page.cursor, limits: LIMITS, submitNonce });
    }
    if (route === 'tickets' && method === 'POST') {
      const input = await body(request);
      // Check the submission token before any ticket reads. It is only used up once the request is valid,
      // so a 422 or a full open slot lets the user fix it and send again without a refresh.
      const submitKey = typeof input.submitNonce === 'string' && sessionId.test(input.submitNonce) ? `ticket-submit:${input.submitNonce}` : '';
      const owner = `${session.sid}:${session.wallet}`;
      if (!submitKey || await store.get(submitKey) !== owner) fail(400, 'That ticket request was used or expired. Please refresh and try again.');
      const fields = ticketText(input);
      await assertOpenSlot(store, session.wallet, now);
      if (await store.take(submitKey) !== owner) fail(400, 'That ticket request was used or expired. Please refresh and try again.');
      await rate(store, `ticket-wallet:${session.wallet}`, 5, 86400);
      await rate(store, `ticket-ip:${ip}`, 10, 86400);
      const ticket = await createTicket(store, session.wallet, fields, now);
      const { wallet: _w, ...visible } = ticket;
      return json({ ticket: visible }, 201);
    }
    if (route.startsWith('admin/')) {
      if (!isAdmin(config, session.wallet, stored.publicKey ?? session.publicKey)) fail(403, 'This page is only for the support owner.');
      if (route === 'admin/tickets' && method === 'GET') {
        const page = await listTickets(store, null, now, cursor);
        return json({ tickets: page.tickets, cursor: page.cursor });
      }
      if (route === 'admin/tickets' && method === 'POST') {
        const input = await body(request);
        if (input.action !== 'reply' && input.action !== 'close') fail(400, 'Choose reply or close.');
        return json({ ticket: await adminUpdate(store, input.id, input.action as 'reply' | 'close', input, now) });
      }
      if (route === 'admin/backfill' && method === 'POST') {
        // Dry run unless the body says {"apply": true}. Idempotent. See site/README.md "Support tickets".
        const input = await body(request);
        return json({ report: await backfillOpenTickets(store, now, input.apply === true) });
      }
    }
    return fail(404, 'Not found.');
  } catch (e) {
    if (e instanceof HttpError) return json({ error: e.message }, e.status);
    console.error(JSON.stringify({ event: 'api_error', route, kind: e instanceof Error ? e.name : 'unknown' }));
    return json({ error: 'Something went wrong on our side. Please try again.' }, 500);
  }
}
