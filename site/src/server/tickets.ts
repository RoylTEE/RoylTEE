import { randomId, fail, text } from './security';
import { sensitive } from './sensitive';
import type { Store } from './store';
export type Status = 'open' | 'answered' | 'closed';
export type Ticket = { id: string; wallet: string; subject: string; message: string; status: Status; reply: string | null; createdAt: number; updatedAt: number; closedAt: number | null };
export const RETAIN_CLOSED_MS = 30 * 24 * 3600 * 1000;
export const OPEN_SECONDS = 30 * 24 * 3600;
export const OPEN_PER_WALLET = 10;
export const PAGE_SIZE = 100;
const LIVE_SECONDS = OPEN_SECONDS;
export const LIMITS = { subject: 120, message: 2000, reply: 2000, page: PAGE_SIZE, openPerWallet: OPEN_PER_WALLET, openDays: 30 };
const wallet = (w: string) => `tickets:wallet:${w}`;
/** Open and answered tickets for one wallet, scored by the last write, which also sets the record TTL. */
const openIndex = (w: string) => `tickets:open:${w}`;
/**
 * Open and answered tickets of every wallet, member `wallet|id`, scored by the time the record TTL
 * ends (last write + OPEN_SECONDS). The sweep uses it to drop the index entries of tickets that
 * expired without being closed (audit F5).
 */
const EXPIRY = 'tickets:expiry';
const expiryMember = (w: string, id: string) => `${w}|${id}`;
export type TicketPage = { tickets: Ticket[]; cursor: string | null };
export function encodeCursor(createdAt: number, id: string) { return `${createdAt}:${id}`; }
export function decodeCursor(value: string | null): { createdAt: number; id: string } | null {
  if (!value) return null;
  const at = value.indexOf(':');
  if (at < 1) return null;
  const createdAt = Number(value.slice(0, at));
  const id = value.slice(at + 1);
  if (!Number.isFinite(createdAt) || !/^[a-f0-9]{48}$/.test(id)) return null;
  return { createdAt, id };
}
function clean(value: unknown, max: number) {
  const v = text(value, max);
  const kind = sensitive(v);
  if (kind) fail(422, `That looks like ${kind}. Please remove it and use demo details only.`);
  return v;
}
export function ticketText(input: Record<string, unknown>) {
  return { subject: clean(input.subject, LIMITS.subject), message: clean(input.message, LIMITS.message) };
}
/**
 * Bounded: one prune of entries whose record TTL has passed, one range of at most `limit`
 * (OPEN_PER_WALLET, or one more for the check after a write) members, and at most that many record reads. It never scans the wallet's full history.
 */
export async function openCount(store: Store, owner: string, now = Date.now(), limit = OPEN_PER_WALLET) {
  const index = openIndex(owner);
  await store.removeBelow(index, now - OPEN_SECONDS * 1000);
  let n = 0;
  for (const id of await store.rangeNewest(index, Number.MAX_SAFE_INTEGER, limit)) {
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (t && t.status !== 'closed') n++;
    else await store.remove(index, id);
  }
  return n;
}
export async function assertOpenSlot(store: Store, owner: string, now = Date.now()) {
  if (await openCount(store, owner, now) >= OPEN_PER_WALLET) fail(429, 'You already have the maximum number of open tickets. Please wait for a reply or for an older ticket to close.');
}
/** Seconds a per wallet slot reservation lives if a worker dies while holding it. */
const SLOT_LOCK_SECONDS = 15;
export async function createTicket(store: Store, owner: string, input: Record<string, unknown>, now: number) {
  const fields = ticketText(input);
  // Reserve the wallet's open slot atomically (one INCR) so the count check and the write cannot
  // interleave with another submit from the same wallet (review 03b L3, second reviewer item 6).
  const lock = `tickets:slot:${owner}`;
  if (await store.count(lock, SLOT_LOCK_SECONDS) > 1) {
    await store.uncount(lock);
    fail(429, 'Another ticket from this wallet is being saved. Please try again in a moment.');
  }
  try {
    await assertOpenSlot(store, owner, now);
    const ticket: Ticket = { id: randomId(), wallet: owner, subject: fields.subject, message: fields.message, status: 'open', reply: null, createdAt: now, updatedAt: now, closedAt: null };
    await store.put(`ticket:${ticket.id}`, ticket, LIVE_SECONDS);
    await store.add(wallet(owner), now, ticket.id);
    await store.add(openIndex(owner), now, ticket.id);
    await store.add('tickets:all', now, ticket.id);
    await store.add(EXPIRY, now + OPEN_SECONDS * 1000, expiryMember(owner, ticket.id));
    // Audit F4: the slot reservation is a lease. If a submit outlives SLOT_LOCK_SECONDS, a second
    // submit can pass the count check too, so both could write and the wallet would hold
    // OPEN_PER_WALLET + 1 open tickets. Check again after the write and take this ticket back if the
    // cap is exceeded. A submit whose write lands last always sees the extra ticket, so the cap
    // holds; if two writes land before either check, both are refused, which is safe.
    if (await openCount(store, owner, now, OPEN_PER_WALLET + 1) > OPEN_PER_WALLET) {
      await dropTicket(store, owner, ticket.id);
      fail(429, 'You already have the maximum number of open tickets. Please wait for a reply or for an older ticket to close.');
    }
    return ticket;
  } finally {
    await store.uncount(lock);
  }
}
async function dropTicket(store: Store, owner: string, id: string) {
  await store.del(`ticket:${id}`);
  await store.remove(wallet(owner), id);
  await store.remove(openIndex(owner), id);
  await store.remove('tickets:all', id);
  await store.remove(EXPIRY, expiryMember(owner, id));
}
export type BackfillReport = { scanned: number; indexed: number; ttlSet: number; expired: number; applied: boolean };
/**
 * One time, idempotent backfill for tickets written before open tickets had a TTL and the per wallet
 * open index (review 03b L1, second reviewer item 4). For every ticket that is not closed: if its last
 * update is older than 30 days it is deleted as expired; otherwise it is put again with the TTL left
 * from its last update and added to the open index. Dry run unless `apply` is true. Running it again
 * changes nothing new. Closed tickets are left to the existing closed sweep.
 */
export async function backfillOpenTickets(store: Store, now: number, apply = false): Promise<BackfillReport> {
  const report: BackfillReport = { scanned: 0, indexed: 0, ttlSet: 0, expired: 0, applied: apply };
  for (const id of await store.range('tickets:all', Number.MAX_SAFE_INTEGER, 1_000_000)) {
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (!t || t.status === 'closed') continue;
    report.scanned++;
    const left = Math.floor((t.updatedAt + OPEN_SECONDS * 1000 - now) / 1000);
    if (left <= 0) {
      report.expired++;
      if (apply) await dropTicket(store, t.wallet, id);
      continue;
    }
    report.ttlSet++; report.indexed++;
    if (apply) {
      await store.put(`ticket:${id}`, t, left);
      await store.add(openIndex(t.wallet), t.updatedAt, id);
      await store.add(EXPIRY, t.updatedAt + OPEN_SECONDS * 1000, expiryMember(t.wallet, id));
    }
  }
  return report;
}
export async function listTickets(store: Store, owner: string | null, now: number, cursorValue: string | null = null, limit = PAGE_SIZE): Promise<TicketPage> {
  await sweep(store, now);
  const cursor = decodeCursor(cursorValue);
  const index = owner ? wallet(owner) : 'tickets:all';
  const take = Math.min(Math.max(limit, 1), 500);
  // Skip by position (createdAt, then id, in the index order: newest first, ties by id descending),
  // not by finding the cursor record, so a cursor ticket that expired between pages does not end the
  // list. Tickets on the cursor's own millisecond that were already shown are skipped, and the window
  // grows if they fill it, so ties never end the list early either.
  let tickets: Ticket[] = [];
  for (let window = take + 2; ; window *= 2) {
    const ids = await store.rangeNewest(index, cursor ? cursor.createdAt : Number.MAX_SAFE_INTEGER, window);
    tickets = [];
    for (const id of ids) {
      const t = await store.get<Ticket>(`ticket:${id}`);
      if (!t) { await store.remove(index, id); continue; }
      if (cursor && !(t.createdAt < cursor.createdAt || (t.createdAt === cursor.createdAt && id < cursor.id))) continue;
      if (owner !== null && t.wallet !== owner) continue;
      tickets.push(t);
      if (tickets.length === take + 1) break;
    }
    if (tickets.length === take + 1 || ids.length < window) break;
  }
  const more = tickets.length > take;
  const page = more ? tickets.slice(0, take) : tickets;
  const last = page[page.length - 1];
  return { tickets: page, cursor: more && last ? encodeCursor(last.createdAt, last.id) : null };
}
export async function adminUpdate(store: Store, id: unknown, action: 'reply' | 'close', input: Record<string, unknown>, now: number) {
  if (typeof id !== 'string' || !/^[a-f0-9]{48}$/.test(id)) fail(400, 'Ticket id is not valid.');
  const t = await store.get<Ticket>(`ticket:${id}`);
  if (!t) fail(404, 'Ticket not found.');
  const ticket = { ...t! };
  if (ticket.status === 'closed') fail(409, 'That ticket is already closed.');
  if (action === 'reply') Object.assign(ticket, { reply: clean(input.reply, LIMITS.reply), status: 'answered', updatedAt: now });
  else Object.assign(ticket, { status: 'closed', closedAt: now, updatedAt: now });
  if (action === 'close') await store.add('tickets:closed', now, `${ticket.wallet}|${id}`);
  if (!await store.cas(`ticket:${id}`, t, ticket, action === 'close' ? RETAIN_CLOSED_MS / 1000 : LIVE_SECONDS)) {
    fail(409, 'This ticket changed. Please refresh and try again.');
  }
  if (action === 'close') {
    await store.remove(openIndex(ticket.wallet), id);
    await store.remove(EXPIRY, expiryMember(ticket.wallet, id));
  } else {
    await store.add(openIndex(ticket.wallet), now, id);
    await store.add(EXPIRY, now + LIVE_SECONDS * 1000, expiryMember(ticket.wallet, id));
  }
  return ticket;
}
/**
 * Expiry is enforced by store TTL; the sweep removes due records and indexes. Closed tickets past
 * RETAIN_CLOSED_MS go first. Then open or answered tickets whose TTL has ended (audit F5): their
 * records are gone, so their entries leave `tickets:all`, the wallet index, the open index and the
 * expiry index. Each pass handles at most 500 of each.
 */
export async function sweep(store: Store, now: number) {
  let removed = 0;
  for (const member of await store.range(EXPIRY, now, 500)) {
    const [owner, id] = member.split('|');
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (t && t.status !== 'closed') { await store.add(EXPIRY, t.updatedAt + OPEN_SECONDS * 1000, member); continue; }
    if (t) { await store.remove(EXPIRY, member); continue; }
    await dropTicket(store, owner, id);
    removed++;
  }
  for (const member of await store.range('tickets:closed', now - RETAIN_CLOSED_MS, 500)) {
    const [owner, id] = member.split('|');
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (t && (t.closedAt === null || t.closedAt > now - RETAIN_CLOSED_MS)) { await store.remove('tickets:closed', member); continue; }
    await store.del(`ticket:${id}`);
    await store.remove('tickets:closed', member);
    await store.remove('tickets:all', id);
    await store.remove(wallet(owner), id);
    await store.remove(openIndex(owner), id);
    removed++;
  }
  return removed;
}
