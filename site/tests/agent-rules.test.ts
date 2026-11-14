import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { grantChecks, lineChecks, revokeChecks, withdrawChecks, allOk, parseUnits, isDirectSub, grantDeposit, type GrantInput } from '../src/scripts/agent-rules';

const org = 'buyer-pub.royltee-dev.testnet';
const base: GrantInput = { org, orgActive: true, agent: `pay-agent.${org}`, agentHasCredential: false, agentIsPayer: false, methods: ['pay_from_allowance'], perTx: 1_000_000n, daily: 5_000_000n, maxTx: 10, expiresMs: 2e12, nowMs: 1e12, role: null };
const blocks = (c: { ok: boolean; text: string }[]) => c.filter((x) => !x.ok).map((x) => x.text);

describe('organisation agent controls mirror agents.rs', () => {
  it('a clean grant passes', () => expect(allOk(grantChecks(base))).toBe(true));
  it('blocks each grant_role require', () => {
    expect(blocks(grantChecks({ ...base, orgActive: false }))[0]).toMatch(/not an active verified member/);
    expect(blocks(grantChecks({ ...base, agent: `a.b.${org}` }))[0]).toMatch(/direct sub-account/);
    expect(blocks(grantChecks({ ...base, agentHasCredential: true }))[0]).toMatch(/no member credential/);
    expect(blocks(grantChecks({ ...base, agentIsPayer: true }))[0]).toMatch(/not a payer/);
    expect(blocks(grantChecks({ ...base, methods: [] }))[0]).toMatch(/At least one method/);
    expect(blocks(grantChecks({ ...base, perTx: 6_000_000n }))[0]).toMatch(/not above the daily limit/);
    expect(blocks(grantChecks({ ...base, maxTx: 1001 }))[0]).toMatch(/1000 payments/);
    expect(blocks(grantChecks({ ...base, expiresMs: 1 }))[0]).toMatch(/future/);
    expect(blocks(grantChecks({ ...base, role: { org, revoked: false, allowance: '0' } }))[0]).toMatch(/revoke it first/);
    expect(blocks(grantChecks({ ...base, role: { org: 'other.testnet', revoked: true, allowance: '0' } }))[0]).toMatch(/another organisation/);
    expect(allOk(grantChecks({ ...base, role: { org, revoked: true, allowance: '0' } }))).toBe(true);
  });
  it('line credits, revoke and withdraw follow their callers and bounds', () => {
    const role = { org, revoked: false, allowance: '400000' };
    expect(allOk(lineChecks(org, role, 3))).toBe(true);
    expect(allOk(lineChecks('x.testnet', role, 3))).toBe(false);
    expect(allOk(revokeChecks('registrar.royltee-dev.testnet', 'registrar.royltee-dev.testnet', role))).toBe(true);
    expect(allOk(revokeChecks(org, 'r', { ...role, revoked: true }))).toBe(false);
    expect(allOk(withdrawChecks(org, false, role, 400000n))).toBe(true);
    expect(allOk(withdrawChecks(org, false, role, 400001n))).toBe(false);
    expect(allOk(withdrawChecks(org, true, role, 1n))).toBe(false);
  });
  it('parses amounts and computes deposits exactly', () => {
    expect(parseUnits('0.4', 6)).toBe(400000n);
    expect(parseUnits('1.0000001', 6)).toBeNull();
    expect(parseUnits('-1', 6)).toBeNull();
    expect(isDirectSub(`pay-agent.${org}`, org)).toBe(true);
    expect(grantDeposit(10)).toBe(2n * 10n ** 22n);
  });
  it('the controls say they need the org wallet, stay on the wallet page, and /agents stays strict', () => {
    const agents = readFileSync(new URL('../src/pages/agents.astro', import.meta.url), 'utf8');
    const member = readFileSync(new URL('../src/pages/member.astro', import.meta.url), 'utf8');
    const csp = readFileSync(new URL('../src/server/csp.ts', import.meta.url), 'utf8');
    expect(agents).toMatch(/id="controls"/); expect(agents).toMatch(/Needs the org wallet/); expect(agents).not.toMatch(/prerender = false/);
    expect(member).toMatch(/id="org" data-tab-panel/); expect(member).toMatch(/Needs the org wallet/);
    expect(csp).toMatch(/WALLET_ROUTES = \['\/admin', '\/help', '\/member'\]/);
  });
});

import { REGISTRAR_EVENTS, REGISTRAR_EVENT_KINDS, SCENES } from '../src/data/testnet';
describe('registrar log', () => {
  it('every recorded registrar event has a doc hash, a transaction and a known kind', () => {
    const kinds = REGISTRAR_EVENT_KINDS.map(([k]) => k);
    for (const e of REGISTRAR_EVENTS) { expect(e.docHash).toMatch(/^[0-9a-f]{64}$/); expect(e.tx).toMatch(/^[1-9A-HJ-NP-Za-km-z]{43,44}$/); expect(kinds).toContain(e.kind); }
    const resolve = SCENES.find((s) => s.step === 'Resolve')!;
    expect(REGISTRAR_EVENTS.find((e) => e.kind === 'dispute_resolved')!.tx).toBe(resolve.txs[0]);
  });
  it('labels placeholder hashes, lists kinds with no events, and has a Members check', () => {
    const page = readFileSync(new URL('../src/pages/registrar.astro', import.meta.url), 'utf8');
    expect(page).toMatch(/placeholder test values, not hashes of real documents/);
    expect(page).toMatch(/Not in the record/);
    const ui = readFileSync(new URL('../src/scripts/registrar.ts', import.meta.url), 'utf8');
    expect(ui).toMatch(/'get_credential'/); expect(ui).toMatch(/'is_active_member'/);
  });
});
