# Review 03d status

Branch: `fix/review-03b` (site), `fix/review-03a` (contract test for L7). Local only, not pushed, not merged. The full 03d report file was not available here; this maps the reviewer's summary.

| Item | Verdict | Status |
|---|---|---|
| Correction to 03c M3 | Accepted | `/member` is in `WALLET_ROUTES`; rendering is text only. Residual risk is the third party wallet code (03b M1). |
| M1 claimable can be inflated | REAL | `chain.ts` caps each token's claimable at the vault's real mock USDC balance (`claimable.ts`, `capClaimable` on every token loader); the ledger figure is kept as `ledger_claimable`. Test in `site/tests/review-03b.test.ts`. |
| M2 draft flow on the live registry | REAL | Text now leads with: on the registry deployed today each new share is paid as soon as it is minted. Right before signing the page re-reads `get_group` for each role (active, pending and claimant basis points) and refuses if the draft would exceed 100 percent. Explicit `contests` are not sent: a draft never means to open a dispute, and on both builds a `contests` list is refused unless the group is over 100 percent, so refusing is the only safe choice. Residual: a registration that lands between the re-read and the signature can still push the group over on the deployed build (documented). |
| L1 "sent" without checking the outcome | REAL | `tx-outcome.ts` reads the wallet's result: failure in the transaction or any receipt is reported as failed with the panic text; a missing result is reported as not confirmed. Used by claim, transfer, approve or reject, register and the agent controls. |
| L2 agent account keys | REAL | Agent controls tell the organisation to create the agent with a function call key only (receiver the registry, `pay_from_allowance` and `report`) and delete any full access key. |
| L3 two test names overclaim | PARTLY (exact two UNVERIFIED without the full report) | Renamed "only view calls are made outside the wallet module" and "every change call from Your shares attaches a deposit" to what they check. |
| L4 `/dashboard.js` still served | REAL | Not changed: removing it needs a file delete. Queued for the owner's yes: `git rm site/public/dashboard.js site/src/content/dashboard.html` plus the test line in `site/tests/review-fixes.test.ts` that reads them. |
| L5 verify page false "Differs" after redeploy | REAL | Vault check accepts the current or an earlier recorded vault build (`DEPLOY_RECORD.earlierVaultCodeHashes`), and a test compares `DEPLOY_RECORD` with `deployments/testnet.json` whenever that file is present (after the merge). |
| L6 CSP route test can miss wallet pages | REAL | New test follows imports of the wallet module through other scripts, walks nested page folders, and requires layouts and components not to load wallet code. |
| L7 grant_role deposit | Checked | Code: role storage is measured and charged, the rest buys lines at 0.001 NEAR (`LINE_STORAGE_BYTES` = 100), any fraction refunded. Unit test `r03d_l7_grant_deposit_of_site_covers_role_storage` (branch `fix/review-03a`): 0.01 NEAR covers role storage with 2 spare lines; 0.015 NEAR buys at least 5 lines. Measured on the branch code in the unit test environment; on the deployed build UNVERIFIED. |

Must fix list from the reviewer: merge (plan in the final report), redeploy decision (recommendation: redeploy after the merge, the in place upgrade test `u01` exists), vault money headline (reworded on `fix/review-03a`), hosted site check (owner's yes, steps in the final report).

## Second reviewer, part 03d

Tests: `site/tests/review-03d-model2.test.ts` (19 tests). Site total on this branch: 219 tests in 18 files.

| Item | Verdict | Change |
|---|---|---|
| 1 Draft claims approval protection that the deployed build lacks | REAL | `approvalLive()` asks the registry for `get_share_approval`. If the method is missing (as on the deployed build), the draft says each new share becomes active and is paid as soon as it is minted. Approval wording shows only when the method exists. |
| 2 Peer shares in the review card always show Approved | REAL | The review card shows each peer's approval as read (Approved, Waiting for approval, Rejected, or Not read). |
| 3 Dispute id 0 reads as no dispute; approval not defaulted | REAL | `hasDispute` checks only null or undefined, and `shareState` and `transferChecks` use it. `shareState` falls back to `extra.approval`. |
| 4 Split totals per right only | REAL | `groupTotals` totals per right and territory, and the footer names the territory. |
| 5 Claims figures mixed; "claimed down to zero" | REAL | `breakdown` separates the vault balance, claimable (the lower of ledger and vault), unsynced and escrow held. The claims page shows unsynced and held figures. The "claimed down to zero" wording is removed from claims and overview. |
| 6 Draft lists only works that already have shares | REAL | `allWorks()` reads works by id from 0 until the first missing id, so a work with no shares can get its first share. |
| 7 Keyless shown for accounts with function call keys | REAL | `keyStatus` shows Keyless only when the account has zero keys. Function call keys are listed with their receiver and methods. |
| 8 Integrated ref and README counts | REAL | Handled in the merge plan. Counts are taken on the merged tree after the owner merges. |
| 9 Claim tied to transfer rules (void cannot claim) | REAL | `canClaim` follows the contract's claim checks (holder, not suspended, no transfer pending, no payout in flight, balance above zero). Share status is not a condition, so a void share keeps its claim. |
| 10 Holder tokens and disputes read one page only | REAL | `tokensFor` and `allDisputes` page by 50 until a short page. |
| 11 Fallback storage price shown as live | REAL | `storagePriceOrEstimate` reports whether the price was read live. The share card and accounts page label the fallback as an UNVERIFIED estimate. |
| 12 Slow share read can overwrite a newer selection | REAL | A selection generation per target means a stale read does not render. |
| 13 Thousands separators in input defaults | REAL | `musdcInput` writes input defaults without separators. |

Not changed: the "credential suspended" check on the member claim button passes `suspended: false`, because the page does not read the holder's credential before showing the button. The contract still refuses a suspended holder, and the outcome text reports the refusal.
