# Transfer threat model

Status: written on 2026-11-13, BEFORE any contract code existed in this repo. Block 2 code landed later the same day; rows covered by passing tests say so under "Block 2 status" at the end. Owner decisions 3, 4 and 10 (2026-11-13) changed T23 and T25 and added T34 and T35; those edits are marked. Block 3 added T36 and T37 and a "Block 3 status" section. Block 4 changed T24 (held deposits by refusing sync instead of a snapshot cap), added T38 and a "Block 4 status" section. The contract review fixes (branch `fix/contract-review`) changed T6, T7, T11, T12, T13, T15, T19, T20, T21, T24, T35 and T38, added T39 and a "Review fixes status" section; those edits are marked "review".

Scope: every way a seller, a buyer, an agent, a payer, an outside attacker or a privileged role could keep money or access, move money to the wrong place, or lose money, around a rights-share NFT and its vault. Testnet only. Mock USDC only.

Terms used below (full detail in DESIGN.md):

- `REG`: the registry contract account. NEP-171 share NFTs, ledger, credentials, roles, disputes.
- Vault: `s<N>.REG`, one per share token, zero access keys, runs a tiny vault contract whose only method is `withdraw`, callable only by `REG`.
- Holder: the current `owner_id` of a share NFT, read from the registry at call time.
- Member: an account with an active credential. Only members can receive share NFTs.
- Agent: a sub-account of a member organisation, holding only a function-call key scoped to `REG`.
- `payout_in_flight`: per-share lock set by `claim` until its callback finishes.
- `transfer_pending`: per-share lock set by `nft_transfer_call` until `nft_resolve_transfer` finishes.
- Known balance: what the registry's ledger believes sits in a vault. Earned = known balance + total paid out.

## Facts this model relies on

| Fact | Source | Status |
|---|---|---|
| A function-call key can only call one contract (optionally a list of methods) and cannot attach NEAR to the call. | docs.near.org, Access Keys page, section "Function-Call Keys" | Verified 2026-11-12 |
| Requiring exactly 1 yoctoNEAR on a method means a function-call key cannot call it; ownership checks are still needed. | docs.near.org, Security, "Require one yoctoNEAR" | Verified 2026-11-12 |
| An account with all keys removed is locked: only its own contract can move assets, create sub-accounts or change its code. A locked account can still add keys if its contract has a method for it. | docs.near.org, Access Keys page, "Locked Accounts" | Verified 2026-11-12 |
| A callback always runs but can still fail (gas, panic, limits). Only the callback's own changes revert; it is not retried. Withdrawals should debit before the call. | docs.near.org, Security, "Callbacks can fail" | Verified 2026-11-12 |
| `ft_on_transfer` must check that the predecessor is the expected token contract and return the full amount otherwise. | docs.near.org, Security, "Fungible Token Implementation" | Verified 2026-11-12 |
| `nft_transfer` and `nft_transfer_call` require 1 yoctoNEAR; `nft_resolve_transfer` settles the call path. | NEP-171 text in the near/NEPs repo | Verified 2026-11-12 |
| A sub-account `x.REG` can only be created by `REG`. | Sharded NFT items draft (houseofstake/tla-contracts, docs/sharded-nft-items.md), "Naming" paragraph | Verified as stated in that draft |
| Once a sub-account exists, its parent cannot delete it, redeploy its code or add keys to it (only the account itself can). | NEAR runtime behaviour | Shown in the sandbox by test 21 (`t21_parent_cannot_delete_redeploy_or_key_a_vault`): the parent's DeleteAccount, DeployContract and AddKey on a vault all fail. Not checked against the protocol spec text. |

## How to read the table

- Attack: what the adversary does.
- Impact: what they gain or what breaks if unmitigated.
- Mitigation: what the design does (see DESIGN.md for exact methods and fields).
- Test: test number from the test plan (1 to 17 are from the brief, 18 and up are added here), plus named unit and sandbox tests. Review regression tests live in `contracts/registry/src/review_fixes.rs` (unit) and `contracts/tests/tests/review.rs` (sandbox). Live checks: `npm run verify:testnet` (`scripts/verify-testnet.mjs`, read-only RPC, added in block 7) reads every vault's key list, the code hashes of the registry, mock USDC and vaults, the registry and mock USDC key counts, and the solvency sum, and compares them with `deployments/testnet.json`. It replaces the separate `verify-keys` script planned earlier.

## Rows from the brief (T1 to T16)

### T1. Seller keeps a key to the vault after sale
- Attack: the seller (or whoever minted) holds an access key on `s<N>.REG` and uses it to send the USDC out directly, or to delete the account.
- Impact: total loss of the share's money for every later holder.
- Mitigation: the vault is created by `REG` in a single batch of `CreateAccount`, `Transfer`, `DeployContract`, with no `AddKey` action, so it never has any key. The vault contract has no method that adds keys, deploys code or deletes the account. Holders never receive a key; control is a registry check, so there is nothing to hand over or rotate.
- Test: test 1 (mint, then `view_access_key_list` on the vault returns empty). On testnet: `npm run verify:testnet` checks that every vault key list is empty.

### T2. Seller claims after sale
- Attack: the old holder calls `claim(token_id)` after the NFT moved.
- Impact: seller takes money that travels with the token.
- Mitigation: `claim` reads `owner_id` from the token record at call time and requires `predecessor == owner_id`. The payout receiver is that same `owner_id`, never a caller-supplied account.
- Test: test 3 (non-owner claim fails), test 4 (pay, transfer, pay again; seller claim fails; buyer gets both).

### T3. Claim-then-sell race (sale lands while a payout is in flight)
- Attack: holder calls `claim`, then in the same or next block transfers the NFT, hoping the buyer also claims the same money, or that a failed payout gets credited back to the new holder after the seller already received it.
- Impact: double pay or lost funds.
- Mitigation: `claim` sets `payout_in_flight`, debits the known balance and adds to total paid out BEFORE calling the vault. `nft_transfer` and `nft_transfer_call` panic while `payout_in_flight` is set. `on_claim` clears the flag and rolls back the debit only if the vault's `ft_transfer` failed. A second `claim` while the flag is set panics.
- Test: test 6 (transfer during in-flight payout fails; after callback it succeeds; no double pay).

### T4. nft_transfer_call bounce: receiver contract claims during the call then returns the token
- Attack: a member contract receives the NFT through `nft_transfer_call`, calls `claim` from inside `nft_on_transfer`, then returns `true` so the token goes back to the sender.
- Impact: receiver walks off with the vault balance while the seller gets the token back.
- Mitigation: `nft_transfer_call` sets `transfer_pending` before calling the receiver. `claim`, `nft_transfer`, `nft_transfer_call`, `set_share_note` and `sync` all panic while it is set. Only `nft_resolve_transfer` (private) clears it, then returns the token if asked. NEP-171 rule kept: the token only goes back if the current owner is still the receiver.
- Test: test 7 (receiver tries to claim during `nft_on_transfer` and returns the token; claim fails; funds intact; flag clear afterwards). Test 7 also covers the receiver trying to transfer onward during the call (fails).

### T5. Claim while listed on a marketplace
- Attack: holder lists the NFT with an approval, a marketplace sells it, holder claims between sale and settlement; or a stale approval lets an old approved account move the token later.
- Impact: buyer pays for a share that was just emptied; stale approvals move tokens after sale.
- Mitigation: NEP-178 approvals are disabled. `nft_approve`, `nft_revoke` and `nft_revoke_all` panic. `nft_is_approved` returns false. `nft_transfer` and `nft_transfer_call` panic if `approval_id` is `Some`. Catalog sales settle off chain and the holder signs the transfer.
- Test: test 16 (`nft_approve` panics; transfer with `approval_id: Some(..)` panics).

### T6. Agent key used to move ownership or withdraw
- Attack: a compromised or prompt-injected agent calls `nft_transfer`, `claim`, `withdraw_allowance`, `grant_role`, split changes or registrar methods.
- Impact: theft of shares or allowance.
- Mitigation: two locks. Protocol lock: the agent account holds only a function-call key scoped to `REG` with methods `pay_from_allowance` and `report` (there is no `buy_license` method: licenses were cut, review); such a key cannot attach a deposit. Contract lock: every owner and admin method requires exactly 1 yoctoNEAR and an ownership or role check. Agents are never holders (agent accounts cannot receive a credential while they hold an agent role).
- Test: test 8 (agent calls to `nft_transfer`, `claim`, `withdraw_allowance`, `amend_split` and `grant_role` all fail). Whether near-workspaces can sign with a function-call key to show the protocol-level rejection is UNVERIFIED; if not, test 8 shows the contract-level rejection (0 deposit) and the method-list rejection, and the testnet scripted agent shows the protocol rejection on chain.

### T7. Agent overspends
- Attack: agent pays more than intended per transaction or per day.
- Impact: drains the organisation's allowance.
- Mitigation: per-agent `per_tx_limit`, rolling `daily_limit` and `max_tx_per_day`, all checked in the contract before any transfer. Allowance is debited before the transfer. The window is 24 hourly buckets plus the bucket of the hour 24 hours back, kept in its own storage slot (`l2t:<agent>`), so a payment counts for at least 24 full hours and at most 25 (review L2; before, the limit could be spent twice 23 hours and 1 minute apart). The org pays for the slot at `grant_role`; roles granted before the fix get it at their next payment, paid by the registry.
- Test: test 9 (over per-tx fails; over daily fails; under both passes); unit tests `l2_rolling_limit_covers_a_full_24_hours`, `l2_tail_counts_within_the_same_hour_and_for_tx_counts`, `per_tx_daily_and_count_limits_with_rolling_window`.

### T8. Agent pays wrong or attacker payee (prompt injection in metadata, stale cache)
- Attack: song metadata, a share note or a statement says "pay account X"; the agent passes X.
- Impact: money to an attacker.
- Mitigation: `pay_from_allowance` takes only a target (work group or share token id), amount, statement line id, period and statement hash. It has no payee argument. The contract resolves vaults from the registry. Money only ever lands in share vaults.
- Test: test 11 (metadata and note contain an injected payee; payment still lands only in registered vaults; attacker balance unchanged).

### T9. Double payment of the same statement line
- Attack: agent retries or replays the same statement line.
- Impact: payer pays twice.
- Mitigation: idempotency key = hash of (organisation, `statement_line_id`). Inserted before any transfer; a second call panics with a clear message and no debit.
- Test: test 12 (second submit pays nothing; allowance unchanged).

### T10. Token drifts to an unknown holder
- Attack: holder sends the NFT to an account that is not a verified member.
- Impact: the payout address ends up controlled by an unknown party; compliance breaks.
- Mitigation: `nft_transfer` and `nft_transfer_call` require the receiver to hold an active credential. No mint path to non-members.
- Test: test 5 (transfer to `outsider` fails).

### T11. Bad-faith duplicate claim to freeze a rival
- Attack: a member registers a share that pushes a group over 100 percent, or re-registers an existing ISRC or ISWC, to freeze a rival's income.
- Impact: rival stops getting paid.
- Mitigation: only the contested fraction freezes and only new money. A claimant above 100 percent must name the contested shares in `contests` (there is no default of "all shares", review H2), and the list must be minimal: dropping its smallest share must leave less than the excess, so a 1 bps claim names one share, not the group. Residual (review 03a M2): minimal bounds the number of named shares, not the amount. Each named share's whole new credit is escrowed, and the claimant picks which shares to name, so a 1 bps claim may name a 9999 bps share and hold 99.99 percent of the group's new money until resolve. Code options, not built: escrow only the excess, or require the smallest shares that cover it. A share held by the claimant or by the caller cannot be named, so an incumbent cannot dispute its own share to block a real claimant. Shares still waiting for the work registrant's approval cannot be named (T39: the registrant rejects them instead). Failed and Void shares are pruned from the group's list, so repeated claims cannot fill `MAX_GROUP_ENTRIES` and close the group. Shares not named keep getting paid. Money already in vaults stays claimable (T24). A duplicate code freezes nothing on the original work; payments to the duplicate work are refused. Only credentialed members can register. Residual: a member can still use an accomplice account to open a dispute against a rival's share; it freezes only the named shares' new money until the registrar resolves it. Challenger bond and auto-expiry are roadmap, not code.
- Test: test 14 (only contested fraction escrowed; other shares paid; frozen share cannot transfer; resolve releases escrow). Test 14b (duplicate code: original work still paid; payment to duplicate refused). Unit tests `h2_one_bps_claim_freezes_only_one_named_share`, `h2_repeat_claims_do_not_close_the_group`, `h2_incumbent_cannot_dispute_its_own_share`, `h2_zero_bps_outcome_is_pruned`, `over_allocation_rules`, `failed_shares_leave_the_group_list`.

### T12. Registrar abuse (rewritten, review H3)
- Attack: the registrar grants credentials to accomplices, suspends rivals, or resolves disputes unfairly.
- Impact: control of who can hold and how disputes end.
- Trust: the registrar is a TRUSTED role in this demo. It is the one key the system relies on. What it can do: grant, suspend, reinstate and remove credentials (including to an accomplice account); suspend a holder and then move that holder's share with `registrar_transfer` to any active member; resolve disputes (decide which side keeps the bps and where escrow goes, within the dispute's parties); release a group's unallocated escrow to any active approved share of that group; refund a void work's unallocated escrow to any account other than itself (`refund_unallocated`); approve or reject a pending share as backup signer (T39); send plain deposits out of a rejected share's vault to any account other than itself (`recover_rejected_vault`); grant and revoke the payer role; revoke agent roles; sync any share. So a registrar acting in bad faith together with an accomplice member could redirect shares and held money.
- Controls: the registrar account cannot itself hold a credential (`grant_credential` refuses it, review H3), so it can never be a holder, receive a share or `claim`; the two methods that send money to a named account refuse the registrar as receiver; it cannot transfer a share of an active member; it cannot touch money already in an active holder's vault; it has no upgrade or sweep method (T13, T34). Every registrar action except `sync` needs 1 yoctoNEAR (a full-access key) and emits an event; all except `sync` and `revoke_role` also take a document hash (review 03a L6). So every use of the power is public and attributable. Production plan: a multisig council with a time delay (roadmap, not code).
- Test: unit test `h3_registrar_cannot_hold_a_credential`; test 30 (registrar cannot call `claim` or `nft_transfer` on a share it does not own). Event presence checked in tests 14 and 15.

### T13. Contract upgrade swaps the logic
- Attack: whoever holds a full-access key on `REG` redeploys a contract that pays them.
- Impact: total loss.
- Mitigation: no upgrade method in `REG` or the vault. Vaults have no keys from birth. Code hashes are published in `deployments/testnet.json` and compared with a local build.
- Current state (review, Security Suite F-12; read on 2026-11-14 02:50 UTC): the full-access keys on `REG` and on the mock USDC account are STILL PRESENT on testnet. Deleting them is planned but parked (it is a delete and needs the owner's yes), so today whoever holds the `REG` key can redeploy the registry. That key was used twice, by the owner, for the planned in-place redeploys: step 15 on 2026-11-13 (`9oR5awwxiJPouDhpYdTxTquskc8YpoFHdmAJy4SrrAMp`, review fixes) and the audit redeploy on 2026-11-14 (`BGBUJDTnnjpevSUZkdmfeVASxbKHn1FQDAZqj75kH5rU`, audit F1). The registry code hash on testnet is now `5Taw8Syz7g1QxNa4Faw8mz7fnNgoXGj7MwX2VS3obRFo`; earlier hashes are listed in `deployments/testnet.json`. The 13 vaults still have zero keys; `s1` to `s7` keep the first vault build `BBLu1w41...` and `s8` to `s13` the current one `BgC1pQYr...`. This is a testnet demo state, not the production design.
- Test: `npm run verify:testnet` compares the live code hashes and keys with `deployments/testnet.json` (today it expects one full-access key on `REG` and on mock USDC, as recorded) and checks that the vault code the registry deploys (`get_vault_code_hash`) is the recorded vault build. `scripts/build-wasm.sh` reproduces the deployed registry and vault sha256 on one machine; other machines are UNVERIFIED. The deployed mock USDC is an older build that a fresh build does not reproduce.

### T14. Revoked agent keeps acting
- Attack: agent keeps calling after the organisation revokes it.
- Impact: unwanted payments.
- Mitigation: `revoke_role` sets `revoked = true` in the same call; every agent method checks it first. Expiry checked too. The organisation also deletes the agent's function-call key (off-contract, recommended in the runbook).
- Test: test 10 (revoked agent's next call fails; expired role fails).

### T15. Storage griefing / unregistered vault loses USDC
- Attack: vault is not storage-registered with the token, so incoming transfers fail or the share cannot receive; or an attacker grows registry state cheaply.
- Impact: payments bounce or are lost; registry runs out of storage balance.
- Mitigation: `register_share` is two-phase. The share is `Pending` until the vault account is created AND `storage_deposit` on mock USDC succeeds; only then is the NFT minted. If either step fails, the share is marked `Failed`, no NFT exists, its bps reservation is released, and the unused deposit is refunded. Every state-adding call is payable and charges the caller for storage; free calls (`sync`, `retry_forward`, `release_stale_lock`, `release_stale_mint`) add no state. A share whose mint callbacks never committed no longer blocks its group: after `STALE_MINT_NS` (20 minutes) anyone may call `release_stale_mint`, which fails the share and releases its reserved bps; a late callback then finds the share Failed and mints nothing (review M1). The NEAR attached at registration is not refunded on that path, because it may already be spent on the vault account. Exception to "the unused deposit is refunded" (review 03a L3): on the L6 path (holder suspended before mint) and on `release_stale_mint` the vault account already exists; it stays keyless with no NFT, the NEAR spent on it is not refunded, and a plain deposit sent to it (its address is in the `share_registered` event) can be neither claimed nor recovered, because `recover_rejected_vault` accepts only Rejected shares. Accepted for the demo. Bounded string lengths and per-group share cap.
- Test: test 1 (vault registered with mock USDC at mint); test 15b (forced registration failure leaves no NFT and refunds); unit test `m1_stale_mint_is_released_and_late_callbacks_mint_nothing`.

### T16. Rounding loss in splits
- Attack: repeated payments leak or create units through rounding.
- Impact: money created or lost; disputes over cents.
- Mitigation: integer math only (u128), each credit = floor(amount * bps / 10000). Remainder (always less than the number of shares) goes to the share with the largest bps; ties go to the lowest token number. Sum of credits plus escrow equals the payment exactly. The rule is in DESIGN.md so anyone can recompute it.
- Test: test 13 (split sums to the payment to the last unit, including awkward amounts like 1, 7, 9999 and a 3-way 3333/3333/3334 split; remainder lands on the defined share).

## Added rows (T17 and up)

### T17. New holder not storage-registered with the token
- Attack or fault: a share moves to a member who never registered with mock USDC; `claim` sends `ft_transfer` to an unregistered account.
- Impact: the token contract rejects the transfer; if the registry did not roll back, the money would be debited from the ledger but still sit in the vault, or be lost from the ledger view.
- Mitigation: `grant_credential` is payable and registers the member with mock USDC in the same flow, so every member is registered. If a claim still fails, `vault.withdraw` returns the `ft_transfer` promise, `on_claim` sees the failure and restores the known balance and total paid out. Money never leaves the vault on failure.
- Test: test 18 (holder unregistered on purpose; claim fails cleanly; ledger restored; second claim after registration succeeds once).

### T18. ft_on_transfer spoofing by a fake token contract or a direct call
- Attack: an attacker deploys a fake token named "USDC" and calls `ft_transfer_call` to `REG` with `{"fund_allowance": ...}` or a work payment, or calls `REG.ft_on_transfer` directly.
- Impact: fake allowance credit, then real mock USDC paid out of `REG` to vaults; fake payment events.
- Mitigation: `ft_on_transfer` checks `predecessor == usdc` (the token account fixed at init, no setter). Otherwise it returns the full amount and records nothing. Unknown or malformed `msg` also returns the full amount.
- Test: test 19 (fake token funds allowance: allowance unchanged; direct call: nothing recorded; malformed msg: full refund).

### T19. Callback failure or gas exhaustion leaves a lock stuck
- Attack or fault: `on_claim` or `nft_resolve_transfer` runs out of gas or panics, so `payout_in_flight` or `transfer_pending` is never cleared; the share is frozen forever, or a griefer forces this by attaching just enough gas.
- Impact: share cannot be claimed or transferred; funds stuck.
- Mitigation: (a) methods that schedule a callback assert enough prepaid gas up front and reserve a fixed amount for the callback; (b) callbacks are small, bounded, do no loops over user-sized data, and never panic (they use `#[callback_result]` and a size-checked result read); (c) each lock stores the block height it was set; after `STALE_LOCK_BLOCKS` anyone may call `release_stale_lock(token_id)`, which clears the lock WITHOUT rolling back any debit and bumps the ledger epoch, then a `sync` reconciles the known balance with the real vault balance. Clearing without rollback can never pay twice: if the withdraw happened the money is gone and the debit is right; if it did not, `sync` finds it again. `STALE_LOCK_BLOCKS` is 1200 (block 3); whether it is long enough under congestion is UNVERIFIED. Callback gas (review M1): measured in the sandbox at about 2.2 Tgas for `on_forward` (reserved 10, raised from 5; each forward reserves 5 + 10 Tgas since audit F1, where the token transfer reservation came down from 8 so that a 16-share payment fits in 300 Tgas; the token transfer measured 1.7 Tgas, and if it ever runs out the forward fails into `undelivered` and `retry_forward` sends it again; a call with 100 Tgas, as the deploy plan uses for agents, pays a group of at most 5 shares: sandbox `t13b_agent_group_size_at_100_tgas`, review 03a L4), 3.5 for `on_vault_ready` (20), 2.5 for `on_sync` (5) and 2.3 for `on_claim` (10), with a 16-share fan-out fitting one 300 Tgas payment (sandbox `a04_full_group_of_16_is_paid_with_300_tgas`, audit F1; before the fix 16 forwards needed 298 Tgas inside `ft_on_transfer` and were refused at 300 Tgas). Not covered: if `on_forward` itself never commits, `inbound_pending` stays above zero and `sync` on that share is refused; `release_stale_lock` does not clear it, because without a recorded forward time and outcome the registry cannot tell whether the money arrived. The registry keeps its counts consistent (the amount stays in `forwards_in_flight_total`), and the 10 Tgas reservation is about 4 times the measured use, so this needs a callback that fails for a reason other than gas.
- Test: test 20 (unit: callbacks given a failed promise result do not panic; claim with too little gas fails before any state change; stale lock release before the timeout fails, after it succeeds; sync then restores the correct balance).
- Residual (audit F3, documented, not fixed in code): when a stale payout lock is released and the vault withdraw had in fact failed, the claim's debit is not rolled back. `total_paid_out` keeps the amount that never left the vault, and the next `sync` puts the same amount back into the known balance, so the share's `earned` (known balance plus paid out) and `total_paid_out` are both overstated by that amount for good. No money moves and nobody can take the amount: it is still in the vault and the current holder can claim it. The `anomaly` event `payout_lock_released_stale` marks the share, so a reader can correct the history by hand. Not fixed because the registry cannot tell, when the callback never ran, whether the withdraw happened; telling the cases apart would need the vault to report or a second balance read with a recorded expected balance. This path needs a callback that never commits, which has not been produced in the sandbox (unit tests only).

### T20. Vault account deleted, redeployed or keyed later
- Attack: someone deletes `s<N>.REG` (balance goes to a beneficiary), redeploys it, or adds a key.
- Impact: total loss of the share's money.
- Mitigation: vault has zero keys; vault contract has only `withdraw` and no method that issues `DeleteAccount`, `DeployContract` or `AddKey`; `REG` has no method that targets an existing vault with anything except a `withdraw` function call. `withdraw` only calls `ft_transfer` on the token argument, so it cannot call `storage_unregister` either. Relies on the runtime rule that a parent cannot act on an existing sub-account (shown in the sandbox by test 21). New vaults also export `ft_on_transfer`, which returns "0" and does nothing else (T21). Exception to the facts table row on `ft_on_transfer` (review 03a L2): the vault does not check the predecessor, because it is stateless and does not know the token id. Any NEP-141 token sent with `ft_transfer_call` is kept; only Mock USDC can be withdrawn (the registry always passes its own `usdc`), so another token sent that way stays in the vault for good. No ledger effect: the vault writes nothing and the registry only reads the Mock USDC balance.
- Test: test 21, sandbox `t21_parent_cannot_delete_redeploy_or_key_a_vault` (`REG`'s account signs batches with `DeleteAccount`, `DeployContract` and `AddKey` on an existing vault; all fail; code hash unchanged; key list empty; balance intact). Testnet vault key lists and code hashes are checked by `npm run verify:testnet`.

### T21. Sync race with claim or with an inbound forward
- Attack or fault: `sync` reads the vault balance while a claim or a routed payment is moving; the callback then writes a stale balance, double-counting an inbound payment or resurrecting money already paid out.
- Impact: claimable shows more than the vault holds (claim fails) or earned history is wrong.
- Mitigation: every ledger change bumps a per-share `epoch`, and so do opening a dispute (for each contested share and the claimant) and `nft_transfer_call` (review L4, Security Suite F-04). `sync` panics if `payout_in_flight`, `transfer_pending` or `inbound_pending > 0`, and only the current holder or the registrar may call it (review H1), so nobody can time a balance read against someone else's vault. `on_sync` discards the result (emits `sync_discarded`) if the epoch changed, if the share is now in a dispute, or if `transfer_pending` is set. If a valid sync sees less than the known balance, it sets the known balance to what it saw and emits `anomaly`.
- Phantom balance (review H1): `ft_transfer_call` into a vault with no `ft_on_transfer` is refunded a block later, and a sync read in between saw money that then left. New vaults export `ft_on_transfer` returning "0", so the token keeps the transfer and there is no refund to race. The 7 vaults created on testnet before this fix keep the old code (vault code cannot change, T20); there, once the branch registry is deployed (DEPLOY_PLAN step 15, not run yet; until then anyone can sync on testnet and this race is open live, review 03a H2), only the holder (or the registrar) can sync, so the holder could at most race its own vault, which reaches only its own share's balance: a claim of the inflated figure fails cleanly (T17 rollback) and the next sync corrects it. If the holder inflates the figure and a dispute then opens on that share, it can claim a plain deposit that T24 would otherwise hold until resolve; T24 credits that deposit to the same vault's holder after resolve anyway, so this moves timing, not ownership. No other share's money and no escrow is reachable. The figure shown in the UI should be the lower of the known balance and the vault's real balance (front-end change, not done in this branch).
- Test: test 22 (sync during in-flight claim refused; sync started then a forward lands: result discarded; earned equals real inflow exactly across a mixed sequence); unit tests `h1_sync_is_holder_or_registrar_only`, `l4_inflight_sync_across_dispute_open_is_discarded`, `l4_sync_does_not_land_during_transfer_pending`; sandbox `r01_h1_outsider_cannot_sync_and_new_vaults_keep_transfer_call`.

### T22. effective_from abuse
- Attack: seller back-dates or forward-dates `effective_from` on a transfer to claim income for a period after the sale, or to confuse history.
- Impact: misleading history; in production, wrong period allocation.
- Mitigation: in this demo `effective_from` is recorded only. Payout logic ignores it: the holder at claim time gets the whole vault balance. Bounds: defaults to the block timestamp; must be at least the previous transfer's `effective_from` for that token and at most block timestamp plus `MAX_EFFECTIVE_FUTURE_NS` (design parameter). README says period allocation is roadmap.
- Test: test 23 (out-of-range values panic; a past `effective_from` does not change who can claim).

### T23. Suspended credential edge cases
- Attack or fault: (a) suspended member keeps receiving tokens; (b) a suspended organisation's agents keep paying or the org keeps funding; (c) suspension is used to seize property; (d) `nft_transfer_call` returns a token to a sender who was suspended meanwhile; (e) suspended member confirms shares or registers works.
- Impact: compliance bypass, or loss of property by administrative action.
- Mitigation: (a) receiver must be active; (b) agent methods check the organisation's credential is active; `fund_allowance` from a suspended org is refunded; (c) suspension never moves a share by itself; the share keeps accruing. Owner decision 3 (2026-11-13): while suspended the holder can neither claim nor transfer out nor receive; only the registrar can move the share, with `registrar_transfer`, 1 yoctoNEAR, a document hash and an event, and only while the holder is suspended (so the registrar cannot move an active member's share); (d) the return path in `nft_resolve_transfer` is allowed, because it restores the prior state and the receiver cannot keep a token it refused; (e) register, confirm and attest methods require an active credential.
- Test: test 15 (suspended member keeps share, accrues, cannot receive); block 2 unit test `suspended_holder_accrues_but_cannot_claim_send_or_receive` and sandbox test `d3_suspended_holder_frozen_registrar_can_move`; test 24 (suspended holder claims; transfers out; suspended org's agent blocked; return to suspended sender works; suspended member cannot register or confirm).

### T24. Plain transfer straight into a contested vault during a dispute
- Attack: a payer (or the contested holder's friend) sends USDC with a plain `ft_transfer` directly to a contested vault, skipping the registry's escrow, and the holder claims it at once.
- Impact: dispute freeze bypassed for direct payments.
- Mitigation (block 4, replaces the snapshot plan): `sync` is refused on a share in an open dispute, so the ledger only knows money routed through the registry (which goes to escrow for contested shares) plus what it knew before. A plain deposit straight into a contested vault stays in the vault, unattributed and unclaimable, until resolve; then `sync` picks it up for the vault's holder whatever the outcome. Unsynced plain deposits made before the dispute opened are held the same way (conservative). A sync already in flight when the dispute opens is discarded (T21, review L4). Production payers pay through the registry; a plain transfer straight into a vault is not split or escrowed by the registry's math, it is credited to that vault's holder by `sync`.
- Test: sandbox `t14_over_allocation_dispute_freezes_only_contested_fraction` (plain deposit into the contested vault held, sync refused, old balance claimable, after resolve sync and claim pick it up).

### T25. Payment to an under-allocated or over-allocated group
- Attack or fault: a payer or agent pays a work group whose active shares sum below 100 percent (unallocated part has no owner) or above (no dispute recorded yet because of a pending mint).
- Impact: money credited to nobody, or over-credited.
- Mitigation (owner decision 4, 2026-11-13): above 10000 bps a group payment is refused (`ft_on_transfer` returns the full amount; `pay_from_allowance` will panic before any debit). Below 10000 bps each registered share gets exactly floor(amount * bps / 10000) and everything else goes to the group's unallocated escrow, held in `REG`'s token balance and counted in the solvency sum, so no share is over-credited. Only the registrar can release it, to an active share of the same group, with a document hash and an event; the escrow is debited before the forward. Pending shares reserve their bps for the 100 percent check so two parallel registrations cannot both slip under the cap.
- Test: unit test `under_allocated_group_pays_exact_bps_and_escrows_the_rest`, sandbox test `d4_under_allocated_payment_escrows_unassigned_part`; test 26 ( two parallel registrations that together exceed 100 percent open a dispute instead of both minting clean).

### T26. Forward from registry to a vault fails
- Attack or fault: after the registry receives a group payment, one of its `ft_transfer` calls to a vault fails.
- Impact: money stuck in `REG` with no record, or credited in the ledger but never delivered.
- Mitigation: forwards are counted in `inbound_pending` and settled in `on_forward`. On failure, the amount goes to that share's `undelivered` balance (still physically in `REG`'s USDC balance) and `undelivered_total`. Anyone may call `retry_forward(token_id)`, which sends it once (debit before call, roll back on failure).
- Test: test 27 (forced failure goes to undelivered; retry delivers exactly once; a second retry sends nothing).

### T27. Agent storage spam
- Attack: an agent submits many tiny payments with fresh statement line ids, growing the idempotency set paid for by `REG`.
- Impact: registry storage balance drained.
- Mitigation: `max_tx_per_day` in the role; minimum payment `MIN_GROUP_PAYMENT`, for group and share payments alike (review L8); statement line ids at most 64 bytes and stored as a 32-byte hash; organisation prepays statement-line storage at `grant_role` (100 bytes per line) and each paid line uses one credit; with no credits left the agent cannot pay (block 3).
- Test: test 28 (call over `max_tx_per_day` fails; payment below minimum fails).

### T28. Role and allowance namespace abuse
- Attack: (a) a member grants an agent role to an account outside its namespace (for example another member's account or a vault); (b) an agent grants itself a role or funds itself; (c) someone funds another organisation's agent to make it look active; (d) an agent account is also given a credential so it can hold shares.
- Impact: confusing accountability; an account that is both agent and holder weakens T6.
- Mitigation: `grant_role` requires the agent to be a direct sub-account of the caller (`agent == "<label>.<caller>"`), caller active member, 1 yoctoNEAR. `fund_allowance` credits only if `sender_id` is the agent's organisation; otherwise full refund. `grant_credential` refuses accounts that hold an agent role, and `grant_role` refuses accounts that hold a credential.
- Test: test 29.

### T29. Token sent to the registry, a vault or the token contract
- Attack or fault: holder transfers the NFT to `REG`, to its own vault or to another system account, so nobody can ever claim.
- Impact: share bricked.
- Mitigation: `grant_credential` refuses `REG`, any account ending in `.REG`, and the token account, so they can never be receivers.
- Test: test 30 (credential grant to these accounts fails; transfer to them fails; registrar cannot claim or transfer a share it does not own).

### T30. Dust payments bias the remainder
- Attack: an agent or payer splits one payment into many tiny ones so the remainder repeatedly lands on the largest share, or small shares get zero.
- Impact: small, systematic skew.
- Mitigation: `MIN_GROUP_PAYMENT` (design value 10000 base units, which is 0.01 mock USDC, so every 1 bps maps to at least 1 base unit); the skew per payment is under one base unit per share and the rule is public. Largest-remainder rounding is listed as an alternative (pending owner).
- Test: test 13 (below minimum fails) and test 28.

### T31. Registry solvency drift
- Attack or fault: a bug credits allowance, escrow or undelivered balances that `REG` does not actually hold.
- Impact: later payouts from `REG` fail or pay one party with another's money.
- Mitigation: invariant `allowance_total + unallocated_total + escrow_total + undelivered_total + forwards_in_flight_total <= ft_balance_of(REG)`. Every debit happens before a transfer, every credit after money arrived.
- Test: invariant asserted at the end of every sandbox test (helper `assert_solvent`). On testnet `npm run verify:testnet` checks the tracked counters against the registry's mock USDC balance.

### T32. Note and metadata used for injection or oversize griefing
- Attack: holder sets a share note (holder power) containing script, a fake payee or a huge string.
- Impact: UI injection, agent confusion, storage growth.
- Mitigation: note max 280 bytes, caller pays storage, 1 yoctoNEAR; the UI renders it as text; agents never read payees from it (T8).
- Test: test 11 (injected payee does nothing) and test 31 (oversize note fails; non-holder cannot set note).

### T33. License NFT used as a share
- Attack: a license NFT (same contract, `kind = license`) is transferred, claimed against or counted in splits.
- Impact: confusion between a paid license and a rights share.
- Mitigation: license tokens have no vault, cannot be claimed and are non-transferable in the demo; split math reads only `kind = share`.
- Test: test 17 (only if licenses are built).

### T34. Stray plain transfers to the registry (owner decision 10)
- Attack or fault: someone sends mock USDC to `REG` with a plain `ft_transfer` (no `msg`), by mistake or to bait a sweep.
- Impact: those tokens sit in `REG` unattributed.
- Mitigation: by design there is no sweep or admin withdraw. No method moves registry funds except the tracked paths (forwards, claims from vaults, allowance withdrawals, escrow and unallocated releases, unallocated refunds of a void work, undelivered retries), each bounded by its own counter. Stray tokens stay stuck and only show up as `REG` balance above the solvency sum. This removes a registrar power and the race between a balance read and in-flight forwards.
- Test: none needed (absence of a method); the solvency check reads the surplus.

### T35. Registrar powers added by decisions 3 and 4
- Attack: the registrar uses `registrar_transfer` to take an active member's share, or `release_unallocated` to send escrow to the wrong share.
- Impact: misdirected property or money.
- Trust: the registrar is trusted (T12). The rules below bound HOW it acts; they do not stop a registrar acting in bad faith with an accomplice member from choosing the receiver or the share.
- Mitigation: `registrar_transfer` works only while the current holder's credential is suspended, needs an active receiver and an approved share, respects both locks, needs 1 yoctoNEAR and a document hash and emits an event. `release_unallocated` can only pay an active, approved share of the same work, right and territory, never more than that group's escrow, and emits an event with the document hash. `refund_unallocated` works only once the work is Void and can send at most that group's escrow, to any account named by the registrar except itself (review L9). `recover_rejected_vault` works only on a rejected share whose ledger is empty, and not to the registrar. The registrar cannot hold a credential, so it cannot claim or receive a share (review H3). Multisig with a time delay is roadmap (T12).
- Test: unit tests `registrar_cannot_move_active_holders_share`, `unallocated_release_must_stay_in_its_group`, `l9_registrar_refunds_unallocated_of_a_void_work`, `m2_registrar_recovers_plain_deposits_of_a_rejected_vault`, `h3_registrar_cannot_hold_a_credential`, and the registrar-only checks inside the decision 3 and 4 tests.

### T36. Late callback after a stale lock release
- Attack or fault: `release_stale_lock` clears a lock, then the original `on_claim` or `nft_resolve_transfer` finally runs; or a new lock was set in between.
- Impact: a late claim rollback after a `sync` would count the same money twice; a late resolve could move a token that has since been sold, or clear a newer lock.
- Mitigation: callbacks carry the lock they belong to (`since_block` plus amount or receiver). If the lock is gone or different, `on_claim` changes no balance and emits an `anomaly`; `nft_resolve_transfer` returns true and leaves owner and locks alone. Balances after a stale release come only from `sync`, which reads the vault.
- Test: unit tests `stale_payout_lock_released_only_after_timeout_and_late_callback_is_safe`, `late_failed_claim_after_stale_release_changes_no_balance`, `late_resolve_after_stale_release_keeps_current_owner`.

### T37. Agent pays a valid but wrong target
- Attack or fault: a misled agent pays a real, registered work or share that the statement line does not belong to.
- Impact: money reaches registered holders of the wrong song; there is no clawback.
- Mitigation: the money can only reach registered vaults (T8); per-transaction, rolling daily and count limits cap the damage (T7, T27); every payment event carries the agent, statement line id, period and statement hash, so the organisation can reconcile and revoke at once (T14). Refunds between members are off chain. Residual risk, accepted for the demo.
- Test: tests 9 and 10 (limits and revoke); test 11 (no payee outside the registry).

### T38. Dispute powers used to seize or misroute escrow
- Attack: a member opens a frivolous over-allocation claim to freeze rivals; or the registrar resolves to redirect escrow; or a claimant whose mint fails leaves contested shares frozen forever.
- Impact: income frozen or misdirected.
- Mitigation: only the contested shares are frozen and only for new money; the claimant must name shares whose bps cover the excess, and no more than needed (T11, review H2); the claimant and the caller cannot name their own shares; one open dispute per group. `uphold_existing` removes the voided claimant from the group's list, and a share left with 0 bps after `uphold_claimant` is removed too. `resolve` can only send escrow to the dispute's own parties, by bps that keep the group total unchanged, with 1 yoctoNEAR, a document hash and an event. A failed claimant mint cancels the dispute and moves escrow to undelivered for `retry_forward`. Challenger bond, expiry and appeal stay roadmap (T11, T12).
- Test: unit tests `over_allocation_rules`, `uphold_claimant_splits_escrow_by_new_bps`, `failed_claimant_mint_cancels_dispute_and_escrow_goes_to_undelivered`, the `h2_` review tests; sandbox `t14b_uphold_existing_voids_claimant` (non-registrar resolve fails).

### T39. Share registered on someone else's work (on-chain split sheet, review M2)
- Attack: a member registers a share (up to 10000 bps) on a work another member registered, before the real owners do, and collects its payments; the real owner could only open a dispute while the money already paid stayed claimable.
- Impact: income taken by whoever registers first.
- Residual (review 03a M1): the control moves the first-registrant advantage from shares to works. The signer is `works[work_id].registered_by`, fixed at work registration. (a) Whoever registers an ISRC or ISWC first is the signer, so its own shares are approved at once; the real owner's later registration of the same code opens a DuplicateCode dispute that freezes nothing on the first work (T11), so the first work keeps being paid until the registrar resolves. (b) After a catalog sale, a seller that registered the work keeps the signer power: it can register auto-approved shares into unallocated bps and approve or reject other members' pending shares. It cannot touch the sold share or its vault. Code options, not built: the signer follows the current holders, registrar co-signature for shares registered after any transfer on the work, or holding the first work's payments while a duplicate-code dispute on its code is open.
- Control: a share registered by anyone other than the work's registrant is `pending` approval. While pending it is minted (vault and NFT exist, its bps count toward the 100 percent check) but it is not paid: its part of every group payment and every direct payment is held in the registry's escrow (`escrowed` event, `reason: awaiting_approval`). It cannot be claimed, synced, transferred, confirmed, attested, released to or named in `contests`. The work registrant signs the split sheet with `approve_share` (1 yoctoNEAR): the held money goes to the share's vault and the share is paid normally from then on. `reject_share` voids the share, frees its bps and moves its held money to the group's unallocated escrow, where the registrar releases it to the real owner's share once registered (T25). The registrar is the backup signer for both, with a document hash. A plain deposit sent straight into a rejected share's vault can be sent back by the registrar with `recover_rejected_vault`. Events: `share_approved`, `share_rejected`, `rejected_vault_recovered`; `share_registered` carries `approval` and `registered_by`; `get_share_approval` and the token's `extra` show the state. Shares registered by the work registrant, dispute claimants (approved by the registrar's resolve) and every share that existed before this rule are approved. Approval is kept in its own storage slot (`apv:<token>`), so existing records are unchanged and the deployed registry can be upgraded in place.
- Test: unit tests `m2_share_by_non_registrant_is_held_until_approved`, `m2_reject_voids_share_and_keeps_money_in_the_group`, `m2_registrant_shares_and_old_shares_are_approved`, `m2_registrar_recovers_plain_deposits_of_a_rejected_vault`, `m2_void_work_moves_held_money_to_unallocated`; sandbox `r02_m2_split_sheet_approval_and_rejection`.

## Test mapping summary

| Test | Covers |
|---|---|
| 1 | T1, T15 |
| 2 | earned accounting (sync), T21 baseline |
| 3 | T2 |
| 4 | T2, balance travels with token |
| 5 | T10 |
| 6 | T3 |
| 7 | T4 |
| 8 | T6 |
| 9 | T7 |
| 10 | T14 |
| 11 | T8, T32 |
| 12 | T9 |
| 13 | T16, T30 |
| 14, 14b | T11 |
| 15, 15b | T15, T23 |
| 16 | T5 |
| 17 | T33 (licenses, cut first) |
| 18 | T17 |
| 19 | T18 |
| 20 | T19 |
| 21 | T20 (sandbox `t21_parent_cannot_delete_redeploy_or_key_a_vault`) |
| 22 | T21 |
| 23 | T22 |
| 24 | T23 |
| 25 | T24 |
| 26 | T25 |
| 27 | T26 |
| 28 | T27, T30 |
| 29 | T28 |
| 30 | T12, T29 |
| 31 | T32 |
| `npm run verify:testnet` | T1, T13, T20, T31 (live, read only) |
| `assert_solvent` helper | T31 |
| `review_fixes.rs`, `review.rs` | T7, T11, T12, T15, T19, T21, T24, T27, T35, T38, T39 |

Never cut: T1 to T5 and tests 1, 4, 5, 6, 7 and the zero-keys proof.

## Block 2 status (2026-11-13)

Passing in the sandbox (`contracts/tests/tests/sandbox.rs`): test 1 (T1, T15), 2, 3 (T2), 4 (T2, catalog sale with zero keys before and after), 5 (T10), 6 (T3), 7 (T4, claim and onward transfer both tried during `nft_on_transfer`), 16 (T5), T16 split exactness, decision 3 (T23, T35), decision 4 (T25, T35). Every sandbox test ends with the solvency check (T31). Unit tests in `contracts/registry/src/tests.rs` and `logic.rs` cover the same paths with mocked callbacks, plus failed vault creation and failed token registration (T15, part of test 15b), failed claim rollback (T17 path), failed forward to undelivered (T26), stale sync discard (T21) and `effective_from` bounds (T22).

Test 6 uses a holder contract that sends `claim` and `nft_transfer` in one receipt, so the transfer lands while the payout is in flight; this ordering is deterministic in the single-shard sandbox and the test asserts which call won, so a change in ordering fails loudly.

Not yet covered: tests 8 to 15, 17 to 31 except the unit-level parts above (blocks 3 and 4), and the `verify-keys` and `verify:testnet` scripts (block 5).

## Block 3 status (2026-11-13)

Passing in the sandbox (`contracts/tests/tests/agents.rs`), every test ending with the solvency check (T31):
- test 8 (T6): an agent account with only function-call keys. A deposit from either key is rejected by the protocol (`DepositWithFunctionCall`); a key allowing any registry method, without a deposit, is refused by each method (`nft_transfer`, `claim`, `withdraw_allowance`, `revoke_role`, `register_share`, `grant_role`, `set_share_note`); the scoped key cannot name them (`MethodNameMismatch`); it cannot call the token at all.
- test 9 (T7), test 10 (T14), test 11 (T8, T32: injection in the work metadata, in the share note and as extra payee arguments; outsider receives nothing), test 12 (T9, also across target forms), test 13 (T16 through the allowance, minimum payment), test 15 (T23, T28: suspended organisation's agent stopped, share accrues, cannot receive, resumes after reinstatement).
- payer attestation sets `payer_confirmed`; `release_stale_lock` and `retry_forward` refuse when there is nothing to do.

Unit tests cover the role checks (T28), limits and the rolling window, line credits (T27), failed allowance withdrawal, note rules, retry of an undelivered forward (T26), stale lock release only after the timeout (T19) and late callbacks (T36). A stuck lock and a failed forward cannot be produced in the sandbox, so those paths are unit-tested only.

Not yet covered: tests 14, 17 to 31 beyond the unit-level parts (block 4), counterparty confirmation, licenses, and the live `verify-keys` and `verify:testnet` scripts (block 5).

## Block 4 status (2026-11-13)

Passing in the sandbox (`contracts/tests/tests/disputes.rs`), each ending with the solvency check (T31) and, where a vault is involved, the zero-keys proof:
- test 14 (T11, T24): a claimant above 100 percent contests one share; the uncontested share keeps being paid; the contested share's new money is escrowed in the registry; claimant and contested share cannot transfer; a plain deposit into the contested vault is held and `sync` is refused; old money stays claimable; `uphold_claimant` splits the escrow 2500/2500; the held deposit becomes claimable after resolve; the next payment splits exactly over the new bps.
- test 14b: `uphold_existing` voids the claimant (cannot move, never paid), returns escrow, and only the registrar can resolve (T38).
- test 14c (T11 duplicate path): payment to a duplicate ISRC is refused and refunded; `swap_payable` flips the payable entry; the voided share keeps what it already earned.
- `confirm_share`: counterparty_confirmed, dropped after the confirming holder's share is sold.

Unit tests also cover: contests validation, default contests, a second claim while a dispute is open, `keep_both`, `void_duplicate`, voiding a work only after its share dispute, failed claimant mint (dispute cancelled, escrow to undelivered, retry_forward), Void shares still syncing and claiming, `split_by_total`.

Not yet covered: tests 17 to 31 beyond the unit-level parts, licenses (cut first), AUDIT.md (the audit skill named in the brief is not available here), and the live `verify-keys` and `verify:testnet` scripts (block 5).

## Review fixes status (2026-11-13)

Branch `fix/contract-review`, on testnet since step 15 (2026-11-13 18:54 UTC). The contracts review findings and Security Suite F-04 are fixed in code, and F-12 in these docs, except the items listed at the end of this section; see the rows marked "review". Regression tests: `contracts/registry/src/review_fixes.rs` (unit, converted from the review's proofs of concept) and `contracts/tests/tests/review.rs` (sandbox: H1, M1 gas, M2, test 21). Other fixes covered there: payer_confirmed only counts while the attesting account still has the payer role, and payer accounts cannot be agents (review L5); a holder suspended before mint gets no NFT and the share fails (review L6).

Not fixed in code, stated here instead: a lost `on_forward` keeps that share's `sync` refused (T19); vaults created before the fix have no `ft_on_transfer` (T21); the full-access keys on `REG` and mock USDC are still present (T13); the registrar is trusted (T12).

## Review 03a status (2026-11-13)

Branch `fix/review-03a`, on testnet since step 15 (2026-11-13 18:54 UTC). Documented here as residual risk: M1 first registrant (T39), M2 contest size (T11), L2 vault `ft_on_transfer` (T20), L3 failed mint vaults (T15), L4 agent group size at 100 Tgas (T19), L6 registrar deposit and document hash wording (T12), H2 deployed state (T21). New tests: sandbox `u01_testnet_wasm_upgrades_in_place_without_migration` (M3: the exact testnet registry wasm, state, in-place upgrade, read back, then claims, a dispute resolve and a new mint), sandbox `t13b_agent_group_size_at_100_tgas` (L4), unit `r03a_l5_approve_before_any_payment_sends_nothing` (L5), unit `r03a_l4_forward_gas_reservation_per_share` (L4). `npm run verify:testnet` now asserts every token owner against `token_owners` in the record and fails on a missing counter (L1). Full map: `docs/reviews/03a-status.md`.

