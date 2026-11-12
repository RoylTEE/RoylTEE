# Design

Status: block 1 design, written 2026-11-13 before any contract code. Signatures below are the plan; they may change in block 2 and any change will be logged in DECISIONS_LOG.md. Read THREAT_MODEL.md alongside this file.

Product in one line: the verified payout address for every share of every song, human or AI. The payout address never changes, only who controls it. An agent can pay it, but it cannot steal from it.

This is a permissioned, business-to-business record among verified industry members. It is not sold to the public and is not an investment product.

## 1. Toolchain facts

| Item | Value | Source | Status |
|---|---|---|---|
| near-sdk (Rust) | 5.30.1 (latest stable) | crates.io API, read 2026-11-13 | Verified |
| near-contract-standards (NEP-171, NEP-177, NEP-181, NEP-141, NEP-145 helpers) | 5.30.1 | crates.io API and docs.rs, read 2026-11-13. Modules `non_fungible_token::{core, enumeration, metadata, approval, events}`, `fungible_token`, `storage_management` present. `NonFungibleToken` exposes `internal_transfer`, `internal_mint`, `nft_resolve_transfer`. | Verified |
| near-workspaces (sandbox tests) | 0.24.0 | crates.io API, read 2026-11-13 | Verified |
| `near_sdk::assert_one_yocto` | exists in 5.30.1 | docs.rs | Verified |
| `env::promise_result_checked` (size-capped promise result read) | exists in 5.30.1 | docs.rs env module | Verified |
| `Promise::use_global_contract` and `deploy_global_contract` | exist in 5.30.1 | docs.rs Promise page | Verified (availability in sandbox and testnet UNVERIFIED) |
| Storage staking price | 1E19 yoctoNEAR per byte (100 KB per NEAR) | docs.near.org, Storage page | Verified |
| Function-call keys cannot attach deposits | | docs.near.org, Access Keys page | Verified |
| NEP-297 event format: `EVENT_JSON:` + `{standard, version, event, data}` | | NEP-297 text, near/NEPs repo | Verified |
| Circle USDC address on NEAR testnet | | | UNVERIFIED. Not used. Mock token instead. |

Versions will be pinned in `Cargo.toml` in block 2 and re-checked then.

## 2. Account layout (testnet)

Exact names are chosen at deploy time and recorded in `deployments/testnet.json`. Placeholders below.

| Account | Role | Keys after setup |
|---|---|---|
| `REG` (for example `rightsreg-xxxx.testnet`) | Registry contract: NEP-171 + NEP-177 + NEP-181, factory for vaults, credentials, roles, allowances, ledger, locks, disputes, events. Holds allowance funds, dispute escrow and undelivered forwards in its own mock USDC balance. | None. Full-access keys deleted after deploy and init (owner's yes required, it is a delete). |
| `s<N>.REG` | One vault per share token, `N` is a monotonic counter never reused. Created by `REG` at mint. Runs the vault contract. Storage-registered with mock USDC at mint. | None, ever. |
| `usdc-mock.REG` | Mock NEP-141 token, name "Mock USDC (testnet)", 6 decimals, symbol placeholder `mUSDC`. Built on near-contract-standards `FungibleToken` + `StorageManagement`. `mint` restricted to one `minter` account set at init (the simulated payer). | Created by `REG` before its keys are deleted. Keys on the mock may be deleted too; it is not part of the trust argument, it is a test token. |
| `DEV` (for example `rrdemo-xxxx.testnet`) | Separate dev parent for demo members. Deliberately NOT under `REG`, so the registry cannot have created or controlled member accounts. | Normal dev keys. |
| `registrar.DEV` | Registrar: grants and suspends credentials, grants payer role, resolves disputes, posts license prices. Single account in the demo; multisig with time delay is roadmap. | Full-access key (dev). |
| `label.DEV`, `writer-a.DEV`, `writer-b.DEV`, `buyer-pub.DEV`, `ai-co.DEV` | Credentialed members. | Full-access key (dev). |
| `outsider.DEV` | No credential. Used to show transfers to non-members fail. | Full-access key (dev). |
| `payer.DEV` | Simulated payer, also the mock token `minter`. Holds the payer role for attestations. | Full-access key (dev). |
| `pay-agent.label.DEV` | Scripted payment agent of `label.DEV`. | Only a function-call key: receiver `REG`, methods `pay_from_allowance`, `report`, `buy_license`, allowance unset (docs warn that an exhausted allowance locks a function-call-only account). Full-access key deleted after the function-call key is added (owner's yes). |
| `license-agent.ai-co.DEV` (optional) | Scripted license agent of `ai-co.DEV`. | Same pattern. |

### Work-level account: decision

The brief allows either a `w<N>.REG` account per song or handling the work level inside the registry ledger, "pick the simpler one", as long as money physically sits in on-chain USDC balances.

Choice: no `w<N>.REG` accounts. A work-level payment is one `ft_transfer_call` from the payer to `REG` with `msg` naming the work group. In the same receipt chain `REG` splits by bps and sends each share's part with `ft_transfer` to that share's vault. Money sits in `REG` only between receipts, then in the share vaults.

Why:
1. One payment, one transaction, already split: payers pay once per work, which is the goal.
2. A plain `ft_transfer` to a work account carries no payer, period or statement hash, so the payment could not be attributed. `ft_transfer_call` carries them in `msg`.
3. No extra account per song, no extra storage registration, no idle balance waiting for someone to call `distribute`, and no second sync race (T21) on a work account.
4. The split and forward code is needed anyway for `pay_from_allowance` and license fees, so this adds no new money path.
5. Money still physically sits in on-chain balances: per share in vaults; allowance, escrow and undelivered in `REG`'s balance, with the solvency invariant (T31).

Cost: a payer must use `ft_transfer_call` with the right `msg` for a work payment. A plain `ft_transfer` straight to a share vault still works and is picked up by `sync`. A plain `ft_transfer` to `REG` is not attributed; it shows up as `REG` balance above the invariant sum and no method moves it (listed in open questions).

Pending owner approval (DECISIONS_LOG item 1). If the owner wants `w<N>.REG`, the fallback is the same vault contract plus a public `distribute(work_id, right_type, territory)` that reads the balance and splits it.

### Vault deployment: decision

Default: plain `DeployContract` with the vault wasm embedded in the registry wasm (`include_bytes!`). Simple, works in sandbox, each vault locks its own code storage. Option: global contract referenced by code hash (`use_global_contract`), which pins immutable code and cuts per-vault storage (docs.near.org Global Contracts page; by-hash references cannot be updated, by-account references can, so only by-hash is acceptable). Sandbox and testnet support UNVERIFIED. Pending owner (DECISIONS_LOG item 4).

## 3. Vault contract

Stateless. No owner field, no upgrade method, no init method (so no init front-running), no other methods. Target about 60 lines.

```rust
// Parent is derived from the vault's own account id: "s12.<REG>" -> "<REG>".
// No stored state is needed. Whether near-sdk codegen writes an empty state
// record for a unit struct is UNVERIFIED and checked in block 2.

/// Only callable by the parent registry. Sends `amount` of `token` to `receiver`
/// with ft_transfer and 1 yoctoNEAR from the vault's own NEAR balance.
/// Returns the ft_transfer promise so the caller's callback sees success or failure.
/// Panics: predecessor != parent; any attached deposit; amount == 0.
pub fn withdraw(&mut self, token: AccountId, receiver: AccountId, amount: U128) -> Promise;
```

The brief also lists `balance(token)`. A view method cannot make cross-contract calls, so a vault-side balance view would need state. Deviation: dropped; read `ft_balance_of(vault)` on the token contract directly (the brief allows this). DECISIONS_LOG item 3.

Optional (pending owner, from the track's sharded NFT items draft): a pure view `vault_info() -> { parent, token_id }` derived from the account id, so a consumer can cross-check the vault against `nft_token`. Adds no state and no power.

The vault's NEAR balance at creation covers its code storage plus a small buffer for the 1 yoctoNEAR per withdraw. Gas for every call is paid by the transaction signer, not the vault. Amount measured in block 2 (`VAULT_NEAR`), not guessed here.

## 4. Registry state

Rust-style, `near_sdk::store` collections, borsh in storage, JSON at the boundary. Prefixes are single bytes to keep keys small.

```rust
pub type TokenId = String;      // "s<N>" for shares, "l<N>" for licenses
pub type WorkId = u64;
pub type DisputeId = u64;
pub type Bps = u16;             // 1..=10000
pub type Hash32 = String;       // 64-char lowercase hex sha256 of an off-chain document

pub enum CodeType { Isrc, Iswc }
pub enum Provenance { Human, AiAssisted, FullyAi }
pub enum Basis { Copyright, Contract, None }
pub enum RightType { Master, Publishing, Performance, Mechanical }
pub enum EvidenceTier { SelfAsserted, CounterpartyConfirmed, PayerConfirmed } // computed at view time
pub enum CredentialStatus { Active, Suspended }
pub enum TokenKind { Share, License }
pub enum LicenseType { Sync, Voice, Training } // only Sync implemented
pub enum AgentMethod { PayFromAllowance, Report, BuyLicense }

pub struct GroupKey { work_id: WorkId, right_type: RightType, territory: String } // "WW" or ISO 3166-1 alpha-2

pub struct Credential {
    org_name_hash: Hash32, kyb_ref_hash: Hash32, status: CredentialStatus,
    granted_at: u64, updated_at: u64,
}

pub enum WorkStatus { Active, DuplicateOpen(DisputeId), Void }
pub struct Work {
    id: WorkId, code_type: CodeType, code: String,   // normalised: uppercase, no separators
    title_hash: Hash32, provenance: Provenance, metadata_uri: String, // max 256 bytes
    registered_by: AccountId, status: WorkStatus, created_at: u64,
}

pub enum ShareStatus { Pending, Active, Claimant(DisputeId), Contested(DisputeId), Void, Failed }
pub struct Share {
    token_id: TokenId, group: GroupKey, percent_bps: Bps, basis: Basis,
    evidence_hash: Hash32, confirmations: Vec<AccountId>, // bounded by group size
    payer_attestation: Option<(AccountId, Hash32)>, status: ShareStatus,
    vault: AccountId, note: Option<String>,  // max 280 bytes, holder power
    last_effective_from: u64, created_at: u64,
}

pub struct Ledger {
    known_balance: u128,       // what the registry believes is in the vault
    total_paid_out: u128,      // sum of successful claims
    total_routed_in: u128,     // sum delivered through the registry (history only)
    undelivered: u128,         // forwarded amount that failed, held in REG
    escrowed: u128,            // this share's part in open dispute escrow, held in REG
    claim_cap: Option<u128>,   // set while contested (T24)
    snapshot_pending: bool,
    inbound_pending: u32,
    payout_in_flight: Option<Lock>, // amount, receiver, since_block
    transfer_pending: Option<Lock>, // from, to, since_block
    epoch: u64,                // bumped on every ledger change (T21)
}
// earned = known_balance + total_paid_out (view), claimable = min(known_balance, claim_cap)
// a successful claim also lowers claim_cap by the amount (cap rolls back with the debit on failure)

pub struct Group { shares: Vec<TokenId>, active_bps: u32, reserved_bps: u32, open_dispute: Option<DisputeId> }

pub struct AgentRole {
    org: AccountId, methods: Vec<AgentMethod>, per_tx_limit: u128, daily_limit: u128,
    max_tx_per_day: u32, expires_at: u64, revoked: bool, allowance: u128,
    spent_buckets: [u128; 24], count_buckets: [u32; 24], bucket_hour: u64, // rolling 24h, 1h granularity
}

pub enum DisputeKind { Overallocation { group: GroupKey }, DuplicateCode { original: WorkId, duplicate: WorkId } }
pub enum Outcome { UpholdExisting, UpholdClaimant { new_bps: Vec<(TokenId, Bps)> }, VoidDuplicate, KeepBoth }
pub struct Dispute {
    id: DisputeId, kind: DisputeKind, claimant: Option<TokenId>, contested: Vec<TokenId>,
    escrow: Vec<(TokenId, u128)>, opened_by: AccountId, opened_at: u64,
    resolution: Option<(Outcome, Hash32, u64)>,
}

pub struct LicensePrice { price: u128, pay_to: GroupKey, doc_hash: Hash32 }

#[near(contract_state)]
pub struct Registry {
    registrar: AccountId,                // fixed at init, no setter in demo
    usdc: AccountId,                     // fixed at init, no setter
    tokens: NonFungibleToken,            // near-contract-standards, enumeration + metadata, approvals None
    kinds: LookupMap<TokenId, TokenKind>,            // b"k"
    credentials: LookupMap<AccountId, Credential>,   // b"c"
    payers: LookupSet<AccountId>,                    // b"p"
    works: IterableMap<WorkId, Work>,                // b"w"
    code_index: LookupMap<String, WorkId>,           // b"x", key "ISRC:<code>" or "ISWC:<code>"
    shares: IterableMap<TokenId, Share>,             // b"s"
    ledgers: LookupMap<TokenId, Ledger>,             // b"l"
    groups: LookupMap<GroupKey, Group>,              // b"g"
    work_groups: LookupMap<WorkId, Vec<GroupKey>>,   // b"q"
    roles: LookupMap<AccountId, AgentRole>,          // b"r"
    statement_lines: LookupSet<[u8; 32]>,            // b"i", sha256(org || 0x00 || line_id)
    disputes: IterableMap<DisputeId, Dispute>,       // b"d"
    license_prices: LookupMap<(WorkId, LicenseType), LicensePrice>, // b"o"
    licenses: LookupMap<TokenId, (WorkId, LicenseType, AccountId, u128)>, // b"L"
    allowance_total: u128, escrow_total: u128, undelivered_total: u128, // solvency (T31)
    next_share: u64, next_license: u64, next_work: WorkId, next_dispute: DisputeId,
}
```

Bounds (design parameters, final values measured in block 2 or 3): `MAX_SHARES_PER_GROUP = 16`, statement line id 1..=64 bytes, note at most 280 bytes, metadata URI at most 256 bytes, `MIN_GROUP_PAYMENT = 10000` base units (0.01 mock USDC), `STALE_LOCK_BLOCKS` and `MAX_EFFECTIVE_FUTURE_NS` set in block 2. Gas constants (`GAS_FT_TRANSFER`, `GAS_WITHDRAW`, `GAS_ON_CLAIM`, `GAS_ON_FORWARD`, `GAS_RESOLVE`) measured in sandbox, not guessed.

Code checks: ISRC is 12 characters (2 letters, 3 alphanumerics, 7 digits) after removing hyphens; ISWC is `T` plus 10 digits after removing separators. Shape only; the ISWC check digit is not verified (UNVERIFIED algorithm, not implemented).

## 5. Registry methods

Conventions: "1y" = `#[payable]` and `assert_one_yocto()`. "deposit" = `#[payable]`, requires at least the stated amount, refunds the excess. Methods with no marker are not payable; near-sdk rejects any attached deposit on them. Every method that changes state emits an event (section 6).

### Init and NEP standards

```rust
#[init]
pub fn new(registrar: AccountId, usdc: AccountId, metadata: NFTContractMetadata) -> Self;
// Called once in the deploy batch. Panics if already initialised. Name and symbol come
// from config/project.json via the deploy script, not from contract source.

#[payable] pub fn nft_transfer(&mut self, receiver_id: AccountId, token_id: TokenId,
    approval_id: Option<u64>, memo: Option<String>, effective_from: Option<U64>);
// 1y. Caller == owner. Panics: approval_id.is_some(); receiver == caller; receiver has no
// active credential; kind == License; status Pending/Failed/Void/Claimant/Contested
// (dispute freeze); payout_in_flight; transfer_pending; effective_from out of bounds (T22).
// The extra optional effective_from argument is outside NEP-171; omitting it must still
// deserialize (UNVERIFIED for near-sdk codegen, tested in block 2).

#[payable] pub fn nft_transfer_call(&mut self, receiver_id: AccountId, token_id: TokenId,
    approval_id: Option<u64>, memo: Option<String>, msg: String,
    effective_from: Option<U64>) -> PromiseOrValue<bool>;
// 1y. Same checks as nft_transfer, then sets transfer_pending, moves the token, calls
// receiver.nft_on_transfer, then nft_resolve_transfer. Asserts enough prepaid gas.

#[private] pub fn nft_resolve_transfer(&mut self, previous_owner_id: AccountId,
    receiver_id: AccountId, token_id: TokenId,
    approved_account_ids: Option<HashMap<AccountId, u64>>) -> bool;
// Clears transfer_pending first. Returns the token to previous_owner_id only if the
// receiver returned true (or failed) and is still the owner. Return path skips the
// credential check (T23d). Never panics.

pub fn nft_token(&self, token_id: TokenId) -> Option<Token>;
// Dynamic fields in metadata.extra (pending owner, from the track brief): kind, work id,
// right, territory, bps, basis, provenance, tier, status, earned, paid out, claimable.

#[payable] pub fn nft_approve(&mut self, token_id: TokenId, account_id: AccountId, msg: Option<String>) -> Option<Promise>; // always panics
#[payable] pub fn nft_revoke(&mut self, token_id: TokenId, account_id: AccountId);   // always panics
#[payable] pub fn nft_revoke_all(&mut self, token_id: TokenId);                      // always panics
pub fn nft_is_approved(&self, token_id: TokenId, approved_account_id: AccountId, approval_id: Option<u64>) -> bool; // always false
pub fn nft_metadata(&self) -> NFTContractMetadata;
pub fn nft_total_supply(&self) -> U128;
pub fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token>;
pub fn nft_supply_for_owner(&self, account_id: AccountId) -> U128;
pub fn nft_tokens_for_owner(&self, account_id: AccountId, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token>;
```

### Membership and roles (registrar)

```rust
#[payable] pub fn grant_credential(&mut self, account: AccountId, org_name_hash: Hash32,
    kyb_ref_hash: Hash32, doc_hash: Hash32) -> Promise;
// Registrar only. deposit >= mock USDC storage_balance_bounds.min (read and passed by the
// script). Calls usdc.storage_deposit(account, registration_only = true) so the member can
// receive claims (T17). Panics: not registrar; account is REG, ends with ".REG", is usdc,
// holds an agent role (T28, T29); already active. KYB is SIMULATED: hashes of dummy docs.
// Deviation from the brief: extra doc_hash argument, so every registrar action carries one.

#[payable] pub fn suspend_credential(&mut self, account: AccountId, doc_hash: Hash32);   // 1y, registrar
#[payable] pub fn reinstate_credential(&mut self, account: AccountId, doc_hash: Hash32); // 1y, registrar
#[payable] pub fn grant_payer_role(&mut self, account: AccountId, doc_hash: Hash32);     // 1y, registrar
#[payable] pub fn revoke_payer_role(&mut self, account: AccountId, doc_hash: Hash32);    // 1y, registrar
```

### Works and shares

```rust
#[payable] pub fn register_work(&mut self, code_type: CodeType, code: String,
    title_hash: Hash32, provenance: Provenance, metadata_uri: String) -> WorkId;
// deposit >= storage used. Caller: active member. Personal data stays off chain.
// Panics: bad code shape; oversize uri; caller not active.
// Duplicate code: work is created with status DuplicateOpen and a DuplicateCode dispute
// opens. Payments to the duplicate work are refused until resolve (T11).

#[payable] pub fn register_share(&mut self, work_id: WorkId, right_type: RightType,
    territory: String, percent_bps: Bps, basis: Basis, holder: AccountId,
    evidence_hash: Hash32, contests: Option<Vec<TokenId>>) -> Promise;
// deposit >= SHARE_MINT_DEPOSIT (vault NEAR + mock USDC registration + registry storage,
// measured in block 2). Caller and holder: active members. Panics: bps 0 or > 10000; group
// full; work Void; contests names a token outside the group.
// Reserves bps. If active + reserved + percent_bps > 10000, the share becomes a Claimant
// and an Overallocation dispute opens; `contests` (default: all active shares in the group)
// become Contested.
// Two-phase mint (T15): Pending -> create vault (CreateAccount, Transfer, DeployContract,
// no AddKey) -> on_vault_created -> usdc.storage_deposit(vault) -> on_vault_ready -> mint
// NFT to holder, status Active (or Claimant). Any failure: status Failed, reservation
// released, unused deposit refunded, no NFT.

#[private] pub fn on_vault_created(&mut self, token_id: TokenId, #[callback_result] r: Result<(), PromiseError>) -> PromiseOrValue<()>;
#[private] pub fn on_vault_ready(&mut self, token_id: TokenId, payer: AccountId, refund: U128, #[callback_result] r: Result<StorageBalance, PromiseError>);

#[payable] pub fn confirm_share(&mut self, token_id: TokenId);
// 1y. Caller must currently own another Active share of the same work and right_type.
// Tier is computed at view time: CounterpartyConfirmed if every current owner of the
// other Active shares in the group is in confirmations; PayerConfirmed if a payer attested.

#[payable] pub fn attest_share(&mut self, token_id: TokenId, statement_hash: Hash32); // 1y, payer role

#[payable] pub fn set_share_note(&mut self, token_id: TokenId, note: String);
// 1y. Holder power. Caller == owner. Panics: note > 280 bytes; transfer_pending.

#[payable] pub fn amend_split(&mut self, token_id: TokenId, new_bps: Bps, doc_hash: Hash32);
// 1y. Registrar only, and only to correct a registration error inside an open dispute
// (otherwise panics). Holders cannot change splits alone. Listed so test 8 can show an
// agent cannot reach it.
```

### Money in, money out

```rust
pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128>;
// Predecessor must be usdc, else returns amount (T18). msg is JSON, one of:
//   {"pay_work": {"work_id", "right_type", "territory", "period", "statement_hash", "statement_line_id"?}}
//   {"pay_share": {"token_id", "period", "statement_hash"}}
//   {"fund_allowance": "<agent account>"}        (sender must be the agent's org)
//   {"buy_license": {"work_id", "license_type"}} (sender must be an active member)
// Unknown or malformed msg, refused group (T25), duplicate work, or failed check:
// returns amount (full refund, no state change). On success returns 0 and schedules
// forwards. Never panics after state changes.

#[private] pub fn on_forward(&mut self, token_id: TokenId, amount: U128, #[callback_result] r: Result<(), PromiseError>);
// inbound_pending -= 1, epoch += 1. Ok: known_balance += amount, total_routed_in += amount.
// Err: undelivered += amount, undelivered_total += amount (T26).

pub fn retry_forward(&mut self, token_id: TokenId) -> Promise;  // anyone; debit undelivered, forward, roll back on failure

#[payable] pub fn claim(&mut self, token_id: TokenId) -> Promise;
// 1y. Caller == current owner. Panics: kind License; transfer_pending; payout_in_flight;
// snapshot_pending; claimable == 0; prepaid gas too low.
// amount = min(known_balance, claim_cap). Set payout_in_flight, known_balance -= amount,
// total_paid_out += amount, epoch += 1, all BEFORE calling vault.withdraw(usdc, owner, amount).

#[private] pub fn on_claim(&mut self, token_id: TokenId, receiver: AccountId, amount: U128, #[callback_result] r: Result<(), PromiseError>);
// Clears payout_in_flight. Err: known_balance += amount, total_paid_out -= amount. Never panics.

pub fn sync(&mut self, token_id: TokenId) -> Promise;
// Anyone, no deposit. Panics if payout_in_flight, transfer_pending or inbound_pending > 0.
// Calls usdc.ft_balance_of(vault) then on_sync with the current epoch.

#[private] pub fn on_sync(&mut self, token_id: TokenId, epoch: u64);
// Reads the result with promise_result_checked (size cap). Epoch changed: discard. Else set
// known_balance = observed (observed < known emits anomaly). If snapshot_pending: set
// claim_cap = observed, clear snapshot_pending.

pub fn release_stale_lock(&mut self, token_id: TokenId);
// Anyone. Only if a lock is older than STALE_LOCK_BLOCKS. Clears it without rollback,
// epoch += 1 (T19).
```

### Agents

```rust
#[payable] pub fn grant_role(&mut self, agent: AccountId, methods: Vec<AgentMethod>,
    per_tx_limit: U128, daily_limit: U128, max_tx_per_day: u32, expires_at: U64);
// 1y plus storage deposit sized for max_tx_per_day (measured in block 3). Caller: active
// member. Panics: agent is not a direct sub-account of caller; agent holds a credential;
// limits zero; per_tx > daily; expires_at in the past.

#[payable] pub fn revoke_role(&mut self, agent: AccountId);   // 1y; org or registrar; immediate

#[payable] pub fn withdraw_allowance(&mut self, agent: AccountId, amount: U128) -> Promise;
// 1y; org only. Debit before ft_transfer to org; callback restores on failure.

pub fn pay_from_allowance(&mut self, target: PayTarget, amount: U128,
    statement_line_id: String, period: String, statement_hash: Hash32) -> Promise;
// No deposit (function-call key). PayTarget = Work { work_id, right_type, territory }
// | Share { token_id }. There is no payee argument (T8).
// Checks in order: role exists, not revoked, not expired, method allowed, org credential
// active, amount >= MIN_GROUP_PAYMENT, amount <= per_tx, rolling daily sum and count within
// limits, amount <= allowance, statement line unused, target payable (T25).
// Then: mark line used, debit allowance and allowance_total, update buckets, split and
// forward. Emits allowance_spent and payment_in.

pub fn report(&mut self, statement_line_id: String, report_hash: Hash32);  // agent; event only, no state
pub fn buy_license(&mut self, work_id: WorkId, license_type: LicenseType) -> Promise; // agent; from allowance (cut first)
```

### Disputes and licenses

```rust
#[payable] pub fn resolve(&mut self, dispute_id: DisputeId, outcome: Outcome, doc_hash: Hash32) -> Promise;
// 1y, registrar. Outcome must match kind: Overallocation -> UpholdExisting | UpholdClaimant;
// DuplicateCode -> VoidDuplicate | KeepBoth. Releases escrow (section 9), lifts freezes and
// claim caps, emits dispute_resolved with the doc hash.

#[payable] pub fn set_license_price(&mut self, work_id: WorkId, license_type: LicenseType,
    price: U128, pay_to: GroupKey, doc_hash: Hash32);
// 1y, registrar. Only Sync accepted; Voice and Training panic "not implemented, roadmap".
```

### Views

`get_work`, `get_share`, `get_ledger` (known, paid out, earned, claimable, cap, locks, epoch), `get_group`, `get_credential`, `get_role` (with current rolling spend), `get_dispute`, `list_disputes(from, limit)`, `is_statement_line_used(org, line_id)`, `get_accounting` (allowance, escrow and undelivered totals for the solvency check), `get_config` (registrar, usdc).

## 6. Events (NEP-297)

Standard NEP-171 events from near-contract-standards: `nft_mint` and `nft_transfer` (standard `nep171`). Custom events use standard `share_registry`, version `1.0.0` (name-neutral on purpose, see section 13).

| Event | Data fields |
|---|---|
| `work_registered` | work_id, code_type, code, title_hash, provenance, metadata_uri, registered_by |
| `share_registered` | token_id, work_id, right_type, territory, percent_bps, basis, holder, vault, evidence_hash, status |
| `vault_ready` / `vault_failed` | token_id, vault, reason (failed only) |
| `share_transfer` | token_id, from, to, effective_from, via_call |
| `transfer_returned` | token_id, from, to |
| `payment_in` | source (direct, allowance, license), payer, target, amount, period, statement_hash, statement_line_id, credits [{token_id, amount}], escrowed [{token_id, amount, dispute_id}] |
| `forward_failed` / `forward_retried` | token_id, amount, ok |
| `claim_started` / `claim_settled` | token_id, owner, amount, ok (settled only) |
| `sync` / `sync_discarded` | token_id, observed, previous_known, earned / reason |
| `credential_granted` | account, org_name_hash, kyb_ref_hash, doc_hash |
| `credential_suspended` / `credential_reinstated` | account, doc_hash |
| `payer_role_granted` / `payer_role_revoked` | account, doc_hash |
| `role_granted` | agent, org, methods, per_tx_limit, daily_limit, max_tx_per_day, expires_at |
| `role_revoked` | agent, by |
| `allowance_funded` | agent, org, amount, allowance |
| `allowance_spent` | agent, amount, statement_line_id, target |
| `allowance_withdrawn` | agent, org, amount, ok |
| `agent_report` | agent, statement_line_id, report_hash |
| `share_confirmed` / `share_attested` | token_id, by, statement_hash (attested only) |
| `note_set` | token_id, by, note_hash |
| `dispute_opened` | dispute_id, kind, work_id, group, claimant, contested, opened_by |
| `dispute_frozen` | dispute_id, token_id, snapshot |
| `dispute_resolved` | dispute_id, outcome, doc_hash, released [{token_id, amount}] |
| `license_price_set` | work_id, license_type, price, pay_to, doc_hash |
| `license_minted` | license_id, work_id, license_type, buyer, price, credits |
| `lock_released_stale` | token_id, lock, since_block |
| `anomaly` | token_id, kind, detail |

Runtime limit: at most 100 logs and 16 KiB of logs per call (docs.near.org, "Callbacks can fail"). With `MAX_SHARES_PER_GROUP = 16`, one `payment_in` stays well under that; checked in test 13.

The per-token history page is built from these events plus views, each row linking to the testnet explorer transaction.

## 7. Claim state machine (per share)

```
Idle
  claim (1y, owner, no locks, claimable > 0)
    -> debit ledger, set payout_in_flight, epoch+1
    -> InFlight
InFlight
  on_claim(Ok)  -> clear flag -> Idle (paid)
  on_claim(Err) -> clear flag, restore known_balance and total_paid_out -> Idle
  callback lost -> after STALE_LOCK_BLOCKS: release_stale_lock -> Idle, then sync reconciles
While InFlight: claim, nft_transfer, nft_transfer_call, sync all panic.
```

## 8. Transfer state machine (per share)

```
Free
  nft_transfer (1y, owner, receiver active, no locks, not frozen, approval_id None)
    -> owner = receiver, record effective_from -> Free
  nft_transfer_call (same checks)
    -> owner = receiver, set transfer_pending -> Pending
Pending
  nft_resolve_transfer: clear transfer_pending;
    receiver returned true or failed, and still owner -> owner = previous -> Free
    else -> Free (receiver keeps it)
  callback lost -> after STALE_LOCK_BLOCKS: release_stale_lock -> Free (current owner keeps it)
While Pending: claim, nft_transfer, nft_transfer_call, set_share_note, sync all panic.

Frozen (dispute): status Claimant or Contested. Transfers panic. Claim allowed for Contested
only up to claim_cap, and blocked while snapshot_pending. Claimant has no claimable money.
Resolve -> Active (or Void) and the cap is lifted.
```

## 9. Bps rounding and dispute escrow math

Rounding rule (every split: work payment, allowance payment, license fee, escrow release):

1. Payment set P = the shares that are paid: Active and Contested shares of the group (not the Claimant), each with bps b_i. A group payment requires sum of b_i over P = 10000 exactly, else it is refused (T25).
2. Credit x_i = floor(A * b_i / 10000) using u128 (A is at most u128; A * 10000 cannot overflow for any realistic mock amount; checked multiplication panics before any state change otherwise).
3. Remainder R = A - sum(x_i). 0 <= R < |P|.
4. R goes to the share with the largest b_i; ties go to the lowest token number. That share is the remainder share.
5. Sum of all credits equals A exactly.

Dispute escrow, Overallocation: Contested shares C (subset of P) and Claimant c.

- For each payment A: compute x_i (with R added to the remainder share) for all of P. For i in P minus C: forward x_i to the vault. For i in C: add x_i to `escrow[i]`, `ledger.escrowed`, `escrow_total`. Claimant gets nothing directly. Vault forwards + escrow = A.
- Contested fraction = sum of b_i over C divided by 10000. Only new money is affected. Money already in vaults stays claimable up to the snapshot (T24).
- Resolve UpholdExisting: c becomes Void (its NFT stays as a void record, non-transferable, never paid), each `escrow[i]` is forwarded to vault i.
- Resolve UpholdClaimant { new_bps }: new_bps must cover exactly C plus c, and its sum must equal the sum of old b_i over C, so the group still totals 10000. Escrow pool E = sum of escrow[i]. E is split over C plus c by new_bps with the same rounding rule (denominator = sum of new_bps). Group bps and statuses update; c becomes Active.
- Example: A writer 5000, B writer 5000. C registers 5000 contesting B. Payment 1000000: A vault gets 500000, escrow[B] = 500000. UpholdClaimant with B 2500, C 2500: escrow 500000 splits 250000 to B, 250000 to C.

Dispute, DuplicateCode: no escrow. Payments to the duplicate work are refused (money stays with the payer); the original work is untouched. VoidDuplicate: duplicate work and its shares become Void. KeepBoth: duplicate becomes Active (for example two different recordings wrongly sharing a code, fixed off chain). Voiding the original is roadmap.

## 10. Agent role model

- A function-call key cannot attach a deposit (docs.near.org, Access Keys). `ft_transfer` needs 1 yoctoNEAR. So an agent with only a function-call key cannot send USDC itself. The design does not work around this: the agent spends from an allowance held inside `REG`, and `REG` does the transfers.
- Each agent is a named sub-account of its organisation, enforced in `grant_role`.
- The organisation funds the allowance with one `ft_transfer_call` to `REG`, `msg: {"fund_allowance": "<agent>"}`.
- Agent key: function-call only, receiver `REG`, methods `pay_from_allowance`, `report`, optionally `buy_license`. None of them take a deposit.
- The contract decides payees; the agent supplies a target, amount, statement line id, period and statement hash.
- Owner and admin methods (`nft_transfer`, `nft_transfer_call`, `claim`, `amend_split`, `grant_role`, `revoke_role`, `withdraw_allowance`, `set_share_note`, `confirm_share`, all registrar methods) need 1 yoctoNEAR. An agent key cannot call them even if a role check had a bug.
- `revoke_role` is immediate. The organisation should also delete the agent's key.
- The demo agent is a scripted Node agent (no model). Called "scripted agent" everywhere.

## 11. Test plan mapping

Rust unit tests plus near-workspaces sandbox tests. `npm run test` runs all of them with no network.

| # | Test | Block | Threat rows |
|---|---|---|---|
| 1 | Mint creates a vault with zero keys, registered with mock USDC | 2 | T1, T15 |
| 2 | Plain ft_transfer into a vault, sync, earned updates | 2 | T21 |
| 3 | Owner claim pays current owner; non-owner claim fails | 2 | T2 |
| 4 | Pay, transfer, pay again: seller gets nothing, buyer claims both | 2 | T2 |
| 5 | Transfer to non-credentialed account fails | 2/3 | T10 |
| 6 | Transfer during in-flight payout fails; after callback succeeds; no double pay | 2 | T3 |
| 7 | nft_transfer_call bounce: claim during on_transfer fails, funds intact | 2 | T4 |
| 8 | Agent cannot call nft_transfer, claim, amend_split, withdraw_allowance, grant_role | 3 | T6 |
| 9 | Over per-tx or daily limit fails | 3 | T7 |
| 10 | Revoked or expired agent's next call fails | 3 | T14 |
| 11 | Injected payee in metadata or note changes nothing | 3 | T8, T32 |
| 12 | Same statement line twice pays once | 3 | T9 |
| 13 | Work-level split exact to the last unit; minimum payment | 3 | T16, T30 |
| 14 | Over 100 percent opens dispute; contested fraction escrowed; others paid; frozen share cannot transfer; resolve releases | 4 | T11 |
| 14b | Duplicate code: original paid, duplicate refused | 4 | T11 |
| 15 | Suspended member keeps share and accrues, cannot receive | 3 | T23 |
| 15b | Failed vault registration leaves no NFT, refunds | 2 | T15 |
| 16 | nft_approve panics; approval_id Some panics | 2 | T5 |
| 17 | buy_license mints license NFT, splits fee, license non-transferable (cut first) | late | T33 |
| 18 | Claim to unregistered holder rolls back | 2 | T17 |
| 19 | Fake token or direct ft_on_transfer credits nothing | 3 | T18 |
| 20 | Callbacks survive failure; stale lock release; low gas fails early | 2 | T19 |
| 21 | Parent cannot delete, redeploy or key an existing vault | 2 | T20 |
| 22 | Sync race refused or discarded; earned exact | 2 | T21 |
| 23 | effective_from bounds; payout ignores it | 2 | T22 |
| 24 | Suspended edge cases | 3 | T23 |
| 25 | Direct transfer into contested vault capped until resolve | 4 | T24 |
| 26 | Under-allocated group refused; parallel registrations over cap open dispute | 3/4 | T25 |
| 27 | Forward failure to undelivered; retry once | 3 | T26 |
| 28 | max_tx_per_day and minimum payment enforced | 3 | T27, T30 |
| 29 | Role namespace and allowance funding rules | 3 | T28 |
| 30 | System accounts cannot be credentialed or receive; registrar cannot claim others' shares | 3 | T12, T29 |
| 31 | Oversize note and non-holder note fail | 2 | T32 |

Scripts: `verify-keys` (T1, T13, T20), `npm run verify:testnet` (T1, T13, T31). Helper `assert_solvent` at the end of every sandbox test (T31). Also `cargo clippy -- -D warnings`, `cargo fmt --check`, frontend typecheck, and the audit checklist into AUDIT.md.

## 12. What is SIMULATED

Real on testnet: registry and vault contracts, vault accounts with zero keys, mock USDC transfers, splits, limits, idempotency, locks, disputes and escrow, events and per-token history.

Simulated, and labeled "Simulated" in the UI and README:

- KYB and organisation identity (credential hashes are hashes of dummy documents).
- The fiat rail: payer pays a virtual USD account, converted to USDC. Represented by the simulated payer script that mints mock USDC and pays with a statement hash.
- Payer statements and statement line ids (generated by the script).
- Payer attestations (`attest_share`) by the simulated payer.
- TEE attestation of statements (not built; roadmap).
- Provenance evidence (human, AI-assisted, fully AI is a declared field; the evidence behind it is not checked).
- Arbitration and dispute documents (doc hashes of dummy documents); the registrar is one dev account, not a council.
- The participation agreement that would give the record legal effect (roadmap).
- License legal terms (only a sync license record and fee split are real; voice and training are not implemented).
- The agent: a scripted Node agent, not a model.
- USDC: mock token "Mock USDC (testnet)", not Circle USDC.

## 13. Project name constant

The working name is a placeholder. It lives in exactly two places: `config/project.json` (`projectName`, plus the NFT symbol and event standard string) and the README title. Contracts contain no project name: the NFT contract name and symbol are passed to `new` by the deploy script from `config/project.json`, and the custom event standard string is name-neutral. Scripts and the frontend import `config/project.json`. A check script in block 7 will grep the repo to confirm the name appears only in those two places.

## 14. Legal and product framing

The token is a verified record of a signed assignment plus a binding payment instruction among members. It is evidence and a payout address, not legal title. Legal effect would come from a participation agreement (roadmap, labeled as such). The brief's statement that a fully AI track has no US copyright is a legal claim this repo does not verify (UNVERIFIED; the `basis` field lets such a share record a contract-based payment entitlement instead). No investment or return language anywhere.

## 15. Gaps against the track example prompt

The track example (nearai/near-hackathon-prompt-examples, tracks/10-nfts.md) asks for things the brief does not fully cover:

1. Dynamic NFTs are listed under "Lean on". The brief's token metadata is static. Proposal: `nft_token` fills `metadata.extra` at view time with earned, paid out, claimable, tier, status and provenance. Pending owner.
2. "A gated page that only current holders can open." The brief's `/holder` page uses a view-call check on an entered account, which anyone can type. Real gating needs proof of control: a NEP-413 signed message (wallet `signMessage`, Final status in near/NEPs) checked server-side against the account's full-access keys. Pending owner; fallback is to label the page "holder view, not access-controlled".
3. The example says whoever holds the NFT controls the sub-account and its access rights move with it. This design is deliberately narrower: holders never get keys; control is a registry check at call time. The README must say so and why (T1).
4. "Start here" links not referenced by the brief: the near-smart-contracts agent skill, Top Level Accounts contracts, and the sharded NFT items draft. The draft's `nft_item_info` idea maps to the optional vault view in section 3. The Top Level Accounts contracts were not reviewed (UNVERIFIED relevance).
