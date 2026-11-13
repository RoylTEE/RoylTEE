import { randomId, fail, text } from './security';
import { sensitive } from './sensitive';
import type { Store } from './store';
export type Status = 'open' | 'answered' | 'closed';
export type Ticket = { id: string; wallet: string; subject: string; message: string; status: Status; reply: string | null; createdAt: number; updatedAt: number; closedAt: number | null };
export const RETAIN_CLOSED_MS = 30 * 24 * 3600 * 1000;
const LIVE_SECONDS = 0;
export const LIMITS = { subject: 120, message: 2000, reply: 2000 };
const wallet = (w: string) => `tickets:wallet:${w}`;
function clean(value: unknown, max: number) {
  const v = text(value, max);
  const kind = sensitive(v);
  if (kind) fail(422, `That looks like ${kind}. Please remove it and use demo details only.`);
  return v;
}
export async function createTicket(store: Store, owner: string, input: Record<string, unknown>, now: number) {
  const ticket: Ticket = { id: randomId(), wallet: owner, subject: clean(input.subject, LIMITS.subject), message: clean(input.message, LIMITS.message), status: 'open', reply: null, createdAt: now, updatedAt: now, closedAt: null };
  await store.put(`ticket:${ticket.id}`, ticket, LIVE_SECONDS);
  await store.add(wallet(owner), now, ticket.id);
  await store.add('tickets:all', now, ticket.id);
  return ticket;
}
export async function listTickets(store: Store, owner: string | null, now: number) {
  await sweep(store, now);
  const ids = await store.range(owner ? wallet(owner) : 'tickets:all', Number.MAX_SAFE_INTEGER, 500);
  const tickets: Ticket[] = [];
  for (const id of ids) {
    const t = await store.get<Ticket>(`ticket:${id}`);
    if (!t) { await store.remove(owner ? wallet(owner) : 'tickets:all', id); continue; }
    if (owner !== null && t.wallet !== owner) continue;
    tickets.push(t);
  }
  return tickets.reverse();
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
    removed++;
  }
  return removed;
}
