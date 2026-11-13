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
| L1 `verify:testnet` weaker than README | REAL. | fixed | Owners asserted against new `token_owners` in `deployments/testnet.json`; every token's vault must be in the record; a missing counter fails. Run on 2026-11-13: 41 PASS, ALL CHECKS PASSED. The fields `variable`, `code_hash_base58`, `key_count` exist in the record. |
| L2 vault `ft_on_transfer` keeps any token | REAL. | documented | A predecessor check needs vault state (the vault is stateless); T20 and README Known limits state the exception. |
| L3 orphan vaults on failed mints | REAL. | documented | T15 exception. Not widened `recover_rejected_vault` (would widen a registrar vault power). |
| L4 agent group size | REAL. Each forward reserves 18 Tgas. | fixed (measured) and documented | Sandbox `t13b_agent_group_size_at_100_tgas`: 4 shares paid, 5 refused with no debit. Unit `r03a_l4_forward_gas_reservation_per_share`. T19 and README updated. |
| L5 approve with nothing held | PARTLY. `forward` already returns on 0 (`money.rs:641`), so no zero transfer was ever sent; it was untested. | fixed (test) | Unit `r03a_l5_approve_before_any_payment_sends_nothing`: approve at 15 Tgas, no receipt created, nothing in flight. |
| L6 threat model summary overclaims | REAL. | fixed (wording) | README threat model paragraph; T12 Controls (`sync` no deposit, `sync` and `revoke_role` no document hash). |
| L7 refusal with no proof | PARTLY. The CLI error text was already in `deployments/testnet.json` step 12. | fixed (wording) | README row labelled "local CLI log" and quotes the recorded error. |
| L8 fresh-clone details | (a) REAL, now sourced: `astro` 7.4.11 `engines.node >=22.23.4`. (b) withdrawn by 03b: dev binds 127.0.0.1:4321. (c) FALSE: `VAULT_WASM` is `include_bytes!` only on `wasm32` (`lib.rs:39`); `test:unit` passes on a fresh clone with no `contracts/res/`. (d) REAL, minor: `lint` uses POSIX env syntax. | fixed (a, c wording); (d) deferred | README prerequisites and step 3. Windows users can use WSL (not documented yet). |
| L9 disclosure facts | Checked: `near-sdk` 5.30.1, `near-contract-standards` 5.30.1, `near-sys` 0.4.1, `near-workspaces` 0.24.0 (Cargo.lock); Rust 1.99.1 (`rust-toolchain.toml`); first commit 2026-11-12 18:48 UTC; Upstash used over REST with `fetch`, no client library. Hackathon window start date and the absence of a CLI scaffold are UNVERIFIED. | fixed (wording) | README libraries section. |
| I1 gas comment | REAL. Measured 2.151 Tgas. | fixed | `lib.rs` comment now "about 2.2 (2.151 measured)". |
| I2 DESIGN account names | REAL. | fixed | DESIGN account table names the deployed accounts. |
| I3 storage prefix collision | FALSE. Collection prefixes are the Borsh index of `Key` (0x00 to 0x0D) and the NFT per-owner sets start with 0x00; raw slots start with `a` or `l`. | fixed (wording) | DESIGN section 19 explains why. |
| I4 `revoke_role` keeps `l2t:` slot | REAL, minor. The role record itself is also kept (revoked flag), by design. | deferred | Small storage only. |
| I5 verify fails after step 6, 15 or 16 | REAL. | fixed (wording) | DEPLOY_PLAN step 15 note. |
| I6 "verified" tagline | REAL. The tagline is fixed by the build prompt. | fixed (wording) | README bullet: "verified" means a registrar credential; the identity check is Simulated. |
| I7 LICENSE holder | UNVERIFIED that "RoylTEE Labs" is a legal entity. | deferred | Owner decision. |
| I8 positives | n/a | n/a | |

Tests on `fix/review-03a` (2026-11-13): unit 109 (mock USDC 4, registry 100, vault 5); sandbox 32 (agents 9, disputes 4, limits 3, review 4, sandbox 11, upgrade 1); 141 in total, all passing. `npm run lint` (fmt, clippy host and wasm, `-D warnings`) exit 0.

Still needs the owner: the merge (C1), DEPLOY_PLAN step 15 and 16 on testnet (closes H2 on chain), deleting the registry and mock USDC keys (H1 headline becomes unconditional), and the code options for M1 and M2.
