import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { handle } from '../src/server/api';
import { setup, ORIGIN, ADMIN } from './helpers';
import { createTicket, listTickets, adminUpdate, OPEN_PER_WALLET } from '../src/server/tickets';
import { MemoryStore, RedisStore } from '../src/server/store';
import { storeFor, loadConfig } from '../src/server/config';
import { BODY_LIMIT, clientKey, cookieSecure, normalizeClientAddress, sessionCookieName, sessionSetCookie, setCookie } from '../src/server/security';
import { sensitive } from '../src/server/sensitive';
import { startChat, signTurn, nearAiModel, VERSION, BUSY_MS } from '../src/server/chat';

const src = (rel: string) => readFileSync(new URL(`../src/${rel}`, import.meta.url), 'utf8');

describe('F-03 / M2 chat budget and IPv6 /64 keys', () => {
  it('groups IPv6 addresses in one /64 and leaves IPv4 unchanged', () => {
    const secret = 's'.repeat(40);
    expect(normalizeClientAddress('2001:db8::1')).toBe('2001:db8:0:0::');
    expect(normalizeClientAddress('2001:db8:0:0::20')).toBe('2001:db8:0:0::');
    expect(normalizeClientAddress('2001:DB8:0:1::1')).toBe('2001:db8:0:1::');
    expect(clientKey('2001:db8::1', secret)).toBe(clientKey('2001:db8::20', secret));
    expect(clientKey('2001:db8:0:1::1', secret)).not.toBe(clientKey('2001:db8:0:0::1', secret));
    expect(normalizeClientAddress('198.51.100.9')).toBe('198.51.100.9');
    expect(normalizeClientAddress('::ffff:198.51.100.9')).toBe('198.51.100.9');
  });

  it('does not charge the global daily budget on refused turns', async () => {
    const t = setup({ CHAT_GLOBAL_DAY_CAP: '2', CHAT_SIGNED_RESERVE: '1', CHAT_DAY_CAP: '40' });
    for (let i = 0; i < 5; i++) {
      const start = await t.call('chat/start', {}, { address: `198.51.100.${i + 1}` });
      const r = await t.call('chat', { sessionId: start.body.sessionId, history: [], message: 'tell me a joke' }, { address: `198.51.100.${i + 1}` });
      expect(r.status).toBe(200);
    }
    expect(t.modelCalls.length).toBe(0);
    const honest = await t.call('chat/start', {}, { address: '203.0.113.9' });
    const ok = await t.call('chat', { sessionId: honest.body.sessionId, history: [], message: 'How does a vault pay the holder?' }, { address: '203.0.113.9' });
    expect(ok.status).toBe(200);
    expect(t.modelCalls.length).toBe(1);
  });

  it('reserves part of the global budget for signed-in wallets', async () => {
    const t = setup({ CHAT_GLOBAL_DAY_CAP: '3', CHAT_SIGNED_RESERVE: '1', CHAT_DAY_CAP: '40' });
    for (let i = 0; i < 2; i++) {
      const start = await t.call('chat/start', {}, { address: `198.51.100.${i + 20}` });
      const r = await t.call('chat', { sessionId: start.body.sessionId, history: [], message: 'How does a vault pay the holder?' }, { address: `198.51.100.${i + 20}` });
      expect(r.status).toBe(200);
    }
    const blocked = await t.call('chat/start', {}, { address: '198.51.100.30' });
    expect((await t.call('chat', { sessionId: blocked.body.sessionId, history: [], message: 'How does a vault pay the holder?' }, { address: '198.51.100.30' })).status).toBe(429);
    const cookie = await t.signIn('writer-a.royltee-dev.testnet');
    const signed = await t.call('chat/start', {}, { address: '198.51.100.40' });
    const reserved = await t.call('chat', { sessionId: signed.body.sessionId, history: [], message: 'How does a vault pay the holder?' }, { cookie, address: '198.51.100.40' });
    expect(reserved.status).toBe(200);
    const signed2 = await t.call('chat/start', {}, { address: '198.51.100.41' });
    expect((await t.call('chat', { sessionId: signed2.body.sessionId, history: [], message: 'How does a vault pay the holder?' }, { cookie, address: '198.51.100.41' })).status).toBe(429);
  });
});

describe('M3 ticket list is newest first with a cursor', () => {
  it('returns the newest page first and walks older tickets with a cursor, including after closes', async () => {
    const clock = { now: 1_000_000 };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < 501; i++) {
      await createTicket(store, `w${i}.royltee-dev.testnet`, { subject: `S${i}`, message: 'demo ticket text' }, clock.now + i);
    }
    const first = await listTickets(store, null, clock.now + 1000, null, 100);
    expect(first.tickets).toHaveLength(100);
    expect(first.tickets[0].subject).toBe('S500');
    expect(first.tickets[99].subject).toBe('S401');
    expect(first.cursor).toBeTruthy();
    let cursor = first.cursor;
    const seen = new Set(first.tickets.map(t => t.subject));
    while (cursor) {
      const page = await listTickets(store, null, clock.now + 1000, cursor, 100);
      for (const t of page.tickets) {
        expect(seen.has(t.subject)).toBe(false);
        seen.add(t.subject);
      }
      cursor = page.cursor;
    }
    expect(seen.has('S0')).toBe(true);
    expect(seen.size).toBe(501);
    for (const t of first.tickets) await adminUpdate(store, t.id, 'close', {}, clock.now + 2000);
    const afterClose = await listTickets(store, null, clock.now + 3000, null, 100);
    expect(afterClose.tickets[0].subject).toBe('S500');
    const older = await listTickets(store, null, clock.now + 3000, afterClose.cursor, 100);
    expect(older.tickets.some(t => t.subject === 'S400')).toBe(true);
  });

  it('caps open tickets per wallet without spending the daily quota first', async () => {
    const t = setup();
    await t.signIn('writer-a.royltee-dev.testnet');
    for (let day = 0; day < 3; day++) {
      if (day) {
        t.clock.now += 86400_000 + 1;
        await t.signIn('writer-a.royltee-dev.testnet');
      }
      for (let i = 0; i < 5; i++) {
        const open = (await t.call('tickets')).body.tickets.filter((x: { status: string }) => x.status !== 'closed').length;
        if (open >= OPEN_PER_WALLET) break;
        const r = await t.call('tickets', { subject: `Q${day}-${i}`, message: 'demo' });
        expect(r.status).toBe(201);
      }
    }
    const listed = (await t.call('tickets')).body.tickets.filter((x: { status: string }) => x.status !== 'closed');
    expect(listed.length).toBe(OPEN_PER_WALLET);
    const denied = await t.call('tickets', { subject: 'One more', message: 'demo' });
    expect(denied.status).toBe(429);
    expect(denied.body.error).toMatch(/maximum number of open tickets/);
  });
});

describe('F-05 finish_reason and I2 tee in the turn HMAC', () => {
  it('returns 503 for finish_reason length, signs nothing, and does not set a TEE label', async () => {
    const t = setup();
    const fetcher = (async (url: string) => {
      if (String(url).endsWith('/chat/completions')) {
        return new Response(JSON.stringify({ id: 'chatcmpl-poc', choices: [{ index: 0, message: { role: 'assistant', content: 'The registry pays the vault, and the holder can claim only after' }, finish_reason: 'length' }] }));
      }
      throw new Error('signature must not be read for a cut-off reply');
    }) as typeof fetch;
    t.deps.model = nearAiModel(t.config, fetcher);
    const start = await t.call('chat/start', {});
    const r = await t.call('chat', { sessionId: start.body.sessionId, history: [], message: 'When can a holder claim?' });
    expect(r.status).toBe(503);
    expect(r.body.assistant).toBeUndefined();
    expect(r.body.tee).toBeUndefined();
    expect(r.body.error).toMatch(/could not finish/);
    t.deps.model = async () => ({ text: 'A vault pays the current holder. [How it works](/#how)', tee: false });
    const retry = await t.call('chat', { sessionId: start.body.sessionId, history: [], message: 'When can a holder claim?' });
    expect(retry.status).toBe(200);
    expect(retry.body.tee).toBe(false);
  });

  it('accepts only finish_reason stop, and binds tee in the signature', async () => {
    const t = setup();
    const fetcher = (async (url: string) => {
      if (String(url).endsWith('/chat/completions')) {
        return new Response(JSON.stringify({ id: 'chatcmpl-ok', choices: [{ index: 0, message: { role: 'assistant', content: 'A vault pays the holder. [How it works](/#how)' }, finish_reason: 'stop' }] }));
      }
      return new Response(JSON.stringify({ signature_kind: 'provider_tee' }));
    }) as typeof fetch;
    t.deps.model = nearAiModel(t.config, fetcher);
    const start = await t.call('chat/start', {});
    const r = await t.call('chat', { sessionId: start.body.sessionId, history: [], message: 'How does a vault pay the holder?' });
    expect(r.status).toBe(200);
    expect(r.body.tee).toBe(true);
    expect(r.body.assistant.signature).toBe(signTurn(t.config.chatSecret, start.body.sessionId, 0, r.body.assistant.text, true));
    expect(r.body.assistant.signature).not.toBe(signTurn(t.config.chatSecret, start.body.sessionId, 0, r.body.assistant.text, false));
    expect(r.body.version).toBe(VERSION);
    const flipped = { ...r.body.assistant, tee: false };
    const replay = await t.call('chat', { sessionId: start.body.sessionId, history: [{ role: 'user', text: 'How does a vault pay the holder?' }, flipped], message: 'And after a sale?' });
    expect(replay.status).toBe(400);
  });
});

describe('L3 L4 L5 L6 L7', () => {
  it('L3 forces a Secure __Host- cookie on Vercel even when request.url is http', () => {
    const prev = process.env.VERCEL;
    process.env.VERCEL = '1';
    try {
      const req = new Request('http://royltee.example/api/auth/verify');
      expect(cookieSecure(req)).toBe(true);
      expect(sessionCookieName(true)).toBe('__Host-royltee_support');
      const header = sessionSetCookie('tok', 900, true);
      expect(header.startsWith('__Host-royltee_support=')).toBe(true);
      expect(header).toContain('Secure');
      expect(header).toContain('Path=/');
      expect(header).not.toMatch(/Domain=/);
    } finally {
      if (prev === undefined) delete process.env.VERCEL;
      else process.env.VERCEL = prev;
    }
    expect(setCookie('royltee_support', 'tok', 900, false)).not.toContain('Secure');
  });

  it('L4 expires a stuck busy flag after about 90 seconds without dropping the chat', async () => {
    const t = setup();
    const id = 'ab'.repeat(24);
    await startChat(t.store, id);
    const state = await t.store.get<{ busy: boolean; count: number; digest: string }>(`chat:${id}`);
    await t.store.cas(`chat:${id}`, state, { ...state, busy: true, busyUntil: t.clock.now + BUSY_MS }, 3600);
    expect((await t.call('chat', { sessionId: id, history: [], message: 'How does a vault pay the holder?' })).status).toBe(400);
    t.clock.now += 91_000;
    const r = await t.call('chat', { sessionId: id, history: [], message: 'How does a vault pay the holder?' });
    expect(r.status).toBe(200);
  });

  it('L5 validates before spending the daily ticket quota, and 1.000.000 is not a phone number', async () => {
    expect(sensitive('Payment of 1.000.000 mUSDC to s1')).toBeNull();
    expect(sensitive('call +1 (555) 010-0199')).toBe('a phone number');
    const t = setup();
    const a = await t.signIn('writer-a.royltee-dev.testnet');
    for (let i = 0; i < 5; i++) {
      const r = await t.call('tickets', { subject: 'Help', message: 'reach me at sam@example.com' }, { cookie: a });
      expect(r.status).toBe(422);
    }
    const ok = await t.call('tickets', { subject: 'Real question', message: 'demo ticket about a vault' }, { cookie: a });
    expect(ok.status).toBe(201);
  });

  it('L6 builds the store inside handle so a bad storage URL becomes a generic 500', async () => {
    expect(() => new RedisStore('http://example.invalid', 't'.repeat(40))).toThrow(/HTTPS/);
    const config = loadConfig({
      STORAGE_MODE: 'redis',
      KV_REST_API_URL: 'http://example.invalid',
      KV_REST_API_TOKEN: 't'.repeat(40),
      CHAT_HMAC_SECRET: 'c'.repeat(40),
      SESSION_SECRET: 's'.repeat(40),
      ALLOWED_ORIGINS: ORIGIN,
    });
    expect(() => storeFor(config)).toThrow(/HTTPS/);
    const req = new Request(`${ORIGIN}/api/chat/start`, { method: 'POST', headers: { origin: ORIGIN, 'content-type': 'application/json' }, body: '{}' });
    const res = await handle(req, 'chat/start', { config, model: async () => 'x', address: '203.0.113.7' });
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: 'Something went wrong on our side. Please try again.' });
  });

  it('L7 accepts a legal 11-turn history under the 128 KiB body cap', async () => {
    const t = setup();
    const start = await t.call('chat/start', {});
    const honest = [];
    for (let i = 0; i < 11; i++) {
      honest.push({ role: 'user', text: 'u'.repeat(1500) });
      honest.push({ role: 'assistant', text: 'a'.repeat(6000), index: i, signature: 'f'.repeat(64), tee: false });
    }
    const payload = JSON.stringify({ sessionId: start.body.sessionId, history: honest, message: 'm'.repeat(1500) });
    expect(Buffer.byteLength(payload)).toBeGreaterThan(65536);
    expect(Buffer.byteLength(payload)).toBeLessThan(BODY_LIMIT);
    expect(BODY_LIMIT).toBe(128 * 1024);
    const r = await t.call('chat', payload, { type: 'application/json' });
    expect(r.status).not.toBe(413);
    expect(r.status).toBe(400);
  });
});

describe('M4 L1 L2 optional admin public key pin', () => {
  it('when ADMIN_PUBLIC_KEY is set, only that key can open an admin session', async () => {
    const t = setup();
    const w = await t.wallet(ADMIN);
    const key = w.kp.getPublicKey().toString();
    const pinned = setup({ ADMIN_PUBLIC_KEY: key });
    pinned.fullAccess(ADMIN, key);
    const other = await pinned.wallet(ADMIN);
    const ch = await pinned.call('auth/challenge', {});
    expect((await pinned.call('auth/verify', await other.sign(ch.body))).status).toBe(401);
    const ok = await pinned.call('auth/verify', await w.sign((await pinned.call('auth/challenge', {})).body));
    expect(ok.status).toBe(200);
    expect(ok.body.admin).toBe(true);
    expect((await pinned.call('admin/tickets', undefined, { cookie: ok.cookie!.split(';')[0] })).status).toBe(200);
    expect(src('server/config.ts')).toMatch(/adminPublicKey/);
  });
});

describe('F-07 F-08 F-15 static review', () => {
  it('vercel.json sets framing, CSP, nosniff, referrer, permissions and HSTS headers', () => {
    const conf = JSON.parse(readFileSync(new URL('../vercel.json', import.meta.url), 'utf8'));
    const headers = Object.fromEntries((conf.headers ?? []).flatMap((b: { source: string; headers: { key: string; value: string }[] }) => b.headers.map(h => [h.key, h.value])));
    expect(headers['X-Frame-Options']).toBe('DENY');
    expect(headers['X-Content-Type-Options']).toBe('nosniff');
    expect(headers['Referrer-Policy']).toBe('strict-origin-when-cross-origin');
    expect(headers['Permissions-Policy']).toMatch(/camera=\(\)/);
    expect(headers['Strict-Transport-Security']).toMatch(/max-age=63072000/);
    expect(headers['Content-Security-Policy']).toMatch(/default-src 'self'/);
    expect(headers['Content-Security-Policy']).toMatch(/frame-ancestors 'none'/);
    expect(headers['Content-Security-Policy']).toMatch(/connect-src[^;]*https:\/\/test\.rpc\.fastnear\.com/);
    expect(headers['Content-Security-Policy']).not.toMatch(/fonts\.googleapis|fonts\.gstatic/);
  });

  it('admin.astro checks for an empty session secret, and the dashboard does not use innerHTML or Google Fonts', () => {
    expect(src('pages/admin.astro')).toMatch(/!config\.sessionSecret/);
    const dash = src('content/dashboard.html') + readFileSync(new URL('../public/dashboard.js', import.meta.url), 'utf8');
    expect(dash).not.toMatch(/innerHTML|fonts\.googleapis|fonts\.gstatic/);
    const registry = ['catalog', 'sharecard', 'share', 'member', 'agents', 'disputes', 'ui', 'chain', 'nav'].map(f => src(`scripts/${f}.ts`)).join('\n');
    expect(registry).not.toMatch(/innerHTML|outerHTML|insertAdjacentHTML|document\.write|fonts\.googleapis|fonts\.gstatic|eval\(/);
  });

  it('documents the optional admin public key pin and open ticket expiry', () => {
    const readme = readFileSync(new URL('../README.md', import.meta.url), 'utf8');
    expect(readme).toMatch(/ADMIN_PUBLIC_KEY/);
    expect(readme).toMatch(/Open tickets expire 30 days/);
    expect(readme).not.toMatch(/Open tickets have no expiry/);
  });
});
