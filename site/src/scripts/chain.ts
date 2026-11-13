/** Read only NEAR testnet client for the registry pages. View calls only: nothing here signs or sends. */
import { RPC_URLS, REGISTRY, TOKEN, TOKEN_DECIMALS } from '../data/testnet';
import { shownClaimable } from './claimable';

export class ViewError extends Error { constructor(message: string, readonly missingMethod = false) { super(message); } }

async function rpc<T>(method: string, params: unknown): Promise<T> {
  let last: unknown;
  for (const url of RPC_URLS) {
    const ctrl = new AbortController(); const t = setTimeout(() => ctrl.abort(), 12000);
    try {
      const r = await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 'royltee', method, params }), signal: ctrl.signal });
      if (!r.ok) { last = new Error(`RPC ${r.status}`); continue; }
      const data = await r.json();
      if (data.error) {
        const msg = JSON.stringify(data.error);
        if (/MethodNotFound/.test(msg)) throw new ViewError('method not on the deployed contract', true);
        if (/UNKNOWN_ACCOUNT|does not exist/.test(msg)) throw new ViewError('account does not exist');
        last = new Error(data.error.message ?? 'RPC error'); continue;
      }
      if (data.result?.error) {
        const msg = String(data.result.error);
        if (/MethodNotFound|MethodResolveError/.test(msg)) throw new ViewError('method not on the deployed contract', true);
        throw new ViewError(msg.replace(/^.*Smart contract panicked: /, '').slice(0, 160));
      }
      return data.result as T;
    } catch (err) { if (err instanceof ViewError) throw err; last = err; }
    finally { clearTimeout(t); }
  }
  throw new ViewError(last instanceof Error && last.name === 'AbortError' ? 'testnet RPC timed out' : 'testnet RPC is not reachable');
}

const toB64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));
export async function view<T>(method: string, args: Record<string, unknown> = {}, contract: string = REGISTRY): Promise<T> {
  const r = await rpc<{ result: number[] }>('query', { request_type: 'call_function', finality: 'final', account_id: contract, method_name: method, args_base64: toB64(JSON.stringify(args)) });
  return JSON.parse(new TextDecoder().decode(new Uint8Array(r.result))) as T;
}
export type AccountView = { amount: string; locked: string; storage_usage: number; code_hash: string };
export const viewAccount = (account_id: string) => rpc<AccountView>('query', { request_type: 'view_account', finality: 'final', account_id });
export type AccessKey = { public_key: string; access_key: { permission: 'FullAccess' | { FunctionCall: { receiver_id: string; method_names: string[]; allowance: string | null } } } };
export const accessKeys = async (account_id: string) => (await rpc<{ keys: AccessKey[] }>('query', { request_type: 'view_access_key_list', finality: 'final', account_id })).keys;
export const tokenBalance = (account_id: string) => view<string>('ft_balance_of', { account_id }, TOKEN);
/** yoctoNEAR per byte of storage, read from the live protocol config. */
export async function storagePrice(): Promise<bigint> {
  const r = await rpc<{ runtime_config: { storage_amount_per_byte: string } }>('EXPERIMENTAL_protocol_config', { finality: 'final' });
  return BigInt(r.runtime_config.storage_amount_per_byte);
}

/* Shapes returned by the registry views (see DESIGN.md in the repository). */
export type Group = { work_id: number; right_type: string; territory: string };
export type Work = { id: number; code_type: string; code: string; provenance: string; registered_by: string; status: string; groups: Group[]; created_at: string };
export type Share = { token_id: string; number: number; group: Group; percent_bps: number; basis: string; status: string; vault: string; holder_at_mint: string; registered_by: string; dispute: unknown; confirmations: string[]; last_effective_from: string | null; created_at: string; note: string | null };
export type Ledger = { known_balance: string; total_paid_out: string; earned: string; claimable: string; total_routed_in: string; undelivered: string; escrowed: string; payout_in_flight: unknown; transfer_pending: unknown };
export type TokenExtra = { work_id: number; right_type: string; territory: string; percent_bps: number; basis: string; provenance: string; evidence_tier: string; status: string; vault: string; earned: string; paid_out: string; claimable: string; escrowed: string; payout_in_flight: boolean; transfer_pending: boolean; approval?: string; dispute: unknown; ledger_claimable?: string; vault_balance?: string | null };
export type Token = { token_id: string; owner_id: string; extra: TokenExtra };
export type Dispute = { id: number; kind: Record<string, { group?: Group; claimant?: string; contested?: string[]; work_id?: number; original?: number }>; opened_by: string; opened_at: string; status: 'open' | { resolved: { outcome: string; doc_hash: string; at: string } } | string };
export type Role = { agent: string; org: string; methods: string[]; per_tx_limit: string; daily_limit: string; max_tx_per_day: number; expires_at: string; revoked: boolean; expired: boolean; allowance: string; line_credits: number; spent_last_24h: string; tx_last_24h: number };
export type Accounting = { allowance_total: string; unallocated_total: string; escrow_total: string; undelivered_total: string; forwards_in_flight_total: string };

type RawToken = { token_id: string; owner_id: string; metadata: { extra: string | null } };
const parseToken = (t: RawToken): Token => ({ token_id: t.token_id, owner_id: t.owner_id, extra: JSON.parse(t.metadata.extra || '{}') });
/**
 * Caps each token's claimable at its vault's real mock USDC balance (review 03d M1, T21). The ledger
 * figure is kept in `ledger_claimable`. If the vault balance cannot be read, the ledger figure is shown
 * and `vault_balance` is null.
 */
async function capClaimable(tokens: Token[]): Promise<Token[]> {
  await pool(tokens, async (t) => {
    let bal: string | null = null;
    try { bal = await tokenBalance(t.extra.vault); } catch { bal = null; }
    t.extra.ledger_claimable = t.extra.claimable;
    t.extra.vault_balance = bal;
    t.extra.claimable = shownClaimable(t.extra.claimable, bal);
  });
  return tokens;
}
export async function allTokens(): Promise<Token[]> {
  const out: Token[] = [];
  for (let from = 0; ; from += 50) {
    const page = await view<RawToken[]>('nft_tokens', { from_index: String(from), limit: 50 });
    out.push(...page.map(parseToken));
    if (page.length < 50) return capClaimable(out);
  }
}
/** Every token of one holder, page by page (second reviewer 03d item 10). */
export async function tokensFor(account_id: string): Promise<Token[]> {
  const out: Token[] = [];
  for (let from = 0; ; from += 50) {
    const page = await view<RawToken[]>('nft_tokens_for_owner', { account_id, from_index: String(from), limit: 50 });
    out.push(...page.map(parseToken));
    if (page.length < 50) return capClaimable(out);
  }
}
/** Every work by id. Ids start at 0 and are never reused, so the first missing id ends the list. */
export async function allWorks(max = 1000): Promise<Work[]> {
  const out: Work[] = [];
  for (let id = 0; id < max; id++) { const w = await view<Work | null>('get_work', { work_id: id }); if (!w) break; out.push(w); }
  return out;
}
/** Every dispute, page by page. */
export async function allDisputes(): Promise<Dispute[]> {
  const out: Dispute[] = [];
  for (let from = 0; ; from += 50) {
    const page = await view<Dispute[]>('list_disputes', { from_index: from, limit: 50 });
    out.push(...page);
    if (page.length < 50) return out;
  }
}
/** Whether the deployed registry has split sheet approval: true only if get_share_approval exists. */
export async function approvalLive(): Promise<boolean> {
  try { await view('get_share_approval', { token_id: 's1' }); return true; }
  catch (err) { if (err instanceof ViewError && err.missingMethod) return false; if (err instanceof ViewError && !/RPC/.test(err.message)) return true; throw err; }
}
/** Storage price per byte read live, or a fixed estimate marked as such if the read fails (second reviewer 03d item 11). */
export async function storagePriceOrEstimate(): Promise<{ perByte: bigint; live: boolean }> {
  try { return { perByte: await storagePrice(), live: true }; } catch { return { perByte: 10n ** 19n, live: false }; }
}
export const oneToken = async (token_id: string) => { const t = await view<RawToken | null>('nft_token', { token_id }); return t ? (await capClaimable([parseToken(t)]))[0] : null; };

/** Split sheet status. The deployed registry predates this view, so a missing method is reported as such. */
export async function approvalOf(token: Token): Promise<'approved' | 'pending' | 'rejected' | 'not deployed'> {
  if (token.extra.approval) return token.extra.approval as 'approved';
  try { return await view('get_share_approval', { token_id: token.token_id }); }
  catch (err) { if (err instanceof ViewError && err.missingMethod) return 'not deployed'; throw err; }
}

/** Runs tasks with a small concurrency limit so a page does not flood the public RPC. */
export async function pool<T, R>(items: T[], fn: (x: T) => Promise<R>, n = 4): Promise<R[]> {
  const out: R[] = new Array(items.length); let i = 0;
  await Promise.all(Array.from({ length: Math.min(n, items.length) }, async () => { while (i < items.length) { const k = i++; out[k] = await fn(items[k]); } }));
  return out;
}

/* Formatting */
export function units(raw: string | bigint, decimals: number, min = 2, max = decimals): string {
  const v = BigInt(raw); const neg = v < 0n; const a = neg ? -v : v;
  const base = 10n ** BigInt(decimals); const whole = a / base; let frac = (a % base).toString().padStart(decimals, '0').slice(0, max);
  while (frac.length > min && frac.endsWith('0')) frac = frac.slice(0, -1);
  return `${neg ? '-' : ''}${whole.toLocaleString('en-US')}${frac ? '.' + frac : ''}`;
}
export const musdc = (raw: string | bigint) => units(raw, TOKEN_DECIMALS, 2);
/** For input defaults: no thousands separators, so the value parses back unchanged (second reviewer 03d item 13). */
export const musdcInput = (raw: string | bigint) => musdc(raw).replace(/,/g, '');
export const near = (yocto: string | bigint) => units(yocto, 24, 4, 4);
export const pct = (bps: number) => `${(bps / 100).toFixed(bps % 100 ? 2 : 0)}%`;
export const utc = (ns: string | null | undefined) => ns ? new Date(Number(BigInt(ns) / 1_000_000n)).toISOString().slice(0, 16).replace('T', ' ') + ' UTC' : 'none';
export const short = (account: string) => account.replace(/\.royltee-dev\.testnet$/, '');

const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
/** Base58 (as in NEAR code_hash) to lowercase hex, so it can be compared with a sha256 hex. */
export function base58ToHex(s: string): string {
  let n = 0n;
  for (const c of s) { const i = B58.indexOf(c); if (i < 0) throw new Error('not base58'); n = n * 58n + BigInt(i); }
  let hex = n.toString(16); if (hex.length % 2) hex = '0' + hex;
  const lead = s.match(/^1*/)![0].length;
  return '00'.repeat(lead) + hex;
}
