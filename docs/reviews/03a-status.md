# Review 03a status

Review 03a covered the contracts diff `ace5138..e0c2819` (branch `fix/contract-review`) and the judge-facing README, from a pack file only. This file maps each finding to its state on branch `fix/review-03a` (off `fix/contract-review`). Nothing here is deployed. Review 03b later withdrew L8(b) and narrowed M4.

States: **fixed** (changed in code, script or record, with a test or a run), **documented** (stated as residual risk or reworded, no code change), **deferred** (needs the owner's yes or a later step), **false** (the code already does what the finding asks).

| Finding | Verdict on checking | State | Evidence |
|---|---|---|---|
| C1 no branch has README and code | REAL. The default branch on GitHub is `main` (2d9a1bf, read with `git ls-remote --symref`), which has the placeholder README and no `verify:testnet`. `fix/contract-review` has no `site/`. Its history (root 281c2c6) is unrelated to `main`, `fix/server-review` and `ui/pass` (root 2d9a1bf). | deferred | Merge needs the owner's yes. Merge plan and trial-merge conflicts are in the report to the owner. README steps 7 and 8 and the site count are marked "after the merge". |
| H1 headline vs registry key and `recover_rejected_vault` | REAL. `approval.rs:105` sends rejected-vault money to any receiver except the registrar; the registry key is present (`verify:testnet`: 1 full-access key). | documented | README intro qualified; registrar list now matches T12 and T35 in full. |
| H2 deployment state understated | REAL. Testnet runs code hash `GZkyojcH...` (pre-review). | documented | README table of every review finding, testnet versus branch; branch-only statements marked; T21 says "after step 15". |
| M1 first registrant of a work | REAL (by design of the split sheet). `registration.rs:290`, `approval.rs:206`. | documented | THREAT_MODEL T39 residual; README Known limits; options listed; DECISIONS_LOG 83 (pending owner). |
| M2 contest bounds count not amount | REAL. Minimality check at `registration.rs:234` lets a claimant name a 9999 bps share for a 1 bps excess. | documented | T11 and README "Disputes" and Known limits reworded. No code change (money path; needs the owner's call). |
| M3 upgrade untested | REAL. | fixed | Sandbox `u01_testnet_wasm_upgrades_in_place_without_migration`: the exact testnet registry wasm (fixture, code hash checked in the sandbox), state from the demo flow including an open dispute with escrow, in-place deploy of the branch wasm with no migration, every token, owner, counter and ledger read back unchanged, then claims, a payment, the dispute resolve and a new pending share with the new vault code. Passes. |
| M4 site claims | PARTLY. Narrowed by 03b: `provider_tee` is implemented on `fix/server-review`. Row 73 said "later". | fixed (wording) | DECISIONS_LOG row 73 updated; README site section and site count marked "after the merge". |
| M5 TODOs | REAL. | fixed (wording), deferred (checks) | Clone URL is `https://github.com/RoylTEE/RoylTEE`. The hosted-site check and screenshots are marked as pre-submission items. |
| L1 `verify:testnet` weaker than README | REAL. | fixed | Owners asserted against new `token_owners` in `deployments/testnet.json`; every token's vault must be in the record; a missing counter fails. Extended for the second reviewer (below). Run on 2026-11-13: 41 PASS, ALL CHECKS PASSED. The fields `variable`, `code_hash_base58`, `key_count` exist in the record. |
| L2 vault `ft_on_transfer` keeps any token | REAL. | documented | A predecessor check needs vault state (the vault is stateless); T20 and README Known limits state the exception. |
| L3 orphan vaults on failed mints | REAL. | documented | T15 exception. Not widened `recover_rejected_vault` (would widen a registrar vault power). |
| L4 agent group size | REAL. Each forward reserves 18 Tgas. | fixed (measured) and documented | Sandbox `t13b_agent_group_size_at_100_tgas`: 4 shares paid, 5 refused with no debit. Unit `r03a_l4_forward_gas_reservation_per_share`. T19 and README updated. |
| L5 approve with nothing held | PARTLY. `forward` already returns on 0 (`money.rs:641`), so no zero transfer was ever sent; it was untested. | fixed (test) | Unit `r03a_l5_approve_before_any_payment_sends_nothing`: approve at 15 Tgas, no receipt created, nothing in flight. |
| L6 threat model summary overclaims | REAL. | fixed (wording) | README threat model paragraph; T12 Controls (`sync` no deposit, `sync` and `revoke_role` no document hash). |
| L7 refusal with no proof | PARTLY. The CLI error text was already in `deployments/testnet.json` step 12. | fixed (wording) | README row labelled "local CLI log" and quotes the recorded error. |
| L8 fresh-clone details | (a) REAL, now sourced: `astro` 7.4.11 `engines.node >=22.23.4`. (b) withdrawn by 03b: dev binds 127.0.0.1:4321. (c) FALSE: `VAULT_WASM` is `include_bytes!` only on `wasm32` (`lib.rs:39`); `test:unit` passed on a fresh clone of `feb7746` with no `contracts/res/` (109 passed, exit 0; the L7 unit test added later makes it 110). (d) REAL, minor: `lint` uses POSIX env syntax. | fixed (a, c wording); (d) deferred | README prerequisites and step 3. Windows users can use WSL (not documented yet). |
| L9 disclosure facts | Checked: `near-sdk` 5.30.1, `near-contract-standards` 5.30.1, `near-sys` 0.4.1, `near-workspaces` 0.24.0 (Cargo.lock); Rust 1.99.1 (`rust-toolchain.toml`); first commit 2026-11-12 18:48 UTC; Upstash used over REST with `fetch`, no client library. Hackathon window start date and the absence of a CLI scaffold are UNVERIFIED. | fixed (wording) | README libraries section. |
| I1 gas comment | REAL. Measured 2.151 Tgas. | fixed | `lib.rs` comment now "about 2.2 (2.151 measured)". |
| I2 DESIGN account names | REAL. | fixed | DESIGN account table names the deployed accounts. |
| I3 storage prefix collision | FALSE. Collection prefixes are the Borsh index of `Key` (0x00 to 0x0D) and the NFT per-owner sets start with 0x00; raw slots start with `a` or `l`. | fixed (wording) | DESIGN section 19 explains why. |
| I4 `revoke_role` keeps `l2t:` slot | REAL, minor. The role record itself is also kept (revoked flag), by design. | deferred | Small storage only. |
| I5 verify fails after step 6, 15 or 16 | REAL. | fixed (wording) | DEPLOY_PLAN step 15 note. |
| I6 "verified" tagline | REAL. The tagline is fixed by the build prompt. | fixed (wording) | README bullet: "verified" means a registrar credential; the identity check is Simulated. |
| I7 LICENSE holder | UNVERIFIED that "RoylTEE Labs" is a legal entity. | deferred | Owner decision. |
| I8 positives | n/a | n/a | |

Tests on `fix/review-03a` (2026-11-13, after the second reviewer fixes): unit 110 (mock USDC 4, registry 101, vault 5); sandbox 32 (agents 9, disputes 4, limits 3, review 4, sandbox 11, upgrade 1); 142 in total, all passing (`npm test` exit 0). `npm run lint` (fmt check, clippy -D warnings host and wasm32) exit 0. `npm run test:verify` 8 of 8. `npm run test:unit` on a fresh clone with no wasm: 109 passed at `feb7746`.

Still needs the owner: the merge (C1), DEPLOY_PLAN step 15 and 16 on testnet (closes H2 on chain), deleting the registry and mock USDC keys (H1 headline becomes unconditional), and the code options for M1 and M2.

## Second reviewer, part 03a

Saved verbatim as a separate report. No critical or high finding.

| # | Item | Verdict | State | Evidence |
|---|---|---|---|---|
| 1 | Deployed versus reviewed differences understated | REAL (overlaps H2) | fixed (wording) | README table now also lists `recover_rejected_vault`; a line says every row is open on testnet until step 15 and links this file and THREAT_MODEL for the full inventory. |
| 2 | Holder-only headline vs `recover_rejected_vault` | REAL (overlaps H1) | fixed (wording) | README headline names the registrar exception and that the registry account still holds a full access key, so the guarantee is only as strong as that key until it is deleted. |
| 3 | "verified record of a signed assignment plus a binding payment instruction" | REAL | fixed (wording) | README "What the token legally is": records an assignment as declared, with evidence hashes (Simulated in the demo); verification of a signed assignment and binding legal effect are UNVERIFIED and planned. |
| 4 | Verifier misses discovered vaults, no pagination | REAL | fixed | `scripts/verify-testnet.mjs` pages `nft_tokens` to the end, fails on a token not in `token_owners`, checks keys and code of every discovered vault, and fails when a recorded token or vault is missing. `scripts/verify-testnet.test.mjs` (fake RPC): extra `s8` fails and its vault is queried; 127 tokens read past 100. |
| 5 | Missing counters treated as zero | REAL | fixed | All five counters must be nonnegative integers or the run fails and solvency is not computed. Tests: `{}` and `-1`/`1.5` fail. |
| 6 | No clone URL or submission revision | REAL (overlaps M5) | fixed (URL), deferred (revision) | Step 1 uses `https://github.com/RoylTEE/RoylTEE.git` plus `git checkout <submission revision>`; the revision is the merged `main` commit, to be written in after the merge (owner). |
| 7 | Key permissions not compared | REAL | fixed | Registry, mock USDC and payment agent keys are compared key by key with the record: public key, full access or function call, receiver and method names. Tests: a full access key swapped for a function call key fails; an extra agent method fails. |
| 8 | Parent-permission test accepts any error | REAL | fixed | `t21_parent_cannot_delete_redeploy_or_key_a_vault` propagates submission errors with `?` and accepts only an executed failure containing `ActorNoPermission` (all three actions confirmed so in a sandbox run). |
| 9 | Registrar doc hash claim | REAL (overlaps L6) | fixed (wording) | README lists `sync` and `revoke_role` as taking no document hash. |
| 10 | Threat model chronology | REAL (overlaps L6) | fixed (wording) | README: T1 to T33 before code; T34 to T39 and the rows marked review added after code. |

The second reviewer's read-only RPC at block 277527740 confirmed the published code hashes, 7 keyless vaults, the registry and mock USDC full access keys, the agent's scoped function call key, the `s2` owner, and registry liabilities equal to its balance at 400000 base units. A live `npm run verify:testnet` with the stricter checker on 2026-11-13 (about 22:30 UTC+7) printed 36 PASS lines and ALL CHECKS PASSED, including `nft_tokens read to the end: 7 token(s)` and solvency 400000 <= 400000.
