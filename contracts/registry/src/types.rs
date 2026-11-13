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
    /// Voided by a resolved duplicate-code dispute. Never paid again; its
    /// shares are Void.
    Void {
        dispute_id: DisputeId,
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
    /// Every work + right + territory group with shares of this work.
    pub groups: Vec<GroupKey>,
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
    /// Lost a dispute, or its work was voided. The NFT stays as a record,
    /// cannot move and is never paid; money already known stays claimable.
    Void,
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
    /// Set by an account with the payer role (evidence tier payer_confirmed).
    pub payer_attestation: Option<Attestation>,
    /// Open over-allocation dispute this share is part of, as claimant or
    /// as contested share. Frozen while set.
    pub dispute: Option<DisputeId>,
    /// Holders of other shares of the group who confirmed this share
    /// (evidence tier counterparty_confirmed, computed at view time).
    pub confirmations: Vec<AccountId>,
    pub last_effective_from: U64,
    pub created_at: U64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attestation {
    pub payer: AccountId,
    pub statement_hash: Hash32,
    pub at: U64,
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
    /// This share's part of payments held in dispute escrow, in the registry.
    pub escrowed: u128,
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
    pub escrowed: U128,
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
    /// Bps of a minted over-allocation claimant, not paid until resolve.
    pub claimant_bps: u32,
    /// At most one open over-allocation dispute per group.
    pub open_dispute: Option<DisputeId>,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisputeKind {
    DuplicateCode {
        original: WorkId,
        duplicate: WorkId,
    },
    /// A share pushed its group above 10000 bps. The claimant is not paid
    /// and the contested shares' new money goes to escrow until resolve.
    Overallocation {
        group: GroupKey,
        claimant: TokenId,
        contested: Vec<TokenId>,
    },
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BpsEntry {
    pub token_id: TokenId,
    pub bps: Bps,
}

/// Registrar's decision. Must fit the dispute kind.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Over-allocation: the claimant is voided, escrow goes back to the
    /// contested shares.
    UpholdExisting,
    /// Over-allocation: new bps for the contested shares plus the claimant,
    /// summing to the contested shares' old total. Escrow is split by them.
    UpholdClaimant { new_bps: Vec<BpsEntry> },
    /// Duplicate code: the duplicate work and its shares are voided.
    VoidDuplicate,
    /// Duplicate code: both works stay payable (for example two recordings
    /// wrongly sharing a code, fixed off chain).
    KeepBoth,
    /// Duplicate code: the duplicate becomes the payable entry and the
    /// original work and its shares are voided.
    SwapPayable,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisputeStatus {
    Open,
    Resolved {
        outcome: Outcome,
        doc_hash: Hash32,
        at: U64,
    },
    /// The claimant's mint failed; escrow moved to undelivered for retry.
    Cancelled,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispute {
    pub id: DisputeId,
    pub kind: DisputeKind,
    pub opened_by: AccountId,
    pub opened_at: U64,
    pub status: DisputeStatus,
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
    /// Credits the allowance of this agent. Sender must be the agent's organisation.
    FundAllowance(AccountId),
}

/// What an agent pays. There is no payee: the registry resolves vaults (T8).
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PayTarget {
    Work {
        work_id: WorkId,
        right_type: RightType,
        territory: String,
    },
    Share {
        token_id: TokenId,
    },
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMethod {
    PayFromAllowance,
    Report,
}

pub const ROLLING_HOURS: usize = 24;

/// One agent's role inside the registry. Limits are in token base units.
#[near(serializers = [borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRole {
    pub org: AccountId,
    pub methods: Vec<AgentMethod>,
    pub per_tx_limit: u128,
    pub daily_limit: u128,
    pub max_tx_per_day: u32,
    /// Nanoseconds. The role is expired from this time on.
    pub expires_at: u64,
    pub revoked: bool,
    /// Held in the registry's token balance, counted in allowance_total.
    pub allowance: u128,
    /// Statement lines this role may still record; each is prepaid storage.
    pub line_credits: u32,
    /// Rolling 24 hour window, one bucket per hour.
    pub spent: [u128; ROLLING_HOURS],
    pub counts: [u32; ROLLING_HOURS],
    pub bucket_hour: u64,
    pub granted_at: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleView {
    pub agent: AccountId,
    pub org: AccountId,
    pub methods: Vec<AgentMethod>,
    pub per_tx_limit: U128,
    pub daily_limit: U128,
    pub max_tx_per_day: u32,
    pub expires_at: U64,
    pub revoked: bool,
    pub expired: bool,
    pub allowance: U128,
    pub line_credits: u32,
    pub spent_last_24h: U128,
    pub tx_last_24h: u32,
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
