# Transfer threat model

Status: written on 2026-11-13, BEFORE any contract code existed in this repo. Block 2 code landed later the same day; rows covered by passing tests say so under "Block 2 status" at the end. Owner decisions 3, 4 and 10 (2026-11-13) changed T23 and T25 and added T34 and T35; those edits are marked.

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
| A function-call key can only call one contract (optionally a list of methods) and cannot attach NEAR to the call. | docs.near.org, Access Keys page, section "Function-Call Keys" | Verified 2026-11-13 |
| Requiring exactly 1 yoctoNEAR on a method means a function-call key cannot call it; ownership checks are still needed. | docs.near.org, Security, "Require one yoctoNEAR" | Verified 2026-11-13 |
| An account with all keys removed is locked: only its own contract can move assets, create sub-accounts or change its code. A locked account can still add keys if its contract has a method for it. | docs.near.org, Access Keys page, "Locked Accounts" | Verified 2026-11-13 |
| A callback always runs but can still fail (gas, panic, limits). Only the callback's own changes revert; it is not retried. Withdrawals should debit before the call. | docs.near.org, Security, "Callbacks can fail" | Verified 2026-11-13 |
| `ft_on_transfer` must check that the predecessor is the expected token contract and return the full amount otherwise. | docs.near.org, Security, "Fungible Token Implementation" | Verified 2026-11-13 |
| `nft_transfer` and `nft_transfer_call` require 1 yoctoNEAR; `nft_resolve_transfer` settles the call path. | NEP-171 text in the near/NEPs repo | Verified 2026-11-13 |
| A sub-account `x.REG` can only be created by `REG`. | Sharded NFT items draft (houseofstake/tla-contracts, docs/sharded-nft-items.md), "Naming" paragraph | Verified as stated in that draft |
| Once a sub-account exists, its parent cannot delete it, redeploy its code or add keys to it (only the account itself can). | NEAR runtime behaviour | UNVERIFIED in docs read so far. Proven or disproven by test 21 before we rely on it. |

## How to read the table

- Attack: what the adversary does.
- Impact: what they gain or what breaks if unmitigated.
- Mitigation: what the design does (see DESIGN.md for exact methods and fields).
- Test: test number from the test plan (1 to 17 are from the brief, 18 and up are added here), plus scripts. `verify-keys` is the script that lists access keys on `REG` and every vault. `verify:testnet` is the read-only live check.

## Rows from the brief (T1 to T16)

### T1. Seller keeps a key to the vault after sale
- Attack: the seller (or whoever minted) holds an access key on `s<N>.REG` and uses it to send the USDC out directly, or to delete the account.
- Impact: total loss of the share's money for every later holder.
- Mitigation: the vault is created by `REG` in a single batch of `CreateAccount`, `Transfer`, `DeployContract`, with no `AddKey` action, so it never has any key. The vault contract has no method that adds keys, deploys code or deletes the account. Holders never receive a key; control is a registry check, so there is nothing to hand over or rotate.
- Test: test 1 (mint, then `view_access_key_list` on the vault returns empty). Script `verify-keys` checks every vault on testnet, before and after the catalog-sale scene. `npm run verify:testnet` repeats it read-only.

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
- Mitigation: two locks. Protocol lock: the agent account holds only a function-call key scoped to `REG` with methods `pay_from_allowance`, `report`, `buy_license`; such a key cannot attach a deposit. Contract lock: every owner and admin method requires exactly 1 yoctoNEAR and an ownership or role check. Agents are never holders (agent accounts cannot receive a credential while they hold an agent role).
- Test: test 8 (agent calls to `nft_transfer`, `claim`, `withdraw_allowance`, `amend_split` and `grant_role` all fail). Whether near-workspaces can sign with a function-call key to show the protocol-level rejection is UNVERIFIED; if not, test 8 shows the contract-level rejection (0 deposit) and the method-list rejection, and the testnet scripted agent shows the protocol rejection on chain.

### T7. Agent overspends
- Attack: agent pays more than intended per transaction or per day.
- Impact: drains the organisation's allowance.
- Mitigation: per-agent `per_tx_limit`, rolling 24-hour `daily_limit` (24 hourly buckets) and `max_tx_per_day`, all checked in the contract before any transfer. Allowance is debited before the transfer.
- Test: test 9 (over per-tx fails; over daily fails; under both passes).

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
- Mitigation: only the contested fraction freezes and only new money. Shares the claimant does not name in `contests` keep getting paid. Money already in vaults stays claimable (up to the snapshot, see T24). A duplicate code freezes nothing on the original work; payments to the duplicate work are refused. Only credentialed members can register. Challenger bond and auto-expiry are roadmap, not code.
- Test: test 14 (only contested fraction escrowed; other shares paid; frozen share cannot transfer; resolve releases escrow). Test 14b (duplicate code: original work still paid; payment to duplicate refused).

### T12. Registrar abuse
- Attack: the registrar grants credentials to accomplices, suspends rivals, or resolves disputes unfairly.
- Impact: control of who can hold and how disputes end.
- Mitigation: the registrar cannot move vault money to itself, cannot transfer NFTs, cannot claim. Every registrar action needs 1 yoctoNEAR and emits an event with a document hash. Suspension never moves a share. Production: multisig council with a time delay (roadmap, README only).
- Test: README section plus test 30 (registrar cannot call `claim` or `nft_transfer` on a share it does not own). Event presence checked in tests 14 and 15.

### T13. Contract upgrade swaps the logic
- Attack: whoever holds a full-access key on `REG` redeploys a contract that pays them.
- Impact: total loss.
- Mitigation: no upgrade method in `REG` or the vault. After deploy and init, all full-access keys on `REG` are deleted (needs the owner's yes, it is a delete). Code hashes are published in `deployments/testnet.json` and checked against a local build. Vaults have no keys from birth.
- Test: script `verify-keys` (REG and vault key lists empty); `npm run verify:testnet` (code hash on chain equals the published hash). Reproducible build tooling for an exact hash match is UNVERIFIED and will be checked in block 5.

### T14. Revoked agent keeps acting
- Attack: agent keeps calling after the organisation revokes it.
- Impact: unwanted payments.
- Mitigation: `revoke_role` sets `revoked = true` in the same call; every agent method checks it first. Expiry checked too. The organisation also deletes the agent's function-call key (off-contract, recommended in the runbook).
- Test: test 10 (revoked agent's next call fails; expired role fails).

### T15. Storage griefing / unregistered vault loses USDC
- Attack: vault is not storage-registered with the token, so incoming transfers fail or the share cannot receive; or an attacker grows registry state cheaply.
- Impact: payments bounce or are lost; registry runs out of storage balance.
- Mitigation: `register_share` is two-phase. The share is `Pending` until the vault account is created AND `storage_deposit` on mock USDC succeeds; only then is the NFT minted. If either step fails, the share is marked `Failed`, no NFT exists, its bps reservation is released, and the unused deposit is refunded. Every state-adding call is payable and charges the caller for storage; free calls (`sync`, `retry_forward`, `release_stale_lock`) add no state. Bounded string lengths and per-group share cap.
- Test: test 1 (vault registered with mock USDC at mint); test 15b (forced registration failure leaves no NFT and refunds).

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
- Mitigation: (a) methods that schedule a callback assert enough prepaid gas up front and reserve a fixed amount for the callback; (b) callbacks are small, bounded, do no loops over user-sized data, and never panic (they use `#[callback_result]` and a size-checked result read); (c) each lock stores the block height it was set; after `STALE_LOCK_BLOCKS` anyone may call `release_stale_lock(token_id)`, which clears the lock WITHOUT rolling back any debit and bumps the ledger epoch, then a `sync` reconciles the known balance with the real vault balance. Clearing without rollback can never pay twice: if the withdraw happened the money is gone and the debit is right; if it did not, `sync` finds it again. `STALE_LOCK_BLOCKS` value: to be set in block 2; whether it is long enough under congestion is UNVERIFIED.
- Test: test 20 (unit: callbacks given a failed promise result do not panic; claim with too little gas fails before any state change; stale lock release before the timeout fails, after it succeeds; sync then restores the correct balance).

### T20. Vault account deleted, redeployed or keyed later
- Attack: someone deletes `s<N>.REG` (balance goes to a beneficiary), redeploys it, or adds a key.
- Impact: total loss of the share's money.
- Mitigation: vault has zero keys; vault contract has only `withdraw` and no method that issues `DeleteAccount`, `DeployContract` or `AddKey`; `REG` has no method that targets an existing vault with anything except a `withdraw` function call. `withdraw` only calls `ft_transfer` on the token argument, so it cannot call `storage_unregister` either. Relies on the runtime rule that a parent cannot act on an existing sub-account (UNVERIFIED, see facts table).
- Test: test 21 (sandbox: `REG`'s account, using a test-only batch built by the test harness, tries `DeleteAccount` and `DeployContract` on an existing vault and fails; vault code hash unchanged; key list empty). `verify-keys` on testnet.

### T21. Sync race with claim or with an inbound forward
- Attack or fault: `sync` reads the vault balance while a claim or a routed payment is moving; the callback then writes a stale balance, double-counting an inbound payment or resurrecting money already paid out.
- Impact: claimable shows more than the vault holds (claim fails) or earned history is wrong.
- Mitigation: every ledger change bumps a per-share `epoch`. `sync` panics if `payout_in_flight`, `transfer_pending` or `inbound_pending > 0`. `on_sync` discards the result if the epoch changed since the read began (emits `sync_discarded`). If a valid sync sees less than the known balance, it sets the known balance to what it saw and emits `anomaly`.
- Test: test 22 (sync during in-flight claim refused; sync started then a forward lands: result discarded; earned equals real inflow exactly across a mixed sequence).

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
- Mitigation: when a dispute opens, the registry schedules a snapshot `sync` of each contested vault; `claim` on a contested share is blocked until the snapshot lands, then capped at the snapshot balance (old money) until resolve. Direct deposits after the snapshot stay in the vault, visible, unclaimable until resolve. After resolve the cap is lifted for the vault's holder, whatever the outcome (documented limitation: production payers pay through the registry; direct transfers bypass escrow math).
- Test: test 25 (direct transfer during open dispute is not claimable; old balance is; after resolve all is claimable).

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
- Mitigation: `max_tx_per_day` in the role; minimum group payment `MIN_GROUP_PAYMENT`; statement line ids at most 64 bytes and stored as a 32-byte hash; organisation pays a storage deposit at `grant_role` sized for its daily cap (sizing measured in block 3).
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
- Mitigation: invariant `allowance_total + escrow_total + undelivered_total <= ft_balance_of(REG)`. Every debit happens before a transfer, every credit after money arrived. The live check script reads both sides.
- Test: invariant asserted at the end of every sandbox test (helper `assert_solvent`); `npm run verify:testnet`.

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
- Mitigation: by design there is no sweep or admin withdraw. No method moves registry funds except the tracked paths (forwards, claims from vaults, and later allowance, escrow and undelivered releases), each bounded by its own counter. Stray tokens stay stuck and only show up as `REG` balance above the solvency sum. This removes a registrar power and the race between a balance read and in-flight forwards.
- Test: none needed (absence of a method); the solvency check reads the surplus.

### T35. Registrar powers added by decisions 3 and 4
- Attack: the registrar uses `registrar_transfer` to take an active member's share, or `release_unallocated` to send escrow to the wrong share.
- Impact: misdirected property or money.
- Mitigation: `registrar_transfer` works only while the current holder's credential is suspended, needs an active receiver, respects both locks, needs 1 yoctoNEAR and a document hash and emits an event. `release_unallocated` can only pay an active share of the same work, right and territory, never more than that group's escrow, and emits an event with the document hash. The registrar still cannot claim or send money to itself. Multisig with a time delay is roadmap (T12).
- Test: unit tests `registrar_cannot_move_active_holders_share`, `unallocated_release_must_stay_in_its_group`, and the registrar-only checks inside the decision 3 and 4 tests.

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
| 21 | T20 |
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
| `verify-keys` script | T1, T13, T20 |
| `verify:testnet` script | T1, T13, T31 |
| `assert_solvent` helper | T31 |

Never cut: T1 to T5 and tests 1, 4, 5, 6, 7 and the zero-keys proof.

## Block 2 status (2026-11-13)

Passing in the sandbox (`contracts/tests/tests/sandbox.rs`): test 1 (T1, T15), 2, 3 (T2), 4 (T2, catalog sale with zero keys before and after), 5 (T10), 6 (T3), 7 (T4, claim and onward transfer both tried during `nft_on_transfer`), 16 (T5), T16 split exactness, decision 3 (T23, T35), decision 4 (T25, T35). Every sandbox test ends with the solvency check (T31). Unit tests in `contracts/registry/src/tests.rs` and `logic.rs` cover the same paths with mocked callbacks, plus failed vault creation and failed token registration (T15, part of test 15b), failed claim rollback (T17 path), failed forward to undelivered (T26), stale sync discard (T21) and `effective_from` bounds (T22).

Test 6 uses a holder contract that sends `claim` and `nft_transfer` in one receipt, so the transfer lands while the payout is in flight; this ordering is deterministic in the single-shard sandbox and the test asserts which call won, so a change in ordering fails loudly.

Not yet covered: tests 8 to 15, 17 to 31 except the unit-level parts above (blocks 3 and 4), and the `verify-keys` and `verify:testnet` scripts (block 5).
