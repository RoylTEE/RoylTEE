# Audit

Manual audit pass, 2026-11-13, by the builder, over the code at the commit that adds this file. It is not an independent review. The bundled `near-contract-audit` skill named in the brief is not available in this environment (DECISIONS_LOG row 60), so this pass follows the checklist the owner set: access control, deposits, callbacks and locks, reentrancy across promises, integer overflow and rounding, storage staking, gas on loops, events, the solvency invariant, the zero full-access-keys plan, and every THREAT_MODEL row mapped to a test or marked as a gap. After this pass, review passes 03a to 03d were run by reviewers other than the builder, from review packs (contracts, site and server, README); each finding and its state are in [docs/reviews/03a-status.md](docs/reviews/03a-status.md) to [03d-status.md](docs/reviews/03d-status.md). They were read-only reviews of the code and docs, not a formal audit. A formal audit is still recommended before any mainnet use (none is planned: testnet only).

Scope: `contracts/registry`, `contracts/vault`, `contracts/mock-usdc`. `contracts/test-receiver` is a test helper and is never deployed.

## Findings by severity

| ID | Severity | Finding | Status |
|---|---|---|---|
| M1 | Medium | Init front-running. `new` on the registry and on mock USDC was an open `#[init]`. If deploy and init land in separate transactions, anyone could initialise a fresh deploy with their own registrar (or minter) and own the contract's admin role. | Fixed: both `new` methods now require `predecessor == current_account_id`. Tests: unit `only_the_registry_account_itself_can_initialise`, mock USDC `stranger_cannot_initialise`, sandbox `a03_stranger_cannot_initialise_a_fresh_deploy`. DEPLOY_PLAN.md also deploys and inits in one transaction. |
| M2 | Medium (accepted) | Single registrar key, fixed at init, no rotation. Loss of the key freezes resolve, suspension and payer-role management; compromise allows every registrar power (T12, T35, T38), though never a direct payout to itself. | Accepted for the demo; multisig with time delay is roadmap. |
| L1 | Low | A group's token list kept Failed and Void entries without a cap, so loops in payments, voiding and tier views could grow past `MAX_SHARES_PER_GROUP`. Not cheaply exploitable (failed mints and voids are not attacker controlled), but unbounded. | Fixed: `MAX_GROUP_ENTRIES = 32` on the whole list. Tests: unit `failed_shares_leave_the_group_list` (tests.rs: `MAX_GROUP_ENTRIES` failed mints leave the list empty and a later share still mints) and `h2_repeat_claims_do_not_close_the_group` (review_fixes.rs: `MAX_GROUP_ENTRIES + 5` voided claims keep the list at 2). |
| L2 | Low (accepted) | Funds with no release path by design: stray plain transfers to the registry (decision 10), a suspended organisation's allowance until reinstatement (row 51), and unallocated escrow of a voided work's groups (row 59). All counted in the solvency sum. | Accepted; documented. |
| L3 | Low (accepted) | A credential is active from `grant_credential` until `on_member_registered` runs; if the token registration then fails, the credential is removed although the account might have received a share in that window. Registration fails only on a bad deposit, which the registry controls. | Accepted. |
| L4 | Low (accepted) | `STALE_LOCK_BLOCKS = 1200` is a choice (about 12 minutes at the 0.59 to 0.60 s block time measured on testnet on 2026-11-14; the protocol has no receipt expiry); whether it is long enough under testnet congestion is UNVERIFIED. Too short would let a lock be cleared while its callback is still pending; T36 makes such a late callback harmless. | Open, UNVERIFIED. |
| L5 | Low (accepted) | Only allowance payments are idempotent per statement line (T9 scope). A direct payer can pay the same `statement_line_id` twice. | Accepted; payers deduplicate off chain. |
| L6 | Low (accepted) | Freed storage is not refunded (note shortened, confirmations pruned, credential removed after a failed registration); a token-side refund of a `registration_only` deposit for an already registered account goes to the registry. Small NEAR amounts stay with the registry. | Accepted. |
| L7 | Low (accepted) | Held deposits (T24) are conservative: unsynced plain deposits made before a dispute opened are held until resolve as well. | Accepted, documented (row 54). |
| I1 | Info | The vault emits no events. Withdrawals are visible as the token's `ft_transfer` events and the registry's `claim_started` and `claim_settled`. | By design (tiny vault). |
| I2 | Info | `nft_tokens` and `nft_tokens_for_owner` follow NEP-181; with many tokens a call without a limit can exceed view gas. The frontend must page. | Note for block 6. |
| I3 | Info | A late `nft_resolve_transfer` after a stale release returns quietly (no event); `on_claim` in the same case emits `anomaly`. | Accepted. |
| I4 | Info | The registry wasm is 674606 bytes after the audit fixes, about 6.75 NEAR of code storage on testnet (UNVERIFIED on testnet). | Budgeted in DEPLOY_PLAN.md. |
| I5 | Info | `amend_split` is not built; splits change only through `uphold_claimant`. Licenses are not built. | Scope (rows 46, 47). |

No High or Critical findings. No finding allowed a holder's money to reach anyone other than the current holder of the share.

## Method by method

Legend. Deposit: `1y` exactly 1 yoctoNEAR; `>=1` any nonzero deposit that also covers storage, rest refunded; `storage` deposit must cover storage (nonzero in practice); `none` no deposit. Any deposit blocks a function-call key at the protocol level (`DepositWithFunctionCall`, sandbox test 8).

### Registry: admin and membership

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `new` | the account itself (M1) | none | Sets registrar and token; no setters; no upgrade or migrate method exists. |
| `grant_credential` | registrar | storage + 0.00125 NEAR | Refuses system accounts (registry, its sub-accounts, the token) and agent accounts. Registers the member with the token; `on_member_registered` (private) removes the credential if that fails (L3). Event `credential_granted` or `credential_grant_failed`. |
| `suspend_credential`, `reinstate_credential` | registrar | 1y | Doc hash required. Events. |
| `grant_payer_role`, `revoke_payer_role` | registrar | 1y | Doc hash; refuses system and agent accounts. Events. |
| `registrar_transfer` | registrar | 1y | Only while the holder is suspended; receiver active; respects both locks; refused for Void or disputed shares. Event with doc hash. |
| `release_unallocated` | registrar | 1y | Only to an active, undisputed share of the same group; never more than the group's escrow; debit before forward. Event. |
| `resolve` | registrar | 1y | Outcome must fit the kind; escrow only to the dispute's own parties; bps total preserved; gas checked for all forwards before any change. Event `dispute_resolved` with doc hash and released amounts. |

### Registry: works, shares, holder powers

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `register_work` | active member | storage | Code shape checked; duplicate code opens a dispute. |
| `register_share` | active member, holder active | 0.1 NEAR vault + 0.00125 + storage + 1000 bytes reserve | Group caps: 16 live shares, 32 entries (L1), 16 groups per work. Over 100 percent opens an over-allocation dispute; refused while minting or while the group is disputed. Two-phase mint with private callbacks `on_vault_created`, `on_vault_ready`; failure marks Failed, releases bps, refunds, cancels a claimant's dispute. |
| `nft_transfer` | current holder | 1y | Receiver active; sender not suspended; refused while `payout_in_flight`, `transfer_pending`, disputed or Void; approvals must be None. |
| `nft_transfer_call` | current holder | 1y | Same checks; sets `transfer_pending` with block height and receiver; `nft_resolve_transfer` (private) acts only if that lock is still its own (T36). Gas floor asserted. |
| `nft_approve`, `nft_revoke`, `nft_revoke_all` | anyone | n/a | Always panic (T5). |
| `claim` | current holder, not suspended | 1y | Locks checked; debit and `payout_in_flight` before the vault call; `on_claim` (private) clears only its own lock and rolls back only if its own lock failed (T3, T36). Allowed on Void shares for money already known. |
| `set_share_note` | current holder, not suspended | >=1 | 280 bytes max; refused while transfer pending; never read by payout logic. |
| `confirm_share` | active member holding another active share of the group | >=1 | Not on own share; prunes stale confirmations; bounded by the group. |
| `attest_share` | payer role | >=1 | Share active and undisputed. |

### Registry: money

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `ft_on_transfer` | the configured token only | n/a | Any other predecessor, malformed msg, or refused routing returns the full amount (T18). Routes `pay_work`, `pay_share`, `fund_allowance`. |
| `sync` | holder or registrar | none | Refused while either lock, a forward in flight, a quarantined forward or a dispute is open; `on_sync` (private) discards results if the epoch moved (T21, T24). |
| `retry_forward` | anyone | none | Debits undelivered before re-forwarding; failure puts it back. Active or Void shares. |
| `release_stale_lock` | anyone | none | Only after `STALE_LOCK_BLOCKS`. Transfer lock cleared at once. Payout lock: never cleared by the call; it only schedules a balance read that emits `payout_uncertain` (diagnostic, changes nothing); settled by the real `on_claim` or `resolve_stale_payout`; returns a promise; epoch bump for a transfer lock; events (T19, L4, A-02, A-03, RR-2). |
| `release_stale_forward` | anyone | none | Only after `STALE_FORWARD_BLOCKS` (about a day) from the busy period start, and only if `inbound_pending` equals the recorded count. Reads no balance, credits and resends nothing: moves the recorded amount into `written_off` (T19b, A-01, A-04). |
| `resolve_written_off` | registrar | 1 yocto | Settles a quarantined amount from the final public receipt outcome: `delivered_amount` credits the share (refused for a dispute or pending approval, A-05), `failed_amount` goes to `undelivered`; the sum must be at most `written_off`, the rest stays quarantined; sets `resolved_through` so old callbacks are fenced. Needs `expected_seq` (the `settle_seq` the registrar reviewed; a callback in between makes the call fail, RF-H1) and a `doc_hash` not yet used for the share (at most 64 per share); event `written_off_resolved` (T19b, T12, RR-1, RR-3). |
| `assert_quiescent` | contract itself (`#[private]`) | none | Changes nothing. Panics unless every ledger has `inbound_pending` 0 and no lock, no watch or quarantine exists and `forwards_in_flight_total` is 0. Called in the same transaction as the deploy (`with-init-call`), so a busy registry rolls the deploy back. Loops over all tokens, about 0.21 Tgas each (T19b, RR-4). |
| `resolve_stale_payout` | registrar | 1 yocto | Settles a stale payout lock that no callback and no exact balance settled: `paid` keeps the debit, else undoes it. Needs `expected_since_block` (the lock reviewed) and a `doc_hash` not yet used for the share; event `payout_resolved` (T19, T12). |
| `get_forward_watch` | view | n/a | Watch entry of a share: busy period start, newest start, amount and count in flight, `released_through`, `written_off`. |
| `on_forward`, `on_release_payout`, `on_claim`, `on_sync`, `on_vault_created`, `on_vault_ready`, `on_member_registered`, `on_allowance_withdrawn`, `nft_resolve_transfer` | the registry itself (`#[private]`) | none | Never panic on a failed promise; results read with a size cap. |

### Registry: agents

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `grant_role` | active member; agent must be `<label>.<caller>` without a credential | storage, rest buys line credits | Limits validated; re-grant only after revoke. |
| `add_line_credits` | the agent's organisation | at least one credit | Refunds the remainder. |
| `revoke_role` | organisation or registrar | 1y | Immediate. |
| `withdraw_allowance` | organisation, not suspended | 1y | Debit, transfer, private callback restores on failure. |
| `pay_from_allowance` | agent | none | No payee argument; every check before any state change; line marked used, credit spent, allowance debited only after routing succeeded; a panic reverts all of it. |
| `report` | agent | none | Event only. |

### Vault (raw host functions, 7420 bytes)

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `withdraw` | the parent registry only (parent = everything after the first dot of its own id) | must be 0 | Strict borsh decode (no trailing bytes, account-like ids at most 64 bytes, amount > 0) so the hand-built `ft_transfer` JSON cannot be injected. Attaches 1 yocto from its own 0.1 NEAR balance; returns the promise so the registry sees failures. No other exported function: no key, deploy or delete path. |

### Mock USDC (testnet only)

| Method | Caller | Deposit | Notes |
|---|---|---|---|
| `new` | the account itself (M1) | none | Fixed minter. |
| `mint` | minter only | none | Account must be registered; checked add on balance and total supply. |
| `ft_transfer`, `ft_transfer_call`, `ft_resolve_transfer`, storage methods | NEP-141 / NEP-145 standard implementation | standard (1y on transfers) | `storage_unregister(force=true)` disabled. |

## Checklist

**Callbacks and locks.** Every cross-contract step that moves value has one private callback that never panics, reads results with a size cap, and acts only on its own lock (`since_block` plus amount or receiver). Debits happen before the outgoing call; rollbacks happen only on a failed result of the matching lock. Tests: test 6, test 7, unit callback tests, T36 tests.

**Reentrancy across promises.** NEAR has no synchronous reentrancy; the risk is interleaving between a call and its callback. Covered by: `payout_in_flight` (claim vs transfer, T3), `transfer_pending` (bounce, T4, sandbox test 7 with a receiver that claims and transfers during `nft_on_transfer`), `inbound_pending` plus epoch (sync vs forwards, T21), debit-before-call on allowance withdrawals and forwards. `ft_on_transfer` makes no external call before returning except detached forwards whose callbacks touch only their own share.

**Integer overflow and rounding.** Release profile has `overflow-checks = true`, so any overflow panics instead of wrapping. Split math uses `checked_mul`; credits always sum to the payment (unit property tests, sandbox T16 and test 13); remainder rule is public (largest bps, then lowest number). Counters that decrease are only decreased by amounts previously added to them.

**Storage staking.** Callers pay for what they add (works, shares plus a 1000-byte mint reserve, credentials, notes, confirmations, attestations, roles, statement lines via prepaid credits). Registry-paid growth is limited to dispute status updates. The registry account needs its code storage (about 6.7 NEAR, I4) plus a margin; see DEPLOY_PLAN.md.

**Gas on loops.** Bounded loops only: shares per group (16 live, 32 entries), groups per work (16), confirmations (at most the group's other holders), rolling window (24 buckets), `list_disputes` (100). No on-chain history list exists; history is events. Measured in the sandbox: a 16-share work payment through `ft_transfer_call` burns about 89 Tgas; a resolve releasing escrow to 16 shares about 87 Tgas (tests `a01`, `a02`).

**Events.** Every state change emits a NEP-297 event under `share_registry` 1.0.0, plus standard `nep171` mint and transfer events and `nep141` events on the token. Registrar actions all carry a doc hash. Gap: I1, I3.

**Solvency invariant (T31).** `ft_balance_of(registry) >= allowance_total + unallocated_total + escrow_total + undelivered_total + forwards_in_flight_total`. Checked at the end of every sandbox test (`assert_solvent`). Unit tests check each counter moves with its balance.

**Zero full-access keys plan (T1, T13).** Vaults are created without any key (`CreateAccount`, `Transfer`, `DeployContract`, no `AddKey`), proven by `view_access_key_list` in sandbox tests 1, 4, 8, 14 and others. Planned: the registry and mock USDC accounts get their full-access keys deleted after deploy and init (DEPLOY_PLAN.md step 6), after which their code cannot change; proof would be an empty `list-keys` view plus the published code hashes. Current testnet state: step 6 is parked, both full-access keys are still present, so the registry can still be redeployed (gap, THREAT_MODEL T13).

## THREAT_MODEL rows to tests

| Row | Test(s) | Status |
|---|---|---|
| T1 | sandbox t01 (zero keys, embedded code), `assert_keyless` in t04, t08, t14, d3 | covered (live check in block 5) |
| T2 | t03, t04 | covered |
| T3 | t06; unit `transfer_blocked_while_payout_in_flight_then_allowed` | covered |
| T4 | t07; unit `transfer_call_lock_blocks_claim_and_resolve_returns_token` | covered |
| T5 | t16; unit `nft_approve_panics` | covered |
| T6 | t08 (protocol and contract locks) | covered |
| T7 | t09; unit `per_tx_daily_and_count_limits_with_rolling_window` | covered |
| T8 | t11; unit `agent_pays_registered_shares_only_and_once_per_line` | covered |
| T9 | t12 | covered (allowance payments; L5) |
| T10 | t05 | covered |
| T11 | t14, t14b, t14c | covered (bond and expiry are roadmap) |
| T12 | events with doc hashes in every registrar test | partial: multisig roadmap (M2) |
| T13 | a03 (init lock); no upgrade method exists | gap: live key list and code hash check in block 5 |
| T14 | t10 | covered |
| T15 | t01; unit `failed_vault_creation_mints_nothing_and_releases_bps`, `failed_token_registration_mints_nothing` | covered |
| T16 | t16b, t13; unit split tests | covered |
| T17 | unit `failed_claim_rolls_back_exactly` | partial: no sandbox test with an unregistered holder (every member is registered at grant) |
| T18 | unit `refused_payments_return_the_full_amount` | partial: no sandbox fake-token test |
| T19 | unit stale lock tests; unit and sandbox F3 tests (`final_fixes.rs`, `final.rs`) | covered; the stuck lock is written with `patch_state` in the sandbox, a real lost callback cannot be produced |
| T19b | unit `final_fixes.rs`; sandbox `u02_deployed_build_upgrades_in_place_and_a_lost_forward_is_quarantined_and_resolved` | covered; lost callback written with `patch_state` |
| T20 | t01 (code equals embedded vault, zero keys) | covered by construction; live check block 5 |
| T21 | unit `sync_records_plain_transfers_and_discards_stale_reads`, `sync_refused_during_forward` | covered |
| T22 | unit `effective_from_bounds` | covered |
| T23 | d3, t15; unit suspension tests | covered |
| T24 | t14 (held deposit, sync refused) | covered |
| T25 | d4; unit `under_allocated_group_pays_exact_bps_and_escrows_the_rest`, `over_allocation_rules` | covered |
| T26 | unit `failed_forward_goes_to_undelivered`, `retry_forward_resends_undelivered` | covered in unit tests |
| T27 | unit `line_credits_run_out_and_can_be_topped_up`, `count_limit_alone_blocks`, `allowance_and_minimum_payment_are_enforced` | covered |
| T28 | unit `grant_role_requires_direct_sub_account_and_credits_lines`, `only_the_org_can_fund_and_allowance_is_counted` | covered |
| T29 | unit `system_accounts_cannot_hold_credentials` | covered |
| T30 | unit `below_minimum_group_payment_is_refused`; t13 | covered |
| T31 | `assert_solvent` in every sandbox test | covered |
| T32 | t11; unit `note_is_holder_only_and_never_changes_payouts` | covered |
| T33 | none | gap: licenses not built |
| T34 | none needed (no sweep method exists) | covered by absence |
| T35 | d3; unit `registrar_cannot_move_active_holders_share`, `unallocated_release_must_stay_in_its_group` | covered |
| T36 | unit `stale_payout_lock_released_only_after_timeout_and_late_callback_is_safe`, `late_failed_claim_after_stale_release_changes_no_balance`, `late_resolve_after_stale_release_keeps_current_owner` | covered in unit tests |
| T37 | t09, t10, t11 | residual risk accepted |
| T38 | t14b; unit `over_allocation_rules`, `failed_claimant_mint_cancels_dispute_and_escrow_goes_to_undelivered` | covered |

## Contract review fixes (2026-11-13, branch `fix/contract-review`, now merged into `main`)

An external review of the contracts (H1 to H3, M1, M2, L1 to L9) and Security Suite F-04 and F-12 were worked on this branch; see THREAT_MODEL.md (rows marked "review", new T39) and DESIGN.md section 19. Fixed in code: H1 (holder or registrar only sync; new vaults export `ft_on_transfer`), H2 (explicit minimal contests, pruning), H3 (registrar cannot hold a credential), M2 (on-chain split sheet approval), L2, L4 (F-04), L5, L6, L8, L9, M1 for stuck mints plus 10 Tgas forward callback gas. Not fixed in code at that time: a lost `on_forward` (later fixed, T19b); vaults created before the fix keep the old code (T21); registrar trust (T12, M2 above). The vault is now 7478 bytes.

## Final contract fixes (2026-11-14)

Branch `fix/final-contract`, made while the registry key still existed, because no code can be deployed after the key is deleted. Findings of the first external report ("Deployment and documented design risks", F3), the external review of the first version of this branch (4b688f1: H1, M1, M2, L1 to L8) and a second, deeper audit (A-01 to A-06) are addressed or documented. The stored layout is unchanged.

| Finding | Change | Layout | Evidence |
|---|---|---|---|
| Lost forwarding callback has no timeout recovery; H1 and A-01 (a balance or time based release can make a late transfer resendable) | `release_stale_forward` only quarantines (`written_off`); settlement by the late callback or `resolve_written_off` (T19b) | No stored type changed. The watch is a separate storage key `fwd:<token_id>`; `on_forward` gains an optional argument, so callbacks already in flight at the upgrade still settle | unit `final_fixes.rs`; sandbox `u02_deployed_build_upgrades_in_place_and_a_lost_forward_is_quarantined_and_resolved`, `f03`, `f04` |
| F3, A-02, A-03, RR-2, R-L1 stale failed payout inflates `earned` and `paid_out` | `release_stale_lock` never clears a payout lock (a balance cannot decide it); it only emits `payout_uncertain`; the real `on_claim` or `resolve_stale_payout` settles (T19) | No stored type changed | unit `f3_*`, `a_failed_payout_*`; sandbox `final.rs` `f01`, `f02` |
| M1 a drip of payments postpones release | busy period start only moves when the count is zero; `forward()` and `retry_forward` hold a stuck busy period | none | `a_drip_of_payments_cannot_postpone_a_stale_release` |
| M2 unwatched forward blocks payout release | no blind fallback (A-02 supersedes it); registrar `resolve_stale_payout` | none | `unwatched_pending_forward_does_not_block_stale_payout_release` |
| A-04, A-06 legacy forwards | release needs `inbound_pending` equal to the recorded count; quiescent deploy precondition `npm run check:quiescent`; legacy recovery documented as unsupported | none | `audit_legacy_none_callback_*`, `legacy_missing_watch_*`; `scripts/check-quiescent.test.mjs` |
| A-05 recovery bypasses the dispute hold | `resolve_written_off` with `delivered_amount` above zero refused for a disputed or unapproved share; the release credits nothing | none | `resolve_as_delivered_is_held_*`, `disputed_plain_deposit_*` |
| L4, L6, L7 | transfer lock released beside a waiting payout lock; sandbox `f03`; event `release_discarded` | none | `a_stale_transfer_lock_is_released_*`, `f03` |
| RR-1, R-M1 a late callback of a resolved quarantine settles a later one | `resolved_through` fence; callbacks at or below it change nothing (`late_forward_callback_fenced`) | The watch entry grows to 76 bytes (68 before RF-H1) (separate key, never meets an older format: none exist on the live registry) | `a_late_success_callback_of_a_resolved_quarantine_never_settles_a_later_one`, `a_late_failure_callback_*` |
| RR-3 mixed outcomes under one registrar answer | `resolve_written_off(delivered_amount, failed_amount)`, sum at most `written_off`, remainder stays quarantined | none | `a_mixed_quarantine_*`, `a_remainder_stays_quarantined_*`, `a_delivered_part_is_held_*`; sandbox `f04`, `u02` |
| RF-H1 a real callback settles forward A, then a registrar decision citing A's true failure receipt is counted again against the aggregate remainder | `ForwardWatch.settle_seq` bumped by every change of the quarantine, `expected_seq` checked by `resolve_written_off`; per-share consumed `doc_hash` list (64 at most) for both registrar methods; `expected_since_block` for the payout lock | none | `callback_before_registrar_cannot_double_settle_a_failed_receipt`, `registrar_before_callback_fences_*`, `the_same_evidence_can_never_settle_twice`, `a_later_generation_*`, `a_partial_remainder_*`; sandbox `f04`, `u02` |
| RR-4, R-L3 deploy races a payment | `assert_quiescent` in the deploy transaction; `check:quiescent` pins one block, checks `fwd:` and prints `SNAPSHOT QUIET` | none | `assert_quiescent_*` (5 unit tests); sandbox `u03`; `scripts/check-quiescent.test.mjs` |
| R-L2 registrar procedure undefined | written in DEPLOY_PLAN and T12, T19, T19b | none | n/a |
| L1, L2, L3, L8 | documented in T19 and T19b | none | n/a |

What the evidence does not show: the paths depend on a callback that never commits, which a sandbox node does not produce. The sandbox tests write the state such a callback would leave with `patch_state` and then run the real recovery calls against a real vault and token. The recovery is quarantine plus a registrar decision backed by the public receipt outcome (a stated registrar trust, T12), so its correctness after key deletion depends on the registrar acting. No code decides a stale payout lock any more because a balance cannot tell a failed withdraw from a paid one plus a deposit of the same size, so without a registrar that share stays frozen (T19). The `near contract deploy ... with-init-call` command line was not sent to a network; the sandbox test sends the same two actions in one batch.

## Dependency audits and accepted risks (2026-11-14)

Run on 2026-11-14 at about 12:30 UTC on the merged release tree, with the RustSec advisory database loaded at that time (1509 advisories) and the npm registry advisories. These are tool results and not a review. Nothing was upgraded in this pass: a dependency change would change the wasm that is deployed on testnet, and the code hashes in `deployments/` must keep matching the local build.

### Rust: `cargo audit` (root workspace)

Result: `error: 1 vulnerability found!` and `warning: 3 allowed warnings found`.

| ID | Crate | Kind | Where it comes from | Decision |
|---|---|---|---|---|
| RUSTSEC-2026-0387 | `crypto-bigint` 0.5.5 | Vulnerability: timing variability in operations meant to be constant time. Fix: `>=0.7.6` | Dev and test only (see the evidence below) | Accepted, test only |
| RUSTSEC-2022-0054 | `wee_alloc` 0.4.5 | Unmaintained | Allocator pulled in by `near-sdk` 5.30.1, in the wasm contracts that use `near-sdk` (registry, Mock USDC, auction) | Accepted: no vulnerability is reported, only that it is unmaintained. The vault does not use it (raw host functions, no allocator) |
| RUSTSEC-2024-0449 | `unsafe-libyaml` 0.2.11 | Unmaintained | `serde_yaml` of `near-parameters` 0.38.1, reached through `near-sdk` and `near-workspaces` | Accepted: test tooling only, not in the wasm build |
| RUSTSEC-2025-0026 | `registry` 0.1.0 | Unmaintained | Matched by name. Our contract crate is a local path crate called `registry`, and the advisory is about a different crate on crates.io with the same name | Accepted: a name collision, not our dependency. Not renamed, because the package name is part of the build record |

Evidence for RUSTSEC-2026-0387 (`cargo tree -i crypto-bigint`, same day):

- `cargo tree -i crypto-bigint -e normal` prints `nothing to print` for the host and for `--target wasm32-unknown-unknown`: the crate is in no normal dependency path.
- `cargo tree -p registry -e normal --target wasm32-unknown-unknown` has no line for `crypto-bigint`, `p256` or `near-vm-runner`: the registry wasm that is deployed does not contain it.
- `cargo tree -i crypto-bigint -e normal,dev` shows the one path: `crypto-bigint` 0.5.5, `elliptic-curve` 0.13.8, `ecdsa` 0.16.9, `p256` 0.13.2, `near-vm-runner` 0.38.1, `near-sdk` 5.30.1, reached only with the `near-sdk` `unit-testing` feature that the contracts turn on as a dev dependency, and by `near-workspaces` in the sandbox test crate.

So the vulnerable code runs only in unit and sandbox tests, which handle no secret. The fix needs a newer `near-sdk` that moves off `crypto-bigint` 0.5; that is a dependency change that would alter the deployed wasm, so it is left for the audit phase of the roadmap.

### Rust: the other lock files

- `tee/auction-wasi` (the WASI module that runs in the TEE, 42 crates): `cargo audit` exit code 0, no vulnerability and no warning.
- `spike/outlayer` (throwaway spike, 281 crates, not deployed): the same three advisories as above (`crypto-bigint`, `unsafe-libyaml`, `wee_alloc`). Not shipped.

### npm: `npm audit` in `site/`

Result: `6 vulnerabilities (3 moderate, 3 high)`. All six reach the project only through `@astrojs/vercel` 11.0.21 (`npm ls`):

- High (3 entries): `path-to-regexp` 6.1.0 (GHSA-9wv6-86v2-598j, backtracking regular expressions), reached through `@vercel/routing-utils` 6.6.1, and `@astrojs/vercel` itself, which depends on it.
- Moderate (3 entries): `glob` 13.0.7 (GHSA-3g3m-2fjr-6pw4), `minimatch` 10.2.6 (GHSA-g3hv-75wg-m249) and `@vercel/nft` 1.11.1, which depends on them.

Decision: accepted as build-time findings. `npm audit fix` would change the lock file, and `npm audit fix --force` would install `@astrojs/vercel` 8.0.4, an older major version and a breaking change, so neither was run. These packages are the adapter's build helpers (route rules and file tracing). Our own code does not import any of them (a search of `site/src` and the Astro config finds no use). Whether the adapter's deployed function bundle includes any of them is UNVERIFIED. The repository root has no npm dependencies and no lock file, so `npm audit` does not apply there.
