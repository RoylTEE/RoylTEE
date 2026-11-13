import { describe, expect, it, vi } from 'vitest';
import { setup } from './helpers';
import { adminUpdate, createTicket, openCount, OPEN_PER_WALLET, OPEN_SECONDS } from '../src/server/tickets';

describe('F-17 ticket submission checks the token before bounded reads', () => {
  const W = 'writer-a.royltee-dev.testnet';
  async function withHistory(closed: number) {
    const t = setup();
    for (let i = 0; i < closed; i++) {
      const ticket = await createTicket(t.store, W, { subject: `Old ${i}`, message: 'demo' }, t.clock.now - 1000 + i);
      await adminUpdate(t.store, ticket.id, 'close', {}, t.clock.now);
    }
    await t.signIn(W);
    return t;
  }
  const ticketReads = (t: ReturnType<typeof setup>) => {
    const spy = vi.spyOn(t.store, 'get');
    return { count: () => spy.mock.calls.filter(([k]) => String(k).startsWith('ticket:')).length, stop: () => spy.mockRestore() };
  };

  it('PoC: a bad or missing submission token is refused with zero ticket reads', async () => {
    const t = await withHistory(300);
    const reads = ticketReads(t);
    expect((await t.call('tickets', { subject: 'Hi', message: 'demo', submitNonce: 'f'.repeat(48) })).status).toBe(400);
    expect((await t.call('tickets', { subject: 'Hi', message: 'demo', submitNonce: 'nope' })).status).toBe(400);
    expect(reads.count()).toBe(0);
    reads.stop();
  });

  it('PoC: a valid submission reads a bounded number of tickets however long the history is', async () => {
    const t = await withHistory(300);
    const nonce = (await t.call('tickets')).body.submitNonce;
    const reads = ticketReads(t);
    expect((await t.call('tickets', { subject: 'New one', message: 'demo', submitNonce: nonce })).status).toBe(201);
    expect(reads.count()).toBeLessThanOrEqual(2 * OPEN_PER_WALLET);
    reads.stop();
  });

  it('a 422 or a full open slot does not use up the submission token', async () => {
    const t = await withHistory(0);
    const nonce = (await t.call('tickets')).body.submitNonce;
    expect((await t.call('tickets', { subject: 'Mail me', message: 'me@example.com', submitNonce: nonce })).status).toBe(422);
    expect((await t.call('tickets', { subject: 'Fixed', message: 'demo', submitNonce: nonce })).status).toBe(201);
    expect((await t.call('tickets', { subject: 'Again', message: 'demo', submitNonce: nonce })).status).toBe(400);
  });

  it('the open-ticket index tracks create, reply, close and record expiry', async () => {
    const t = setup();
    const ids: string[] = [];
    for (let i = 0; i < OPEN_PER_WALLET; i++) ids.push((await createTicket(t.store, W, { subject: `S${i}`, message: 'demo' }, t.clock.now)).id);
    expect(await openCount(t.store, W, t.clock.now)).toBe(OPEN_PER_WALLET);
    await expect(createTicket(t.store, W, { subject: 'over', message: 'demo' }, t.clock.now)).rejects.toMatchObject({ status: 429 });
    await adminUpdate(t.store, ids[0], 'close', {}, t.clock.now);
    expect(await openCount(t.store, W, t.clock.now)).toBe(OPEN_PER_WALLET - 1);
    t.clock.now += OPEN_SECONDS * 1000 - 10_000;
    await adminUpdate(t.store, ids[1], 'reply', { reply: 'Thanks' }, t.clock.now);
    t.clock.now += 20_000;
    expect(await openCount(t.store, W, t.clock.now)).toBe(1);
  });
});
