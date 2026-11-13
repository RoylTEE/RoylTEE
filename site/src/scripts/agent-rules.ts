/**
 * Organisation agent controls, pure. Mirrors the require! checks in contracts/registry/src/agents.rs
 * (grant_role, add_line_credits, revoke_role, withdraw_allowance) so the page can say why a call
 * would fail before the wallet opens. The contract stays the authority.
 */
export const MAX_TX_PER_DAY = 1000;
/** LINE_STORAGE_BYTES (100) at the testnet storage price of 1e19 yoctoNEAR per byte: 0.001 NEAR per statement line. */
export const LINE_COST_YOCTO = 10n ** 21n;
/** Attached for role storage on grant_role. The contract credits what storage does not use as more lines, and refunds any fraction. */
export const ROLE_STORAGE_YOCTO = 10n ** 22n;

export type Check = { ok: boolean; text: string };
export type RoleFacts = { org: string; revoked: boolean; allowance: string } | null;

/** Decimal string to integer units, or null when it is not a plain positive amount with at most `decimals` places. */
export function parseUnits(s: string, decimals: number): bigint | null {
  const t = s.trim();
  if (!/^\d+(\.\d+)?$/.test(t)) return null;
  const [i, f = ''] = t.split('.');
  if (f.length > decimals) return null;
  return BigInt(i) * 10n ** BigInt(decimals) + BigInt(f.padEnd(decimals, '0') || '0');
}

export const isDirectSub = (agent: string, org: string) => {
  if (!org || !agent.endsWith(`.${org}`)) return false;
  const label = agent.slice(0, -(org.length + 1));
  return label.length > 0 && !label.includes('.');
};

export type GrantInput = {
  org: string; orgActive: boolean | null; agent: string; agentHasCredential: boolean; agentIsPayer: boolean;
  methods: string[]; perTx: bigint | null; daily: bigint | null; maxTx: number; expiresMs: number; nowMs: number; role: RoleFacts;
};
export function grantChecks(i: GrantInput): Check[] {
  return [
    { ok: i.orgActive === true, text: i.orgActive === null ? 'The organisation wallet is checked once it is connected' : i.orgActive ? 'The organisation is an active verified member' : 'The organisation is not an active verified member' },
    { ok: isDirectSub(i.agent, i.org), text: `The agent is a direct sub-account of the organisation, like pay-agent.${i.org || '<org>'}` },
    { ok: !i.agentHasCredential, text: 'The agent account holds no member credential' },
    { ok: !i.agentIsPayer, text: 'The agent account is not a payer' },
    { ok: i.methods.length > 0, text: 'At least one method: pay_from_allowance or report' },
    { ok: i.perTx !== null && i.daily !== null && i.perTx > 0n && i.perTx <= i.daily, text: 'Per payment limit above 0 and not above the daily limit' },
    { ok: Number.isInteger(i.maxTx) && i.maxTx >= 1 && i.maxTx <= MAX_TX_PER_DAY, text: `Between 1 and ${MAX_TX_PER_DAY} payments a day` },
    { ok: i.expiresMs > i.nowMs, text: 'The role expires in the future' },
    { ok: !i.role || (i.role.org === i.org && i.role.revoked), text: !i.role ? 'No role yet for this agent' : i.role.org !== i.org ? 'The agent belongs to another organisation' : i.role.revoked ? 'The old role is revoked, so it can be granted again' : 'The role is already active; revoke it first' },
  ];
}
export const lineChecks = (caller: string, role: RoleFacts, lines: number): Check[] => [
  { ok: !!role, text: role ? 'The agent has a role' : 'No role for this agent' },
  { ok: !!role && role.org === caller, text: "Signed by the agent's organisation" },
  { ok: Number.isInteger(lines) && lines >= 1, text: 'At least one line credit (0.001 NEAR each)' },
];
export const revokeChecks = (caller: string, registrar: string, role: RoleFacts): Check[] => [
  { ok: !!role, text: role ? 'The agent has a role' : 'No role for this agent' },
  { ok: !!role && (role.org === caller || caller === registrar), text: 'Signed by the organisation (or the registrar)' },
  { ok: !!role && !role.revoked, text: role?.revoked ? 'Already revoked' : 'The role is active' },
];
export const withdrawChecks = (caller: string, callerSuspended: boolean, role: RoleFacts, amount: bigint | null): Check[] => [
  { ok: !!role, text: role ? 'The agent has a role' : 'No role for this agent' },
  { ok: !!role && role.org === caller, text: "Signed by the agent's organisation" },
  { ok: !callerSuspended, text: callerSuspended ? 'The organisation credential is suspended' : 'The organisation credential is not suspended' },
  { ok: !!role && amount !== null && amount > 0n && amount <= BigInt(role.allowance), text: 'Amount above 0 and not above the allowance left' },
];
export const allOk = (c: Check[]) => c.every((x) => x.ok);
/** Deposit for grant_role: role storage plus the prepaid lines. */
export const grantDeposit = (lines: number) => ROLE_STORAGE_YOCTO + BigInt(Math.max(0, lines)) * LINE_COST_YOCTO;
