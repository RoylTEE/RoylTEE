import { randomBytes } from 'node:crypto';
import type { Config } from './config';
import type { Store } from './store';
import { ACCOUNT_ID, HttpError, body, clientKey, cookie, digest, equal, fail, json, originCheck, randomId, rate, setCookie } from './security';
import { fullAccessKey, readSession, sessionToken, verifySignature, type Challenge, type Session } from './auth';
import { chat, startChat, type Model } from './chat';
import { adminUpdate, createTicket, listTickets, sweep, LIMITS } from './tickets';
export type Deps = { config: Config; store: Store | null; model: Model; now?: () => number; fetcher?: typeof fetch; address?: string };
export const SESSION_COOKIE = 'royltee_support';
export const SESSION_SECONDS = 15 * 60;
export const SIGN_IN_MESSAGE = 'Sign in to RoylTEE support on NEAR testnet. This does not create a transaction.';
const sessionId = /^[a-f0-9]{48}$/;
export async function handle(request: Request, route: string, deps: Deps): Promise<Response> {
  const now = deps.now?.() ?? Date.now();
  try {
    const { config } = deps;
    const store = deps.store ?? fail(503, 'Support storage is not configured yet.');
    if (!config.allowedOrigins.length) fail(503, 'This feature is not configured yet.');
    const method = request.method;
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
      return json(await chat(store, config, input.sessionId as string, input, ip, deps.model));
    }
    if (!config.sessionSecret || !config.rpcUrl) fail(503, 'Support sign in is not configured yet.');
    if (route === 'auth/challenge' && method === 'POST') {
      await rate(store, `challenge:${ip}`, 30, 600);
      const recipient = new URL(request.headers.get('origin')!).host;
      const challenge: Challenge = { message: SIGN_IN_MESSAGE, recipient, nonce: randomBytes(32).toString('base64'), expires: now + 300000, binding: ip };
      await store.put(`challenge:${Buffer.from(challenge.nonce, 'base64').toString('hex')}`, challenge, 300);
      return json({ message: challenge.message, recipient, nonce: challenge.nonce, expiresAt: challenge.expires });
    }
    if (route === 'auth/verify' && method === 'POST') {
      await rate(store, `verify:${ip}`, 20, 600);
      const input = await body(request);
      const { accountId, publicKey, signature, nonce } = input;
      if (typeof accountId !== 'string' || !ACCOUNT_ID.test(accountId) || typeof publicKey !== 'string' || typeof signature !== 'string' || typeof nonce !== 'string' || !/^[A-Za-z0-9+/]{43}=$/.test(nonce)) fail(400, 'Sign in data is not valid.');
      const challenge = await store.take<Challenge>(`challenge:${Buffer.from(nonce as string, 'base64').toString('hex')}`);
      if (!challenge || challenge.expires <= now || challenge.nonce !== nonce || challenge.binding !== ip || challenge.recipient !== new URL(request.headers.get('origin')!).host) fail(400, 'That sign in request was used or expired. Please try again.');
      if (!verifySignature(challenge!, publicKey as string, signature as string)) fail(401, 'The wallet signature did not check out.');
      if (!await fullAccessKey(config.rpcUrl, accountId as string, publicKey as string, deps.fetcher)) fail(401, 'That key is not a full access key for this account.');
      const session: Session = { sid: randomId(), wallet: accountId as string, exp: now + SESSION_SECONDS * 1000 };
      await store.put(`session:${session.sid}`, session.wallet, SESSION_SECONDS);
      return json({ accountId, expiresAt: session.exp, admin: accountId === config.adminWallet }, 200, { 'set-cookie': setCookie(SESSION_COOKIE, sessionToken(config.sessionSecret, session), SESSION_SECONDS, new URL(request.url).protocol === 'https:') });
    }
    const session = readSession(config.sessionSecret, cookie(request, SESSION_COOKIE), now);
    if (await store.get<string>(`session:${session.sid}`) !== session.wallet) fail(401, 'Please sign in with your wallet again.');
    if (route === 'auth/logout' && method === 'POST') {
      await store.del(`session:${session.sid}`);
      return json({ ok: true }, 200, { 'set-cookie': setCookie(SESSION_COOKIE, '', 0, new URL(request.url).protocol === 'https:') });
    }
    await rate(store, `support-read:${session.wallet}`, 60, 60);
    if (route === 'tickets' && method === 'GET') {
      const submitNonce = randomId();
      await store.put(`ticket-submit:${submitNonce}`, `${session.sid}:${session.wallet}`, 300);
      return json({ accountId: session.wallet, tickets: (await listTickets(store, session.wallet, now)).map(({ wallet: _w, ...t }) => t), limits: LIMITS, submitNonce });
    }
    if (route === 'tickets' && method === 'POST') {
      await rate(store, `ticket-wallet:${session.wallet}`, 5, 86400);
      await rate(store, `ticket-ip:${ip}`, 10, 86400);
      const input = await body(request);
      if (typeof input.submitNonce !== 'string' || !sessionId.test(input.submitNonce) || await store.take(`ticket-submit:${input.submitNonce}`) !== `${session.sid}:${session.wallet}`) fail(400, 'That ticket request was used or expired. Please refresh and try again.');
      const ticket = await createTicket(store, session.wallet, input, now);
      const { wallet: _w, ...visible } = ticket;
      return json({ ticket: visible }, 201);
    }
    if (route.startsWith('admin/')) {
      if (!config.adminWallet || session.wallet !== config.adminWallet) fail(403, 'This page is only for the support owner.');
      if (route === 'admin/tickets' && method === 'GET') return json({ tickets: await listTickets(store, null, now) });
      if (route === 'admin/tickets' && method === 'POST') {
        const input = await body(request);
        if (input.action !== 'reply' && input.action !== 'close') fail(400, 'Choose reply or close.');
        return json({ ticket: await adminUpdate(store, input.id, input.action as 'reply' | 'close', input, now) });
      }
    }
    return fail(404, 'Not found.');
  } catch (e) {
    if (e instanceof HttpError) return json({ error: e.message }, e.status);
    console.error(JSON.stringify({ event: 'api_error', route, kind: e instanceof Error ? e.name : 'unknown' }));
    return json({ error: 'Something went wrong on our side. Please try again.' }, 500);
  }
}
