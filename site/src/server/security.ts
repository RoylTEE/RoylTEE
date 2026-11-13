import { createHash, createHmac, timingSafeEqual, randomBytes } from 'node:crypto';
import { isIP } from 'node:net';
import type { Store } from './store';
export class HttpError extends Error { constructor(public status: number, message: string) { super(message); } }
export function fail(status: number, message: string): never { throw new HttpError(status, message); }
export const randomId = () => randomBytes(24).toString('hex');
export const digest = (value: unknown) => createHash('sha256').update(JSON.stringify(value)).digest('hex');
export function mac(secret: string, value: unknown) { return createHmac('sha256', secret).update(JSON.stringify(value)).digest('hex'); }
export function equal(a: string, b: string) {
  if (!/^[a-f0-9]{64}$/.test(a) || !/^[a-f0-9]{64}$/.test(b)) return false;
  return timingSafeEqual(Buffer.from(a, 'hex'), Buffer.from(b, 'hex'));
}
export function json(value: unknown, status = 200, headers: Record<string, string> = {}) {
  return new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', 'x-content-type-options': 'nosniff', ...headers } });
}
export function originCheck(request: Request, allowed: string[], mutating = true) {
  const origin = request.headers.get('origin');
  if (!allowed.includes(new URL(request.url).origin) || (mutating && !origin) || (origin && (origin !== new URL(request.url).origin || !allowed.includes(origin)))) fail(403, 'Please use this feature from the RoylTEE site.');
  if (request.headers.get('sec-fetch-site') === 'cross-site') fail(403, 'Please use this feature from the RoylTEE site.');
  if (mutating && request.headers.get('content-type')?.split(';')[0].trim().toLowerCase() !== 'application/json') fail(415, 'Please send JSON.');
}
/** Largest accepted JSON body. Sized for 12 chat turns at the server text caps (about 90 KiB of history plus framing). */
export const BODY_LIMIT = 128 * 1024;
export async function body(request: Request): Promise<Record<string, unknown>> {
  const reader = request.body?.getReader(); if (!reader) return {};
  let bytes = 0; const chunks: Uint8Array[] = [];
  while (true) { const { done, value } = await reader.read(); if (done) break; bytes += value.length; if (bytes > BODY_LIMIT) { await reader.cancel(); fail(413, 'That request is too long.'); } chunks.push(value); }
  try { const v = JSON.parse(Buffer.concat(chunks).toString('utf8')); if (!v || Array.isArray(v) || typeof v !== 'object') throw new Error(); return v; } catch { return fail(400, 'Please send a valid request.'); }
}
export function text(v: unknown, max: number, min = 1) {
  if (typeof v !== 'string' || v.trim().length < min || v.length > max || /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(v)) fail(400, 'Please keep the text within the shown length.');
  return (v as string).trim();
}
export async function rate(store: Store, id: string, max: number, seconds: number) {
  if (await store.count(`rate:${id}`, seconds) > max) fail(429, 'You have reached the limit. Please try again later.');
}
export function cookie(request: Request, name: string) { return request.headers.get('cookie')?.split(';').map(v => v.trim()).find(v => v.startsWith(`${name}=`))?.slice(name.length + 1) ?? ''; }
export const SESSION_COOKIE = 'royltee_support';
export const SESSION_COOKIE_HOST = '__Host-royltee_support';
/** Secure on Vercel even if request.url is http, and on any https request or https forwarded proto. */
export function cookieSecure(request: Request): boolean {
  if (process.env.VERCEL) return true;
  const proto = request.headers.get('x-forwarded-proto')?.split(',')[0]?.trim().toLowerCase();
  if (proto === 'https') return true;
  try { return new URL(request.url).protocol === 'https:'; } catch { return false; }
}
export function sessionCookieName(secure: boolean) { return secure ? SESSION_COOKIE_HOST : SESSION_COOKIE; }
/** Prefer the __Host- cookie; fall back to the legacy name so a mid-deploy session still works. */
export function sessionCookie(request: Request) { return cookie(request, SESSION_COOKIE_HOST) || cookie(request, SESSION_COOKIE); }
export function setCookie(name: string, value: string, seconds: number, secure: boolean) {
  return `${name}=${value}; HttpOnly; SameSite=Strict; Path=/; Max-Age=${seconds}${secure ? '; Secure' : ''}`;
}
export function sessionSetCookie(value: string, seconds: number, secure: boolean) {
  return setCookie(sessionCookieName(secure), value, seconds, secure);
}
/** Expand a valid IPv6 address to 8 hextets, or null. */
export function expandIPv6(address: string): string[] | null {
  const lower = address.toLowerCase();
  if (lower.includes('.')) return null;
  const ends = lower.split('::');
  if (ends.length > 2) return null;
  const head = ends[0] ? ends[0].split(':') : [];
  const tail = ends.length === 2 ? (ends[1] ? ends[1].split(':') : []) : [];
  if (head.includes('') || tail.includes('')) return null;
  const parts = ends.length === 1 ? head : [...head, ...Array(8 - head.length - tail.length).fill('0'), ...tail];
  if (parts.length !== 8 || parts.some(p => !/^[0-9a-f]{1,4}$/.test(p))) return null;
  return parts.map(p => p.padStart(4, '0'));
}
/** IPv4 unchanged. IPv4-mapped IPv6 becomes IPv4. Other IPv6 is masked to /64. */
export function normalizeClientAddress(address: string): string {
  const raw = address.trim().replace(/^\[|\]$/g, '').split('%')[0];
  if (isIP(raw) === 4) return raw;
  if (isIP(raw) !== 6) return raw;
  const mapped = raw.match(/^::ffff:(\d{1,3}(?:\.\d{1,3}){3})$/i);
  if (mapped && isIP(mapped[1]) === 4) return mapped[1];
  const parts = expandIPv6(raw);
  if (!parts) return raw;
  const hex = (h: string) => h.replace(/^0+/, '') || '0';
  return `${parts.slice(0, 4).map(hex).join(':')}::`;
}
export const clientKey = (address: string, secret: string) => mac(secret, ['ip-v1', normalizeClientAddress(address)]).slice(0, 32);
export const ACCOUNT_ID = /^(?=.{2,64}$)(?:(?:[a-z\d]+[-_])*[a-z\d]+\.)*(?:[a-z\d]+[-_])*[a-z\d]+$/;
export const ED25519_KEY = /^ed25519:[1-9A-HJ-NP-Za-km-z]{32,44}$/;
