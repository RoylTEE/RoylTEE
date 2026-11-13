import { createHash, createHmac, timingSafeEqual, randomBytes } from 'node:crypto';
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
export async function body(request: Request): Promise<Record<string, unknown>> {
  const reader = request.body?.getReader(); if (!reader) return {};
  let bytes = 0; const chunks: Uint8Array[] = [];
  while (true) { const { done, value } = await reader.read(); if (done) break; bytes += value.length; if (bytes > 65536) { await reader.cancel(); fail(413, 'That request is too long.'); } chunks.push(value); }
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
export function setCookie(name: string, value: string, seconds: number, secure: boolean) {
  return `${name}=${value}; HttpOnly; SameSite=Strict; Path=/; Max-Age=${seconds}${secure ? '; Secure' : ''}`;
}
export const clientKey = (address: string, secret: string) => mac(secret, ['ip-v1', address]).slice(0, 32);
export const ACCOUNT_ID = /^(?=.{2,64}$)(?:(?:[a-z\d]+[-_])*[a-z\d]+\.)*(?:[a-z\d]+[-_])*[a-z\d]+$/;
