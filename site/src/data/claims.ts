/**
 * Every headline claim, mapped to the code that implements it, the test that checks it and the
 * on-chain evidence. The /verify page and the README table "Every claim, checked" read the same
 * facts. Transaction hashes must exist in deployments/testnet.json (tests/claims.test.ts checks
 * this); their outcome text comes from src/data/evidence.json, a build-time copy of that file.
 * Tests named here must exist in the contracts or in site/tests (the same test greps for them).
 */
import evidence from './evidence.json';

export type CodeRef = { file: string; fn: string };
export type Ev = { tx: string; what: string } | { account: string; what: string };
export type LiveCheckId = 'vault-keys' | 'split' | 'owner-s2' | 'agent-key' | 'agent-limits' | 'dispute' | 'credential' | 'usdc';
/** live: a read-only check the page runs now. recorded: on-chain evidence is shown with its transactions, the page runs no live check for it. test: no on-chain artifact exists, the test is the proof. pending: waits for a chain write. */
export type Status = 'live' | 'recorded' | 'test' | 'pending';
export type Claim = { id: string; claim: string; code: CodeRef[]; tests: string[]; evidence: Ev[]; check?: LiveCheckId; status: Status; note?: string };

const REG = 'contracts/registry/src';
export const CLAIMS: Claim[] = [
  {
    id: 'vault-zero-keys', status: 'live', check: 'vault-keys',
    claim: 'Each share vault is a NEAR account with zero access keys. Only the registry can move money out of it.',
    code: [{ file: `${REG}/registration.rs`, fn: 'register_share' }, { file: 'contracts/vault/src/lib.rs', fn: 'withdraw' }],
    tests: ['t01_mint_creates_keyless_registered_vault', 't21_parent_cannot_delete_redeploy_or_key_a_vault'],
    evidence: [
      { tx: '7wxZ7Bmstzu9qTUayrvzbGP97tcD6GRLMaZo7pTVAqzh', what: 'mint of share s1, vault created with no key' },
      { tx: 'FGrwBa58Empz2zW1Yj2BTHPvaSw9ZjsrVrDT6PrbN4gf', what: 'mint of share s8 on the current registry build' },
      { account: 's2.reg.royltee-dev.testnet', what: 'view_access_key_list returns an empty list' },
    ],
  },
  {
    id: 'auto-split', status: 'live', check: 'split',
    claim: 'One payment to the registry is split by basis points to the share vaults, exact to the last unit.',
    code: [{ file: `${REG}/money.rs`, fn: 'ft_on_transfer' }, { file: `${REG}/logic.rs`, fn: 'split' }],
    tests: ['t13_allowance_work_payment_splits_exactly', 't16b_split_sums_to_payment_exactly', 'awkward_amounts_always_sum_exactly'],
    evidence: [
      { tx: 'HBzKi5ptr5sGvPSgwWQSUNMKBRJksNCp3tAeT4xByo8u', what: 'work 0 paid 1.0 mUSDC, 0.5 into each of s1 and s2' },
      { tx: '2aAgWtcaEC1RinbQvVS1AoLpZn7zQeEmzAGW3FshXHQA', what: 'work 0 paid again after the sale of s2' },
    ],
  },
  {
    id: 'transfer-passes-control', status: 'live', check: 'owner-s2',
    claim: 'Selling the NFT moves control of its vault and the unclaimed money. The seller keeps nothing.',
    code: [{ file: `${REG}/nft.rs`, fn: 'nft_transfer' }, { file: `${REG}/money.rs`, fn: 'claim' }],
    tests: ['t03_owner_claims_non_owner_fails', 't04_catalog_sale_balance_travels_with_token', 't06_transfer_during_inflight_payout_fails', 't07_transfer_call_bounce_cannot_claim'],
    evidence: [
      { tx: 'HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy', what: 'writer-b sells s2 to buyer-pub' },
      { tx: 'M4pWDbEgnUfz4TiJA8ZZdNVCnwV9vzsBnnEqRgALDrp', what: 'buyer-pub claims the 0.5 paid before the sale' },
      { tx: 'EYDV3Tsv7Q1ZFPnJzmVdk6D4Rtosqpn8fYNY71ofVWJU', what: 'the seller tries to claim and is refused' },
    ],
  },
  {
    id: 'holder-vs-agent', status: 'live', check: 'agent-key',
    claim: 'Holder powers and agent powers are separate. An agent key holds two methods and cannot claim, transfer or grant.',
    code: [{ file: `${REG}/money.rs`, fn: 'claim' }, { file: `${REG}/agents.rs`, fn: 'pay_from_allowance' }, { file: `${REG}/agents.rs`, fn: 'report' }],
    tests: ['t08_agent_key_cannot_reach_owner_methods', 'agent_cannot_use_owner_methods', 'note_is_holder_only_and_never_changes_payouts'],
    evidence: [
      { tx: 'GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB', what: 'outsider tries to claim s1 and is refused' },
      { tx: 'CmRvd9ZNcfsR5EnMsc3VJ6YYncE3mQgLWqF21YhS1w5G', what: 'agent report signed with its function call key' },
      { tx: '7Bwi9rmQk4R4iApdVTXMWLkGjMgDnh2qZiVEfGPY5NAH', what: 'the agent full access key is deleted' },
      { account: 'pay-agent.buyer-pub.royltee-dev.testnet', what: 'one function call key, scoped to the registry' },
    ],
  },
  {
    id: 'agent-limits', status: 'live', check: 'agent-limits',
    claim: 'An agent is held to a per-transaction limit, a daily limit and one payment per statement line, and a revoked role stops at the next call.',
    code: [{ file: `${REG}/agents.rs`, fn: 'pay_from_allowance' }, { file: `${REG}/agents.rs`, fn: 'revoke_role' }],
    tests: ['t09_payment_over_limits_fails', 't10_revoked_agent_next_call_fails', 't12_same_statement_line_pays_once', 't11_injected_payee_changes_nothing'],
    evidence: [
      { tx: '7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW', what: 'one unit over the per-transaction limit is refused' },
      { tx: '2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3', what: 'a second payment for the same statement line is refused' },
      { tx: 'CUW8qssJVCJZuUgzzWHf9bdEyNCkFXpQjUKrJeWXeVPS', what: 'extra payee arguments naming outsider are ignored' },
    ],
    note: 'Revocation has no transaction on testnet. The sandbox test is its only proof.',
  },
  {
    id: 'dispute-freeze-escrow', status: 'live', check: 'dispute',
    claim: 'A claim that pushes a right over 100 percent opens a dispute. Only the contested share is frozen, the new money waits in escrow, and the registrar resolves it.',
    code: [{ file: `${REG}/registration.rs`, fn: 'register_share' }, { file: `${REG}/disputes.rs`, fn: 'resolve' }],
    tests: ['t14_over_allocation_dispute_freezes_only_contested_fraction', 't14b_uphold_existing_voids_claimant', 'h2_one_bps_claim_freezes_only_one_named_share'],
    evidence: [
      { tx: '3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba', what: 'a 20 percent claim on work 2 opens dispute 0' },
      { tx: 'C8H28Nx1MZ5Pjb13uzkvheAh7S6Jf5mdqqHqmM1n7Mu', what: 'work 2 paid: s6 gets 0.3, the 0.7 for s5 goes to escrow' },
      { tx: '7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi', what: 'registrar resolves: claimant void, 0.7 released to s5' },
    ],
  },
  {
    id: 'credential-gating', status: 'live', check: 'credential',
    claim: 'Only members with an active credential can receive a share. The registrar grants credentials and cannot hold one.',
    code: [{ file: `${REG}/membership.rs`, fn: 'grant_credential' }, { file: `${REG}/nft.rs`, fn: 'nft_transfer' }],
    tests: ['t05_transfer_to_outsider_fails', 'suspended_holder_accrues_but_cannot_claim_send_or_receive', 'h3_registrar_cannot_hold_a_credential'],
    evidence: [
      { tx: 'Bv7cDUgGrGxgqaXr5FJnGpauqw1DcLpJmUuTMnrydFa6', what: 'registrar grants a credential' },
      { tx: '8QffMekUyzGzF6UvPX256tUBBBcXsjhy5M6GTHibUjkB', what: 'registrar grants a credential' },
      { tx: '3dqBCz6QkbdbDqkZFSqvmfzHjuZHRkVt45A5kbAvg6Vq', what: 'registrar grants a credential' },
      { account: 'outsider.royltee-dev.testnet', what: 'is_active_member returns false' },
    ],
    note: 'A refused transfer to a non-member has no transaction on testnet. The sandbox test is its proof.',
  },
  {
    id: 'mock-usdc-labeled', status: 'live', check: 'usdc',
    claim: 'The payment token is labeled Mock USDC (testnet), symbol mUSDC, 6 decimals, with no value. It is not Circle USDC.',
    code: [{ file: 'contracts/mock-usdc/src/lib.rs', fn: 'ft_metadata' }],
    tests: ['metadata_is_labeled_mock_with_six_decimals'],
    evidence: [
      { account: 'usdc.royltee-dev.testnet', what: 'ft_metadata returns the name and symbol' },
      { tx: 'D1kHgerP4xVj7jmCp8JoGUSvHZE8grp19U7XcBHCh8Hs', what: 'the only mint: 1000 mUSDC to the Simulated payer' },
    ],
  },
  {
    id: 'registry-redeploy', status: 'recorded',
    claim: 'The registry was redeployed in place on 2026-11-14 (09:09 UTC) with the final contract fixes. The deploy and a quiescence check ran in one transaction, so a busy registry would have rolled the deploy back. The code hash on chain is 5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG, and every share was read back afterwards (npm run verify:testnet, 2026-11-14 UTC: ALL CHECKS PASSED, 72 checks).',
    code: [{ file: `${REG}/lib.rs`, fn: 'get_vault_code_hash' }],
    tests: ['a03_stranger_cannot_initialise_a_fresh_deploy', 'f01_stale_payout_lock_stays_until_the_registrar_settles_it', 'f04_a_written_off_forward_is_quarantined_then_resolved_and_resent_once'],
    evidence: [
      { tx: '52jp95BVJCk6a9bUGyYtYEQq99R56Xiz5zxJ1vCHBJrD', what: 'deploy of the final registry build and the quiescence check, one transaction (block 277636549)' },
      { account: 'reg.royltee-dev.testnet', what: 'code hash read from chain, equal to the deploy record' },
    ],
    note: 'The source of that build is contracts/registry; its sandbox tests of the final fixes are in contracts/tests/tests/final.rs. The registry keeps its full access key on testnet by decision, so upgrades stay possible; on mainnet it would be deleted or handed to a multisig or DAO.',
  },
  {
    id: 'split-sheet', status: 'recorded',
    claim: 'A share that someone other than the work registrant registers waits for the registrant to sign, and its money waits in escrow. On testnet (work 6, Simulated money): share s14, registered by the registrant, was approved at once. Share s15, registered by the other writer, was pending, so its half of one mUSDC stayed in escrow with reason awaiting_approval and its vault held 0. After the registrant signed approve_share, the event share_approved released the money to the s15 vault.',
    code: [{ file: `${REG}/approval.rs`, fn: 'approve_share' }],
    tests: ['m2_share_by_non_registrant_is_held_until_approved', 'm2_registrant_shares_and_old_shares_are_approved'],
    evidence: [
      { tx: 'EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy', what: 'the registrant registers work 6' },
      { tx: '9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt', what: 'the registrant registers its own 50 percent share s14, approved at once' },
      { tx: '5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB', what: 'the other writer registers the other 50 percent, share s15, pending' },
      { tx: '8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL', what: 'one mUSDC paid to work 6: s14 got 0.5 at once, the s15 half was held in escrow' },
      { tx: '5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa', what: 'the registrant signs approve_share on s15, the held 0.5 goes to its vault' },
      { account: 's15.reg.royltee-dev.testnet', what: 'the vault of the approved share, with no access keys' },
    ],
  },
  {
    id: 'tee-auction', status: 'recorded',
    claim: 'A sealed-bid auction of a share ran on testnet. Bids were encrypted, a module in a TEE opened them through the real OutLayer, and the contract moved the share and the money. On 2026-11-14 (10:39 UTC) share s10 sold to buyer-pub at the second price, 70 mUSDC (Simulated). outlayer-verify passed all 5 checks on the settle transaction: Authenticity (Intel TDX), Identity, Binding, Input and Output.',
    code: [{ file: 'contracts/auction/src/settle.rs', fn: 'settle' }],
    tests: ['a02_full_sale_with_the_real_module', 'settle_needs_time_gas_and_deposit', 'a07_bad_answers_never_move_money_or_the_share'],
    evidence: [
      { tx: 'EWWMu3chsa56hBybTpZ5hgiM6bazXRBekESv7KzpWYEt', what: 'deploy and init of the auction contract, code sha256 48d32c71de83d94a4225502a726b80bd678ac3dfb1471952c93c4c16595a8d09' },
      { tx: 'Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR', what: 'the seller lists share s10 (auction 0)' },
      { tx: 'GHCfH4PhhcNCG4mmDCZXKmy9WvDucEnhCmerYVSjNm1v', what: 'sealed bid 1, ciphertext only on chain' },
      { tx: 'AK88NkPkXsik1xPbTNDNZB6VCCRi2rKhxeVbap9bHkSc', what: 'sealed bid 2, ciphertext only on chain' },
      { tx: 'CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW', what: 'settle through the real OutLayer TEE (request 44422): sold to buyer-pub at 70 mUSDC, s10 moved' },
      { tx: '2PMK4vf1dKsPNhZwtNkWL2xninc7ZNRN515HiwndQdVY', what: 'withdraw: seller paid, losing and winning deposits returned, owed total 0' },
    ],
    note: 'The outlayer-verify output is deployments/auction-settle-verify.txt (verifier git 90c27d9). That check proves this input produced this output in genuine TEE hardware running the approved build; it does not prove the auction rules were followed, which stay with the executor. The auction code is in contracts/auction and tee/auction-wasi; its tests are 68 contract unit tests, 14 module tests, 18 Node tests and 15 sandbox tests (the sandbox runs the real module against a mock OutLayer, Simulated). The auction account keeps its full access key on testnet by decision.',
  },
];

export type Attack = { id: string; attack: string; result: string; tests: string[]; tx?: string; more?: string[]; status: 'refused' | 'contained' | 'test' | 'pending'; note?: string };
/** refused: the contract refused it on chain (transaction shown). contained: the call went through and the contract limited its effect. test: sandbox or unit test only. pending: waits for a chain write. */
export const ATTACKS: Attack[] = [
  { id: 'seller-claims', attack: 'The seller of s2 claims it after the sale.', result: 'Refused: only the current holder can claim.', tests: ['t03_owner_claims_non_owner_fails'], tx: 'EYDV3Tsv7Q1ZFPnJzmVdk6D4Rtosqpn8fYNY71ofVWJU', status: 'refused' },
  { id: 'outsider-claims', attack: 'An account with no credential claims s1.', result: 'Refused: only the current holder can claim.', tests: ['t03_owner_claims_non_owner_fails'], tx: 'GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB', status: 'refused' },
  { id: 'agent-over-limit', attack: 'The agent pays one unit over its per-transaction limit.', result: 'Refused: over per-transaction limit.', tests: ['t09_payment_over_limits_fails'], tx: '7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW', status: 'refused' },
  { id: 'agent-replay', attack: 'The agent sends the same statement line twice.', result: 'Refused: statement line already paid.', tests: ['t12_same_statement_line_pays_once'], tx: '2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3', status: 'refused' },
  { id: 'agent-injection', attack: 'Work metadata and extra arguments tell the agent to pay outsider.', result: 'Ignored: the contract resolves payees from the registry. s12 and s13 got 0.05 each, outsider got 0.', tests: ['t11_injected_payee_changes_nothing'], tx: 'CUW8qssJVCJZuUgzzWHf9bdEyNCkFXpQjUKrJeWXeVPS', status: 'contained' },
  { id: 'agent-owner-method', attack: 'The agent key tries claim with 1 yoctoNEAR attached.', result: 'Refused by the protocol before any transaction: a function call key cannot attach a deposit. The contract also checks the holder.', tests: ['t08_agent_key_cannot_reach_owner_methods'], status: 'test', note: 'No transaction exists. The CLI error text is in deployments/testnet.json, step 12.' },
  { id: 'overallocation', attack: 'A member registers a 20 percent share on a work that is already full.', result: 'A dispute opens. Only the named share is frozen, its new money goes to escrow, the rest keeps being paid.', tests: ['t14_over_allocation_dispute_freezes_only_contested_fraction'], tx: '3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba', status: 'contained' },
  { id: 'transfer-outsider', attack: 'A holder transfers a share to an account with no credential.', result: 'Refused: receiver has no active credential.', tests: ['t05_transfer_to_outsider_fails'], status: 'test' },
  { id: 'claim-then-sell', attack: 'A holder starts a claim and sells the share while the payout is in flight.', result: 'Refused: payout in flight, transfer after it settles.', tests: ['t06_transfer_during_inflight_payout_fails'], status: 'test' },
  { id: 'bounce', attack: 'A receiver of nft_transfer_call claims during the call, then returns the token.', result: 'Refused: claims wait until the transfer resolves.', tests: ['t07_transfer_call_bounce_cannot_claim'], status: 'test' },
  { id: 'marketplace-approval', attack: 'A marketplace approval is used to move or claim a share.', result: 'Refused: approvals are disabled.', tests: ['t16_approvals_are_disabled'], status: 'test' },
  { id: 'revoked-agent', attack: 'A revoked agent calls again.', result: 'Refused at the next call.', tests: ['t10_revoked_agent_next_call_fails'], status: 'test' },
  { id: 'outsider-sync', attack: 'A stranger calls sync on a share.', result: 'Refused on the current build: holder or registrar only.', tests: ['r01_h1_outsider_cannot_sync_and_new_vaults_keep_transfer_call'], status: 'test', note: 'On the first build this call succeeded (transaction 2Jsht1EatGqZfeVrZePKyVMQPyqyeqTCpEcoaHWK44LY). The fix is on testnet since the step 15 redeploy and has no refusal transaction.' },
  { id: 'vault-by-parent', attack: 'The registry tries to delete, redeploy or add a key to an existing vault.', result: 'Impossible: the registry has no such call, and the sandbox shows the vault stays keyless.', tests: ['t21_parent_cannot_delete_redeploy_or_key_a_vault'], status: 'test' },
  { id: 're-init', attack: 'A stranger initialises a freshly deployed registry.', result: 'Refused: only the registry account itself can initialise.', tests: ['a03_stranger_cannot_initialise_a_fresh_deploy', 'only_the_registry_account_itself_can_initialise'], status: 'test' },
  { id: 'auction-outsider', attack: 'An outsider asks the TEE to open live auction bids directly, not through the auction contract.', result: 'Refused by the OutLayer keystore: Access denied by access condition. No instructions ran and the answer was null, so no bid was opened. Tried twice: once naming the auction as the contract, once naming the outsider itself. The transactions succeed because they only queue the request; the refusal is in the answer.', tests: ['a13_a_direct_request_from_another_caller_is_refused'], tx: 'EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM', more: ['57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP'], status: 'contained', note: 'Run on testnet on 2026-11-14 before any share was listed (OutLayer requests 44242 and 44261). The caller check inside the module is covered by a sandbox test, because the keystore refused first.' },
  { id: 'split-sheet-early-money', attack: 'A writer registers a share on a work they did not register and tries to receive money before the registrant signs.', result: 'Held: the share is pending, so its half of the payment stayed in escrow with reason awaiting_approval and the vault held 0, until the registrant signed.', tests: ['m2_share_by_non_registrant_is_held_until_approved'], tx: '8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL', status: 'contained' },
];

/** Outcome text for a transaction from the build-time copy of deployments/testnet.json. */
export type TxFact = { step: string; method: string; success: boolean; error?: string | null; at: string | null };
export const TXS = evidence.txs as Record<string, TxFact>;
export const TOKEN_OWNERS = evidence.tokenOwners as Record<string, string>;
export const AGENT_KEYS = evidence.agentKeys as { permission: string; contract: string | null; methods: string[] }[];
export const txOutcome = (hash: string): string => {
  const t = TXS[hash];
  if (!t) return 'not in the record';
  return t.success ? `${t.method}, succeeded` : `${t.method}, refused${t.error ? `: ${t.error}` : ''}`;
};
