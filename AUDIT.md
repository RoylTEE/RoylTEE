# Audit

Manual audit pass, 2026-11-13, by the builder, over the code at the commit that adds this file. It is not an independent review. The bundled `near-contract-audit` skill named in the build prompt is not available in this environment (DECISIONS_LOG row 60), so this pass follows the checklist the owner set: access control, deposits, callbacks and locks, reentrancy across promises, integer overflow and rounding, storage staking, gas on loops, events, the solvency invariant, the zero full-access-keys plan, and every THREAT_MODEL row mapped to a test or marked as a gap. After this pass, review passes 03a to 03d were run by reviewers other than the builder, from review packs (contracts, site and server, README); each finding and its state are in [docs/reviews/03a-status.md](docs/reviews/03a-status.md) to [03d-status.md](docs/reviews/03d-status.md). They were read-only reviews of the code and docs, not a formal audit. A formal audit is still recommended before any mainnet use (none is planned: testnet only).

Scope: `contracts/registry`, `contracts/vault`, `contracts/mock-usdc`. `contracts/test-receiver` is a test helper and is never deployed.

## Findings by severity

| ID | Severity | Finding | Status |
|---|---|---|---|
| M1 | Medium | Init front-running. `new` on the registry and on mock USDC was an open `#[init]`. If deploy and init land in separate transactions, anyone could initialise a fresh deploy with their own registrar (or minter) and own the contract's admin role. | Fixed: both `new` methods now require `predecessor == current_account_id`. Tests: unit `only_the_registry_account_itself_can_initialise`, mock USDC `stranger_cannot_initialise`, sandbox `a03_stranger_cannot_initialise_a_fresh_deploy`. DEPLOY_PLAN.md also deploys and inits in one transaction. |
| M2 | Medium (accepted) | Single registrar key, fixed at init, no rotation. Loss of the key freezes resolve, suspension and payer-role management; compromise allows every registrar power (T12, T35, T38), though never a direct payout to itself. | Accepted for the demo; multisig with time delay is roadmap. |
| L1 | Low | A group's token list kept Failed and Void entries without a cap, so loops in payments, voiding and tier views could grow past `MAX_SHARES_PER_GROUP`. Not cheaply exploitable (failed mints and voids are not attacker controlled), but unbounded. | Fixed: `MAX_GROUP_ENTRIES = 32` on the whole list. Tests: unit `failed_shares_leave_the_group_list` (tests.rs: `MAX_GROUP_ENTRIES` failed mints leave the list empty and a later share still mints) and `h2_repeat_claims_do_not_close_the_group` (review_fixes.rs: `MAX_GROUP_ENTRIES + 5` voided claims keep the list at 2). |
| L2 | Low (accepted) | Funds with no release path by design: stray plain transfers to the registry (decision 10), a suspended organisation's allowance until reinstatement (row 51), and unallocated escrow of a voided work's groups (row 59). All counted in the solvency sum. | Accepted; documented. |
| L3 | Low (accepted) | A credential is active from `grant_credential` until `on_member_registered` runs; if the token registration then fails, the credential is removed although the account might have received a share in that window. Registration fails only on a bad deposit, which the registry controls. | Accepted. |
| L4 | Low (accepted) | `STALE_LOCK_BLOCKS = 1200` is a guess (about 20 minutes); whether it is long enough under testnet congestion is UNVERIFIED. Too short would let a lock be cleared while its callback is still pending; T36 makes such a late callback harmless. | Open, UNVERIFIED. |
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
| `sync` | anyone | none | Refused while either lock, a forward in flight, or a dispute is open; `on_sync` (private) discards results if the epoch moved (T21, T24). |
| `retry_forward` | anyone | none | Debits undelivered before re-forwarding; failure puts it back. Active or Void shares. |
| `release_stale_lock` | anyone | none | Only after `STALE_LOCK_BLOCKS`; clears without rollback; epoch bump; events (T19, L4). |
| `on_forward`, `on_claim`, `on_sync`, `on_vault_created`, `on_vault_ready`, `on_member_registered`, `on_allowance_withdrawn`, `nft_resolve_transfer` | the registry itself (`#[private]`) | none | Never panic on a failed promise; results read with a size cap. |

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
| T19 | unit stale lock tests | covered in unit tests; a stuck lock cannot be produced in the sandbox |
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

## Contract review fixes (2026-11-13, branch `fix/contract-review`)

An external review of the contracts (H1 to H3, M1, M2, L1 to L9) and Security Suite F-04 and F-12 were worked on this branch; see THREAT_MODEL.md (rows marked "review", new T39) and DESIGN.md section 19. Fixed in code: H1 (holder or registrar only sync; new vaults export `ft_on_transfer`), H2 (explicit minimal contests, pruning), H3 (registrar cannot hold a credential), M2 (on-chain split sheet approval), L2, L4 (F-04), L5, L6, L8, L9, M1 for stuck mints plus 10 Tgas forward callback gas. Not fixed in code: a lost `on_forward` (T19); vaults created before the fix keep the old code (T21); registrar trust (T12, M2 above). The vault is now 7478 bytes.

