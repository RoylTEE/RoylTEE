//! Stored and returned types. Borsh in storage, JSON at the boundary.

use near_sdk::json_types::{U128, U64};
use near_sdk::{near, AccountId};

pub type TokenId = String;
pub type WorkId = u64;
pub type DisputeId = u64;
pub type Bps = u16;
/// 64-character lowercase hex sha256 of an off-chain document.
pub type Hash32 = String;

pub const FULL_BPS: u32 = 10_000;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeType {
    Isrc,
    Iswc,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Human,
    AiAssisted,
    FullyAi,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Copyright,
    Contract,
    None,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RightType {
    Master,
    Publishing,
    Performance,
    Mechanical,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Active,
    Suspended,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credential {
    pub org_name_hash: Hash32,
    pub kyb_ref_hash: Hash32,
    pub status: CredentialStatus,
    pub granted_at: U64,
    pub updated_at: U64,
}

/// A work + right + territory. Shares of one group split payments for it.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupKey {
    pub work_id: WorkId,
    pub right_type: RightType,
    /// "WW" for worldwide or an ISO 3166-1 alpha-2 code.
    pub territory: String,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Active,
    /// Duplicate of an already registered code. Payments to it are refused
    /// until the registrar resolves the dispute (block 4).
    DuplicateOpen {
        dispute_id: DisputeId,
        original: WorkId,
    },
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub id: WorkId,
    pub code_type: CodeType,
    /// Normalised: uppercase, no separators.
    pub code: String,
    pub title_hash: Hash32,
    pub provenance: Provenance,
    pub metadata_uri: String,
    pub registered_by: AccountId,
    pub status: WorkStatus,
    pub created_at: U64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShareStatus {
    /// Vault being created; bps reserved; no NFT yet.
    Pending,
    /// NFT minted, vault ready.
    Active,
    /// Vault creation or token registration failed; no NFT; bps released.
    Failed,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    pub token_id: TokenId,
    pub number: u64,
    pub group: GroupKey,
    pub percent_bps: Bps,
    pub basis: Basis,
    pub evidence_hash: Hash32,
    pub status: ShareStatus,
    pub vault: AccountId,
    pub holder_at_mint: AccountId,
    pub registered_by: AccountId,
    pub note: Option<String>,
    pub last_effective_from: U64,
    pub created_at: U64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    pub account: AccountId,
    pub amount: U128,
    pub since_block: U64,
}

/// Per-share money record. earned = known_balance + total_paid_out.
#[near(serializers = [borsh])]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    /// What the registry believes sits in the vault.
    pub known_balance: u128,
    /// Sum of successful claims.
    pub total_paid_out: u128,
    /// Sum delivered through the registry (history only).
    pub total_routed_in: u128,
    /// Forwarded amount that failed, still held by the registry.
    pub undelivered: u128,
    pub inbound_pending: u32,
    pub payout_in_flight: Option<Lock>,
    pub transfer_pending: Option<Lock>,
    /// Bumped on every ledger change, so a stale sync result is discarded.
    pub epoch: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerView {
    pub known_balance: U128,
    pub total_paid_out: U128,
    pub earned: U128,
    pub claimable: U128,
    pub total_routed_in: U128,
    pub undelivered: U128,
    pub inbound_pending: u32,
    pub payout_in_flight: Option<Lock>,
    pub transfer_pending: Option<Lock>,
    pub epoch: U64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// Token ids in registration order (Pending, Active and Failed).
    pub shares: Vec<TokenId>,
    /// Sum of bps of Active shares.
    pub active_bps: u32,
    /// Sum of bps of Pending shares, counted for the 100 percent check.
    pub reserved_bps: u32,
    /// Unassigned part of payments made while the group was under 10000 bps
    /// (decision 4). Held in the registry's token balance. Only the registrar
    /// can release it, to a share of this group, with a document hash.
    pub unallocated: U128,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisputeKind {
    DuplicateCode { original: WorkId, duplicate: WorkId },
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispute {
    pub id: DisputeId,
    pub kind: DisputeKind,
    pub opened_by: AccountId,
    pub opened_at: U64,
    /// Resolution lands in block 4.
    pub resolved: bool,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accounting {
    pub allowance_total: U128,
    pub unallocated_total: U128,
    pub escrow_total: U128,
    pub undelivered_total: U128,
    pub forwards_in_flight_total: U128,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub registrar: AccountId,
    pub usdc: AccountId,
}

/// `msg` of an `ft_transfer_call` to the registry.
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PaymentMsg {
    PayWork(PayWork),
    PayShare(PayShare),
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PayWork {
    pub work_id: WorkId,
    pub right_type: RightType,
    pub territory: String,
    pub period: String,
    pub statement_hash: Hash32,
    pub statement_line_id: Option<String>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PayShare {
    pub token_id: TokenId,
    pub period: String,
    pub statement_hash: Hash32,
}
