/**
 * Transfer pre-checks, mirroring check_transfer in the registry contract (nft.rs).
 * Pure: no DOM and no network, so the rules are tested directly. The contract stays the authority.
 */
export type TransferFacts = {
  sender: string; owner: string; receiver: string; status: string; dispute: unknown;
  approval: string; payoutInFlight: unknown; transferPending: unknown;
  senderCredential: string | null; receiverActive: boolean;
};
export type Check = { ok: boolean; text: string };
/** A dispute id can be 0, so only null or undefined mean no dispute (second reviewer 03d item 3). */
export const hasDispute = (d: unknown) => d !== null && d !== undefined;

export function transferChecks(f: TransferFacts): Check[] {
  return [
    { ok: !!f.sender && f.sender === f.owner, text: f.sender ? (f.sender === f.owner ? 'The connected wallet holds this share' : 'Only the current holder can transfer. Connect the holder wallet') : 'Connect the holder wallet to sign' },
    { ok: !!f.receiver && f.receiver !== f.owner, text: !f.receiver ? 'Enter a receiver account' : f.receiver === f.owner ? 'The receiver already holds this share' : 'The receiver is a different account' },
    { ok: f.receiverActive, text: f.receiverActive ? 'The receiver is an active verified member' : 'The receiver has no active member credential, so the contract refuses' },
    { ok: f.status === 'active', text: f.status === 'active' ? 'The share is active' : `The share is ${f.status} and cannot move` },
    { ok: !hasDispute(f.dispute), text: hasDispute(f.dispute) ? 'The share is frozen by a dispute' : 'No dispute on this share' },
    { ok: f.approval !== 'pending' && f.approval !== 'rejected', text: f.approval === 'pending' ? 'The work registrant has not approved this share yet' : f.approval === 'rejected' ? 'The work registrant rejected this share' : f.approval === 'not deployed' ? 'Split sheet approval is not on the deployed build, so it does not block' : 'Approved on the split sheet' },
    { ok: !f.payoutInFlight, text: f.payoutInFlight ? 'A payout is in flight. Transfer after it settles' : 'No payout in flight' },
    { ok: !f.transferPending, text: f.transferPending ? 'Another transfer is pending' : 'No other transfer pending' },
    { ok: f.senderCredential !== 'suspended', text: f.senderCredential === 'suspended' ? 'Your credential is suspended, so you cannot send' : f.sender ? 'Your credential is not suspended' : 'Your credential is checked once a wallet is connected' },
  ];
}

export const canTransfer = (checks: Check[]) => checks.every((c) => c.ok);

/**
 * Claim pre-checks, mirroring claim in the registry contract (money.rs): holder only, credential not
 * suspended, no transfer pending, no payout in flight, a balance above zero. Share status is not a
 * condition, so a void share keeps its claim (second reviewer 03d item 9).
 */
export type ClaimFacts = { signer: string | null; owner: string; claimable: bigint; payoutInFlight: unknown; transferPending: unknown; suspended: boolean };
export function canClaim(f: ClaimFacts): { ok: boolean; why: string } {
  if (!f.signer) return { ok: false, why: 'Connect this account to claim' };
  if (f.signer !== f.owner) return { ok: false, why: 'Only the current holder can claim' };
  if (f.claimable <= 0n) return { ok: false, why: 'Nothing to claim' };
  if (f.suspended) return { ok: false, why: 'The holder credential is suspended' };
  if (f.transferPending) return { ok: false, why: 'A transfer is pending' };
  if (f.payoutInFlight) return { ok: false, why: 'A payout is already in flight' };
  return { ok: true, why: '' };
}
