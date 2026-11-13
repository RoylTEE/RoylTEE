/* External audit finding F4: a ticket slot lease that expires mid submit cannot let a wallet go over
 * OPEN_PER_WALLET open tickets. */
import { MemoryStore, type Store } from '../src/server/store';
import { createTicket, openCount, OPEN_PER_WALLET } from '../src/server/tickets';

const W = 'writer-a.royltee-dev.testnet';
const status = async (p: Promise<unknown>) => { try { await p; return 200; } catch (e) { return (e as { status?: number }).status ?? 500; } };

/** Pauses the first submit right after its count check, until `release` is called. */
function pausing(inner: MemoryStore) {
  let paused = false, release = () => {}, reached = () => {};
  const atCheck = new Promise<void>((r) => { reached = r; });
  const gate = new Promise<void>((r) => { release = r; });
  const store = new Proxy(inner, {
    get(target, prop, receiver) {
      const v = Reflect.get(target, prop, receiver);
      if (prop === 'put' && !paused) {
        return async (...a: unknown[]) => {
          if (String(a[0]).startsWith('ticket:')) { paused = true; reached(); await gate; }
          return (v as (...x: unknown[]) => unknown).apply(target, a);
        };
      }
      return typeof v === 'function' ? v.bind(target) : v;
    },
  }) as Store;
  return { store, atCheck, release: () => release() };
}

describe('audit F4: the open ticket cap holds when the slot lease expires', () => {
  it('two submits that both pass the count check leave exactly OPEN_PER_WALLET open tickets', async () => {
    const clock = { now: Date.UTC(2026, 10, 14) };
    const inner = new MemoryStore(() => clock.now);
    for (let i = 0; i < OPEN_PER_WALLET - 1; i++) await createTicket(inner, W, { subject: `T${i}`, message: 'demo ticket text' }, clock.now + i);
    const p = pausing(inner);
    const first = status(createTicket(p.store, W, { subject: 'slow', message: 'demo ticket text' }, clock.now + 100));
    await p.atCheck;
    clock.now += 16_000; // the 15 s slot lease of the first submit has expired
    const second = await status(createTicket(inner, W, { subject: 'fast', message: 'demo ticket text' }, clock.now));
    p.release();
    const results = [await first, second].sort();
    expect(results).toEqual([200, 429]);
    expect(await openCount(inner, W, clock.now)).toBe(OPEN_PER_WALLET);
  });
  it('a submit at the cap is still refused before it writes anything', async () => {
    const clock = { now: Date.UTC(2026, 10, 14) };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < OPEN_PER_WALLET; i++) await createTicket(store, W, { subject: `T${i}`, message: 'demo ticket text' }, clock.now + i);
    expect(await status(createTicket(store, W, { subject: 'one more', message: 'demo ticket text' }, clock.now + 50))).toBe(429);
    expect(await openCount(store, W, clock.now + 50)).toBe(OPEN_PER_WALLET);
  });
});
