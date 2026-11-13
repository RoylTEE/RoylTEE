import type { NearConnector } from '@hot-labs/near-connect';
let connector: NearConnector | undefined;
export async function api<T>(path: string, init: { method?: string; body?: unknown } = {}): Promise<T> {
  const r = await fetch(path, { method: init.method ?? 'GET', credentials: 'same-origin', headers: init.body ? { 'Content-Type': 'application/json' } : {}, body: init.body ? JSON.stringify(init.body) : undefined });
  const data = await r.json().catch(() => ({ error: 'Something went wrong. Please try again.' }));
  if (!r.ok) throw Object.assign(new Error(data.error ?? 'Something went wrong.'), { status: r.status });
  return data as T;
}
const fromBase64 = (v: string) => Uint8Array.from(atob(v), c => c.charCodeAt(0));
/** NEP-413 sign in: the server issues the nonce and checks the signature and the full access key. */
export async function signIn(): Promise<{ accountId: string; admin: boolean; expiresAt: number }> {
  if (!connector) { const { NearConnector } = await import('@hot-labs/near-connect'); connector = new NearConnector({ network: 'testnet', features: { signMessage: true } }); }
  const challenge = await api<{ message: string; recipient: string; nonce: string }>('/api/auth/challenge', { method: 'POST', body: {} });
  const wallet = await connector.connect();
  const signed = await wallet.signMessage({ message: challenge.message, recipient: challenge.recipient, nonce: fromBase64(challenge.nonce), network: 'testnet' });
  return api('/api/auth/verify', { method: 'POST', body: { accountId: signed.accountId, publicKey: signed.publicKey, signature: signed.signature, nonce: challenge.nonce } });
}
export async function signOut() { await api('/api/auth/logout', { method: 'POST', body: {} }).catch(() => undefined); if (connector) await connector.disconnect().catch(() => undefined); }
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...children: (Node | string)[]) {
  const node = Object.assign(document.createElement(tag), props); node.append(...children); return node;
}
