# Decisions log

Dates are in Bangkok time (UTC+7). "Owner" is the project owner. Items the brief already fixes are marked "owner, via build prompt". Everything else stays "pending owner" until the owner says yes.

| # | Date | Decision | Who approved |
|---|---|---|---|
| 0 | 2026-11-13 | Build prompt read in full; sha256 checked and matched before work started. Testnet only, mock USDC only, no contract code before the block 1 OK. | owner, via build prompt |
| 1 | 2026-11-13 | Work-level payments handled inside the registry: one `ft_transfer_call` to the registry naming the work group, split and forwarded to share vaults in the same receipt chain. No `w<N>` accounts. Fallback documented in DESIGN.md section 2. | owner, 2026-11-13 (row 22) |
| 2 | 2026-11-13 | Rounding: floor per share, remainder to the largest-bps share, ties to the lowest token number. Alternative: largest-remainder method. | owner, 2026-11-13 (row 23) |
| 3 | 2026-11-13 | Vault is stateless, derives its parent from its own account id, exposes only `withdraw`. The brief's vault `balance(token)` is dropped because a view cannot make cross-contract calls; balances are read with `ft_balance_of` on the token. | pending owner |
| 4 | 2026-11-13 | Vault code deployed per account (embedded wasm) by default. Global contract by code hash is an option if sandbox and testnet support it. | owner, 2026-11-13 (row 29) |
| 5 | 2026-11-13 | NEP-178 approvals disabled; `nft_approve`, `nft_revoke`, `nft_revoke_all` panic; `approval_id` must be empty. | owner, via build prompt |
| 6 | 2026-11-13 | A suspended holder keeps the share, keeps accruing, may still claim, and may transfer out to an active member. Cannot receive. Suspended organisation's agents are blocked. | superseded by row 24 (owner, 2026-11-13) |
| 7 | 2026-11-13 | A work-group payment is accepted only if the paid shares sum to exactly 10000 bps; otherwise it is refunded or refused. | superseded by row 25 (owner, 2026-11-13) |
| 8 | 2026-11-13 | Duplicate ISRC or ISWC: payments to the duplicate work are refused (not escrowed) until the registrar resolves; the original work keeps being paid. | owner, 2026-11-13 (row 26) |
| 9 | 2026-11-13 | Direct plain transfers into a contested vault during a dispute are not claimable above the snapshot until resolve, then released to that vault's holder whatever the outcome. | owner, 2026-11-13 (row 27); built in block 4 |
| 10 | 2026-11-13 | Agents must be direct sub-accounts of their organisation, enforced on chain. Agent accounts cannot hold a credential and vice versa. | pending owner (brief sets the naming; enforcement and exclusivity added) |
| 11 | 2026-11-13 | `grant_credential` takes an extra `doc_hash` and is payable so it can storage-register the member with mock USDC. | pending owner |
| 12 | 2026-11-13 | `nft_transfer` and `nft_transfer_call` take an extra optional `effective_from`, recorded only, bounded, ignored by payout logic in the demo. | pending owner (brief asks for `effective_from`; the argument shape is ours) |
| 13 | 2026-11-13 | Custom event standard string `share_registry`, name-neutral; mock token symbol placeholder `mUSDC`; NFT symbol placeholder in `config/project.json`. | owner, 2026-11-13 for `mUSDC` and `RRS` (row 30) |
| 14 | 2026-11-13 | Project name lives only in `config/project.json` and the README title. Contracts contain no project name. | owner, via build prompt |
| 15 | 2026-11-13 | Dynamic token fields in `nft_token` metadata (earned, paid out, claimable, tier, status), from the track example. | owner, 2026-11-13 (row 28) |
| 16 | 2026-11-13 | Holder page gating via a NEP-413 signed message, or label it "not access-controlled". | owner, 2026-11-13: only if time (row 28) |
| 17 | 2026-11-13 | Optional vault view `vault_info` (parent and token id) from the sharded NFT items draft. | pending owner |
| 18 | 2026-11-13 | License fee is paid to one work group named when the registrar posts the price. Licenses stay cut-first. | pending owner |
| 19 | 2026-11-13 | Hackathon rules allow one person up to 2 projects and say each README must name the other. Owner's current setting: README names no other entries. Needs a decision before block 7. | pending owner |
| 20 | 2026-11-13 | This log uses "owner" instead of a personal name, to follow the repo rule of no personal names. | pending owner |
| 21 | 2026-11-13 | Placeholder git author and email from the brief; unsigned commits; no tags; no remote; nothing pushed. | owner, via build prompt |
| 22 | 2026-11-13 | Decision 1: no per-song account. The registry splits each `ft_transfer_call` payment by bps and forwards each part straight into the share vaults. | owner, 2026-11-13 |
| 23 | 2026-11-13 | Decision 2: floor per share; when the paid shares total exactly 10000 bps the leftover goes to the largest share, ties to the lowest token number. Credits always sum to the payment. | owner, 2026-11-13 |
| 24 | 2026-11-13 | Decision 3 (owner's choice, replaces the earlier default and row 6): a suspended member's share stays put and keeps accruing, but the holder cannot claim, cannot transfer out and cannot receive. Only the registrar can move it, with `registrar_transfer` (1 yoctoNEAR, document hash, `registrar_transfer` event), and only while the holder is suspended. | owner, 2026-11-13 |
| 25 | 2026-11-13 | Decision 4 (owner's choice, replaces the earlier default and row 7): a payment to a work + right + territory whose active shares total under 10000 bps pays each registered share its exact floor bps; everything not credited (unassigned bps plus rounding dust) goes to that group's unallocated escrow, held in the registry's token balance. Only the registrar can release it, to an active share of the same group, with `release_unallocated` (1 yoctoNEAR, document hash, `unallocated_released` event, no co-sign). Over 10000 bps is still refused. First item to cut back to "refuse" if time gets tight. | owner, 2026-11-13 |
| 26 | 2026-11-13 | Decision 5: a duplicate ISRC or ISWC registers as a `DuplicateOpen` work and opens a duplicate-code dispute; payments to the duplicate are refused. Resolve lands in block 4. | owner, 2026-11-13 |
| 27 | 2026-11-13 | Decision 6: plain transfers into a disputed vault are held until resolve (block 4). | owner, 2026-11-13 |
| 28 | 2026-11-13 | Decision 7: live `earned`, `paid_out`, `claimable` and lock flags in `nft_token` metadata `extra`. NEP-413 holder gating only if time allows. | owner, 2026-11-13 |
| 29 | 2026-11-13 | Decision 8: each vault gets its own code copy, embedded in the registry wasm and deployed at mint. | owner, 2026-11-13 |
| 30 | 2026-11-13 | Decision 9: placeholder symbols `mUSDC` (mock token) and `RRS` (share NFT). | owner, 2026-11-13 |
| 31 | 2026-11-13 | Decision 10 (owner's choice, replaces the earlier sweep default): no sweep method. Stray plain `ft_transfer`s to the registry account stay stuck by design; no method can move registry funds except the tracked paths. | owner, 2026-11-13 |
| 32 | 2026-11-13 | Block 2 build: the vault is written against raw NEAR host functions (`near-sys`), no SDK, no allocator. Result: 7420-byte wasm instead of about 110 KB with near-sdk, so each vault costs about 0.1 NEAR instead of about 1.2 NEAR. `withdraw` takes borsh arguments (token, receiver, amount) because only the registry calls it. Deviation from the repo layout note "Rust, near-sdk, tiny". | builder, logged for owner review |
| 33 | 2026-11-13 | Block 2 build: over-allocation (a new share that would push a group above 10000 bps) is refused at `register_share` until block 4 adds the over-allocation dispute. | builder, logged for owner review |
| 34 | 2026-11-13 | Block 2 build: the registry account itself must be storage-registered with the token contract to receive `ft_transfer_call` payments. The sandbox setup does it; the block 5 deploy plan must include it. | builder, logged for owner review |
| 35 | 2026-11-13 | Block 2 build: toolchain pinned with `rust-toolchain.toml` to Rust 1.99.1 with the wasm32 target. Contracts build with plain `cargo build --target wasm32-unknown-unknown` and `RUSTFLAGS="--cfg near"` (what cargo-near sets), no cargo-near needed. Bulk-memory opcodes from Rust 1.87+ need a protocol 84+ node (nearcore 2.12+); the sandbox is 2.14.1. Testnet protocol version is UNVERIFIED until block 5. | builder, logged for owner review |
