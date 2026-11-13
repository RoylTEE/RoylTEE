/* External audit finding F5: open tickets that expire without being closed leave no index entries. */
import { MemoryStore } from '../src/server/store';
import { adminUpdate, backfillOpenTickets, createTicket, listTickets, OPEN_SECONDS, sweep, type Ticket } from '../src/server/tickets';

const W = 'writer-a.royltee-dev.testnet';
const DAY = 24 * 3600 * 1000;
type Peek = { indexes: Map<string, Map<string, number>> };
const members = (store: MemoryStore, index: string) => [...((store as unknown as Peek).indexes.get(`${index}`)?.keys() ?? [])];

describe('audit F5: the sweep drops the index entries of expired open tickets', () => {
  it('removes an expired open ticket from every index and keeps a live answered one', async () => {
    const clock = { now: Date.UTC(2026, 10, 14) };
    const store = new MemoryStore(() => clock.now);
    const stale = await createTicket(store, W, { subject: 'Old', message: 'demo ticket text' }, clock.now);
    const kept = await createTicket(store, W, { subject: 'Kept', message: 'demo ticket text' }, clock.now + 1);
    clock.now += 20 * DAY;
    await adminUpdate(store, kept.id, 'reply', { reply: 'demo reply text' }, clock.now); // renews its TTL
    clock.now += 11 * DAY; // the first ticket's TTL has ended, the answered one has 19 days left
    expect(await store.get(`ticket:${stale.id}`)).toBeNull();
    expect(await sweep(store, clock.now)).toBe(1);
    for (const index of ['tickets:all', `tickets:wallet:${W}`, `tickets:open:${W}`]) {
      expect(members(store, index)).toEqual([kept.id]);
    }
    expect(members(store, 'tickets:expiry')).toEqual([`${W}|${kept.id}`]);
    expect((await listTickets(store, null, clock.now)).tickets.map((t: Ticket) => t.id)).toEqual([kept.id]);
    expect(await sweep(store, clock.now)).toBe(0);
  });
  it('a closed ticket leaves the expiry index at once and is left to the closed sweep', async () => {
    const clock = { now: Date.UTC(2026, 10, 14) };
    const store = new MemoryStore(() => clock.now);
    const t = await createTicket(store, W, { subject: 'Done', message: 'demo ticket text' }, clock.now);
    await adminUpdate(store, t.id, 'close', {}, clock.now + 1);
    expect(members(store, 'tickets:expiry')).toEqual([]);
    expect(await sweep(store, clock.now + 10 * DAY)).toBe(0); // closed 10 days ago: kept for 30 days after close
    expect(members(store, 'tickets:all')).toEqual([t.id]);
    expect(await sweep(store, clock.now + OPEN_SECONDS * 1000 + 2)).toBe(1); // the closed sweep removes it
  });
  it('the legacy backfill adds live tickets to the expiry index', async () => {
    const clock = { now: Date.UTC(2026, 10, 14) };
    const store = new MemoryStore(() => clock.now);
    const legacy: Ticket = { id: 'a'.repeat(48), wallet: W, subject: 'Legacy', message: 'demo ticket text', status: 'open', reply: null, createdAt: clock.now - DAY, updatedAt: clock.now - DAY, closedAt: null };
    await store.put(`ticket:${legacy.id}`, legacy, 0);
    await store.add('tickets:all', legacy.createdAt, legacy.id);
    await store.add(`tickets:wallet:${W}`, legacy.createdAt, legacy.id);
    await backfillOpenTickets(store, clock.now, true);
    expect(members(store, 'tickets:expiry')).toEqual([`${W}|${legacy.id}`]);
    clock.now += OPEN_SECONDS * 1000;
    expect(await sweep(store, clock.now)).toBe(1);
    expect(members(store, 'tickets:all')).toEqual([]);
    expect(members(store, `tickets:wallet:${W}`)).toEqual([]);
  });
});
