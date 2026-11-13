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
 * Bounded: one prune of entries whose record TTL has passed, one range of at most OPEN_PER_WALLET
 * members, and at most OPEN_PER_WALLET record reads. It never scans the wallet's full history.
 */
export async function openCount(store: Store, owner: string, now = Date.now()) {
  const index = openIndex(owner);
  await store.removeBelow(index, now - OPEN_SECONDS * 1000);
  let n = 0;
  for (const id of await store.rangeNewest(index, Number.MAX_SAFE_INTEGER, OPEN_PER_WALLET)) {
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (t && t.status !== 'closed') n++;
    else await store.remove(index, id);
  }
  return n;
}
export async function assertOpenSlot(store: Store, owner: string, now = Date.now()) {
  if (await openCount(store, owner, now) >= OPEN_PER_WALLET) fail(429, 'You already have the maximum number of open tickets. Please wait for a reply or for an older ticket to close.');
}
export async function createTicket(store: Store, owner: string, input: Record<string, unknown>, now: number) {
  const fields = ticketText(input);
  await assertOpenSlot(store, owner, now);
  const ticket: Ticket = { id: randomId(), wallet: owner, subject: fields.subject, message: fields.message, status: 'open', reply: null, createdAt: now, updatedAt: now, closedAt: null };
  await store.put(`ticket:${ticket.id}`, ticket, LIVE_SECONDS);
  await store.add(wallet(owner), now, ticket.id);
  await store.add(openIndex(owner), now, ticket.id);
  await store.add('tickets:all', now, ticket.id);
  return ticket;
}
/** Upper bound on index entries read for one page. */
const MAX_WINDOW = 4096;
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
  for (let window = take + 2; ; window = Math.min(window * 2, MAX_WINDOW)) {
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
    if (tickets.length === take + 1 || ids.length < window || window === MAX_WINDOW) break;
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
  if (action === 'close') await store.remove(openIndex(ticket.wallet), id);
  else await store.add(openIndex(ticket.wallet), now, id);
  return ticket;
}
/** Expiry is enforced by store TTL; the sweep removes due records and indexes. */
export async function sweep(store: Store, now: number) {
  let removed = 0;
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
