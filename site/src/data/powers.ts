/**
 * Every power a person or contract has on the registry, with where it lives in this UI.
 * `ui: null` means the method works on chain but this demo has no screen for it: the page shows
 * "CLI only in this demo" next to it. Method names checked against the deployed contract code on 2026-11-13.
 */
export type Power = { method: string; who: 'holder' | 'organisation' | 'registrar' | 'payer' | 'anyone'; what: string; ui: string | null; note?: string };

export const CLI_ONLY = 'CLI only in this demo';

export const POWERS: Power[] = [
  { method: 'claim', who: 'holder', what: 'Pay the share account balance to the current holder', ui: '/member#holdings' },
  { method: 'nft_transfer', who: 'holder', what: 'Move the share to another verified member; unclaimed money goes with it', ui: '/member#transfer' },
  { method: 'register_share', who: 'holder', what: 'Add shares to a work from a reviewed split sheet', ui: '/member#draft', note: 'UNVERIFIED' },
  { method: 'approve_share, reject_share', who: 'holder', what: 'The work registrant signs or rejects a share someone else registered', ui: '/member#approvals', note: 'On the deployed build; not sent from this site on testnet yet' },
  { method: 'set_share_note', who: 'holder', what: 'Set a short public note. Payouts and agents never read it', ui: null },
  { method: 'confirm_share', who: 'holder', what: 'A co-holder confirms a share, raising its evidence tier', ui: null },
  { method: 'sync', who: 'holder', what: 'Pull a plain deposit into the share ledger', ui: null },
  { method: 'register_work', who: 'holder', what: 'Register a work by ISRC or ISWC', ui: null },
  { method: 'nft_transfer_call', who: 'holder', what: 'Send the share into a contract (developer path)', ui: null },
  { method: 'grant_role', who: 'organisation', what: 'Give a payment agent a role with limits', ui: '/agents#controls', note: 'UNVERIFIED' },
  { method: 'add_line_credits', who: 'organisation', what: 'Add statement line credits to an agent', ui: '/agents#controls', note: 'UNVERIFIED' },
  { method: 'revoke_role', who: 'organisation', what: 'Stop an agent at its next call (kill switch)', ui: '/agents#controls', note: 'UNVERIFIED' },
  { method: 'withdraw_allowance', who: 'organisation', what: 'Take back unused agent allowance', ui: '/agents#controls', note: 'UNVERIFIED' },
  { method: 'ft_transfer_call with fund_allowance', who: 'organisation', what: 'Fund an agent allowance in Mock USDC', ui: null },
  { method: 'ft_transfer_call with pay_work or pay_share', who: 'payer', what: 'Pay a work or one share once, split by basis points', ui: null },
  { method: 'attest_share', who: 'payer', what: 'A payer confirms a share, raising its evidence tier', ui: null },
  { method: 'grant_credential, suspend_credential, reinstate_credential', who: 'registrar', what: 'Admit, pause and restore verified members', ui: null },
  { method: 'grant_payer_role, revoke_payer_role', who: 'registrar', what: 'Name which accounts may pay works', ui: null },
  { method: 'sync', who: 'registrar', what: 'Pull a plain deposit into the share ledger for any share (no document hash)', ui: null },
  { method: 'revoke_role', who: 'registrar', what: 'Stop any agent at its next call (no document hash)', ui: null },
  { method: 'resolve', who: 'registrar', what: 'Decide a dispute with a document hash', ui: null },
  { method: 'release_unallocated', who: 'registrar', what: 'Release money that reached the registry for no share, within fixed rules', ui: null },
  { method: 'registrar_transfer', who: 'registrar', what: 'Move the share of a suspended member, with a document hash', ui: null },
  { method: 'resolve_written_off, resolve_stale_payout', who: 'registrar', what: 'Settle a quarantined payment forward (a delivered and a failed amount) or a stale payout lock from the final public receipt outcome, with a document hash', ui: null },
  { method: 'release_stale_lock, release_stale_forward, retry_forward', who: 'anyone', what: 'Release a stale transfer lock, ask for a vault balance report on a stale payout lock (it changes nothing), or quarantine a forward that did not settle (it credits and resends nothing)', ui: null },
];
export const powersFor = (who: Power['who']) => POWERS.filter((p) => p.who === who);
