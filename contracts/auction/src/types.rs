use near_sdk::json_types::{U128, U64};
use near_sdk::{near, AccountId};

/// Resource limits sent to OutLayer. Mirrors its `ResourceLimits`.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_instructions: 1_000_000_000,
            max_memory_mb: 64,
            max_execution_seconds: 30,
        }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Bidding, or waiting to be settled (after `ends_at`).
    Open,
    /// An OutLayer request is in flight.
    Settling,
    /// A winner was chosen and the share transfer is in flight.
    Delivering,
    Sold,
    Cancelled,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Bid {
    pub bidder: AccountId,
    /// base64 ciphertext, opaque to this contract.
    pub ciphertext: String,
    pub at: U64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct AuctionRec {
    pub id: u64,
    pub seller: AccountId,
    pub token_id: String,
    pub reserve: U128,
    pub max_deposit: U128,
    pub created_at: U64,
    pub ends_at: U64,
    pub bids: Vec<Bid>,
    pub status: Status,
    /// Settle requests started so far; the callback must match the latest.
    pub attempt: u32,
    pub failures: u32,
    pub settling_since: U64,
    /// True while this contract still holds the share.
    pub share_held: bool,
    /// True while a transfer of the share back to the seller is in flight.
    pub returning: bool,
    pub winner: Option<AccountId>,
    pub price: Option<U128>,
    pub cancel_reason: Option<String>,
    pub last_error: Option<String>,
}

/// What views return: same fields, plus the bid count.
#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct AuctionView {
    #[serde(flatten)]
    pub rec: AuctionRec,
    pub bid_count: u32,
}

impl From<&AuctionRec> for AuctionView {
    fn from(a: &AuctionRec) -> Self {
        AuctionView {
            rec: a.clone(),
            bid_count: a.bids.len() as u32,
        }
    }
}

#[near(serializers = [json])]
pub struct Config {
    pub registry: AccountId,
    pub usdc: AccountId,
    pub outlayer: AccountId,
    pub wasm_url: String,
    pub wasm_hash: String,
    pub secrets_account: AccountId,
    pub secrets_profile: String,
    pub limits: Limits,
    pub next_id: U64,
    pub pricing_rule: String,
}

#[near(serializers = [json])]
pub struct Accounting {
    pub escrow_total: U128,
    pub owed_total: U128,
}
