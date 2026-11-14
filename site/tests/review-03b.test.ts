import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { createTicket, listTickets } from '../src/server/tickets';
import { MemoryStore } from '../src/server/store';
import { SCENES, MINTS } from '../src/data/testnet';

describe('review 03b L2: ticket paging survives a cursor ticket that disappears', () => {
  it('keeps walking older tickets when the cursor ticket expired between pages', async () => {
    const clock = { now: 2_000_000 };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < 10; i++) {
      await createTicket(store, `w${i}.royltee-dev.testnet`, { subject: `S${i}`, message: 'demo ticket text' }, clock.now + i);
    }
    const first = await listTickets(store, null, clock.now + 100, null, 3);
    expect(first.tickets.map(t => t.subject)).toEqual(['S9', 'S8', 'S7']);
    await store.del(`ticket:${first.tickets[2].id}`);
    const second = await listTickets(store, null, clock.now + 100, first.cursor, 3);
    expect(second.tickets.map(t => t.subject)).toEqual(['S6', 'S5', 'S4']);
    expect(second.cursor).toBeTruthy();
  });

  it('orders ties on the same millisecond by id and never repeats or drops one', async () => {
    const clock = { now: 3_000_000 };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < 7; i++) await createTicket(store, `t${i}.royltee-dev.testnet`, { subject: `T${i}`, message: 'demo ticket text' }, clock.now);
    const seen: string[] = [];
    let cursor: string | null = null;
    do {
      const page = await listTickets(store, null, clock.now + 1, cursor, 2);
      seen.push(...page.tickets.map(t => t.id));
      cursor = page.cursor;
    } while (cursor);
    expect(new Set(seen).size).toBe(7);
    expect(seen.length).toBe(7);
  });
});

describe('review 03c L3: the agent payment is not labelled Simulated payer', () => {
  it('marks the agent scene as a scripted agent', () => {
    const agent = SCENES.filter(s => s.step === 'Agent' && s.what.startsWith('Agent pays'));
    expect(agent.length).toBe(3);
    for (const s of agent) { expect(s.simulated).toBeFalsy(); expect(s.agent).toBe(true); }
  });
});

describe('review 03c M2: power claims match the deployed build', () => {
  const read = (p: string) => readFileSync(new URL(`../src/${p}`, import.meta.url), 'utf8');
  it('does not say every registrar action has a document hash', () => {
    for (const f of ['pages/registrar.astro', 'pages/index.astro', 'pages/security.astro', 'content/help-notes.md']) {
      expect(read(f)).not.toMatch(/[Ee]very registrar action (is an event with|carries) a document hash/);
    }
  });
  it('states split sheet approval and the registrar credential refusal as deployed', () => {
    expect(read('pages/registrar.astro')).toMatch(/Hold a credential \(refused by the deployed build/);
    expect(read('pages/member.astro')).not.toMatch(/Not on the deployed build yet/);
    expect(read('pages/security.astro')).toMatch(/Change a split alone \(except the registrar/);
  });
});

/* Second reviewer, part 03b, and review 03d items. */
import { sensitive } from '../src/server/sensitive';
import { chat, startChat, BUSY_MS } from '../src/server/chat';
import { loadConfig } from '../src/server/config';
import { backfillOpenTickets, OPEN_PER_WALLET, type Ticket } from '../src/server/tickets';
import { shownClaimable } from '../src/scripts/claimable';
import { overLimit } from '../src/scripts/draft-check';
import { outcomeText } from '../src/scripts/tx-outcome';
import { notes } from '../src/server/prompt';
import { WALLET_ROUTES } from '../src/server/csp';
import { DEPLOY_RECORD } from '../src/data/testnet';
import { readdirSync, statSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const status = async (p: Promise<unknown>) => { try { await p; return 200; } catch (e) { return (e as { status?: number }).status ?? 500; } };

describe('second reviewer 03b item 1: number formats do not hide phones or cards', () => {
  it('flags a dotted phone number and a dotted Luhn valid card number', () => {
    expect(sensitive('call me on 555.123.4567')).toBe('a phone number');
    expect(sensitive('card 411.111.111.111.111.1 please')).toBe('a card number');
  });
  it('still lets a dotted amount through only next to a currency', () => {
    expect(sensitive('Payment of 1.000.000 mUSDC to s1')).toBeNull();
    expect(sensitive('Payment of $1.000.000 to s1')).toBeNull();
    expect(sensitive('reference 123.456.789.012')).toBe('a phone number');
  });
});

describe('second reviewer 03b item 2: anonymous calls cannot reach the signed-in reserve', () => {
  it('anon and global budgets share one UTC day, so a late anonymous call is refused and a signed call still runs', async () => {
    const store = new MemoryStore();
    const config = loadConfig({ CHAT_HMAC_SECRET: 'x'.repeat(40), CHAT_GLOBAL_DAY_CAP: '3', CHAT_SIGNED_RESERVE: '1' });
    const model = async () => 'Royalty shares are split by bps.';
    let n = 0;
    const ask = async (ip: string, at: number, wallet: string | null = null) => { const id = (n++).toString(16).padStart(48, 'e'); await startChat(store, id); return status(chat(store, config, id, { history: [], message: 'How do shares work?' }, ip, model, at, wallet)); };
    const day = Date.UTC(2026, 10, 14);
    expect(await ask('a', day + 10 * 60_000)).toBe(200);
    expect(await ask('b', day + 20 * 60_000)).toBe(200);
    expect(await ask('c', day + 23 * 3600_000)).toBe(429);
    expect(await ask('d', day + 23.5 * 3600_000, 'w.testnet')).toBe(200);
    expect(await ask('e', day + 24 * 3600_000 + 60_000)).toBe(200);
  });
});

describe('second reviewer 03b item 3: paging through ties and an expired cursor', () => {
  it('walks 104 tickets on one millisecond in pages of 100 without hiding any, even if the cursor ticket expires', async () => {
    const clock = { now: 4_000_000 };
    const store = new MemoryStore(() => clock.now);
    const ids: string[] = [];
    for (let i = 0; i < 104; i++) ids.push((await createTicket(store, `x${i}.royltee-dev.testnet`, { subject: `X${i}`, message: 'demo ticket text' }, clock.now)).id);
    const first = await listTickets(store, null, clock.now + 1, null, 100);
    expect(first.tickets).toHaveLength(100);
    await store.del(`ticket:${first.tickets[99].id}`);
    const second = await listTickets(store, null, clock.now + 1, first.cursor, 100);
    expect(second.tickets).toHaveLength(4);
    expect(second.cursor).toBeNull();
    expect(new Set([...first.tickets, ...second.tickets].map(t => t.id)).size).toBe(104);
  });
});

describe('second reviewer 03b item 4: legacy open tickets backfill', () => {
  it('is a dry run by default, then indexes live legacy tickets, expires old ones, and is idempotent', async () => {
    const clock = { now: Date.UTC(2026, 10, 13) };
    const store = new MemoryStore(() => clock.now);
    const day = 86400_000;
    const legacy = (i: number, updatedAt: number): Ticket => ({ id: i.toString(16).padStart(48, 'a'), wallet: 'old.royltee-dev.testnet', subject: `L${i}`, message: 'legacy', status: 'open', reply: null, createdAt: updatedAt, updatedAt, closedAt: null });
    for (let i = 0; i < 12; i++) {
      const t = legacy(i, clock.now - (i < 2 ? 40 : 1) * day);
      await store.put(`ticket:${t.id}`, t, 0);
      await store.add('tickets:all', t.createdAt, t.id);
      await store.add('tickets:wallet:old.royltee-dev.testnet', t.createdAt, t.id);
    }
    const dry = await backfillOpenTickets(store, clock.now);
    expect(dry).toEqual({ scanned: 12, indexed: 10, ttlSet: 10, expired: 2, applied: false });
    await expect(createTicket(store, 'old.royltee-dev.testnet', { subject: 'new', message: 'demo ticket text' }, clock.now)).resolves.toBeTruthy();
    const applied = await backfillOpenTickets(store, clock.now, true);
    expect(applied.expired).toBe(2);
    expect(await status(createTicket(store, 'old.royltee-dev.testnet', { subject: 'one more', message: 'demo ticket text' }, clock.now))).toBe(429);
    const again = await backfillOpenTickets(store, clock.now, true);
    expect(again.expired).toBe(0);
    clock.now += 31 * day;
    expect(await store.get(`ticket:${legacy(5, 0).id}`)).toBeNull();
  });
});

describe('second reviewer 03b item 6: open ticket cap holds under concurrent submits', () => {
  it('with 9 open, five submits at once create exactly one ticket', async () => {
    const store = new MemoryStore();
    const now = Date.now();
    for (let i = 0; i < OPEN_PER_WALLET - 1; i++) await createTicket(store, 'busy.royltee-dev.testnet', { subject: `B${i}`, message: 'demo ticket text' }, now + i);
    const results = await Promise.all(Array.from({ length: 5 }, (_, i) => status(createTicket(store, 'busy.royltee-dev.testnet', { subject: `C${i}`, message: 'demo ticket text' }, now + 100 + i))));
    expect(results.filter(r => r === 200)).toHaveLength(1);
    const page = await listTickets(store, 'busy.royltee-dev.testnet', now + 1000, null, 100);
    expect(page.tickets).toHaveLength(OPEN_PER_WALLET);
    expect(await status(createTicket(store, 'busy.royltee-dev.testnet', { subject: 'after', message: 'demo ticket text' }, now + 2000))).toBe(429);
  });
});

describe('second reviewer 03b item 7: a worker whose lock expired cannot overwrite history', () => {
  it('discards the slow answer after another request took the turn over', async () => {
    const store = new MemoryStore();
    const config = loadConfig({ CHAT_HMAC_SECRET: 'x'.repeat(40) });
    const id = 'f'.repeat(48); await startChat(store, id);
    let release!: () => void;
    const slow = () => new Promise<string>((r) => { release = () => r('slow answer'); });
    const t0 = Date.now();
    const first = status(chat(store, config, id, { history: [], message: 'How do shares work?' }, 'a', slow, t0));
    await new Promise((r) => setTimeout(r, 10));
    const second = await status(chat(store, config, id, { history: [], message: 'How do shares work?' }, 'b', async () => 'fast answer', t0 + BUSY_MS + 1));
    expect(second).toBe(200);
    release();
    expect(await first).toBe(409);
    const state = await store.get<{ count: number }>(`chat:${id}`);
    expect(state!.count).toBe(1);
  });
});

describe('second reviewer 03b item 5: grounding notes match the deployed snapshot', () => {
  it('drops the stale six shares and not on chain lines', () => {
    expect(notes).not.toContain('six shares s1 to s6');
    expect(notes).not.toContain('Not on chain yet: the scripted agent and the dispute scene');
    expect(notes).not.toContain('seven shares s1 to s7');
    expect(notes).toContain('15 shares s1 to s15');
  });
});

describe('review 03d M1: claimable is capped at the vault balance', () => {
  it('shows the lower of the ledger and the real balance', () => {
    expect(shownClaimable('500', '200')).toBe('200');
    expect(shownClaimable('200', '500')).toBe('200');
    expect(shownClaimable('200', null)).toBe('200');
    const chainSrc = readFileSync(new URL('../src/scripts/chain.ts', import.meta.url), 'utf8');
    expect(chainSrc.match(/capClaimable\(/g)!.length).toBeGreaterThanOrEqual(4);
  });
});

describe('review 03d M2: the draft refuses when the group would exceed 100 percent', () => {
  it('counts active, pending and claimant basis points', () => {
    expect(overLimit({ active_bps: 9000, reserved_bps: 0 }, 1000)).toBeNull();
    expect(overLimit({ active_bps: 9000, reserved_bps: 500 }, 1000)).toBe(10500);
    expect(overLimit({ active_bps: 8000, reserved_bps: 0, claimant_bps: 1500 }, 1000)).toBe(10500);
    expect(overLimit(null, 10000)).toBeNull();
    const m = readFileSync(new URL('../src/scripts/member.ts', import.meta.url), 'utf8');
    expect(m).toMatch(/view<GroupTotals>\('get_group'/);
    expect(m).toMatch(/so each new share becomes active and is paid as soon as it is minted/);
  });
});

describe('review 03d L1: wallet results are checked before saying it worked', () => {
  it('reports failure, success and a missing result', () => {
    expect(outcomeText([{ status: { SuccessValue: '' }, receipts_outcome: [] }], 'Claim')).toBe('Claim confirmed on chain.');
    expect(outcomeText([{ status: { SuccessValue: '' }, receipts_outcome: [{ outcome: { status: { Failure: { ActionError: { kind: { FunctionCallError: { ExecutionError: 'Smart contract panicked: nothing to claim' } } } } } } }] }], 'Claim')).toMatch(/Claim failed on chain \(1 of 1\): nothing to claim/);
    expect(outcomeText(undefined, 'Claim')).toMatch(/not confirmed/);
    for (const f of ['scripts/member.ts', 'scripts/org.ts']) expect(readFileSync(new URL(`../src/${f}`, import.meta.url), 'utf8')).not.toMatch(/sent\. Refreshing|sent\. Reading/);
  });
});

describe('review 03d L6: every page that loads wallet code is a wallet route', () => {
  it('follows imports of the wallet module through other scripts, and keeps it out of shared layouts and components', () => {
    const dir = new URL('../src/scripts/', import.meta.url);
    const scripts = readdirSync(dir).filter(f => f.endsWith('.ts'));
    const imports = (f: string) => [...readFileSync(new URL(f, dir), 'utf8').matchAll(/from '\.\/([\w-]+)(?:\.ts)?'/g)].map(m => `${m[1]}.ts`);
    const wallet = new Set(['wallet.ts']);
    for (let grew = true; grew;) { grew = false; for (const f of scripts) if (!wallet.has(f) && imports(f).some(i => wallet.has(i))) { wallet.add(f); grew = true; } }
    const root = new URL('../src/', import.meta.url).pathname;
    const files = (d: string): string[] => readdirSync(d).flatMap(f => statSync(join(d, f)).isDirectory() ? files(join(d, f)) : [join(d, f)]);
    const loads = (file: string) => [...readFileSync(file, 'utf8').matchAll(/scripts\/([\w-]+\.ts)/g)].some(m => wallet.has(m[1]));
    for (const f of [...files(join(root, 'layouts')), ...files(join(root, 'components'))]) expect(loads(f), f).toBe(false);
    const pages = files(join(root, 'pages')).filter(f => f.endsWith('.astro') && loads(f)).map(f => '/' + f.slice(join(root, 'pages').length + 1).replace(/\.astro$/, '').replace(/\/index$/, ''));
    expect(pages.sort()).toEqual([...WALLET_ROUTES].sort());
  });
});

describe('review 03d L5: the verify page accepts recorded vault builds and agrees with the deploy record', () => {
  it('matches deployments/testnet.json when that file is present', () => {
    const v = readFileSync(new URL('../src/scripts/verify.ts', import.meta.url), 'utf8');
    expect(v).toMatch(/earlierVaultCodeHashes\.includes/);
    const rec = new URL('../../deployments/testnet.json', import.meta.url);
    if (!existsSync(rec)) return;
    const r = JSON.parse(readFileSync(rec, 'utf8'));
    expect(r.wasm.registry.on_chain_code_hash_base58).toBe(DEPLOY_RECORD.registry.codeHash);
    expect(r.wasm.registry.on_chain_sha256).toBe(DEPLOY_RECORD.registry.sha256);
    expect(r.wasm.vault.on_chain_code_hash_base58).toBe(DEPLOY_RECORD.vault.codeHash);
    expect(r.wasm.vault.on_chain_sha256).toBe(DEPLOY_RECORD.vault.sha256);
    expect([...DEPLOY_RECORD.earlierVaultCodeHashes].sort()).toEqual(r.wasm.vault.earlier_builds.map((b: { on_chain_code_hash_base58: string }) => b.on_chain_code_hash_base58).sort());
    const vaultHashes = new Set([DEPLOY_RECORD.vault.codeHash, ...DEPLOY_RECORD.earlierVaultCodeHashes]);
    for (const a of Object.values(r.accounts) as { variable: string; code_hash_base58: string }[]) if (/^S\d+$/.test(a.variable)) expect(vaultHashes.has(a.code_hash_base58), a.variable).toBe(true);
    const recorded = new Map((r.transactions as { transaction_hash: string; block_timestamp_utc?: string }[]).map((t) => [t.transaction_hash, t]));
    expect(Object.keys(MINTS).sort()).toEqual(Object.keys(r.token_owners.owners).sort());
    for (const [id, m] of Object.entries(MINTS)) {
      expect(recorded.has(m.tx), id).toBe(true);
      expect(recorded.get(m.tx)!.block_timestamp_utc!.slice(0, 19), id).toBe(m.at.slice(0, 19));
    }
    for (const s of SCENES) for (const t of s.txs) expect(recorded.has(t), t).toBe(true);
  });
});
