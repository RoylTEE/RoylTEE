import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { transferChecks, canTransfer, type TransferFacts } from '../src/scripts/transfer-rules';

const base: TransferFacts = { sender: 'writer-b.royltee-dev.testnet', owner: 'writer-b.royltee-dev.testnet', receiver: 'buyer-pub.royltee-dev.testnet', status: 'active', dispute: null, approval: 'not deployed', payoutInFlight: null, transferPending: null, senderCredential: 'active', receiverActive: true };
const blocked = (f: Partial<TransferFacts>) => transferChecks({ ...base, ...f }).filter((c) => !c.ok).map((c) => c.text);

describe('transfer screen: the same rules as check_transfer in the contract', () => {
  it('a clean transfer to an active member passes every check', () => expect(canTransfer(transferChecks(base))).toBe(true));
  it('blocks each rule the contract enforces, and says why', () => {
    expect(blocked({ sender: 'writer-a.royltee-dev.testnet' })).toEqual(['Only the current holder can transfer. Connect the holder wallet']);
    expect(blocked({ sender: '' })[0]).toMatch(/Connect the holder wallet/);
    expect(blocked({ receiver: base.owner })).toContain('The receiver already holds this share');
    expect(blocked({ receiverActive: false })).toEqual(['The receiver has no active member credential, so the contract refuses']);
    expect(blocked({ status: 'void' })).toEqual(['The share is void and cannot move']);
    expect(blocked({ dispute: { id: 0 } })).toEqual(['The share is frozen by a dispute']);
    expect(blocked({ approval: 'pending' })).toEqual(['The work registrant has not approved this share yet']);
    expect(blocked({ approval: 'rejected' })).toEqual(['The work registrant rejected this share']);
    expect(blocked({ payoutInFlight: { amount: '1' } })).toEqual(['A payout is in flight. Transfer after it settles']);
    expect(blocked({ transferPending: { to: 'x' } })).toEqual(['Another transfer is pending']);
    expect(blocked({ senderCredential: 'suspended' })).toEqual(['Your credential is suspended, so you cannot send']);
  });
  it('the approval view missing on the deployed build does not block', () => expect(blocked({ approval: 'not deployed' })).toEqual([]));
  it('the page warns to claim first and signs nft_transfer with 1 yoctoNEAR through the wallet', () => {
    const m = readFileSync(new URL('../src/scripts/member.ts', import.meta.url), 'utf8');
    expect(m).toMatch(/Claim first/);
    expect(m).toMatch(/is_active_member/);
    expect(m).toMatch(/method: 'nft_transfer', args: \{ receiver_id: receiver, token_id: t\.token_id/);
    expect(m).not.toMatch(/approval_id/);
    expect(readFileSync(new URL('../src/pages/member.astro', import.meta.url), 'utf8')).toMatch(/id="transfer" data-tab-panel[\s\S]*?UNVERIFIED/);
  });
});

import { holderChain } from '../src/scripts/holders';
import { MINTS, TRANSFERS, SCENES } from '../src/data/testnet';
describe('holder chain from the record', () => {
  it('s2 goes from its mint holder to buyer-pub through the recorded sale', () => {
    const c = holderChain('s2', 'writer-b.royltee-dev.testnet', 'buyer-pub.royltee-dev.testnet');
    expect(c.steps.map((s) => s.holder)).toEqual(['writer-b.royltee-dev.testnet', 'buyer-pub.royltee-dev.testnet']);
    expect(c.steps[0].tx).toBe(MINTS.s2.tx);
    expect(c.matches).toBe(true);
  });
  it('flags a live owner that differs from the record instead of hiding it', () => {
    const c = holderChain('s1', 'writer-a.royltee-dev.testnet', 'someone.testnet');
    expect(c.steps).toHaveLength(1);
    expect(c.matches).toBe(false);
  });
  it('every recorded transfer is also a recorded scene, and every share has a mint', () => {
    for (const t of TRANSFERS) expect(SCENES.some((s) => s.txs.includes(t.tx))).toBe(true);
    expect(Object.keys(MINTS)).toEqual(['s1', 's2', 's3', 's4', 's5', 's6', 's7']);
  });
});
