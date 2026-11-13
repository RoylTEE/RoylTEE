# Review 03c status

Branch: `fix/review-03b` for site changes, `fix/review-03a` for README parts. Merged into the submission branch.

| Item | Verdict | Status |
|---|---|---|
| C1 (carried) | REAL | See merge plan. |
| M1 split sheet approval shown as live | REAL | Landing, /member approvals panel, splits page, member script and help notes now say approval comes with the planned registry update and the deployed build pays at once. |
| M2 power overclaims | REAL | Hero notes the remaining registry full access key; "Change a split alone" carries the registrar dispute exception; "Hold a credential" marked for the planned update; document hash claim limited to registrar actions that move a share or money (`sync` and `revoke_role` take none). |
| M3 /member under the wallet CSP | Check FALSE (no defect) | `WALLET_ROUTES` in `site/src/server/csp.ts` includes `/member` and tests enforce it. No `innerHTML`, `set:html` or `insertAdjacentHTML` in `site/src`; DOM is built with `textContent`. Explorer links now use `encodeURIComponent`. The relaxed CSP residual risk is the same as 03b M1. |
| M4 wasm not tied to a commit | REAL (a), FALSE (b) | (a) /verify and README say a rebuild may not match byte for byte; the deployed registry wasm is kept as a test fixture on `fix/review-03a`. (b) The registry uses `include_bytes!` for the vault only on wasm32; `npm run test:unit` passed on a fresh clone of `fix/review-03a` (`feb7746`) with no `contracts/res`: 109 passed. |
| L1 "all pages read live" | REAL | Help notes now list live values and recorded demo sections. |
| L2 "no payer role ever granted" | Claim TRUE | `is_payer` view for the payer account returns false; the deploy record has no payer grant. |
| L3 "Simulated payer" on the agent payment | REAL | Badge "Scripted agent"; test in `site/tests/review-03b.test.ts`. |
| L4 draft deposit and refund | PARTLY | /member states the `charge_storage` rule. Whether 0.2 NEAR always covers it is UNVERIFIED. |
| L5 method names and deposits | PARTLY | `add_line_credits` exists (agents.rs:139). `grant_role` has no fixed 0.01 NEAR fee: it charges the measured role storage and credits the rest as statement lines at 0.001 NEAR each (`LINE_STORAGE_BYTES` = 100); /agents now says that. The registrar's `sync` and `revoke_role` were added to the powers list, marked as taking no document hash. |
| L6 refused agent claim | REAL | Marked as seen in a local CLI log only on /activity, /agents and help notes. |
| I1 to I5 | Noted | I3 (footer TEE line) and I4 (stale "read on" copies after redeploy) deferred to the redeploy pass. |

## Second reviewer, part 03c

| # | Item | Verdict | Status |
|---|---|---|---|
| 1 | Undeployed restrictions shown as deployed (registrar, agents) | REAL | Registrar credential refusal and "not a payer" for agents labelled as after the planned registry update; "Claim from any share account" narrowed to shares it does not hold. |
| 2 | "Nobody can change a split alone" | REAL | Carries the registrar dispute exception (first review M2). |
| 3 | share.astro "no outside contract can list or move a share" | REAL | Reworded: no third party can be approved; a credentialed contract that becomes the holder (for example via `nft_transfer_call`) has normal holder powers. |
| 4 | Agent 1 yoctoNEAR claim attempt listed under "Also on chain" | REAL | Moved to its own help notes line, "Refused before submission, with no transaction", seen in a local CLI log only. |
| 5 | Document hash claim too broad | REAL | Limited to registrar actions that move a share or money; `sync` and `revoke_role` listed as taking none. |

The second reviewer's read-only transaction checks corroborated the sampled mint, credential, sale, claim, agent and dispute transactions. Exact recorded timestamps remain UNVERIFIED.
