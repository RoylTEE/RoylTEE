import { describe, expect, it } from 'vitest';
import { kindOf, lookup, payees } from '../src/scripts/lookup-match';
import type { Token, Work } from '../src/scripts/chain';

const w = (id: number, code_type: string, code: string, by = 'writer-a.royltee-dev.testnet'): Work => ({ id, code_type, code, provenance: 'human', registered_by: by, status: 'active', groups: [], created_at: '0' });
const t = (token_id: string, work_id: number, owner_id: string, bps: number, status = 'active', dispute: unknown = null): Token =>
  ({ token_id, owner_id, extra: { work_id, right_type: 'publishing', territory: 'WW', percent_bps: bps, basis: 'copyright', provenance: 'human', evidence_tier: 'self_asserted', status, vault: `${token_id}.reg.royltee-dev.testnet`, earned: '0', paid_out: '0', claimable: '0', escrowed: '0', payout_in_flight: false, transfer_pending: false, dispute } });
const works = [w(0, 'isrc', 'ZZRYL2600001'), w(1, 'iswc', 'T1234567890')];
const tokens = [t('s1', 0, 'writer-a.royltee-dev.testnet', 5000), t('s2', 0, 'buyer-pub.royltee-dev.testnet', 5000), t('s3', 1, 'writer-a.royltee-dev.testnet', 10000)];

describe('catalog lookup', () => {
  it('tells codes, share ids and accounts apart', () => {
    expect(kindOf('ZZ-RYL-26-00001')).toBe('code');
    expect(kindOf('T-123.456.789-0')).toBe('code');
    expect(kindOf('s12')).toBe('share');
    expect(kindOf('buyer-pub.royltee-dev.testnet')).toBe('account');
  });
  it('finds a work by ISRC with or without separators, and by ISWC', () => {
    expect(lookup('zz-ryl-26-00001', works, tokens).map((x) => x.work.id)).toEqual([0]);
    expect(lookup('T-123.456.789-0', works, tokens).map((x) => x.work.id)).toEqual([1]);
  });
  it('finds a share and everything an account holds', () => {
    expect(lookup('S2', works, tokens)[0].tokens.map((x) => x.token_id)).toEqual(['s2']);
    const a = lookup('writer-a.royltee-dev.testnet', works, tokens);
    expect(a.map((x) => x.why)).toEqual(['Holds s1', 'Holds s3']);
    expect(lookup('nobody.testnet', works, tokens)).toEqual([]);
  });
  it('pays the current owner, not the first holder, and marks void or disputed shares', () => {
    const p = payees(['s2', 's1', 'missing'], [...tokens, t('s9', 0, 'x.testnet', 2000, 'void')]);
    expect(p.map((x) => x.payee)).toEqual(['buyer-pub.royltee-dev.testnet', 'writer-a.royltee-dev.testnet']);
    const q = payees(['s9', 's8'], [t('s9', 0, 'x.testnet', 2000, 'void'), t('s8', 0, 'y.testnet', 7000, 'active', { id: 0 })]);
    expect(q.map((x) => x.paid)).toEqual([false, false]);
  });
});

import { walletFields } from '../src/scripts/wallet-view';
import { readFileSync } from 'node:fs';
describe('wallet view', () => {
  it('reports no media and disabled approvals straight from the views', () => {
    const f = Object.fromEntries(walletFields({ owner_id: 'buyer-pub.royltee-dev.testnet', metadata: { title: 'Rights share s2 of work 0', media: null }, approved_account_ids: null }, { spec: 'nft-1.0.0', name: 'RoylTEE', symbol: 'ROYLT', icon: null }).map(([k, v]) => [k, v]));
    expect(f.Image).toBe('No media');
    expect(f['Marketplace approvals']).toBe('Disabled');
    expect(f.Collection).toBe('RoylTEE (ROYLT)');
  });
  it('says plainly that wallet display is UNVERIFIED', () => {
    const page = readFileSync(new URL('../src/pages/share.astro', import.meta.url), 'utf8');
    expect(page).toMatch(/id="wallet" data-tab-panel/);
    expect(page).toMatch(/How any given wallet displays it is UNVERIFIED/);
  });
});
