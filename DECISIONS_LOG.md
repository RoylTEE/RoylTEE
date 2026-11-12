# Decisions log

Dates are in Bangkok time (UTC+7). "Owner" is the project owner. Items the brief already fixes are marked "owner, via build prompt". Everything else stays "pending owner" until the owner says yes.

| # | Date | Decision | Who approved |
|---|---|---|---|
| 0 | 2026-11-13 | Build prompt read in full; sha256 checked and matched before work started. Testnet only, mock USDC only, no contract code before the block 1 OK. | owner, via build prompt |
| 1 | 2026-11-13 | Work-level payments handled inside the registry: one `ft_transfer_call` to the registry naming the work group, split and forwarded to share vaults in the same receipt chain. No `w<N>` accounts. Fallback documented in DESIGN.md section 2. | pending owner |
| 2 | 2026-11-13 | Rounding: floor per share, remainder to the largest-bps share, ties to the lowest token number. Alternative: largest-remainder method. | pending owner |
| 3 | 2026-11-13 | Vault is stateless, derives its parent from its own account id, exposes only `withdraw`. The brief's vault `balance(token)` is dropped because a view cannot make cross-contract calls; balances are read with `ft_balance_of` on the token. | pending owner |
| 4 | 2026-11-13 | Vault code deployed per account (embedded wasm) by default. Global contract by code hash is an option if sandbox and testnet support it. | pending owner |
| 5 | 2026-11-13 | NEP-178 approvals disabled; `nft_approve`, `nft_revoke`, `nft_revoke_all` panic; `approval_id` must be empty. | owner, via build prompt |
| 6 | 2026-11-13 | A suspended holder keeps the share, keeps accruing, may still claim, and may transfer out to an active member. Cannot receive. Suspended organisation's agents are blocked. | pending owner |
| 7 | 2026-11-13 | A work-group payment is accepted only if the paid shares sum to exactly 10000 bps; otherwise it is refunded or refused. | pending owner |
| 8 | 2026-11-13 | Duplicate ISRC or ISWC: payments to the duplicate work are refused (not escrowed) until the registrar resolves; the original work keeps being paid. | pending owner |
| 9 | 2026-11-13 | Direct plain transfers into a contested vault during a dispute are not claimable above the snapshot until resolve, then released to that vault's holder whatever the outcome. | pending owner |
| 10 | 2026-11-13 | Agents must be direct sub-accounts of their organisation, enforced on chain. Agent accounts cannot hold a credential and vice versa. | pending owner (brief sets the naming; enforcement and exclusivity added) |
| 11 | 2026-11-13 | `grant_credential` takes an extra `doc_hash` and is payable so it can storage-register the member with mock USDC. | pending owner |
| 12 | 2026-11-13 | `nft_transfer` and `nft_transfer_call` take an extra optional `effective_from`, recorded only, bounded, ignored by payout logic in the demo. | pending owner (brief asks for `effective_from`; the argument shape is ours) |
| 13 | 2026-11-13 | Custom event standard string `share_registry`, name-neutral; mock token symbol placeholder `mUSDC`; NFT symbol placeholder in `config/project.json`. | pending owner |
| 14 | 2026-11-13 | Project name lives only in `config/project.json` and the README title. Contracts contain no project name. | owner, via build prompt |
| 15 | 2026-11-13 | Dynamic token fields in `nft_token` metadata (earned, paid out, claimable, tier, status), from the track example. | pending owner |
| 16 | 2026-11-13 | Holder page gating via a NEP-413 signed message, or label it "not access-controlled". | pending owner |
| 17 | 2026-11-13 | Optional vault view `vault_info` (parent and token id) from the sharded NFT items draft. | pending owner |
| 18 | 2026-11-13 | License fee is paid to one work group named when the registrar posts the price. Licenses stay cut-first. | pending owner |
| 19 | 2026-11-13 | Hackathon rules allow one person up to 2 projects and say each README must name the other. Owner's current setting: README names no other entries. Needs a decision before block 7. | pending owner |
| 20 | 2026-11-13 | This log uses "owner" instead of a personal name, to follow the repo rule of no personal names. | pending owner |
| 21 | 2026-11-13 | Placeholder git author and email from the brief; unsigned commits; no tags; no remote; nothing pushed. | owner, via build prompt |
