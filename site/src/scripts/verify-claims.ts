/** Pure evaluation of the live checks behind each claim on /verify. No DOM and no network here: the page reads the facts, tests feed fixtures. */
import type { LiveCheckId } from '../data/claims';

export type Facts = {
  vaultKeyCounts: number[];
  ledgerS1: { total_routed_in: string };
  ledgerS2: { total_routed_in: string };
  accounting: Record<'allowance_total' | 'unallocated_total' | 'escrow_total' | 'undelivered_total' | 'forwards_in_flight_total', string>;
  registryMusdc: string;
  s2Owner: string;
  recordedS2Owner: string;
  agentKeys: { permission: 'FullAccess' | { FunctionCall: { receiver_id: string; method_names: string[] } } }[];
  registry: string;
  recordedAgentMethods: string[];
  role: { methods: string[]; per_tx_limit: string; revoked: boolean } | null;
  dispute0: { status: unknown } | null;
  outsiderActive: boolean;
  memberActive: boolean;
  usdcMeta: { name: string; symbol: string; decimals: number };
};
export type Result = { ok: boolean; detail: string };

const same = (a: string[], b: string[]) => a.length === b.length && [...a].sort().every((x, i) => x === [...b].sort()[i]);

export function evaluate(f: Facts): Record<LiveCheckId, Result> {
  const keys = f.vaultKeyCounts.reduce((a, b) => a + b, 0);
  const counters = Object.values(f.accounting).reduce((a, v) => a + BigInt(v), 0n);
  const r1 = BigInt(f.ledgerS1.total_routed_in); const r2 = BigInt(f.ledgerS2.total_routed_in);
  const fc = f.agentKeys.length === 1 && typeof f.agentKeys[0].permission === 'object' ? f.agentKeys[0].permission.FunctionCall : null;
  const resolved = typeof f.dispute0?.status === 'object' && f.dispute0.status !== null && 'resolved' in f.dispute0.status;
  return {
    'vault-keys': { ok: f.vaultKeyCounts.length > 0 && keys === 0, detail: `${f.vaultKeyCounts.length} vaults, ${keys} access keys in total` },
    split: {
      ok: r1 > 0n && r1 === r2 && counters <= BigInt(f.registryMusdc),
      detail: `s1 and s2 each received ${r1 === r2 ? r1.toString() : `${r1} and ${r2}`} base units of work 0; registry counters ${counters} against ${f.registryMusdc} held`,
    },
    'owner-s2': { ok: f.s2Owner === f.recordedS2Owner, detail: `s2 is held by ${f.s2Owner}` },
    'agent-key': {
      ok: !!fc && fc.receiver_id === f.registry && same(fc.method_names, f.recordedAgentMethods),
      detail: fc ? `1 key, function call, receiver ${fc.receiver_id}, methods ${fc.method_names.join(', ')}` : `${f.agentKeys.length} keys, not a single function call key`,
    },
    'agent-limits': {
      ok: !!f.role && f.role.per_tx_limit === '100000' && same(f.role.methods, f.recordedAgentMethods),
      detail: f.role ? `per-transaction limit ${f.role.per_tx_limit} base units, revoked ${f.role.revoked}` : 'no role found',
    },
    dispute: {
      ok: resolved && BigInt(f.accounting.escrow_total) === 0n,
      detail: resolved ? `dispute 0 is resolved; escrow held now ${f.accounting.escrow_total}` : 'dispute 0 is not resolved',
    },
    credential: { ok: f.memberActive && !f.outsiderActive, detail: `member active ${f.memberActive}, outsider active ${f.outsiderActive}` },
    usdc: {
      ok: f.usdcMeta.symbol === 'mUSDC' && f.usdcMeta.decimals === 6 && /Mock/.test(f.usdcMeta.name) && /testnet/.test(f.usdcMeta.name),
      detail: `${f.usdcMeta.name}, ${f.usdcMeta.symbol}, ${f.usdcMeta.decimals} decimals`,
    },
  };
}
