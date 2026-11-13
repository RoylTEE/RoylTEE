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
