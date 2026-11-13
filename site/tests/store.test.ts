import { RedisStore, MemoryStore } from '../src/server/store';
describe('storage', () => {
  it('sends Upstash REST commands as JSON arrays with a bearer token', async () => {
    const sent: { url: string; init: RequestInit }[] = [];
    const replies = ['OK', '{"a":1}', '{"a":1}', 1, 1, ['a'], ['a']];
    const fetcher = (async (url: string, init: RequestInit) => { sent.push({ url, init }); return new Response(JSON.stringify({ result: replies.shift() })); }) as typeof fetch;
    const s = new RedisStore('https://example.upstash.invalid', 'token-for-test', fetcher);
    await s.put('ticket:x', { a: 1 }, 60);
    expect(await s.get('ticket:x')).toEqual({ a: 1 });
    expect(await s.take('ticket:x')).toEqual({ a: 1 });
    expect(await s.count('rate:x', 60)).toBe(1);
    await s.add('tickets:all', 1, 'a');
    await s.range('tickets:all', 10, 5);
    await s.rangeNewest('tickets:all', 10, 5);
    expect(sent.map(x => JSON.parse(String(x.init.body))[0])).toEqual(['SET', 'GET', 'GETDEL', 'EVAL', 'ZADD', 'ZRANGEBYSCORE', 'ZREVRANGEBYSCORE']);
    expect(JSON.parse(String(sent[0].init.body))).toEqual(['SET', 'royltee:v1:ticket:x', '{"a":1}', 'EX', 60]);
    expect((sent[0].init.headers as Record<string, string>).Authorization).toBe('Bearer token-for-test');
  });
  it('refuses plain HTTP storage and hides storage errors', async () => {
    expect(() => new RedisStore('http://example.invalid', 't')).toThrow();
    const s = new RedisStore('https://example.upstash.invalid', 't', (async () => new Response(JSON.stringify({ error: 'WRONGPASS detail' }))) as typeof fetch);
    await expect(s.get('x')).rejects.toThrow('Storage unavailable');
  });
  it('expires memory values, consumes values once and compares before swap', async () => {
    let now = 0; const s = new MemoryStore(() => now);
    await s.put('a', 1, 10); now = 10_001; expect(await s.get('a')).toBeNull();
    await s.put('b', 2, 10); expect(await s.take('b')).toBe(2); expect(await s.take('b')).toBeNull();
    await s.put('c', { n: 1 }, 10); expect(await s.cas('c', { n: 2 }, { n: 3 }, 10)).toBe(false); expect(await s.cas('c', { n: 1 }, { n: 3 }, 10)).toBe(true);
  });
});
import { loadConfig } from '../src/server/config';
describe('storage env names', () => {
  it('prefers the Vercel marketplace names and falls back to Upstash names', () => {
    expect(loadConfig({ KV_REST_API_URL: 'https://a.invalid', KV_REST_API_TOKEN: 'a', UPSTASH_REDIS_REST_URL: 'https://b.invalid', UPSTASH_REDIS_REST_TOKEN: 'b' })).toMatchObject({ redisUrl: 'https://a.invalid', redisToken: 'a' });
    expect(loadConfig({ UPSTASH_REDIS_REST_URL: 'https://b.invalid', UPSTASH_REDIS_REST_TOKEN: 'b' })).toMatchObject({ redisUrl: 'https://b.invalid', redisToken: 'b' });
  });
});
