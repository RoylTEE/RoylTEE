# RoylTEE

**The verified payout address for every share of every song, human or AI.**

The payout address never changes, only who controls it. An agent can pay it, but it cannot steal from it.

RoylTEE is a permissioned, business to business registry of music rights shares on NEAR testnet. Each rights share is an NEP-171 NFT. Each share NFT controls its own NEAR sub-account (a "vault") that receives that share's royalty money. The vault has zero access keys. Only the registry contract can move money out of it, and only to whoever holds the NFT at that moment, with one registrar exception for rejected shares (see [Who can hold a token](#who-can-hold-a-token)). On testnet today the registry account itself still holds one full access key (kept on testnet by decision so late fixes stay possible; on mainnet it would be deleted after the final deploy, or upgrades handed to a multisig or DAO, and proven by the same zero-keys check the share vaults pass), so whoever holds that key could replace the registry code and with it these rules; the guarantee is only as strong as that key. The mock USDC account keeps its key for the same reason, and so does the auction account.

- Built at NEAR HACKATHON, track "NFTs That Do Something" (track id `nfts`).
- Testnet only. No mainnet contract, no real money. Payments use **Mock USDC (testnet)** (symbol `mUSDC`, 6 decimals), a mock token with no value, not Circle USDC.
- Everything off chain (identity checks, the fiat rail, payer statements, dispute documents) is **Simulated** and labeled so. See [Simulated vs real](#simulated-vs-real).
- License: MIT ([LICENSE](LICENSE)).

## Contents

1. [Try it: do X, see Y](#try-it-do-x-see-y)
2. [The problem](#the-problem)
3. [How it works](#how-it-works)
4. [How control passes on transfer](#how-control-passes-on-transfer)
5. [Holder powers and agent powers](#holder-powers-and-agent-powers)
6. [Who can hold a token](#who-can-hold-a-token)
7. [Human, AI-assisted and fully AI music](#human-ai-assisted-and-fully-ai-music)
8. [Threat model and tests](#threat-model-and-tests)
9. [Live testnet deployment](#live-testnet-deployment)
10. [Website and help chat](#website-and-help-chat)
11. [Simulated vs real](#simulated-vs-real)
12. [What the token legally is](#what-the-token-legally-is)
13. [TEE use: OutLayer call path and the auction settle](#tee-use-outlayer-call-path-and-the-auction-settle)
14. [Sealed-bid share auction](#sealed-bid-share-auction)
15. [Roadmap](#roadmap)
16. [Known limits](#known-limits)
17. [Libraries, templates and AI-written code](#libraries-templates-and-ai-written-code)
18. [Repository layout](#repository-layout)
19. [Built at NEAR HACKATHON](#built-at-near-hackathon)

## Try it: do X, see Y

### Prerequisites

- Git, and Node.js with npm. The root scripts need Node 18 or newer; the site needs Node 22.23.4 or newer (Astro's own requirement).
- Rust through [rustup](https://rustup.rs). `rust-toolchain.toml` pins Rust 1.99.1 with the `wasm32-unknown-unknown` target, and rustup installs it on the first `cargo` call.
- For the sandbox tests only, on Linux: `pkg-config` and the OpenSSL development headers (Debian or Ubuntu: `libssl-dev`), because the `near-workspaces` test crate links OpenSSL. If `pkg-config` is missing, point the build at OpenSSL directly, for example on Debian or Ubuntu: `export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu OPENSSL_INCLUDE_DIR=/usr/include`.
- Network access on the first run: cargo downloads crates, the sandbox tests download the NEAR sandbox node binary, and `verify:testnet` reads public testnet RPC.
- No wallet, no keys and no NEAR are needed for anything in this section.

### Steps

| # | Do | See |
|---|---|---|
| 1 | `git clone https://github.com/RoylTEE/RoylTEE.git royltee && cd royltee && git checkout <submission revision>` (pre-submission item: the submission revision is the merged `main` commit, to be written here once the branches are merged; until then the default branch does not yet contain every part described here) | The files listed under [Repository layout](#repository-layout). |
| 2 | `npm run verify:testnet` (read only, about one minute, no install needed) | One `PASS` line per check and `ALL CHECKS PASSED` at the end: 15 share vaults `s1` to `s15` with `0 access keys`, every code hash equal to `deployments/testnet.json`, tokens `s2` (the catalog sale) and `s10` (the auction sale) owned by `buyer-pub.royltee-dev.testnet`, the registry and mock USDC keys equal to the record (`access keys match the record: full_access`), the payment agent's single function call key scoped to the registry with `pay_from_allowance` and `report`, `nft_tokens read to the end: 15 token(s)`, the solvency line, which needs all five accounting counters, and three auction lines: `auction.royltee-dev.testnet exists, code hash 5uH7oSKP... matches the record`, `auction.royltee-dev.testnet access keys match the record: full_access`, and `token s10 sold in auction 0 is owned by buyer-pub.royltee-dev.testnet, as both records say`. `npm run test:verify` runs the two checkers' own 22 tests (14 for `verify:testnet`, 8 for `check:quiescent`) against a local fake RPC (no network). |
| 3 | `npm run test:unit` | Rust unit tests: `test result: ok. 4 passed` (mock USDC), `155 passed` (registry), `5 passed` (vault), `68 passed` (auction). 232 in total. No wasm build is needed: the registry embeds the vault wasm only when it is compiled to wasm. |
| 4 | `npm test` | Builds all contracts to wasm, prints the size and sha256 of each file in `contracts/res/`, then runs every unit test and the near-workspaces sandbox tests: `9 passed` (agents), `15 passed` (auction), `4 passed` (disputes), `4 passed` (final), `4 passed` (limits), `4 passed` (review), `11 passed` (sandbox), `3 passed` (upgrade), then the 14 WASI host tests and the 18 Node tests. 54 sandbox tests, 318 in total. Takes a few minutes. |
| 5 | Optional: `npm run lint` | `cargo fmt --check`, then `cargo clippy -D warnings` for the host and wasm targets; ends with `Finished` and exit code 0. |
| 6 | Open the explorer links under [Live testnet deployment](#live-testnet-deployment) | The same accounts, the catalog sale transfer of `s2`, the refused outsider claim, the agent payment and the dispute, on NEAR testnet. |
| 7 | Site (in `site/`): `cd site && npm ci && npm test && npm run typecheck && npm run build` | Vitest: `Test Files  22 passed (22)` and `Tests  231 passed (231)`; `astro check`: `0 errors`; a build in `site/.vercel/output`. |
| 8 | Site, local: `npm run dev` (at the repo root or in `site/`, after the `npm ci` of step 7), then open http://127.0.0.1:4321 | The pages listed under [Website and help chat](#website-and-help-chat). `/admin` answers 401 when not signed in. Without server secrets the help chat and support API answer 503 ("not configured yet"). `site/README.md` explains how to run the API locally with in-memory storage. |

Hosted site: https://royltee.vercel.app (UNVERIFIED: which commit it runs, its live security headers, whether chat and support are configured, and whether a wallet sign-in works there have not been checked yet. **Pre-submission item:** check it in a private window and record the result here.)

The numbers above (steps 2 to 5 and 7) come from running these commands on 2026-11-14 (UTC) on the merged release tree: the registry code deployed on testnet since the final redeploy plus the sealed-bid auction (the local registry build equals the on chain sha256, see [Live testnet deployment](#live-testnet-deployment)). Step 8 (every page answered 200 on the local dev server, `/admin` answered 401, chat and ticket API 503 without server secrets) was last run on 2026-11-13 (UTC) and was not re-run for this tree (UNVERIFIED since).

Demo scripts: there are no separate `seed-demo`, `simulated-payer` or `scripted-agent` scripts. The on-chain demo (seed accounts and works, the Simulated payer's payments, the scripted agent's calls) was run as the `near` CLI commands written out step by step in [DEPLOY_PLAN.md](DEPLOY_PLAN.md), steps 1 to 13 (the split sheet scene is step 16 and the auction demo is step 17; the auction bids were sealed with `scripts/auction/encrypt-bid.mjs`), and every resulting transaction id is in `deployments/testnet.json` (the auction's own record is `deployments/auction-testnet.json`). The key check (zero keys on every vault, the registry and mock USDC keys equal to the record, the agent's single scoped function call key) is `npm run verify:testnet` (`scripts/verify-testnet.mjs`), not a separate `verify-keys` script.

## The problem

Ownership data for a song lives in split sheets, PDFs, contracts and payer databases that often disagree. Every payer keeps its own copy of who owns what. When a catalog is sold, each payer has to update its own records, so payee changes after a sale take time to reach every payer (the exact lag is UNVERIFIED; this README cites no number for it). Payments then go to the wrong party, sit unmatched, or get paid twice.

RoylTEE gives every share one stable payout address. A payer pays the share's vault (or pays the work once and the registry splits it). Who controls that vault is decided by the registry at the moment of a claim, so a sale changes the controller without changing the address any payer uses.

## How it works

```
payer --ft_transfer_call (work, period, statement hash)--> registry (reg.royltee-dev.testnet)
                                                                |  split by basis points, exact to the last unit
                                    +---------------------------+---------------------------+
                                    v                                                       v
                         vault s1.reg... (0 keys)                                 vault s2.reg... (0 keys)
                                    |  claim() by the CURRENT NFT holder only                |
                                    v                                                       v
                             holder of share s1                                      holder of share s2
```

- **Work**: a song keyed by ISRC (recording) or ISWC (composition), with a provenance field. Titles are stored as hashes; personal data stays off chain.
- **Share**: one NFT per rights share: right type (`master`, `publishing`, `performance`, `mechanical`), territory, basis points (1 bps = 0.01 percent), basis (`copyright`, `contract`, `none`), evidence tier. Shares of one work, right and territory may not sum above 10000 bps; a claim that would go above opens a dispute.
- **Vault**: `s<N>.reg.royltee-dev.testnet`, created by the registry at mint in one batch with no `AddKey` action, storage-registered with Mock USDC (testnet). Its only change method is `withdraw`, callable only by the registry. New vaults also export an `ft_on_transfer` that keeps tokens and does nothing else.
- **Work payment**: one `ft_transfer_call` to the registry naming the work. The registry splits by bps (floor per share, remainder to the largest share, ties to the lowest token number) and forwards each part to the vaults in the same receipt chain. Credits always sum to the payment exactly. A plain `ft_transfer` straight to a vault also works; `sync` picks it up.
- **Earned**: `earned = known vault balance + total paid out`, shown live in `nft_token` metadata (`extra`: earned, paid out, claimable, status, locks, evidence tier, approval).
- **Split sheet** (on testnet since the step 15 redeploy): a share registered by anyone other than the work's registrant is held (its money goes to escrow) until the work registrant approves it with `approve_share`, or rejects it with `reject_share`. The registrant is whoever registered the work first; see [Known limits](#known-limits).
- **Disputes**: a claimant that pushes a group over 100 percent names the shares it contests (the list must be minimal and cannot include its own shares). Each named share's whole new credit is escrowed, not only the excess, and the claimant chooses which shares to name, so a 1 bps claim can hold a large share's new money. Shares not named keep being paid, and money already in vaults stays claimable. The registrar resolves with a document hash.
- **Agents**: an organisation's payment agent has its own sub-account, a function-call key scoped to the registry, and a capped allowance inside the registry. The contract decides who gets paid, never the agent.

Full method signatures, state layout and events: [DESIGN.md](DESIGN.md). Every decision: [DECISIONS_LOG.md](DECISIONS_LOG.md).

## How control passes on transfer

Nothing is handed over on transfer. There is no key to rotate and the seller keeps no holder power over the sold share. Each owner action reads `nft_token(token_id).owner_id` at call time. One exception: if the seller registered the work, it stays the work's split sheet signer after the sale (see [Known limits](#known-limits)).

| Threat | What stops it | Proof |
|---|---|---|
| T1. Seller keeps a key to the vault | The vault is created with zero access keys and has no method that adds a key, deploys code or deletes the account. The parent registry cannot add keys to, redeploy or delete an existing vault either. | Sandbox `t01_mint_creates_keyless_registered_vault`, `t21_parent_cannot_delete_redeploy_or_key_a_vault`; live: `npm run verify:testnet` shows 0 keys on all 15 vaults. |
| T2. Seller claims after the sale | `claim` requires the caller to be the current `owner_id` and pays only that account. Unclaimed money stays in the vault and travels with the token. | Sandbox `t03_owner_claims_non_owner_fails`, `t04_catalog_sale_balance_travels_with_token`; live: the catalog sale of `s2` below. |
| T3. Claim-then-sell race (payout in flight) | `claim` sets `payout_in_flight` and debits the ledger before calling the vault. Transfers and a second claim are refused until the callback clears the flag; the debit is rolled back only if the transfer failed. | Sandbox `t06_transfer_during_inflight_payout_fails`. |
| T4. `nft_transfer_call` bounce (receiver claims during the call, then returns the token) | `transfer_pending` is set before the receiver is called. `claim`, transfers, notes and `sync` are refused until `nft_resolve_transfer` clears it. | Sandbox `t07_transfer_call_bounce_cannot_claim` (uses a member contract that tries exactly this). |
| T5. Claim while listed on a marketplace | NEP-178 approvals are disabled: `nft_approve`, `nft_revoke`, `nft_revoke_all` panic, and a transfer with an `approval_id` panics. | Sandbox `t16_approvals_are_disabled`. |

Transfers also require exactly 1 yoctoNEAR (so an agent's function-call key cannot call them) and a receiver with an active credential, and are refused while the share is disputed, held for approval or void.

## Holder powers and agent powers

**Holder of a share NFT** (enforced in the contract, nothing else):

1. `claim(token_id)`: move the vault's known balance to themselves. 1 yoctoNEAR.
2. `nft_transfer` / `nft_transfer_call`: transfer the share to another active member. 1 yoctoNEAR.
3. `set_share_note(token_id, note)`: set a public note of at most 280 bytes. Payout logic and agents never read it.
4. `sync(token_id)`: pull the vault's real balance into the ledger (holder or registrar only; before the step 15 redeploy anyone could call it on testnet, see [Live testnet deployment](#live-testnet-deployment)).
5. `confirm_share(token_id)`: as holder of another share of the same work, right and territory, confirm a co-holder's share (counts toward its `counterparty_confirmed` evidence tier).

A holder cannot change the split table, change provenance, touch other shares, approve its own pending share, or move money anywhere except to itself through `claim`.

**Agent** (a direct sub-account of a member organisation, for example `pay-agent.buyer-pub.royltee-dev.testnet`):

- Allowed methods, and the only ones on its function-call key: `pay_from_allowance(target, amount, statement_line_id, period, statement_hash)` and `report(statement_line_id, report_hash)`. Neither needs a deposit.
- `pay_from_allowance` takes a work group or share id, never a payee. The contract resolves the vaults from the registry. It enforces the role's per-transaction limit, rolling 24 hour limit, daily count and expiry, and pays each `statement_line_id` once per organisation.
- An agent key cannot call `nft_transfer`, `claim`, `withdraw_allowance`, `grant_role`, `register_share` or any registrar method. Two locks: those methods need a deposit, which a function-call key cannot attach (protocol lock), and they check ownership or role (contract lock). `revoke_role` takes effect at the next call.
- The agent in the demo is a **scripted agent** (commands run by hand from the deploy plan), not a model.

Proof: sandbox `t08_agent_key_cannot_reach_owner_methods`, `t09_payment_over_limits_fails`, `t10_revoked_agent_next_call_fails`, `t11_injected_payee_changes_nothing`, `t12_same_statement_line_pays_once`; live: the agent transactions below.

## Who can hold a token

Only verified industry members with an active credential: labels, publishers, distributors, societies, artists, AI music companies. The registrar grants, suspends and reinstates credentials; credentials cannot be transferred. Transfers only go to active members (sandbox `t05_transfer_to_outsider_fails`). A suspended member keeps the share and it keeps accruing, but cannot claim, send or receive until reinstated; only the registrar can move it, with a document hash.

This is a business record among members, not a product for the public. Shares are not sold to the public and are not an investment product. In this demo the identity check behind a credential is **Simulated** (the credential stores hashes of dummy documents).

The **registrar is a trusted role** in this demo. Its full list of powers (the same as [THREAT_MODEL.md](THREAT_MODEL.md) T12 and T35):

- grant, suspend, reinstate and remove credentials;
- move a suspended holder's share to an active member (`registrar_transfer`);
- resolve disputes, within the dispute's parties;
- release a group's unallocated escrow to an active share of that group (`release_unallocated`);
- refund a void work's unallocated escrow to any account except itself (`refund_unallocated`);
- approve or reject a pending share as backup split sheet signer (`approve_share`, `reject_share`);
- send plain deposits out of a rejected share's vault to any account except itself (`recover_rejected_vault`). This is the one path where vault money goes to someone other than the NFT holder; a rejected share has no approved holder;
- settle a quarantined payment forward or a stale payout lock by stating the final outcome of the original transfer receipt (`resolve_written_off` with a delivered and a failed amount, `resolve_stale_payout`; each call names the state it reviewed, `expected_seq` or `expected_since_block`, and a single-use evidence hash, so a callback that lands first makes the call fail and one receipt cannot settle twice). A wrong answer can pay a share twice from pooled registry funds or credit a share that the vault does not back, so this is a trust in the registrar reading the public receipt (a missing or pending receipt is not evidence), bounded by the quarantined or locked amount; it can only credit the share's own ledger or its undelivered bucket;
- grant and revoke the payer role, revoke agent roles, and `sync` any share.

It cannot hold a credential (refused on testnet since step 15; the first build could grant it one), claim, move an active member's share, or touch money already in an active holder's vault. Its other methods need 1 yoctoNEAR (so a full-access key) and emit an event, and all but `revoke_role` take a document hash. `sync` needs no deposit and takes no document hash. A multisig council with a time delay is roadmap.

## Human, AI-assisted and fully AI music

- Every work carries `provenance`: `human`, `ai_assisted` or `fully_ai`. It is a declared field; the evidence behind it is **Simulated** (not checked).
- Every share carries `basis`: `copyright`, `contract` or `none`. A share of a fully AI track can record a contract-based payment entitlement instead of claiming copyright. Whether a fully AI track has copyright depends on the jurisdiction; this repo makes no legal claim about it (UNVERIFIED, not legal advice).
- On testnet, demo works 0, 1, 2 and 5 are registered as `human`, work 3 as `ai_assisted` and work 4 as `fully_ai` (its shares `s10` and `s11` use basis `contract`). The `ai_assisted` and `fully_ai` values have no dedicated unit test.

## Threat model and tests

- [THREAT_MODEL.md](THREAT_MODEL.md): 39 registry rows (T1 to T39) and 27 sealed-bid auction rows (T-A1 to T-A27). T1 to T33 were written before any contract code; T34 to T39 and the rows marked "review" were added or rewritten after code, as its status line says. Most rows give the attack, impact, mitigation and test; trust rows (T12 registrar, T13 keys) give the trust and controls instead, and T34 needs no test (it is the absence of a method).
- [AUDIT.md](AUDIT.md): a manual audit pass and the contract review fixes.

Test suites (counts from `npm test` and the site's `npm test` on the merged release tree, 2026-11-14 UTC):

| Suite | Where | Count |
|---|---|---|
| Registry unit tests | `contracts/registry/src/` (`tests.rs`, `review_fixes.rs`, `final_fixes.rs`, `logic.rs`) | 155 |
| Vault unit tests | `contracts/vault/src/lib.rs` | 5 |
| Mock USDC unit tests | `contracts/mock-usdc/src/lib.rs` | 4 |
| Sandbox: core transfer path | `contracts/tests/tests/sandbox.rs` | 11 |
| Sandbox: agents | `contracts/tests/tests/agents.rs` | 9 |
| Sandbox: disputes | `contracts/tests/tests/disputes.rs` | 4 |
| Sandbox: final contract fixes | `contracts/tests/tests/final.rs` | 4 |
| Sandbox: limits and gas | `contracts/tests/tests/limits.rs` | 4 |
| Sandbox: review fixes | `contracts/tests/tests/review.rs` | 4 |
| Sandbox: in-place upgrade from the testnet wasm | `contracts/tests/tests/upgrade.rs` | 3 |
| Auction contract unit tests | `contracts/auction/src/tests.rs` | 68 |
| Auction WASI module host tests | `tee/auction-wasi/src/lib.rs` | 14 |
| Auction Node tests (real module under WASI) | `scripts/auction/auction.test.mjs` | 18 |
| Sandbox: auction (mock OutLayer, real module answers) | `contracts/tests/tests/auction.rs` | 15 |
| Site (help chat, tickets, limits, dashboard) | `site/tests/` | 231 (22 files) |

Brief test list to sandbox tests (every sandbox test ends with a solvency check; tests that touch a vault also check it has zero keys):

| # | Test | Sandbox test |
|---|---|---|
| 1 | Mint creates a keyless, token-registered vault | `t01_mint_creates_keyless_registered_vault` |
| 2 | Plain transfer, sync, earned updates | `t02_plain_transfer_then_sync_updates_earned` |
| 3 | Owner claims, non-owner fails | `t03_owner_claims_non_owner_fails` |
| 4 | Pay, transfer, pay again: seller gets nothing, buyer claims both | `t04_catalog_sale_balance_travels_with_token` |
| 5 | Transfer to a non-member fails | `t05_transfer_to_outsider_fails` |
| 6 | Transfer during an in-flight payout fails, no double pay | `t06_transfer_during_inflight_payout_fails` |
| 7 | `nft_transfer_call` bounce cannot claim | `t07_transfer_call_bounce_cannot_claim` |
| 8 | Agent key cannot reach owner methods | `t08_agent_key_cannot_reach_owner_methods` |
| 9 | Over per-transaction or daily limit fails | `t09_payment_over_limits_fails` |
| 10 | Revoked agent's next call fails | `t10_revoked_agent_next_call_fails` |
| 11 | Payee hidden in metadata changes nothing | `t11_injected_payee_changes_nothing` |
| 12 | Same statement line pays once | `t12_same_statement_line_pays_once` |
| 13 | Split sums to the payment exactly | `t13_allowance_work_payment_splits_exactly`, `t16b_split_sums_to_payment_exactly` |
| 14 | Over 100 percent: only the contested fraction freezes, resolve releases escrow | `t14_over_allocation_dispute_freezes_only_contested_fraction`, `t14b_uphold_existing_voids_claimant`, `t14c_duplicate_code_swap_payable` |
| 15 | Suspended member keeps the share and accrues, cannot receive | `t15_suspended_org_keeps_share_and_its_agent_stops`, `d3_suspended_holder_frozen_registrar_can_move` |
| 16 | `nft_approve` panics | `t16_approvals_are_disabled` |
| 17 | License NFT | Not built (licenses were cut first, as the plan allows). |

Run them: `npm run test:unit` (unit only), `npm run test:sandbox` (sandbox only, builds wasm first), `npm test` (both).

## Live testnet deployment

All accounts, public keys, wasm hashes and transaction ids: [deployments/testnet.json](deployments/testnet.json). The deploy commands: [DEPLOY_PLAN.md](DEPLOY_PLAN.md). Check the live state yourself with `npm run verify:testnet` (read only).

**What is deployed.** Since 2026-11-14 09:09 UTC the registry on testnet runs the code in `contracts/registry` of this repository: the contract review fixes, the audit fix F1 (forward gas budget) and the final contract fixes (quarantine of a lost payment forward, registrar settlement with a settlement sequence and single use evidence, the `assert_quiescent` deploy barrier). The last redeploy was ONE transaction, a deploy action followed by a call of `assert_quiescent` (a busy registry would have rolled the deploy back; it passed): [`52jp95BV...`](https://testnet.nearblocks.io/txns/52jp95BVJCk6a9bUGyYtYEQq99R56Xiz5zxJ1vCHBJrD), code hash `5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG` (sha256 `4378568f0459560130cdb56fa05152f9946116d3332d6c3f7f53e939005dc271`, 748115 bytes), fee 0.0058702528822306 NEAR, 59.0 Tgas, storage 731332 to 766489 bytes. `npm run check:quiescent` printed SNAPSHOT QUIET at block 277636451 before it, and `AFTER_DEPLOY=1 npm run check:quiescent` passes after it. Before that the registry got there in two in-place redeploys, both with no init call and no migration, both signed with the registry's full-access key: DEPLOY_PLAN step 15 on 2026-11-13 18:54 UTC put the review fixes on chain ([`9oR5awwx...`](https://testnet.nearblocks.io/txns/9oR5awwxiJPouDhpYdTxTquskc8YpoFHdmAJy4SrrAMp), code hash `22JaDgJdfFRb2X9yGejNtgwBRbg3yuKQYptgjXMm6xv7`), and the audit redeploy added F1 ([`BGBUJDTn...`](https://testnet.nearblocks.io/txns/BGBUJDTnnjpevSUZkdmfeVASxbKHn1FQDAZqj75kH5rU), code hash `5Taw8Syz7g1QxNa4Faw8mz7fnNgoXGj7MwX2VS3obRFo`). A fresh build of this tree with `scripts/build-wasm.sh` gives the deployed sha256 (checked on one machine). The sandbox test `u01_testnet_wasm_upgrades_in_place_without_migration` starts from the exact first testnet registry wasm (`contracts/tests/fixtures/`, code hash checked), builds state, upgrades in place and reads everything back. The second upgrade, from `22JaDgJd...` to `5Taw8Syz...`, was not rehearsed in the sandbox (UNVERIFIED there); after it, `npm run verify:testnet` read every share, owner, key and counter back as recorded. Each finding of the contract review, on the first build and now:

| Review finding | First build (`GZkyojcH...`, before step 15) | On testnet since step 15 (and kept by the later redeploys) |
|---|---|---|
| H1 sync | Anyone can call `sync`; vaults have no `ft_on_transfer`, so the phantom balance race is open | Holder or registrar only; new vaults (`s8` to `s15`) keep `ft_transfer_call` tokens. The 7 old vaults `s1` to `s7` keep their code forever (T20, T21) |
| H2 contests | No `contests` list means every active share of the group is contested, so a 1 bps claim freezes the whole group | `contests` is mandatory and minimal; own shares cannot be named; dead entries are pruned |
| H3 registrar credential | The registrar account can be granted a credential | Refused |
| M1 stuck mints | No `release_stale_mint`; a mint whose callbacks never commit blocks its bps; forward callback gas 5 Tgas | `release_stale_mint` after 20 minutes; late callbacks mint nothing; 10 Tgas |
| M2 split sheet | Anyone with a credential can register a share on someone else's work and be paid at once | Held until the work registrant (or the registrar) approves it. A rejected share's plain deposits can be sent by the registrar to any account except itself (`recover_rejected_vault`, a trusted registrar power) |
| L2 agent window | The rolling limit can be spent twice about 23 hours apart | Covers a full 24 hours |
| L4 (Security Suite F-04) sync epochs | A sync in flight across a dispute open or `nft_transfer_call` can land | Discarded |
| L5 payer attestation | `payer_confirmed` stays after the payer role is revoked | Counts only while the role holds |
| L6 mint to a suspended holder | Minted | The share fails, no NFT |
| L8 share payment minimum | A share payment of 1 base unit is accepted | Same minimum as a group payment |
| L9 void work escrow | Unallocated escrow of a void work is stranded | Registrar `refund_unallocated` |

Every row above was open on testnet until step 15 ran. The full per finding inventory, with the tests for each, is in [docs/reviews/03a-status.md](docs/reviews/03a-status.md) and the review rows of [THREAT_MODEL.md](THREAT_MODEL.md).

The live demo below ran on the first build up to the dispute resolve, on the step 15 build from the second payment on work 0 until the audit redeploy, and the split sheet scene and the auction sale on the final build. The audit redeploy and the final redeploy moved no state (the final one also asserted it was quiet): after it, `get_accounting` read `allowance_total` 200000 and every other counter 0, and `s2`, `s12` and `s13` kept their owners and approvals.

**Keys.** The 15 vault accounts (`s1` to `s15`) have zero access keys (re-read on 2026-11-14 at about 11:03 UTC, `s14` and `s15` included). The registry (`reg.royltee-dev.testnet`) and the mock USDC (`usdc.royltee-dev.testnet`) accounts still have one full-access key each, kept on purpose on testnet (see [Keys on testnet vs mainnet](#keys-on-testnet-vs-mainnet) below), so whoever holds the registry key can redeploy the registry, as all three in-place updates did (THREAT_MODEL T13). The agent account has only a function-call key; its full-access key was deleted on chain.

| Account | Role | Code hash (base58, from `deployments/testnet.json`) |
|---|---|---|
| [`reg.royltee-dev.testnet`](https://testnet.nearblocks.io/address/reg.royltee-dev.testnet) | Registry, NFT name `RoylTEE`, symbol `ROYLT` | `5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG` (sha256 `4378568f0459560130cdb56fa05152f9946116d3332d6c3f7f53e939005dc271`). Earlier: `GZkyojcH...` (first build), `22JaDgJd...` (step 15), `5Taw8Syz...` (audit F1) |
| [`usdc.royltee-dev.testnet`](https://testnet.nearblocks.io/address/usdc.royltee-dev.testnet) | Mock USDC (testnet), `mUSDC` | `47ybTZxhaJ9ZW3FC9jwaDdLfwgkJ59cCrnkSn5oop67t` (sha256 `2e5c896b2ead7d1bd12a2944971d6a393902084cf47616c54d18e7a577efc0bb`) |
| [`s1.reg.royltee-dev.testnet`](https://testnet.nearblocks.io/address/s1.reg.royltee-dev.testnet) to `s7` | Share vaults, 0 keys, vault code of the first build (no `ft_on_transfer`) | `BBLu1w41doKx3yLRHm1tKS47wQQeB8qev9B5P3iEHKKU` (sha256 `973b2cbdbe904de710dcc7bf54a7b4164156f8c81d2932191e6968514aaaa727`) |
| [`s8.reg.royltee-dev.testnet`](https://testnet.nearblocks.io/address/s8.reg.royltee-dev.testnet) to `s15` | Share vaults, 0 keys, vault code the current registry deploys (`get_vault_code_hash`) | `BgC1pQYrvm91uXYeWFhs6ab1prgVAWMFTSfuj8KQkMXP` (sha256 `9e9ef97840a3a3e6967565b43a9c2a467c1925153ea8c5577d666c9b16966cda`) |
| [`auction.royltee-dev.testnet`](https://testnet.nearblocks.io/address/auction.royltee-dev.testnet) | Sealed-bid share auction contract (code in `contracts/auction`; it keeps its own full-access key on testnet), a credentialed member that held share `s10` while it was for sale | `48d32c71de83d94a4225502a726b80bd678ac3dfb1471952c93c4c16595a8d09` (sha256 of the deployed code) |
| [`registrar.royltee-dev.testnet`](https://testnet.nearblocks.io/address/registrar.royltee-dev.testnet) | Registrar (trusted role) | |
| `writer-a`, `writer-b`, `buyer-pub` `.royltee-dev.testnet` | Members | |
| [`pay-agent.buyer-pub.royltee-dev.testnet`](https://testnet.nearblocks.io/address/pay-agent.buyer-pub.royltee-dev.testnet) | Scripted payment agent, function-call key only | |
| [`payer.royltee-dev.testnet`](https://testnet.nearblocks.io/address/payer.royltee-dev.testnet) | **Simulated** payer and Mock USDC minter | |
| [`outsider.royltee-dev.testnet`](https://testnet.nearblocks.io/address/outsider.royltee-dev.testnet) | No credential, used to show refusals | |
| [`probe.royltee-dev.testnet`](https://testnet.nearblocks.io/address/probe.royltee-dev.testnet) | Throwaway OutLayer call-path probe, not part of the registry. Public key `ed25519:3vSHK273p4gXTuKBQj5v3iUEm4vHxBdmYkP4MPX3EroF` (full access). This key is not in the earlier key backup archive. See [TEE use: OutLayer call path and the auction settle](#tee-use-outlayer-call-path-and-the-auction-settle). | Account and code hash are in `deployments/testnet.json`. On chain the code hash is `HCaVhK4CVa7eF6VufsexRckEtsLAyfRcEwtL8HPEUs1o` (sha256 `f0b24099cb5e37b7d416eba3988995cfa2b18ebee6d15d16e1be3c466c5bf77e`). |

On-chain demo, in order (2026-11-13 UTC up to the last agent row, then 2026-11-14 UTC; amounts in mUSDC, all of it Simulated money):

| Scene | Transaction | What it shows |
|---|---|---|
| Simulated payer pays work 0 once, 1.0 mUSDC, split 0.5 and 0.5 into vaults `s1` and `s2` | [`HBzKi5pt...`](https://testnet.nearblocks.io/txns/HBzKi5ptr5sGvPSgwWQSUNMKBRJksNCp3tAeT4xByo8u) | One payment, auto-split by bps |
| Plain 0.2 mUSDC straight into vault `s3`, then `sync` | [`5wiQVMdf...`](https://testnet.nearblocks.io/txns/5wiQVMdfYtuFvUsYcaTeM31sZPizytAaT8etsDx9dQZo), [`2Jsht1Ea...`](https://testnet.nearblocks.io/txns/2Jsht1EatGqZfeVrZePKyVMQPyqyeqTCpEcoaHWK44LY) | Direct deposits are picked up (on the first build any account could sync; the current build restricts it to the holder or registrar) |
| `writer-a` claims `s1` | [`FLzUW7hf...`](https://testnet.nearblocks.io/txns/FLzUW7hfLkY1waMuwRDoLCdhAxKsXyxv2FmJ4UoRnsNH) | Holder claim |
| **Catalog sale**: `writer-b` transfers `s2` to `buyer-pub` | [`HZ9pD2kL...`](https://testnet.nearblocks.io/txns/HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy) | Control moves with the NFT; vault `s2` keeps 0 keys |
| `buyer-pub` claims `s2` (the 0.5 that was paid before the sale) | [`M4pWDbEg...`](https://testnet.nearblocks.io/txns/M4pWDbEgnUfz4TiJA8ZZdNVCnwV9vzsBnnEqRgALDrp) | Unclaimed balance travelled with the token |
| `outsider` tries to claim `s1` | [`GvcHLtG7...`](https://testnet.nearblocks.io/txns/GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB) | Refused: "only the current holder can claim" |
| Agent pays work 1 from its allowance, line `agent-line-1` | [`7DJThzgC...`](https://testnet.nearblocks.io/txns/7DJThzgCgnZYf9uNhPgH5ZsuJKx8mJqHkiLcqszdRSTu) | Contract resolves payees. This one call was signed with the agent's full-access key before that key was deleted. |
| Agent's full-access key deleted | [`7Bwi9rmQ...`](https://testnet.nearblocks.io/txns/7Bwi9rmQk4R4iApdVTXMWLkGjMgDnh2qZiVEfGPY5NAH) | From here the agent has a function-call key only |
| Agent `report`, signed with the function-call key | [`CmRvd9ZN...`](https://testnet.nearblocks.io/txns/CmRvd9ZNcfsR5EnMsc3VJ6YYncE3mQgLWqF21YhS1w5G) | The scoped key works |
| Agent resubmits `agent-line-1` with the function-call key | [`2547uRUD...`](https://testnet.nearblocks.io/txns/2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3) | Refused: a statement line pays once |
| Agent tries `claim` on `s4` with 1 yoctoNEAR | no transaction (local CLI log) | Refused before sending: a function-call key cannot attach a deposit. There is no on-chain artifact; the CLI error text is recorded in `deployments/testnet.json` (step 12): "Having a deposit with a function call action is not allowed with a function call access key". The sandbox shows the same protocol rule in test 8. |
| `writer-b` registers a 20 percent share on work 2, contesting `s5` | [`3uKby5aP...`](https://testnet.nearblocks.io/txns/3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba) | Over 100 percent: share `s7` minted as claimant, dispute 0 opened |
| Simulated payer pays work 2, 1.0 mUSDC | [`C8H28Nx1...`](https://testnet.nearblocks.io/txns/C8H28Nx1MZ5Pjb13uzkvheAh7S6Jf5mdqqHqmM1n7Mu) | `s6` paid 0.3 as usual; `s5`'s 0.7 held in dispute escrow |
| Registrar resolves dispute 0 `uphold_existing` with a document hash | [`7wvLcnuB...`](https://testnet.nearblocks.io/txns/7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi) | `s7` void, 0.7 escrow released to `s5` only |
| *From here on the step 15 build (2026-11-13 18:54 UTC on).* | | |
| Simulated payer pays work 0 again, 1.0 mUSDC, line `line-sale-2` | [`2aAgWtca...`](https://testnet.nearblocks.io/txns/2aAgWtcaEC1RinbQvVS1AoLpZn7zQeEmzAGW3FshXHQA) | `s1` and `s2` get 0.5 each; the 0.5 for `s2` now belongs to its new holder |
| `writer-b`, who sold `s2`, tries to claim it | [`EYDV3Tsv...`](https://testnet.nearblocks.io/txns/EYDV3Tsv7Q1ZFPnJzmVdk6D4Rtosqpn8fYNY71ofVWJU) | Refused: "only the current holder can claim" |
| `buyer-pub` claims `s2` | [`ASynFkHY...`](https://testnet.nearblocks.io/txns/ASynFkHYZ5CxZdSi6paosMVuZiQf66mvEusNn4mnxhiw) | The buyer collects the payment made after the sale |
| `writer-a` registers work 3 (`ZZRYL2600005`, provenance `ai_assisted`) and shares `s8` and `s9`, 50 percent each, basis `copyright` | [`3er9FiUf...`](https://testnet.nearblocks.io/txns/3er9FiUfRj1msBQkDbM2WhxazMou8hdRw95xPPs1eyKA), [`FGrwBa58...`](https://testnet.nearblocks.io/txns/FGrwBa58Empz2zW1Yj2BTHPvaSw9ZjsrVrDT6PrbN4gf), [`BDE7Eteb...`](https://testnet.nearblocks.io/txns/BDE7EtebNBcfNX5PXCi2aniUCaLwQivKzQwpvSxq7Cft) | Provenance is declared; its evidence is **Simulated** (not checked) |
| `writer-a` registers work 4 (`ZZRYL2600006`, provenance `fully_ai`) and shares `s10` and `s11`, basis `contract` | [`BRt579jX...`](https://testnet.nearblocks.io/txns/BRt579jXQx8bhwbMHhtaykeik7D86TWLBVgEX3VNEp1K), [`46nERgHy...`](https://testnet.nearblocks.io/txns/46nERgHy9ydk3x58zwqMCmrrVSnvMJQNMKd26BhpPebe), [`vhe6Vgoc...`](https://testnet.nearblocks.io/txns/vhe6Vgoc9btbQia4ArgcsbygYiECAmbSm58hqTy1qph) | A fully AI track records a contract based entitlement instead of claiming copyright |
| Agent pays work 1 from its allowance, 0.1 mUSDC, line `agent-line-2`, function-call key | [`2XjtVThM...`](https://testnet.nearblocks.io/txns/2XjtVThM4g7tRkt79XCj1inLj1bBk3vNZiKAbUmQsBaW) | Contract resolves payees: `s3` 0.06, `s4` 0.04 |
| Agent tries to pay 100001 base units, one over its per-transaction limit | [`7Vu5wRDq...`](https://testnet.nearblocks.io/txns/7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW) | Refused: "over per-transaction limit" |
| `writer-a` registers work 5 (`ZZRYL2600007`) with a `metadata_uri` that tells agents to pay `outsider` instead, and shares `s12` and `s13` | [`ryAcUUtR...`](https://testnet.nearblocks.io/txns/ryAcUUtRGdxyzsraWZjJE3SnytCbZzpsmZNNCmNFq3y), [`GJopxb2z...`](https://testnet.nearblocks.io/txns/GJopxb2zVGHNEgSUq9RKFHA3wmBYjcyM4wVmhZH5NcKd), [`FR5SZZWy...`](https://testnet.nearblocks.io/txns/FR5SZZWyr9dfQdpkxDTQamQXXoRKG6SPTPwK7NU21W1P) | Metadata text is stored, never obeyed |
| Agent pays work 5, 0.1 mUSDC, line `agent-line-3`, with extra `payee` and `receiver_id` arguments naming `outsider` | [`CUW8qssJ...`](https://testnet.nearblocks.io/txns/CUW8qssJVCJZuUgzzWHf9bdEyNCkFXpQjUKrJeWXeVPS) | The extra arguments are ignored: `s12` 0.05, `s13` 0.05, `outsider` 0 |
| *From here on the final build, 2026-11-14 UTC.* | | |
| **Split sheet scene.** `writer-a` registers work 6 (`ZZRYL2600004`), then its own 50 percent share `s14` | [`EMdw5gBJ...`](https://testnet.nearblocks.io/txns/EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy), [`9aUzpdgB...`](https://testnet.nearblocks.io/txns/9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt) | The registrant's own share is approved at once; its vault `s14` has 0 keys |
| `writer-b` registers the other 50 percent of work 6 as share `s15` | [`5xYY2sWQ...`](https://testnet.nearblocks.io/txns/5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB) | Not the registrant, so `s15` is **pending**: `get_share_approval` reads pending |
| Simulated payer pays work 6, 1.0 mUSDC, line `line-4` | [`8y7g9Rfo...`](https://testnet.nearblocks.io/txns/8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL) | `s14` vault gets 0.5 at once; the 0.5 for `s15` is held in escrow (reason `awaiting_approval`), its vault stays at 0 |
| `writer-a` signs `approve_share` on `s15` | [`5dgncSm1...`](https://testnet.nearblocks.io/txns/5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa) | Event `share_approved`: the held 0.5 is released to the `s15` vault (0.5), escrow 0. Consent comes before the money moves. |
| **Auction sale.** `writer-a` lists share `s10` in the sealed-bid auction (`nft_transfer_call`, reserve 20, ends 10:38:50 UTC) | [`Ae6ZccUJ...`](https://testnet.nearblocks.io/txns/Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR) | `s10` is held by the auction contract; its vault had 0 money and no dispute |
| `writer-b` and `buyer-pub` each place a sealed bid with a 100 mUSDC deposit (sealed amounts 70 and 90) | [`GHCfH4Ph...`](https://testnet.nearblocks.io/txns/GHCfH4PhhcNCG4mmDCZXKmy9WvDucEnhCmerYVSjNm1v), [`AK88NkPk...`](https://testnet.nearblocks.io/txns/AK88NkPkXsik1xPbTNDNZB6VCCRi2rKhxeVbap9bHkSc) | Only ciphertext is on chain; nobody can read the amounts before the end |
| `outsider` asks OutLayer directly to open the bids, naming the auction as contract, and then naming itself | [`EaHtbQ54...`](https://testnet.nearblocks.io/txns/EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM), [`57tzs8Nb...`](https://testnet.nearblocks.io/txns/57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP) | Both refused by the OutLayer keystore (`Access denied by access condition`), 0 instructions run, return value null. Run before the listing. |
| Settle through the real OutLayer TEE | [`CATqXVEa...`](https://testnet.nearblocks.io/txns/CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW) | Sold to `buyer-pub` at the second price, 70 mUSDC; `s10` is now owned by `buyer-pub`. `outlayer-verify` passed all 5 checks on this transaction (the verifier output is kept as evidence in `deployments/auction-settle-verify.txt`) |
| `withdraw` for the three accounts | [`2PMK4vf1...`](https://testnet.nearblocks.io/txns/2PMK4vf1dKsPNhZwtNkWL2xninc7ZNRN515HiwndQdVY) | Seller +70, losing bidder refunded 100, winner change 30; the auction owes 0 afterwards |

Per-token history: each share's `nft_token` metadata `extra` carries earned, paid out, claimable, status and locks, and every change is a NEP-297 event (`EVENT_JSON:`, standard `share_registry`) in the transactions above. The full transaction list (87 on-chain transactions including the three registry redeploys, step 16 and the auction demo, plus the one refused locally above) is in `deployments/testnet.json`. The auction contract is in `contracts/auction` and its tests are listed under [Threat model and tests](#threat-model-and-tests); the auction rows above are here because the sale moved share `s10` (the record's `token_owners` lists `buyer-pub` as its owner, and `npm run verify:testnet` checks it).

Zero-keys proof without Node, with curl:

```bash
curl -s https://rpc.testnet.near.org -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"query","params":{"request_type":"view_access_key_list","finality":"final","account_id":"s2.reg.royltee-dev.testnet"}}'
```

See: `"keys":[]` in the result.

### Keys on testnet vs mainnet

On testnet the registry and mock USDC full-access keys are kept, by the owner's decision on 2026-11-14 (DEPLOY_PLAN step 6 is not run). This is a testnet demo with Simulated money, and keeping the keys means a late fix can still be deployed in place. It also means T13 is open here: whoever holds the registry key can swap the code, which is why this README never calls the registry keyless. `npm run verify:testnet` checks that the live keys equal the record, so a change cannot go unseen. The sealed-bid auction account keeps its own full-access key too, because its recovery calls need it.

On mainnet both full-access keys would be deleted after the final deploy (or upgrades handed to a multisig or DAO with a delay), and the same zero-keys check that the 15 share vaults pass today would prove it for the registry and the token: `view_access_key_list` returns `"keys":[]` and the code hash equals a local build. After that point no code can be fixed, so the deploy would be preceded by the full security pack and a real wallet test of the site's signing screens.

## Website and help chat

The site is an Astro app in `site/`, hosted at https://royltee.vercel.app (hosted state UNVERIFIED, see the note under Steps). This section describes the site on the merged tree (contract, server and UI branches).

- Live reads: most pages read NEAR testnet from your browser with read-only RPC view calls (`call_function`, `view_account` and `view_access_key_list` in `site/src/scripts/chain.ts`, sent to `test.rpc.fastnear.com` with `rpc.testnet.near.org` as the fallback). A view call cannot change anything on chain.
- Recorded demo: the demo steps, mints, transfers and registrar events (transaction ids and times) are copied into `site/src/data/testnet.ts` from `deployments/testnet.json` and this README. Pages that show them link each row to its transaction on the explorer.
- Signing: only `/member` (Your shares) sends transactions, and only after you connect a testnet wallet and approve each call in it. `/help` and `/admin` ask the wallet to sign a message to sign in, never a transaction. Sending transactions from the site has not been run on testnet yet: UNVERIFIED.

| Page | What it shows | Testnet reads |
|---|---|---|
| `/` | Landing page: the pitch, the recorded demo steps with explorer links, Real and Simulated, and a curl check that needs no wallet | None (recorded demo) |
| `/dashboard/` | Overview: the registry statement since mint and what needs attention | Live |
| `/activity` | The recorded on-chain demo, newest first | None (recorded demo) |
| `/catalog/` | Every registered work and how its rights are split | Live |
| `/catalog/lookup` | Find a work by ISRC or ISWC, a share by id, or everything an account holds, and who gets paid now | Live |
| `/catalog/splits` | One split sheet per work, with a check that each right adds up to 100 percent | Live |
| `/shares/` | Ledger: every share NFT with its holder, role, percent, state and what it can claim | Live |
| `/shares/accounts` | Each share's payout account (vault): access keys, NEAR kept for storage and Mock USDC balance | Live |
| `/shares/claims` | Money waiting in share accounts for their holders | Live |
| `/share?id=s1` | One share: holder, the account it controls, the standard NFT data a wallet reads, and its recorded moves | Live, plus recorded moves |
| `/disputes` | Disputes and the money held in escrow | Live, plus a link to the recorded resolve step |
| `/registrar` | Registrar powers, the recorded registrar events, and member credential status | Live credentials and keys, recorded events |
| `/agents` | What a payment agent may do, and its keys | Live |
| `/member` | Your shares: look up any account without a wallet; with a connected wallet, claim, transfer, approve split sheets, draft shares and manage an organisation's agents | Live; transactions only from your wallet (UNVERIFIED on testnet) |
| `/verify` | The code hash on each contract, the share accounts with no keys, and the keys still on the contract accounts | Live |
| `/docs` | The accounts, the contract methods the pages read, and how to check the demo without a wallet | None |
| `/help` | Help chat and wallet support tickets | None |
| `/admin` | Support admin: the server checks the admin wallet on every request and answers 401 when not signed in | None |
| `/whitepaper` | Placeholder until the whitepaper is approved | None |

- Marked Simulated on the pages: payer steps in the recorded demo (badge "Simulated payer"), the identity checks behind member credentials, dispute documents and arbitration (hashes of dummy documents), and the document hashes in registrar events (placeholder test values). Live values are the real testnet state of these demo accounts.
- Split sheet approval (`approve_share`) is deployed on testnet since step 15. The site checks live whether the method exists and says so when it does not.
- Help chat: uses NEAR AI Cloud. A reply shows the label "Inference ran in a TEE (reported by NEAR AI Cloud)" only when NEAR AI Cloud reports `signature_kind: provider_tee` for that completion. The app reads the TEE flag the provider reports; it does not verify the attestation itself. The contracts use no TEE. (The name RoylTEE does not mean the contracts run in a TEE.)
- Support tickets: wallet sign-in with a NEP-413 signed message checked on the server against a full-access key of the testnet account. Live wallet sign-in on the hosted site is UNVERIFIED.
- Details, limits, environment variable names and security headers: [site/README.md](site/README.md). No secrets are in the repo; values live in the host's settings.

Screenshots: none yet. **Pre-submission item:** add the landing page, the help chat with the TEE label and the explorer view of the catalog sale, or remove this line.

## Simulated vs real

| Real (on NEAR testnet or in the tests) | Simulated |
|---|---|
| Registry, vault and Mock USDC (testnet) contracts | Identity checks (KYB) behind a credential |
| Vault accounts with zero keys | The fiat rail and conversion to USDC (the Simulated payer mints Mock USDC instead) |
| Mock USDC flows: payments, splits, claims, transfers | Payer statements and statement line ids |
| Agent limits, idempotency, revocation | Payer attestations by the Simulated payer |
| Sealed-bid auction settle run through the real OutLayer TEE on testnet, with `outlayer-verify` passing 5 of 5 checks on that transaction (fairness still rests on the executor, B-03) | |
| Locks (`payout_in_flight`, `transfer_pending`), credentials, disputes and escrow | Provenance evidence (the field is real, the evidence is not checked) |
| Events and per-token history | Arbitration and dispute documents (hashes of dummy documents) |
| | TEE attestation of payer statements (not built; roadmap) |
| | OutLayer in the auction tests (`contracts/mock-outlayer`); auction money is mock USDC |
| Site pages' live values (read-only view calls to NEAR testnet) | Payer steps in the site's recorded demo (badge "Simulated payer") |
| | Document hashes in the site's registrar events (placeholder test values) |

Mock USDC (testnet) is used instead of Circle USDC because a Circle USDC address on NEAR testnet is UNVERIFIED.

## What the token legally is

A share NFT records an assignment as declared by members, with document hashes of the evidence (in this demo the identity checks and evidence are Simulated), plus the payment instruction it routes. The registry does not check that a signed legal assignment exists: that verification, and any binding legal effect, are UNVERIFIED and planned. It is evidence and a payout address, not legal title. Legal effect in production would come from a participation agreement: paying the registered share account discharges the payer, claimants indemnify payers, and disputes follow the registry process. That agreement is **roadmap**; it does not exist today. Nothing here is legal advice.

## TEE use: OutLayer call path and the auction settle

Where a TEE is used: only the sealed-bid auction's settle step runs inside one (OutLayer's TEE, see [Sealed-bid share auction](#sealed-bid-share-auction)). The registry, vault and mock USDC contracts do not use a TEE: they run as ordinary NEAR contracts. The help chat TEE label is separate: the app reads what NEAR AI Cloud reports (`signature_kind: provider_tee`) and does not verify an attestation itself.

A throwaway testnet probe, not part of the registry, was used to check one OutLayer call path. The contract at `probe.royltee-dev.testnet` called `request_execution` on `outlayer.testnet` with a small WASI echo module. The callback stored the echo of `{"hello":"tee"}`. `outlayer-verify` (git `90c27d9`) then returned PROVEN on all five checks for transaction [`7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1`](https://testnet.nearblocks.io/txns/7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1): Authenticity (genuine Intel TDX, TCB UpToDate), Identity (the worker build is approved on `worker.outlayer.testnet`), Binding, Input (the request bytes `{"hello":"tee"}` hash to `409dcbcaf2467885300523cda3cfba1fc4f5a02c11dac1e930aad5202aeb40de`), and Output (hash `69249e97227b6c576a795b2cf000fcba3e804258c1b9386fba0e4f89fdf344c1`).

This was a call-path proof only. The sealed-bid auction below was then deployed on testnet, refused two outsider requests and settled one sale through the real OutLayer TEE on 2026-11-14 (UTC); `outlayer-verify` passed all five checks on that settle transaction.

OutLayer's own docs say it runs on single operator keys today (UNVERIFIED beyond those docs). A NEAR contract cannot check the quote itself.

How to rebuild, re-test and re-verify the probe and the echo module, including the online command (which needs `--api https://testnet-api.outlayer.ai` because the tool's default API address is retired) and `outlayer-verify bundle verify-bundle.json --offline`, is in [spike/outlayer/README.md](spike/outlayer/README.md).

## Sealed-bid share auction

Status: deployed on NEAR testnet at `auction.royltee-dev.testnet` on 2026-11-14 (UTC) and run once end to end with Simulated mock USDC (steps 17a to 17m of [DEPLOY_PLAN.md](DEPLOY_PLAN.md) step 17; the record is `deployments/auction-testnet.json`). It is also built and tested locally (unit, sandbox and Node tests). The auction has been reviewed by two external reviewers; every finding is mapped to a fix and a test in [THREAT_MODEL.md](THREAT_MODEL.md) (rows T-A1 to T-A27).

**This is a prototype with explicit operator and executor trust.** Four things are stated plainly and are not hidden by the tests: (1) B-02: the live OutLayer keystore refused both outsider requests on chain before any listing (DEPLOY_PLAN 17g, two transactions, see the table below), so the access rule is enforced for those two attacks; that does not prove the module's own caller check on the live worker, because the keystore refused first (the module check is covered by the sandbox and Node tests). (2) B-03: fairness and bid authorization rest on the OutLayer executor. The contract bounds the answer and conserves escrow; it does not prove that bids were decrypted honestly or that the second price was computed correctly, and nothing here is verified fair TEE execution. (3) B-05: recovery is not autonomous. A delivery or return whose callback is lost stays pending until the operator, using the auction account's retained full-access key, settles it from the final receipt outcome. Nothing is decided from the current owner of the share or from elapsed time. (4) B-10: a share sent with a plain `nft_transfer` is not tracked by the auction; only a privileged rescue (registrar or key holder) can move it, per token, by hand. Do not put real value in it.

**Live run on testnet, 2026-11-14 UTC (all mock USDC is Simulated).** Full record: `deployments/auction-testnet.json`; plan and results: DEPLOY_PLAN 17a to 17m.

| Step | Transaction | Result |
|---|---|---|
| 17a Create `auction.royltee-dev.testnet` (6 NEAR) | [`6c1KYbcD...`](https://testnet.nearblocks.io/txns/6c1KYbcDcNvYqzLUMTyiPWWoouSLprQhViPrtoY4punJ) | Account with one full-access key (kept on purpose, see the trust list below) |
| 17b FastFS upload of the WASI module | [`4c4xPqRu...`](https://testnet.nearblocks.io/txns/4c4xPqRuuhfcmj949aMzDB8VuvxGYFNiWK6B5cxMBrG1) | The call returns MethodNotFound on the receiver, by design of FastFS (it reads the transaction data); FastFS serves the module with sha256 `c4a29e55ca61da2e069bdcfebd4d4e6690820620582ed2635d1dd6bc17c2594e` |
| 17c Deploy and init in one transaction | [`EWWMu3ch...`](https://testnet.nearblocks.io/txns/EWWMu3chsa56hBybTpZ5hgiM6bazXRBekESv7KzpWYEt) | Code sha256 `48d32c71de83d94a4225502a726b80bd678ac3dfb1471952c93c4c16595a8d09`; `get_config` read back |
| 17d Registrar credential | [`2zysmt9B...`](https://testnet.nearblocks.io/txns/2zysmt9BZCbFs3bcVQSdwQPouPNgtt4W1DH7RHMw4eeG) | `is_active_member` true |
| 17e Secret access rule | set by the owner in the OutLayer dashboard (no transaction id recorded) | `And[WasmHash(c4a29e55...), Predecessor Whitelist[auction.royltee-dev.testnet]]`, read back with `get_secrets` |
| 17f `publish_key` | [`7TLqoWrn...`](https://testnet.nearblocks.io/txns/7TLqoWrnBbTupg3PLccuc2eXgbquXACtYZqGDNgV58fq) | OutLayer request 44227; TEE key `6260be00...a692f`, key_version 1 |
| 17g Outsider asks OutLayer directly, naming the auction, then itself | [`EaHtbQ54...`](https://testnet.nearblocks.io/txns/EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM), [`57tzs8Nb...`](https://testnet.nearblocks.io/txns/57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP) | Both refused by the keystore (`Access denied by access condition`), 0 instructions run, return null; OutLayer charged 0.001 NEAR each and refunded 0.019 |
| 17h Mint 200 mUSDC each to the two bidders | [`9kwufV5F...`](https://testnet.nearblocks.io/txns/9kwufV5FfUjXbZVwN662kawt2Q9RDRz9Hb6WiYt4vZY), [`6KZx5zga...`](https://testnet.nearblocks.io/txns/6KZx5zga2jSAtWV1PPfucAra6RfECaSmWzx1v2wBnTmj) | Simulated money |
| 17i Seller lists share `s10` (reserve 20, max deposit 100, ends 10:38:50 UTC) | [`Ae6ZccUJ...`](https://testnet.nearblocks.io/txns/Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR) | Auction 0 open; the end time is 20 minutes after sending (the plan said 10) so that both bids fit |
| 17j Two sealed bids (sealed 70 and 90, deposit 100 each) | [`GHCfH4Ph...`](https://testnet.nearblocks.io/txns/GHCfH4PhhcNCG4mmDCZXKmy9WvDucEnhCmerYVSjNm1v), [`AK88NkPk...`](https://testnet.nearblocks.io/txns/AK88NkPkXsik1xPbTNDNZB6VCCRi2rKhxeVbap9bHkSc) | Only ciphertext on chain |
| 17k Settle through the real OutLayer TEE | [`CATqXVEa...`](https://testnet.nearblocks.io/txns/CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW) | Sold to `buyer-pub` at the second price, 70 mUSDC; `s10` now owned by `buyer-pub`. OutLayer request 44422 charged 0.0010383 NEAR (estimate was at most 0.0040001) and refunded 0.0189617. `outlayer-verify` (git `90c27d9`): Authenticity, Identity, Binding, Input and Output all PASS (output in `deployments/auction-settle-verify.txt`) |
| 17m `withdraw` for the three accounts | [`2PMK4vf1...`](https://testnet.nearblocks.io/txns/2PMK4vf1dKsPNhZwtNkWL2xninc7ZNRN515HiwndQdVY) | Seller +70, loser refunded 100, winner change 30; owed_total, pending and escrow all 0 |

Not run: 17l (recovery calls, not needed) and 17n (deleting the auction key, not run by decision). The `outlayer-verify` check proves that this input produced this output inside genuine Intel TDX hardware running the approved build. It does not prove the auction rules were followed (fairness stays on the executor, B-03).

What it does: a holder lists one share for a private auction. Bidders send an encrypted bid. After the end time, a WASI module running in OutLayer's TEE opens the bids and prints only the winner and the price. The contract then moves the share and the money. Bids other than the second highest are never published (see "What stays private").

How a sale runs:

1. **List.** The seller calls the registry's `nft_transfer_call` to the auction with `{"reserve","max_deposit","ends_at"}`. The auction keeps the share (it returns `false` from `nft_on_transfer`). A listing is refused (and the registry hands the share back) if `reserve` or `max_deposit` has more than 38 digits, or if the contract's free NEAR does not cover the floor plus a storage reserve for every open auction. It needs no registry change: it is a normal credentialed member at `auction.royltee-dev.testnet`, a sibling of the registry account because the registry blocks system names under `reg`. Always list with `nft_transfer_call`: a plain `nft_transfer` to the auction account leaves the share stranded with no auction (T-A16). A seller cannot be charged a listing deposit (the registry's `nft_transfer_call` carries no NEAR), so the contract's own balance holds the reserve and the owner keeps it topped up (T-A22).
2. **Bid.** A bidder runs `node scripts/auction/encrypt-bid.mjs`, which seals the amount to the TEE public key (X25519, HKDF-SHA256, ChaCha20-Poly1305, bound to the auction id, the contract and the bidder). The amount is zero padded to 38 digits so the ciphertext length leaks nothing, and the script refuses an amount outside the reserve and `max_deposit`. They send exactly `max_deposit` mock USDC with `ft_transfer_call` and the ciphertext as the message. Every deposit is the same size, so the deposit leaks nothing about the bid. Only active members can bid; at most 8 bids per auction.
3. **Settle.** After `ends_at`, anyone calls `settle` (230 Tgas at least, 0.01 NEAR at least for OutLayer, the rest is refunded). The contract calls `request_execution` on `outlayer.testnet` with the module (pinned by sha256). The module prints `{winner, price, bids_hash}`. The price is the second highest valid bid, or the reserve. The contract accepts the answer only if the winner is a bidder, the price is between the reserve and `max_deposit`, and `bids_hash` matches the bid list it sent.
4. **Pay.** The share goes to the winner first (the registry re-checks membership). Only then the contract credits the seller the price, the winner the change, and each loser the full deposit. The bid ciphertexts are dropped and the auction's storage reserve is released. Money leaves with `withdraw`, which debits first, writes a pending record, sends, and in the callback closes the record and restores the credit if the send failed. A lost callback leaves the pending record visible; it is never refunded automatically, because a late transfer could still land (T-A24).
5. **Failure and recovery.** A failed or empty answer reopens settling. A refused delivery or a stalled settle (1 hour) ends in a cancelled auction: every deposit is credited back and the share returns to the seller. Anyone can also cancel 6 hours after `ends_at` once one settle run has failed, and 7 days after `ends_at` in any case. If a delivery or a share return never reports back (for example the contract was short of NEAR for storage), the auction stays in `Delivering` or keeps `returning` set, and nothing is decided from who holds the share or from how much time has passed. After 15 minutes anyone can call `reconcile`: it reads the registry and emits `reconcile_pending` with the owner it saw, as a diagnostic. It never finishes, cancels, refunds or pays anything, even when the winner holds the share. A delivery is settled only by its own `on_delivered` callback or by the operator with `resolve_delivery`, `resolve_return` and `resolve_withdrawal`, after the final outcome of the receipt in question is known and with the sha256 of that evidence. Those three calls only work when the auction account itself calls them (T-A14, T-A25).

Two layers keep the module from being used as a decryption oracle. All ciphertexts are public, so without them anyone could ask OutLayer to run the module on a crafted request and read a live bid. First, the module fails every request unless `NEAR_PREDECESSOR_ID`, the account that called OutLayer, equals the auction account compiled into the module (so it is part of the pinned hash; an attacker contract that names itself in the request is refused), and the request names the same account. Second, the OutLayer secret must only be readable by a run of this exact build that arrives through the auction contract (`And[WasmHash, Predecessor{Whitelist[auction]}]`); a signer whitelist alone is not enough. DEPLOY_PLAN step 17g tests the refusal on chain before any share is listed. The module also checks that the public key the contract published is the key of its secret, and fails with an explicit error if not (a changed master secret is a failed run, not a silent no sale).

What stays private, and what does not:

- The winner's own top bid and all losing bids except the second highest stay inside the module.
- The sale price is the second highest valid bid and it is public. With two bidders the price is the loser's exact bid, so it is not true that losing bids are never published.
- Account names, the deposit and the time of each bid are public.

What you can and cannot trust:

- Real in the tests: the registry, vault and mock USDC wasm, the auction contract, and the WASI module (run under Node's WASI). Simulated in the tests: OutLayer itself (`contracts/mock-outlayer`), and all money is mock USDC. The real OutLayer was called by this auction on testnet on 2026-11-14 (17f, 17g twice, 17k); see the live table above. That run is one rehearsal on a demo share, not a load or a long run.
- Bid secrecy rests on the TEE and on OutLayer's operators. OutLayer's own docs describe single operator keys today (UNVERIFIED for testnet). A NEAR contract cannot verify an attestation; verification is off chain with `outlayer-verify`.
- Fairness and bid authorization rest on the OutLayer executor. Conservation of escrow holds regardless: the contract never pays out more than it holds, a winner must be a bidder and the price must lie in [reserve, `max_deposit`]. Nothing on chain proves the bid amounts or the second price rule. The contract bounds the answer, it does not verify it. Whoever controls the OutLayer answer can name any bidder as winner at any price between the reserve and `max_deposit`: a winner can be charged the full deposit, or the seller paid only the reserve, and the contract cannot tell. Money is not fully safe against a bad operator; the loss is capped by `max_deposit` per bidder and the share. There is no on-chain fix.
- The contract has no owner or admin method. Stuck states are resolved by `resolve_delivery`, `resolve_return` and `resolve_withdrawal`, which accept only the auction account itself as caller, so they run with the account's retained full-access key. This is a stated trust: while the key exists its holder can also claim vault money of listed shares, move all escrow and redeploy the contract. The key must not be deleted while any auction is Delivering or returning, or while `pending_total` is not zero, because nothing else can finish those states. Deleting it after everything is final is the owner's decision after the rehearsal, not part of the default plan.
- Storage: each open auction holds a reserve of 8000 bytes, released when it ends. Terminal auctions that had bids keep their record (about 2 KB at most) and the contract pays for it, so history grows without bound if the auction is used a lot; the pending withdrawal records written by `withdraw` are not covered by that reserve (each is about 16 bytes larger than the credit it replaces), so `withdraw` refuses to run unless the contract has at least 0.25 NEAR free, and the owner keeps the account topped up. A short contract fails the call before anything is debited. Storage faults are made unlikely, not impossible: the reserve is a measured byte count and not proven on a live node (UNVERIFIED).
- The TEE public key is stored once and is permanent, and it must not be a low order point. If the master secret were ever replaced, every settle would fail with a key mismatch error and the cancel rules would refund everyone.
- Unclaimed vault money travels with the share, like any registry transfer. Sellers should claim before they list.
- Junk bids with a valid deposit can fill the eight slots; each needs its own credentialed member and the deposit is refunded in full (T-A17).
- If OutLayer rejects a request up front, the settle deposit stays in the auction balance.
- Proven on testnet on 2026-11-14 (UTC) by the live run: the live worker sets `NEAR_PREDECESSOR_ID` for a direct WasmUrl run through the auction (the module accepted the settle); the live keystore enforces the `Predecessor` rule (both outsider requests refused); the dashboard built the tightest access condition; OutLayer accepted a hash-bound secret for a WasmUrl run; one settle cost 0.0010383 NEAR (the `estimate_execution_cost` view gave at most 0.0040001 NEAR); the answer OutLayer returned was the guest JSON and the contract accepted it. Still UNVERIFIED: that the live worker and keystore run the source version read at 2d50bfa (the observed behaviour matches it); a live measurement of the storage reserve; a run with more than two bids; a failed or empty answer on the live OutLayer.

Initialisation: `new` only accepts the contract account itself, and the plan deploys and initialises in one transaction signed by that account, then reads back every pinned field (T-A23).

Build notes: the WASI module's own dependencies are pinned to exact versions and its `Cargo.lock` is committed. `scripts/build-wasm.sh` builds it twice: the shipped file `auction_wasi.wasm` serves `auction.royltee-dev.testnet`, and `auction_wasi_test.wasm` (never deployed) differs only in the account compiled in (`auction.test.near`, the name the sandbox gives the auction), so the sandbox tests run the real module logic. Set `ROYLTEE_AUCTION_ACCOUNT` when building for another account.

Gas, measured in the sandbox with eight valid bids of the largest size the contract accepts (512 characters each) and 230 Tgas attached: the settle receipt burnt 4.1 Tgas, the answer callback `on_settled` 3.2 Tgas, the delivery callback `on_delivered` 3.7 Tgas, 18.9 Tgas for the whole transaction. The answer callback has 120 Tgas and must leave 85 Tgas for the two promises it creates.

Reproducible build (the lock files `Cargo.lock` and `tee/auction-wasi/Cargo.lock` are committed): toolchain Rust 1.99.1 (`rustc 1.99.1 (58cf8f9c3 2026-10-13)`, pinned by `rust-toolchain.toml`) with the `wasm32-unknown-unknown` target, then `npm run build:wasm` from a clean checkout (it runs `scripts/build-wasm.sh`, which uses `--locked`, `--remap-path-prefix` for the cargo home and the repo root, and builds the module with `ROYLTEE_AUCTION_ACCOUNT=auction.royltee-dev.testnet`). The WASI module does not compile without `ROYLTEE_AUCTION_ACCOUNT`. The hashes in `deployments/` (`auction-testnet.json` holds the deployed code hash and the module hash, both equal to the local build) and the review packet come from that command on one machine; two clean builds there gave the same bytes. A match on another machine is UNVERIFIED (a reviewer could not reproduce the earlier hashes, likely a different toolchain or an unset account).

Tests: 68 contract unit tests, 14 WASI host tests, 18 Node tests, 15 sandbox tests (`contracts/tests/tests/auction.rs`, a01 to a15: a11 eight bids at 300 Tgas, a12 eight maximum size ciphertexts at 230 Tgas, a13 a direct request from another caller is refused, a14 a sale completes with almost no spare NEAR, a15 listing counts the storage reserve of every open auction). The delivery recovery (`reconcile`, `resolve_delivery`, `resolve_return`, `resolve_withdrawal`) is covered by unit tests only, because the sandbox can no longer produce a stuck delivery.

## Roadmap

Not built, in no particular order:

- Participation agreement that gives the record legal effect.
- Multisig registrar council with a time delay.
- Challenger bonds, dispute auto-expiry, tiered panel and appeal, court order execution.
- Selective disclosure of split percentages (a known open problem: today the bps are public on chain).
- Voice and training licenses (and the sync license, cut from this build).
- TEE-attested payer statements.
- A sealed-bid auction in a TEE is built and was run once on testnet (see [Sealed-bid share auction](#sealed-bid-share-auction)). Not built: verified fair TEE execution of the auction rules (B-03), an autonomous recovery path for a lost delivery callback (B-05), and an auction on mainnet.
- NEAR Intents for cross-chain payers.
- DDEX and CWR import.
- Period allocation using `effective_from` (recorded on every transfer today, not used by payout logic).
- Deleting the registry and Mock USDC full-access keys (for a mainnet deployment; kept on this testnet deployment by decision, see [Keys on testnet vs mainnet](#keys-on-testnet-vs-mainnet)).

## Known limits

Residual risks of the registry code now on testnet, from the review passes. None of them lets anyone take another member's claimable money; each is stated so a judge does not have to find it.

- **First registrant of a work** (review 03a M1). The split sheet signer is whoever registered the work first, fixed at that moment. A member who registers someone else's ISRC or ISWC first gets its own shares approved at once; the real owner's later registration of the same code opens a duplicate-code dispute, which freezes nothing on the first work, so the first work keeps being paid until the registrar resolves. After a catalog sale, a seller that registered the work keeps the signer power (it can register auto-approved shares into unallocated bps and approve or reject other shares). Options, not built: the signer follows the current holders, or shares registered after any transfer need the registrar's co-signature, or a duplicate-code dispute also holds the first work's payments.
- **Contest size** (review 03a M2). A claimant names whole shares and may name the largest one, so a 1 bps claim can escrow the new money of a 9999 bps share until resolve. Option, not built: escrow only the excess.
- **Vault `ft_on_transfer`** (review 03a L2). New vaults keep any NEP-141 token sent with `ft_transfer_call`, from any token contract, because the vault is stateless and does not know which token to expect. Tokens other than Mock USDC sent that way stay in the vault for good. It writes nothing, so the ledger is not affected.
- **Failed mints** (review 03a L3). When a mint fails after the vault account was made (holder suspended before mint, or `release_stale_mint`), the keyless vault stays, the NEAR spent on it is not refunded, and a plain deposit sent to it cannot be claimed or recovered.
- **Agent group size** (review 03a L4, audit F1). Each forward reserves 15 Tgas (5 for the token transfer, 10 for the callback) plus 10 Tgas once per payment. With the 100 Tgas the deploy plan uses for agent calls, an agent pays a group of at most 5 shares in one call (sandbox `t13b_agent_group_size_at_100_tgas`); larger groups need more gas, so a larger function-call allowance. A direct payment with 300 Tgas, what most wallets attach, pays the largest group of 16 shares (sandbox `a04_full_group_of_16_is_paid_with_300_tgas`; a compile-time check keeps it that way).
- **Forward gas on testnet** (audit F1). Fixed on testnet by the audit redeploy on 2026-11-14 (code hash `5Taw8Syz7g1QxNa4Faw8mz7fnNgoXGj7MwX2VS3obRFo`, kept by the final redeploy, now `5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG`), which reserves 5 + 10 Tgas per forward. A direct payment to a full group of 16 shares at 300 Tgas is shown in the sandbox only; it has not been sent on testnet (UNVERIFIED there).
- **Lost callbacks** (THREAT_MODEL T19, T19b). A stuck transfer lock is released by anyone after 1200 blocks (about 12 minutes at the 0.59 to 0.60 s block time measured on testnet). A stuck payout lock is never cleared by code: anyone may ask for a read that only reports the vault balance (`payout_uncertain`), and the lock stays until the real late callback or the registrar settles it. A payment forward whose callback never ran can be quarantined by anyone after about a day (`release_stale_forward`, 150000 blocks): it credits nothing and resends nothing, because a balance read cannot show where funds came from and no block count proves the original transfer cannot still land. A quarantined amount or a stuck payout lock is settled by the real late callback, or by the registrar with `resolve_written_off` / `resolve_stale_payout` and the final public receipt outcome (procedure in DEPLOY_PLAN). After a registrar call the old callbacks of that quarantine are ignored, so a late callback cannot settle a later quarantine. This is a stated registrar trust: a wrong answer can pay a share twice from pooled registry funds or credit it what the vault does not hold. If the registrar is gone, such an amount stays stuck (the money is not taken, there is no method to move it). A forward or lock in flight at upgrade time has no record and cannot be recovered; the redeploy is one transaction that deploys the code and calls `assert_quiescent`, which fails and rolls the deploy back unless the registry is quiet (`npm run check:quiescent` is the precheck). A callback that never commits cannot be produced on a sandbox node: the sandbox tests write the state it would leave and run the real recovery calls. Without a registrar a stale payout lock stays and that share is frozen; the money stays in its vault. These methods exist on testnet only after the final redeploy (see Live testnet deployment).
- **Registrar trust and the registry key**: see [Who can hold a token](#who-can-hold-a-token) and THREAT_MODEL T12, T13.
- **Not done, by decision on 2026-11-14.** The fixes for M1 and M2 above (each needs a registry redeploy). An extra security pack on the final code. A test of the site's signing screens with a real wallet: they stay UNVERIFIED. A payer role for the `payer` account on chain. Keys: see [Keys on testnet vs mainnet](#keys-on-testnet-vs-mainnet).

## Libraries, templates and AI-written code

Disclosed as the hackathon rules ask (Q-005). No pre-hackathon code: the first commit is from 2026-11-12 18:48 UTC (`git log --reverse`). That is inside the hackathon window: the rules (HACKATHON.md, Q-001, https://nearhackathon.com/HACKATHON.md) count code from the announcement on 2026-10-19.

- Contracts (Rust): `near-sdk` 5.30.1, `near-contract-standards` 5.30.1 (NEP-171, NEP-177, NEP-181, NEP-141, NEP-145 helpers), `near-sys` 0.4.1 (the vault is written against raw host functions, no SDK).
- Contract tests: `near-workspaces` 0.24.0 (downloads the NEAR sandbox node), `tokio`, `anyhow`, `serde_json`. Exact versions in `Cargo.lock`.
- Site: Astro with `@astrojs/vercel` and `@astrojs/check`, `@hot-labs/near-connect` (wallet connection), `near-api-js`, `@scure/base`, `@scure/bip39`, `gsap`, `@fontsource-variable/inter-tight` and `@fontsource-variable/jetbrains-mono`, TypeScript, Vitest, and `wasmoon` (a Lua engine, tests only: it runs the Redis counter scripts in `site/tests/audit-f6-store-counter.test.ts`). Exact versions in `site/package-lock.json`.
- Services: NEAR testnet RPC, NEAR AI Cloud (help chat model), Upstash Redis (chat and ticket storage, called over its REST API with `fetch`, no client library), Vercel (hosting), OutLayer on NEAR testnet (the auction's TEE settle, and the earlier call-path spike in [spike/outlayer/README.md](spike/outlayer/README.md)).
- Templates and boilerplate: no starter template repository was used (`site/package.json` has no scaffold marks, but whether a CLI scaffold such as `npm create astro` was used for the first site commit is UNVERIFIED). Standard library patterns (for example the NEP-171 and NEP-141 helpers above) are used as the libraries document them.
- AI-written code: the contracts, tests, scripts, site and docs in this repository were written with AI coding agents, directed and reviewed by the team, and checked by the tests above, a manual audit pass ([AUDIT.md](AUDIT.md)) and separate review passes. The help chat itself calls a model on NEAR AI Cloud at run time.

## Repository layout

```
contracts/registry       Rust, near-sdk: NEP-171/177/181 shares, vault factory, ledger, credentials, agents, disputes, split sheet
contracts/vault          Rust, raw host functions: the keyless vault (withdraw, ft_on_transfer)
contracts/mock-usdc      Rust: Mock USDC (testnet), NEP-141, 6 decimals
contracts/test-receiver  Rust: member contract used by the sandbox bounce test
contracts/auction        Rust, near-sdk: sealed-bid share auction (deployed on testnet, record in deployments/auction-testnet.json)
contracts/mock-outlayer  Rust: TEST ONLY stand-in for OutLayer (Simulated), used by the auction sandbox tests
tee/auction-wasi         Rust, wasm32 WASI module that opens bids inside the TEE (own Cargo workspace)
scripts/auction          Node: bid encryption script, WASI runner, tests
contracts/tests          near-workspaces sandbox tests; fixtures/ holds the registry wasm deployed on testnet (upgrade test)
scripts/build-wasm.sh    builds every contract into contracts/res/ with local paths remapped, checks no local path is left, prints sha256
scripts/verify-testnet.mjs  read-only check of the live deployment against deployments/testnet.json and deployments/auction-testnet.json (auction account, code hash, keys, owner of s10)
deployments/testnet.json accounts, public keys, code hashes, transaction ids (no secrets)
deployments/auction-testnet.json  the auction account, WASI module hash, OutLayer requests, auction 0 result (no secrets)
deployments/auction-settle-verify.txt  the outlayer-verify output for the settle transaction (5 of 5 checks)
config/project.json      project name, NFT symbol, mock token name
site/src/pages           the pages listed under Website and help chat, plus the API route (api/[...route].ts)
site/src/scripts         browser code: read-only testnet view calls (chain.ts), wallet sign in and transactions (wallet.ts)
site/src/server          server side: help chat, tickets, sign in, admin checks, rate limits, security headers
site/src/data/testnet.ts accounts and the recorded demo, copied from deployments/testnet.json
site/tests               Vitest tests for the site (231 in 22 files)
.github/workflows/ci.yml CI: wasm build, fmt, clippy, Rust tests, site typecheck, tests and build, gitleaks
spike/outlayer/          throwaway OutLayer call-path spike (its own Cargo workspace, not a root workspace member): echo module, probe contract, evidence bundle
docs/reviews/            status of each external review finding (fixed, documented, deferred or false)
README.md  THREAT_MODEL.md  DESIGN.md  AUDIT.md  DEPLOY_PLAN.md  DECISIONS_LOG.md  SPEND_LOG.md  LICENSE
```

## Built at NEAR HACKATHON

Built at NEAR HACKATHON.
