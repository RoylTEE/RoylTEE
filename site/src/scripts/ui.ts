/** Small DOM helpers for the registry pages. Text only: no HTML strings are ever parsed. */
import { accountUrl, txUrl, ROLE_OF } from '../data/testnet';
import type { Token } from './chain';

type Child = Node | string | null | undefined | false;
export function h<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') node.className = String(v);
    else if (k === 'text') node.textContent = String(v);
    else if (k.startsWith('on') && typeof v === 'function') node.addEventListener(k.slice(2), v as EventListener);
    else if (k in node && typeof v !== 'string') (node as unknown as Record<string, unknown>)[k] = v;
    else node.setAttribute(k, String(v));
  }
  for (const c of children) if (c !== null && c !== undefined && c !== false) node.append(c);
  return node;
}
export type Tone = 'ok' | 'warn' | 'bad' | 'info' | 'plain' | 'sim';
export const badge = (text: string, tone: Tone = 'plain') => h('span', { class: `badge ${tone}`, text });
export const ext = { target: '_blank', rel: 'noopener' };
export const acct = (account: string, label?: string) => h('a', { class: 'acct-link', href: accountUrl(account), ...ext, text: label ?? account });
export const tx = (hash: string) => h('a', { class: 'acct-link', href: txUrl(hash), ...ext, text: hash.slice(0, 8) });
export const who = (account: string) => ROLE_OF[account] ?? account;
export const rightLabel: Record<string, string> = { publishing: 'Publishing', performance: 'Performance', mechanical: 'Mechanical', master: 'Master' };
export const side = (right: string) => right === 'master' ? 'Recording' : 'Composition';
export const provLabel: Record<string, string> = { human: 'Human', ai_assisted: 'AI assisted', fully_ai: 'Fully AI' };
export const tierLabel: Record<string, string> = { self_asserted: 'Self asserted', counterparty_confirmed: 'Co-holders confirmed', payer_confirmed: 'Payer confirmed' };

/** One workflow state per share, most urgent first. */
export function shareState(t: Token, approval?: string): { text: string; tone: Tone; why: string } {
  const e = t.extra;
  if (approval === 'pending') return { text: 'Waiting for approval', tone: 'warn', why: 'The work registrant has not signed this share yet. Its money is held in escrow.' };
  if (approval === 'rejected') return { text: 'Rejected', tone: 'bad', why: 'The work registrant rejected this share. It is void and is never paid.' };
  if (e.status === 'void') return { text: 'Void', tone: 'plain', why: 'Lost a dispute or its work was voided. Kept as a record, never paid again.' };
  if (e.dispute) return { text: 'Frozen by dispute', tone: 'bad', why: 'New money for this share goes to escrow until the registrar resolves the dispute.' };
  if (e.payout_in_flight) return { text: 'Payout in flight', tone: 'info', why: 'A claim is settling. Transfers wait until it finishes.' };
  if (e.transfer_pending) return { text: 'Transfer pending', tone: 'info', why: 'A transfer call is resolving. Claims wait until it finishes.' };
  if (e.status === 'active' && BigInt(e.claimable || '0') > 0n) return { text: 'Ready to claim', tone: 'ok', why: 'The holder can claim this balance now.' };
  if (e.status === 'active') return { text: 'Active', tone: 'ok', why: 'Paid on every payment to its work.' };
  return { text: e.status, tone: 'plain', why: '' };
}

export function fail(target: Element, err: unknown, retry?: () => void) {
  target.replaceChildren(h('div', { class: 'notice bad', role: 'alert' }, badge('Not loaded', 'bad'),
    h('div', {}, h('p', { text: `Could not read testnet: ${(err as Error).message}.` }), retry ? h('button', { class: 'btn sm', type: 'button', text: 'Try again', onclick: retry }) : null)));
}
export const loading = (text = 'Reading testnet') => h('p', { class: 'loading', text });
export function stamp(target: Element | null) { if (target) target.textContent = `Read from testnet at ${new Date().toISOString().slice(11, 16)} UTC`; }

/** Builds a CSV file in the browser and offers it as a download. */
export function csvLink(name: string, rows: (string | number)[][]) {
  const text = rows.map((r) => r.map((c) => { const s = String(c); return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; }).join(',')).join('\n');
  const a = h('a', { class: 'btn sm', download: name, text: 'Export CSV' });
  a.href = URL.createObjectURL(new Blob([text], { type: 'text/csv' }));
  return a;
}

/** A figure with a long label for wide screens and a short one for narrow screens. */
export const fig = (long: string, short: string, value: string, unit = '', note?: Node | string, opts: { wide?: boolean; keep?: boolean } = {}) =>
  h('div', { class: opts.wide ? 'figure wide' : 'figure' },
    h('span', { class: 'label' }, h('span', { class: 'l-long', text: long }), h('span', { class: 'l-short', text: short })),
    h('span', { class: 'value' }, value, unit ? h('small', { text: unit }) : null),
    note ? h('span', { class: opts.keep ? 'note keep' : 'note' }, note) : null);

