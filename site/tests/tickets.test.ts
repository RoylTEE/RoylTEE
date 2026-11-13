import { setup, ADMIN } from './helpers';
import { nep413Hash, verifySignature } from '../src/server/auth';
import { sensitive } from '../src/server/sensitive';
const A = 'writer-a.royltee-dev.testnet', B = 'writer-b.royltee-dev.testnet';
describe('NEP-413 sign in', () => {
  it('hashes the NEP-413 tag and every payload field', () => {
    const n = Uint8Array.from({ length: 32 }, (_, i) => i);
    const h = nep413Hash('hi', n, 'myapp.com');
    expect(h).toHaveLength(32);
    expect(nep413Hash('hi', n, 'myapp.co').equals(h)).toBe(false);
    expect(nep413Hash('ho', n, 'myapp.com').equals(h)).toBe(false);
    expect(() => nep413Hash('hi', n.slice(1), 'myapp.com')).toThrow();
  });
  it('accepts a real wallet signature and sets a 15 minute HttpOnly session', async () => {
    const t = setup(); const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    expect(ch.body.recipient).toBe('royltee.example');
    const r = await t.call('auth/verify', await w.sign(ch.body));
    expect(r.status).toBe(200);
    expect(r.cookie).toMatch(/^__Host-royltee_support=/);
    expect(r.cookie).toMatch(/HttpOnly; SameSite=Strict; Path=\/; Max-Age=900; Secure/);
    expect(t.rpcCalls[0]).toMatchObject({ request_type: 'view_access_key', account_id: A, finality: 'final' });
  });
  it('rejects a bad signature', async () => {
    const t = setup(); const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    const signed = await w.sign(ch.body);
    const sig = Buffer.from(signed.signature, 'base64'); sig[0] ^= 1;
    expect((await t.call('auth/verify', { ...signed, signature: sig.toString('base64') })).status).toBe(401);
  });
  it('rejects a signature for another recipient or message', async () => {
    const t = setup(); const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    const other = await w.sign({ ...ch.body, recipient: 'evil.example' });
    expect((await t.call('auth/verify', other)).status).toBe(401);
    const ch2 = await t.call('auth/challenge', {});
    expect((await t.call('auth/verify', await w.sign({ ...ch2.body, message: 'Pay me' }))).status).toBe(401);
  });
  it('rejects a replayed signed message', async () => {
    const t = setup(); const w = await t.wallet(A);
    const signed = await w.sign((await t.call('auth/challenge', {})).body);
    expect((await t.call('auth/verify', signed)).status).toBe(200);
    expect((await t.call('auth/verify', signed)).status).toBe(400);
  });
  it('rejects an expired nonce and a nonce the server never issued', async () => {
    const t = setup(); const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    t.clock.now += 301_000;
    expect((await t.call('auth/verify', await w.sign(ch.body))).status).toBe(400);
    expect((await t.call('auth/verify', await w.sign({ ...ch.body, nonce: Buffer.alloc(32, 7).toString('base64') }))).status).toBe(400);
  });
  it('rejects a key that is not a full access key of the account', async () => {
    const t = setup(); const w = await t.wallet(A, false);
    expect((await t.call('auth/verify', await w.sign((await t.call('auth/challenge', {})).body))).status).toBe(401);
    expect(verifySignature({ message: 'x', recipient: 'y', nonce: Buffer.alloc(32).toString('base64'), expires: 0, binding: '' }, 'ed25519:bad', 'x')).toBe(false);
  });
  it('requires a new sign in after the session expires or after sign out', async () => {
    const t = setup(); await t.signIn(A);
    expect((await t.call('tickets')).status).toBe(200);
    t.clock.now += 15 * 60_000 + 1;
    expect((await t.call('tickets')).status).toBe(401);
    await t.signIn(A);
    expect((await t.call('auth/logout', {})).status).toBe(200);
    expect((await t.call('tickets')).status).toBe(401);
  });
  it('rejects a forged session cookie', async () => {
    const t = setup(); const jar = await t.signIn(A);
    const [data, mac] = jar.split('=')[1].split('.');
    const forged = Buffer.from(JSON.stringify({ ...JSON.parse(Buffer.from(data, 'base64url').toString()), wallet: ADMIN })).toString('base64url');
    expect((await t.call('admin/tickets', undefined, { cookie: `royltee_support=${forged}.${mac}` })).status).toBe(401);
  });
});
describe('support tickets', () => {
  it('creates and lists tickets for the signed in wallet only', async () => {
    const t = setup(); const a = await t.signIn(A);
    const made = await t.call('tickets', { subject: 'Vault question', message: 'Does s1 keep its balance after a sale?' }, { cookie: a });
    expect(made.status).toBe(201); expect(made.body.ticket).not.toHaveProperty('wallet');
    const b = await t.signIn(B);
    expect((await t.call('tickets', undefined, { cookie: b })).body.tickets).toEqual([]);
    expect((await t.call('tickets', undefined, { cookie: a })).body.tickets.map((x: any) => x.subject)).toEqual(['Vault question']);
    expect((await t.call('admin/tickets', { action: 'close', id: made.body.ticket.id }, { cookie: b })).status).toBe(403);
  });
  it('rejects ticket posts without a session, from another origin or as plain text', async () => {
    const t = setup(); const a = await t.signIn(A);
    expect((await t.call('tickets', { subject: 's', message: 'm' }, { cookie: '' })).status).toBe(401);
    expect((await t.call('tickets', { subject: 's', message: 'm' }, { cookie: a, origin: 'https://evil.example' })).status).toBe(403);
    expect((await t.call('tickets', JSON.stringify({ subject: 's', message: 'm' }), { cookie: a, type: 'text/plain' })).status).toBe(415);
  });
  it('rejects a replayed ticket submission and a nonce from another session', async () => {
    const t = setup(); const a = await t.signIn(A);
    const submitNonce = (await t.call('tickets', undefined, { cookie: a })).body.submitNonce;
    expect((await t.call('tickets', { subject: 'One', message: 'demo', submitNonce }, { cookie: a })).status).toBe(201);
    expect((await t.call('tickets', { subject: 'One', message: 'demo', submitNonce }, { cookie: a })).status).toBe(400);
    const other = (await t.call('tickets', undefined, { cookie: a })).body.submitNonce;
    const b = await t.signIn(B);
    expect((await t.call('tickets', { subject: 'Two', message: 'demo', submitNonce: other }, { cookie: b })).status).toBe(400);
    expect((await t.call('tickets', { subject: 'Three', message: 'demo', submitNonce: 'x' }, { cookie: b })).status).toBe(400);
  });
  it('rejects oversize and empty input', async () => {
    const t = setup(); const a = await t.signIn(A);
    expect((await t.call('tickets', { subject: 'x'.repeat(121), message: 'ok' }, { cookie: a })).status).toBe(400);
    expect((await t.call('tickets', { subject: 'ok', message: 'x'.repeat(2001) }, { cookie: a })).status).toBe(400);
    expect((await t.call('tickets', { subject: ' ', message: 'ok' }, { cookie: a })).status).toBe(400);
    expect((await t.call('tickets', JSON.stringify({ subject: 'ok', message: 'x'.repeat(200000) }), { cookie: a })).status).toBe(413);
  });
  it.each([
    ['email', 'reach me at sam@example.com'], ['phone', 'call +1 (555) 010-0199'], ['card', 'card 4111 1111 1111 1111'],
    ['seed phrase', 'abandon ability able about above absent absorb abstract absurd abuse access accident'],
    ['private key', `ed25519:${'3'.repeat(88)}`],
  ])('refuses text that looks like a real %s', async (_k, message) => {
    const t = setup(); const a = await t.signIn(A);
    const r = await t.call('tickets', { subject: 'Help', message }, { cookie: a });
    expect(r.status).toBe(422);
  });
  it('allows normal demo text with dates, amounts, hashes and public keys', async () => {
    const publicKey = (await setup().wallet(A, false)).kp.getPublicKey().toString();
    for (const ok of ['Payment of 1000000 base units on 2026-11-13 to s3', 'Payment of 1.000.000 mUSDC to s1', 'doc hash ' + 'a'.repeat(64), `public ${publicKey}`, 'please help with the share vault after a sale today', 'Order 1234567890123 failed']) expect(sensitive(ok), ok).toBeNull();
  });
  it('rate limits per wallet and per IP', async () => {
    const t = setup(); const a = await t.signIn(A);
    for (let i = 0; i < 5; i++) expect((await t.call('tickets', { subject: `Q${i}`, message: 'demo' }, { cookie: a })).status).toBe(201);
    expect((await t.call('tickets', { subject: 'Q6', message: 'demo' }, { cookie: a })).status).toBe(429);
    const b = await t.signIn(B);
    for (let i = 0; i < 5; i++) expect((await t.call('tickets', { subject: `B${i}`, message: 'demo' }, { cookie: b })).status).toBe(201);
    const c = await t.signIn('buyer-pub.royltee-dev.testnet');
    expect((await t.call('tickets', { subject: 'C', message: 'demo' }, { cookie: c })).status).toBe(429);
    expect((await t.call('tickets', { subject: 'C', message: 'demo' }, { cookie: c, address: '198.51.100.9' })).status).toBe(201);
  });
  it('gates admin routes to the owner wallet', async () => {
    const t = setup(); const a = await t.signIn(A);
    await t.call('tickets', { subject: 'Hello', message: 'demo' }, { cookie: a });
    expect((await t.call('admin/tickets', undefined, { cookie: a })).status).toBe(403);
    expect((await t.call('admin/tickets', undefined, { cookie: '' })).status).toBe(401);
    const admin = await t.signIn(ADMIN);
    const list = await t.call('admin/tickets', undefined, { cookie: admin });
    expect(list.status).toBe(200); expect(list.body.tickets[0].wallet).toBe(A);
    const unset = setup({ ADMIN_WALLET: '' }); const x = await unset.signIn(ADMIN);
    expect((await unset.call('admin/tickets', undefined, { cookie: x })).status).toBe(403);
  });
  it('lets the owner reply and close, and the user sees the reply', async () => {
    const t = setup(); const a = await t.signIn(A);
    const id = (await t.call('tickets', { subject: 'Claim', message: 'Claim failed on s1' }, { cookie: a })).body.ticket.id;
    const admin = await t.signIn(ADMIN);
    expect((await t.call('admin/tickets', { action: 'reply', id, reply: 'Simulated reply: retry after the lock clears.' }, { cookie: admin })).body.ticket.status).toBe('answered');
    const mine = (await t.call('tickets', undefined, { cookie: a })).body.tickets[0];
    expect(mine).toMatchObject({ status: 'answered', reply: 'Simulated reply: retry after the lock clears.' });
    expect((await t.call('admin/tickets', { action: 'close', id }, { cookie: admin })).body.ticket.status).toBe('closed');
    expect((await t.call('admin/tickets', { action: 'reply', id, reply: 'late' }, { cookie: admin })).status).toBe(409);
  });
  it('deletes tickets 30 days after close, and expires open tickets 30 days after the last update', async () => {
    const t = setup(); const a = await t.signIn(A);
    const closed = (await t.call('tickets', { subject: 'Old', message: 'demo' }, { cookie: a })).body.ticket.id;
    const openEarly = (await t.call('tickets', { subject: 'Still open', message: 'demo' }, { cookie: a })).body.ticket.id;
    const admin = await t.signIn(ADMIN);
    await t.call('admin/tickets', { action: 'close', id: closed }, { cookie: admin });
    t.clock.now += 20 * 86400_000;
    let a2 = await t.signIn(A);
    const later = (await t.call('tickets', { subject: 'Newer open', message: 'demo' }, { cookie: a2 })).body.ticket.id;
    expect(later).toBeTruthy();
    t.clock.now += 9 * 86400_000;
    a2 = await t.signIn(A);
    expect((await t.call('tickets', undefined, { cookie: a2 })).body.tickets.map((x: any) => x.subject).sort()).toEqual(['Newer open', 'Still open', 'Old'].sort());
    t.clock.now += 2 * 86400_000;
    expect((await t.call('cron/sweep', undefined, { auth: 'Bearer wrong' })).status).toBe(401);
    const swept = await t.call('cron/sweep', undefined, { auth: `Bearer ${'k'.repeat(40)}` });
    expect(swept.body.removed).toBe(2); // the closed ticket, and since audit F5 the expired open ticket's index entries
    expect(await t.store.get(`ticket:${closed}`)).toBeNull();
    expect(await t.store.get(`ticket:${openEarly}`)).toBeNull();
    a2 = await t.signIn(A);
    expect((await t.call('tickets', undefined, { cookie: a2 })).body.tickets.map((x: any) => x.subject)).toEqual(['Newer open']);
  });
  it('closed tickets also expire by storage TTL without a sweep', async () => {
    const t = setup(); const a = await t.signIn(A);
    const id = (await t.call('tickets', { subject: 'TTL', message: 'demo' }, { cookie: a })).body.ticket.id;
    const admin = await t.signIn(ADMIN);
    await t.call('admin/tickets', { action: 'close', id }, { cookie: admin });
    t.clock.now += 30 * 86400_000 + 1;
    expect(await t.store.get(`ticket:${id}`)).toBeNull();
  });
  it('fails closed when storage or secrets are missing', async () => {
    const t = setup({ SESSION_SECRET: 'short' });
    expect((await t.call('auth/challenge', {})).status).toBe(503);
    const t2 = setup(); t2.deps.store = null;
    expect((await t2.call('auth/challenge', {})).status).toBe(503);
  });
});
