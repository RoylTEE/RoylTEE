import { createHash, createPublicKey, verify } from 'node:crypto';
import { base58 } from '@scure/base';
import { equal, fail, mac } from './security';
export type Challenge = { message: string; recipient: string; nonce: string; expires: number; binding: string };
const u32 = (n: number) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const str = (s: string) => Buffer.concat([u32(Buffer.byteLength(s)), Buffer.from(s)]);
/** NEP-413: u32 tag, Borsh string, fixed nonce, Borsh string, Option callback URL = None. */
export function nep413Hash(message: string, nonce: Uint8Array, recipient: string) {
  if (nonce.length !== 32) throw new Error('Invalid nonce');
  return createHash('sha256').update(Buffer.concat([u32(2 ** 31 + 413), str(message), Buffer.from(nonce), str(recipient), Buffer.from([0])])).digest();
}
export function verifySignature(challenge: Challenge, publicKey: string, signature: string) {
  try {
    if (!/^ed25519:[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(publicKey) || !/^[A-Za-z0-9+/]{86}==$/.test(signature)) return false;
    const raw = base58.decode(publicKey.slice(8)); if (raw.length !== 32) return false;
    const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), raw]), format: 'der', type: 'spki' });
    return verify(null, nep413Hash(challenge.message, Buffer.from(challenge.nonce, 'base64'), challenge.recipient), key, Buffer.from(signature, 'base64'));
  } catch { return false; }
}
export async function fullAccessKey(rpc: string, accountId: string, publicKey: string, fetcher = fetch) {
  const r = await fetcher(rpc, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 'support-auth', method: 'query', params: { request_type: 'view_access_key', finality: 'final', account_id: accountId, public_key: publicKey } }), signal: AbortSignal.timeout(8000) });
  if (!r.ok) fail(503, 'Wallet verification is temporarily unavailable.');
  const j = await r.json();
  return j.result?.permission === 'FullAccess';
}
export type Session = { sid: string; wallet: string; exp: number; publicKey?: string };
export type SessionRecord = { wallet: string; publicKey?: string };
export function recordWallet(value: unknown): SessionRecord | null {
  if (typeof value === 'string') return { wallet: value };
  if (value && typeof value === 'object' && typeof (value as SessionRecord).wallet === 'string') return value as SessionRecord;
  return null;
}
export function sessionToken(secret: string, session: Session) {
  const data = Buffer.from(JSON.stringify(session)).toString('base64url'); return `${data}.${mac(secret, ['support-session-v1', data])}`;
}
export function readSession(secret: string, token: string, now: number): Session {
  const [data, signature = '', extra] = token.split('.');
  if (!data || extra || !equal(signature, mac(secret, ['support-session-v1', data]))) return fail(401, 'Please sign in with your wallet again.');
  try { const s = JSON.parse(Buffer.from(data, 'base64url').toString()); if (typeof s.sid !== 'string' || typeof s.wallet !== 'string' || typeof s.exp !== 'number' || s.exp <= now) throw new Error(); if (s.publicKey !== undefined && typeof s.publicKey !== 'string') throw new Error(); return s; } catch { return fail(401, 'Please sign in with your wallet again.'); }
}
