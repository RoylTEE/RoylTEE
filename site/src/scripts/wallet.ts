import type { NearConnector } from '@hot-labs/near-connect';
let connector: NearConnector | undefined;
export async function api<T>(path: string, init: { method?: string; body?: unknown } = {}): Promise<T> {
  const r = await fetch(path, { method: init.method ?? 'GET', credentials: 'same-origin', headers: init.body ? { 'Content-Type': 'application/json' } : {}, body: init.body ? JSON.stringify(init.body) : undefined });
  const data = await r.json().catch(() => ({ error: 'Something went wrong. Please try again.' }));
  if (!r.ok) throw Object.assign(new Error(data.error ?? 'Something went wrong.'), { status: r.status });
  return data as T;
}
/** The page CSP nonce (wallet pages only). The selector adds it to its sandboxed iframe scripts. */
const cspNonce = () => document.querySelector<HTMLScriptElement>('script[data-csp-nonce]')?.nonce || undefined;
const fromBase64 = (v: string) => Uint8Array.from(atob(v), c => c.charCodeAt(0));
/** NEP-413 sign in: the server issues the nonce and checks the signature and the full access key. */
export async function signIn(): Promise<{ accountId: string; admin: boolean; expiresAt: number }> {
  if (!connector) { const { NearConnector } = await import('@hot-labs/near-connect'); connector = new NearConnector({ network: 'testnet', features: { signMessage: true }, cspNonce: cspNonce() }); }
  const challenge = await api<{ message: string; recipient: string; nonce: string }>('/api/auth/challenge', { method: 'POST', body: {} });
  const wallet = await connector.connect();
  const signed = await wallet.signMessage({ message: challenge.message, recipient: challenge.recipient, nonce: fromBase64(challenge.nonce), network: 'testnet' });
  return api('/api/auth/verify', { method: 'POST', body: { accountId: signed.accountId, publicKey: signed.publicKey, signature: signed.signature, nonce: challenge.nonce } });
}
/** Wallet for registry transactions (Your shares page). The user's wallet signs and sends; this site never holds a key. */
let txConnector: NearConnector | undefined;
async function walletForTx() {
  if (!txConnector) { const { NearConnector } = await import('@hot-labs/near-connect'); txConnector = new NearConnector({ network: 'testnet', features: { signAndSendTransaction: true }, cspNonce: cspNonce() }); }
  return txConnector.connect();
}
export async function connectWallet(): Promise<string> {
  const wallet = await walletForTx();
  const [account] = await wallet.getAccounts();
  if (!account) throw new Error('The wallet did not share an account.');
  return account.accountId;
}
export type RegistryCall = { method: string; args: Record<string, unknown>; deposit: string; gas: string };
/** One transaction per call, all to the registry contract. Each owner method needs at least 1 yoctoNEAR. */
export async function sendCalls(receiverId: string, calls: RegistryCall[]) {
  const wallet = await walletForTx();
  const transactions = calls.map((c) => ({ receiverId, actions: [{ type: 'FunctionCall' as const, params: { methodName: c.method, args: c.args, gas: c.gas, deposit: c.deposit } }] }));
  return transactions.length === 1 ? [await wallet.signAndSendTransaction({ network: 'testnet', ...transactions[0] })] : wallet.signAndSendTransactions({ network: 'testnet', transactions });
}
export async function disconnectWallet() { if (txConnector) await txConnector.disconnect().catch(() => undefined); }
export async function signOut() { await api('/api/auth/logout', { method: 'POST', body: {} }).catch(() => undefined); if (connector) await connector.disconnect().catch(() => undefined); }
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...children: (Node | string)[]) {
  const node = Object.assign(document.createElement(tag), props); node.append(...children); return node;
}
