/** All keys use a project namespace. Atomic operations are shared across instances in Redis. */
export interface Store {
  get<T>(key: string): Promise<T | null>;
  put(key: string, value: unknown, seconds: number): Promise<void>;
  take<T>(key: string): Promise<T | null>;
  del(key: string): Promise<void>;
  cas(key: string, before: unknown, after: unknown, seconds: number): Promise<boolean>;
  count(key: string, seconds: number): Promise<number>;
  /** Gives back one unit taken by count. Never goes below zero and keeps the expiry. */
  uncount(key: string): Promise<void>;
  add(index: string, score: number, member: string): Promise<void>;
  range(index: string, max?: number, limit?: number): Promise<string[]>;
  rangeNewest(index: string, max?: number, limit?: number): Promise<string[]>;
  remove(index: string, member: string): Promise<void>;
}
const key = (k: string) => `royltee:v1:${k}`;
export class MemoryStore implements Store {
  private values = new Map<string, { text: string; expires: number }>();
  private indexes = new Map<string, Map<string, number>>();
  constructor(private now = Date.now) {}
  private raw(k: string) {
    const v = this.values.get(k);
    if (v && v.expires <= this.now()) { this.values.delete(k); return null; }
    return v?.text ?? null;
  }
  async get<T>(k: string): Promise<T | null> { const v = this.raw(k); return v === null ? null : JSON.parse(v); }
  async put(k: string, v: unknown, seconds: number) { this.values.set(k, { text: JSON.stringify(v), expires: seconds ? this.now() + seconds * 1000 : Infinity }); }
  async take<T>(k: string): Promise<T | null> { const text = this.raw(k); this.values.delete(k); return text === null ? null : JSON.parse(text); }
  async del(k: string) { this.values.delete(k); }
  async cas(k: string, before: unknown, after: unknown, seconds: number) {
    if (this.raw(k) !== JSON.stringify(before)) return false;
    this.values.set(k, { text: JSON.stringify(after), expires: seconds ? this.now() + seconds * 1000 : Infinity }); return true;
  }
  async count(k: string, seconds: number) {
    const old = this.raw(k); const n = old === null ? 1 : JSON.parse(old) + 1;
    this.values.set(k, { text: JSON.stringify(n), expires: old === null ? seconds ? this.now() + seconds * 1000 : Infinity : this.values.get(k)!.expires }); return n;
  }
  async uncount(k: string) {
    const old = this.raw(k); if (old === null) return;
    const n = JSON.parse(old); if (n > 0) this.values.set(k, { text: JSON.stringify(n - 1), expires: this.values.get(k)!.expires });
  }
  async add(index: string, score: number, member: string) {
    if (!this.indexes.has(index)) this.indexes.set(index, new Map());
    this.indexes.get(index)!.set(member, score);
  }
  async range(index: string, max = Number.MAX_SAFE_INTEGER, limit = 100) {
    return [...(this.indexes.get(index) ?? [])].filter(([, s]) => s <= max).sort((a, b) => a[1] - b[1] || (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0)).slice(0, limit).map(([m]) => m);
  }
  async rangeNewest(index: string, max = Number.MAX_SAFE_INTEGER, limit = 100) {
    return [...(this.indexes.get(index) ?? [])].filter(([, s]) => s <= max).sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? 1 : a[0] > b[0] ? -1 : 0)).slice(0, limit).map(([m]) => m);
  }
  async remove(index: string, member: string) { this.indexes.get(index)?.delete(member); }
}
export class RedisStore implements Store {
  constructor(private url: string, private token: string, private fetcher: typeof fetch = fetch) {
    const parsed = new URL(url); if (parsed.protocol !== 'https:') throw new Error('Private storage requires HTTPS');
  }
  private async command<T>(...command: (string | number)[]): Promise<T> {
    const r = await this.fetcher(this.url, { method: 'POST', headers: { Authorization: `Bearer ${this.token}`, 'Content-Type': 'application/json' }, body: JSON.stringify(command), signal: AbortSignal.timeout(8000) });
    if (!r.ok) throw new Error('Storage unavailable');
    const j = await r.json() as { result: T; error?: string };
    if (j.error) throw new Error('Storage unavailable'); return j.result;
  }
  async get<T>(k: string): Promise<T | null> { const v = await this.command<string | null>('GET', key(k)); return v === null ? null : JSON.parse(v); }
  async put(k: string, v: unknown, s: number) { await this.command('SET', key(k), JSON.stringify(v), ...(s ? ['EX', s] : [])); }
  async take<T>(k: string): Promise<T | null> { const v = await this.command<string | null>('GETDEL', key(k)); return v === null ? null : JSON.parse(v); }
  async del(k: string) { await this.command('DEL', key(k)); }
  async cas(k: string, before: unknown, after: unknown, s: number) {
    return (await this.command<number>('EVAL', "if redis.call('GET',KEYS[1]) == ARGV[1] then if tonumber(ARGV[3])>0 then redis.call('SET',KEYS[1],ARGV[2],'EX',ARGV[3]) else redis.call('SET',KEYS[1],ARGV[2]) end; return 1 else return 0 end", 1, key(k), JSON.stringify(before), JSON.stringify(after), s)) === 1;
  }
  async count(k: string, s: number) { return this.command<number>('EVAL', "local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]) end; return n", 1, key(k), s); }
  async uncount(k: string) { await this.command('EVAL', "local v=redis.call('GET',KEYS[1]); if v and tonumber(v)>0 then redis.call('DECR',KEYS[1]) end; return 0", 1, key(k)); }
  async add(i: string, score: number, m: string) { await this.command('ZADD', key(i), score, m); }
  async range(i: string, max = Number.MAX_SAFE_INTEGER, limit = 100) { return this.command<string[]>('ZRANGEBYSCORE', key(i), '-inf', max, 'LIMIT', 0, limit); }
  async rangeNewest(i: string, max = Number.MAX_SAFE_INTEGER, limit = 100) { return this.command<string[]>('ZREVRANGEBYSCORE', key(i), max, '-inf', 'LIMIT', 0, limit); }
  async remove(i: string, m: string) { await this.command('ZREM', key(i), m); }
}
