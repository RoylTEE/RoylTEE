import { afterEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { hasDispute, transferChecks, canTransfer, canClaim } from '../src/scripts/transfer-rules';
import { shareState } from '../src/scripts/ui';
import { groupTotals } from '../src/scripts/split-rules';
import { breakdown } from '../src/scripts/claimable';
import { keyStatus } from '../src/scripts/key-status';
import { tokensFor, allDisputes, allWorks, approvalLive, storagePriceOrEstimate, musdcInput } from '../src/scripts/chain';
import type { Token, AccessKey } from '../src/scripts/chain';

const read = (p: string) => readFileSync(new URL(`../src/${p}`, import.meta.url), 'utf8');
const tok = (extra: Partial<Token['extra']>): Token => ({ token_id: 's5', owner_id: 'a.royltee-dev.testnet', extra: { work_id: 1, right_type: 'publishing', territory: 'WW', percent_bps: 5000, basis: 'x', provenance: 'p', evidence_tier: 'e', status: 'active', vault: 's5.reg.royltee-dev.testnet', earned: '0', paid_out: '0', claimable: '0', escrowed: '0', payout_in_flight: false, transfer_pending: false, dispute: null, ...extra } });

/** A fake RPC: `views` maps a method name to a function of its args. */
function fakeRpc(views: Record<string, (args: any) => unknown>, calls: string[] = []) {
  vi.stubGlobal('fetch', async (_url: string, init: { body: string }) => {
    const body = JSON.parse(init.body); const p = body.params;
    if (p.request_type === 'call_function') {
      const args = JSON.parse(Buffer.from(p.args_base64, 'base64').toString());
      calls.push(`${p.method_name}:${JSON.stringify(args)}`);
      const f = views[p.method_name];
      if (!f) return { ok: true, json: async () => ({ result: { error: 'wasm execution failed with error: MethodResolveError(MethodNotFound)' } }) };
      return { ok: true, json: async () => ({ result: { result: [...Buffer.from(JSON.stringify(f(args)))] } }) };
    }
    if (p.request_type === 'view_access_key_list') return { ok: true, json: async () => ({ result: { keys: [] } }) };
    return { ok: false, status: 500, json: async () => ({}) };
  });
  return calls;
}
afterEach(() => vi.unstubAllGlobals());

describe('second reviewer 03d item 3: dispute 0 and the approval default', () => {
  it('treats dispute id 0 as a real dispute', () => {
    expect(hasDispute(0)).toBe(true);
    expect(hasDispute(null)).toBe(false);
    expect(hasDispute(undefined)).toBe(false);
    expect(shareState(tok({ dispute: 0 })).text).toBe('Frozen by dispute');
    const c = transferChecks({ ...({} as any), signer: 'a.royltee-dev.testnet', owner: 'a.royltee-dev.testnet', dispute: 0, status: 'active' } as any);
    expect(canTransfer(c)).toBe(false);
  });
  it('uses extra.approval when the caller passes no approval', () => {
    expect(shareState(tok({ approval: 'pending' }))).toEqual(shareState(tok({}), 'pending'));
    expect(shareState(tok({ approval: 'rejected' }))).toEqual(shareState(tok({}), 'rejected'));
  });
});

describe('second reviewer 03d item 9: claim rules are separate from transfer rules', () => {
  const base = { signer: 'a', owner: 'a', claimable: 5n, payoutInFlight: false, transferPending: false, suspended: false };
  it('lets a void share claim, as the contract does', () => { expect(canClaim(base).ok).toBe(true); });
  it('refuses for the cases the contract refuses', () => {
    expect(canClaim({ ...base, signer: null }).ok).toBe(false);
    expect(canClaim({ ...base, signer: 'b' }).why).toMatch(/holder/);
    expect(canClaim({ ...base, claimable: 0n }).why).toBe('Nothing to claim');
    expect(canClaim({ ...base, suspended: true }).ok).toBe(false);
    expect(canClaim({ ...base, transferPending: true }).ok).toBe(false);
    expect(canClaim({ ...base, payoutInFlight: true }).ok).toBe(false);
  });
  it('the member page no longer ties claim to status active', () => {
    const m = read('scripts/member.ts');
    expect(m).toMatch(/canClaim\(/);
    expect(m).not.toMatch(/claimable\) > 0n && t\.extra\.status === 'active'/);
  });
});

describe('second reviewer 03d item 2: each peer shows its real approval status', () => {
  it('reviewCard reads the status map, not a fixed Approved badge', () => {
    const m = read('scripts/member.ts');
    expect(m).toMatch(/peerBadge\(statusOf\.get\(x\.token_id\)/);
    expect(m).not.toMatch(/: badge\('Approved', 'ok'\) \}\)\);/);
  });
});

describe('second reviewer 03d item 1: approval claims depend on the live contract', () => {
  it('approvalLive is false when get_share_approval is missing, true when present', async () => {
    fakeRpc({});
    expect(await approvalLive()).toBe(false);
    fakeRpc({ get_share_approval: () => 'approved' });
    expect(await approvalLive()).toBe(true);
  });
  it('the draft text branches on the capability', () => {
    expect(read('scripts/member.ts')).toMatch(/live \? `You did not register work/);
  });
});

describe('second reviewer 03d item 4: totals per work, right and territory', () => {
  it('keeps territories apart', () => {
    const t = groupTotals([
      { right_type: 'publishing', territory: 'WW', percent_bps: 6000, status: 'active' },
      { right_type: 'publishing', territory: 'US', percent_bps: 6000, status: 'active' },
      { right_type: 'publishing', territory: 'WW', percent_bps: 4000, status: 'active' },
      { right_type: 'publishing', territory: 'WW', percent_bps: 9000, status: 'void' },
    ]);
    expect(t).toEqual([{ right: 'publishing', territory: 'WW', bps: 10000 }, { right: 'publishing', territory: 'US', bps: 6000 }]);
  });
});

describe('second reviewer 03d item 5: actual, claimable, unsynced and held are separate', () => {
  it('caps claimable at the vault and reports unsynced money', () => {
    expect(breakdown({ ledger_claimable: '100', claimable: '100', vault_balance: '40', escrowed: '0' })).toEqual({ inVault: 40n, claimable: 40n, unsynced: 0n, held: 0n });
    expect(breakdown({ ledger_claimable: '10', claimable: '10', vault_balance: '25', escrowed: '7' })).toEqual({ inVault: 25n, claimable: 10n, unsynced: 15n, held: 7n });
    expect(breakdown({ claimable: '3', vault_balance: null })).toEqual({ inVault: null, claimable: 3n, unsynced: 0n, held: 0n });
  });
  it('drops the "claimed down to zero" wording', () => {
    for (const f of ['scripts/claims.ts', 'scripts/overview.ts']) expect(read(f)).not.toMatch(/claimed down to zero/);
  });
});

describe('second reviewer 03d item 7: Keyless means zero keys', () => {
  const fc: AccessKey = { public_key: 'ed25519:F', access_key: { permission: { FunctionCall: { receiver_id: 'reg.royltee-dev.testnet', method_names: ['grant_role'], allowance: null } } } };
  const fa: AccessKey = { public_key: 'ed25519:A', access_key: { permission: 'FullAccess' } };
  it('function call keys are not Keyless and are listed', () => {
    const s = keyStatus([fc]);
    expect(s.text).not.toBe('Keyless');
    expect(s.detail[0]).toMatch(/function call to reg\.royltee-dev\.testnet, grant_role/);
  });
  it('zero keys is Keyless, a full access key is deletion planned', () => {
    expect(keyStatus([]).text).toBe('Keyless');
    expect(keyStatus([fa, fc]).text).toBe('Deletion planned');
  });
});

describe('second reviewer 03d item 10: holder tokens and disputes are paged', () => {
  it('tokensFor reads past the first page', async () => {
    const all = Array.from({ length: 120 }, (_, i) => ({ token_id: `s${i}`, owner_id: 'a', metadata: { extra: JSON.stringify({ claimable: '0', vault: `s${i}.v` }) } }));
    fakeRpc({ nft_tokens_for_owner: (a) => all.slice(Number(a.from_index), Number(a.from_index) + a.limit), ft_balance_of: () => '0' });
    expect((await tokensFor('a')).length).toBe(120);
  });
  it('allDisputes reads past the first page', async () => {
    const all = Array.from({ length: 75 }, (_, i) => ({ id: i }));
    fakeRpc({ list_disputes: (a) => all.slice(a.from_index, a.from_index + a.limit) });
    expect((await allDisputes()).length).toBe(75);
  });
  it('allWorks lists works with no shares yet (item 6)', async () => {
    fakeRpc({ get_work: (a) => (a.work_id < 3 ? { id: a.work_id, status: 'active' } : null) });
    expect((await allWorks()).map((w) => w.id)).toEqual([0, 1, 2]);
    expect(read('scripts/member.ts')).toMatch(/allWorks\(\)/);
  });
});

describe('second reviewer 03d item 11: a fallback storage price is labeled', () => {
  it('reports live false when the protocol config cannot be read', async () => {
    fakeRpc({});
    const r = await storagePriceOrEstimate();
    expect(r).toEqual({ perByte: 10n ** 19n, live: false });
    expect(read('scripts/sharecard.ts')).toMatch(/UNVERIFIED estimate/);
    expect(read('scripts/accounts.ts')).toMatch(/UNVERIFIED estimate/);
  });
});

describe('second reviewer 03d item 12: share inspector ignores stale reads', () => {
  it('a newer selection invalidates the older one', async () => {
    const { nextGeneration } = await import('../src/scripts/sharecard');
    const el = {} as Element;
    const first = nextGeneration(el); const second = nextGeneration(el);
    expect(first()).toBe(false); expect(second()).toBe(true);
  });
});

describe('second reviewer 03d item 13: input defaults have no thousands separators', () => {
  it('musdcInput strips separators', () => {
    expect(musdcInput(1234567_000000n)).toBe('1234567.00');
    expect(read('scripts/org.ts')).toMatch(/musdcInput\(role\.allowance\)/);
  });
});
