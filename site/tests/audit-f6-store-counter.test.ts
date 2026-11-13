/* External audit finding F6: MemoryStore and RedisStore give the same counter expiry. The Redis
 * scripts run in a real Lua engine (wasmoon) against a small in-memory Redis with the documented
 * semantics of GET, EXISTS, INCR, DECR and EXPIRE (EXPIRE with 0 deletes the key). */
import { LuaFactory } from 'wasmoon';
import { MemoryStore, RedisStore, type Store } from '../src/server/store';

type Entry = { value: string; expires: number };
function fakeUpstash(now: () => number) {
  const db = new Map<string, Entry>();
  const live = (k: string) => { const e = db.get(k); if (e && e.expires <= now()) { db.delete(k); return undefined; } return e; };
  const redis: Record<string, (...a: string[]) => unknown> = {
    GET: (k) => live(k)?.value ?? null,
    EXISTS: (k) => (live(k) ? 1 : 0),
    INCR: (k) => { const e = live(k); const n = (e ? Number(e.value) : 0) + 1; db.set(k, { value: String(n), expires: e ? e.expires : Infinity }); return n; },
    DECR: (k) => { const e = live(k); const n = (e ? Number(e.value) : 0) - 1; db.set(k, { value: String(n), expires: e ? e.expires : Infinity }); return n; },
    EXPIRE: (k, s) => { const e = live(k); if (!e) return 0; if (Number(s) <= 0) { db.delete(k); return 1; } e.expires = now() + Number(s) * 1000; return 1; },
  };
  const engine = new LuaFactory().createEngine();
  const fetcher = (async (_url: string, init: RequestInit) => {
    const [cmd, ...args] = JSON.parse(String(init.body)) as (string | number)[];
    let result: unknown;
    if (cmd === 'EVAL') {
      const [script, nkeys, ...rest] = args;
      const lua = await engine;
      const keys = rest.slice(0, Number(nkeys)).map(String), argv = rest.slice(Number(nkeys)).map(String);
      lua.global.set('KEYS', keys); lua.global.set('ARGV', argv);
      // Redis returns false for a missing value inside Lua.
      lua.global.set('redis', { call: (c: string, ...a: string[]) => { const r = redis[c](...a.map(String)); return r === null ? false : r; } });
      result = await lua.doString(String(script));
    } else result = redis[String(cmd)](...args.map(String));
    return new Response(JSON.stringify({ result }));
  }) as typeof fetch;
  return new RedisStore('https://example.upstash.invalid', 'token-for-test', fetcher);
}

async function trace(store: Store, clock: { now: number }) {
  const out: number[] = [];
  out.push(await store.count('a', 10)); clock.now += 4_000;
  await store.uncount('a'); clock.now += 1_000;
  out.push(await store.count('a', 10)); clock.now += 6_000; // 11 s after the first count: expired
  out.push(await store.count('a', 10));
  out.push(await store.count('b', 0)); clock.now += 3_600_000;
  out.push(await store.count('b', 0)); // seconds 0: no expiry at all
  out.push(await store.count('c', 10)); await store.uncount('c'); await store.uncount('c'); clock.now += 2_000;
  out.push(await store.count('c', 10)); clock.now += 9_000; // first expiry kept: 11 s after creation
  out.push(await store.count('c', 10));
  return out;
}

describe('audit F6: one counter expiry rule for memory and Redis', () => {
  it('gives the same counts for the same calls, including uncount to zero and seconds 0', async () => {
    const m = { now: 1_000_000 }, r = { now: 1_000_000 };
    const memory = await trace(new MemoryStore(() => m.now), m);
    const redis = await trace(fakeUpstash(() => r.now), r);
    expect(memory).toEqual([1, 1, 1, 1, 2, 1, 1, 1]);
    expect(redis).toEqual(memory);
  });
  it('a counter brought back to zero keeps its first window in Redis', async () => {
    const clock = { now: 0 }; const s = fakeUpstash(() => clock.now);
    expect(await s.count('w', 10)).toBe(1); await s.uncount('w'); clock.now = 9_000;
    expect(await s.count('w', 10)).toBe(1); clock.now = 10_001;
    expect(await s.count('w', 10)).toBe(1);
    expect(await s.count('w', 10)).toBe(2);
  });
});
