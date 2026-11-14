//! Sealed-bid, second-price share auction (RoylTEE).
//!
//! One contract holds many auctions, one per registry share at a time. It is
//! a plain credentialed member of the registry: it needs one
//! `grant_credential` from the registrar and no registry change.
//!
//! - **Listing.** A seller calls the registry's `nft_transfer_call` to this
//!   contract. `nft_on_transfer` records the auction and returns `false`, so
//!   the registry leaves the share here.
//! - **Sealed bids.** A bidder sends exactly `max_deposit` mock USDC with
//!   `ft_transfer_call`. The message carries a ciphertext only the TEE can
//!   open. Equal deposits mean the deposit reveals nothing about the bid.
//! - **Settle.** After `ends_at` anyone asks OutLayer to run the pinned WASI
//!   module. It decrypts inside the TEE and returns only the winner, the
//!   second price and the hash of the bid list it decided on.
//! - **Payout.** The share goes to the winner first. Only if that works are
//!   the seller (price), the winner (change) and the losers (their deposit)
//!   credited. Any failure ends in a cancelled auction: every deposit is
//!   credited back and the share returns to the seller.
//!
//! This contract has no code path that calls `claim` on the registry. It has
//! no owner and no admin method. Credits are paid out with `withdraw`.

use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::{self, json, Value};
use near_sdk::store::LookupMap;
use near_sdk::{
    env, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise,
};

mod bids;
mod listing;
pub mod logic;
mod payout;
mod settle;
pub mod types;

use types::*;

pub const EVENT_STANDARD: &str = "share_auction";
pub const EVENT_VERSION: &str = "1.0.0";

/// Most bids one auction takes. Bounds the OutLayer input and the payout loop.
pub const MAX_BIDS: usize = 8;
/// Longest ciphertext accepted, in base64 characters.
pub const MAX_CIPHERTEXT: usize = 512;
/// Shortest and longest bidding window, in nanoseconds.
pub const MIN_DURATION_NS: u64 = 60 * NS_PER_SEC;
pub const MAX_DURATION_NS: u64 = 14 * 24 * 3600 * NS_PER_SEC;
pub const NS_PER_SEC: u64 = 1_000_000_000;
/// After `ends_at` plus this, anyone can cancel an auction whose settle runs
/// have failed at least once (see `cancel`).
pub const SETTLE_GRACE_NS: u64 = 6 * 3600 * NS_PER_SEC;
/// After `ends_at` plus this, anyone can cancel an unsettled auction even if
/// no settle run ever failed (nobody could or would settle).
pub const SETTLE_HARD_GRACE_NS: u64 = 7 * 24 * 3600 * NS_PER_SEC;
/// A delivery or a share return that has not reported back after this can be
/// reconciled from the registry (`reconcile`).
pub const RECONCILE_AFTER_NS: u64 = 15 * 60 * NS_PER_SEC;
/// A settle request that has not answered after this can be cancelled.
pub const SETTLING_STALE_NS: u64 = 3600 * NS_PER_SEC;
/// Least NEAR a settle call must attach for OutLayer (the rest is refunded to
/// the caller by OutLayer, the spike's run was charged about 0.001 of 0.01).
pub const MIN_SETTLE_DEPOSIT: NearToken = NearToken::from_millinear(10);
/// NEAR this contract must keep free (balance minus storage) before it takes
/// a new auction or bid, so later credits can always be written.
pub const MIN_FREE_BALANCE: NearToken = NearToken::from_millinear(250);
/// Largest reserve or `max_deposit` a listing may name: 38 decimal digits.
/// The module, the bid script and the callback all read amounts at this
/// width, so one limit holds everywhere.
pub const MAX_TERM: u128 = 99_999_999_999_999_999_999_999_999_999_999_999_999;
/// Storage, in bytes, the contract reserves for every accepted auction until
/// it finishes: eight maximum size bids, the refund or payout entries for
/// nine accounts with 64 character names, and the record itself. A test
/// measures the real worst case against this number.
pub const STORAGE_RESERVE_BYTES: u64 = 8_000;
/// Version of the key and bid format. The module prints it with its public
/// key and refuses a settle request that names another version or another
/// key, so a changed master secret fails loudly instead of ending in no sale.
pub const KEY_VERSION: u64 = 1;
/// Most ids `get_auctions` looks at in one call.
pub const MAX_SCAN: u64 = 200;
/// Most accounts one `withdraw` call pays.
pub const MAX_WITHDRAW: usize = 9;
/// Longest OutLayer answer read in the callback, in bytes.
pub const MAX_RETURN: usize = 4096;

pub const GAS_IS_MEMBER: Gas = Gas::from_tgas(10);
pub const GAS_BID_CB: Gas = Gas::from_tgas(20);
pub const GAS_REQUEST: Gas = Gas::from_tgas(40);
pub const GAS_SETTLED_CB: Gas = Gas::from_tgas(120);
pub const GAS_KEY_CB: Gas = Gas::from_tgas(15);
pub const GAS_NFT_TRANSFER: Gas = Gas::from_tgas(25);
pub const GAS_DELIVERED_CB: Gas = Gas::from_tgas(60);
pub const GAS_RETURNED_CB: Gas = Gas::from_tgas(10);
pub const GAS_NFT_TOKEN: Gas = Gas::from_tgas(10);
/// Longest `nft_token` answer read when checking who holds a share.
pub const MAX_TOKEN_RETURN: usize = 16 * 1024;
pub const GAS_FT_TRANSFER: Gas = Gas::from_tgas(15);
pub const GAS_WITHDRAW_CB: Gas = Gas::from_tgas(8);
/// Gas a `settle` call must carry: the OutLayer call plus the callback chain.
pub const GAS_SETTLE_MIN: Gas = Gas::from_tgas(230);
/// Promise fees per paid account in `withdraw`, on top of the two static gas
/// amounts (measured as about 2.5 Tgas, rounded up).
pub const GAS_WITHDRAW_FEES: Gas = Gas::from_tgas(3);
pub const GAS_RECONCILE_CB: Gas = Gas::from_tgas(60);

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Auctions,
    Owed,
    TokenAuction,
    Pending,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct AuctionContract {
    registry: AccountId,
    usdc: AccountId,
    outlayer: AccountId,
    wasm_url: String,
    wasm_hash: String,
    secrets_account: AccountId,
    secrets_profile: String,
    limits: Limits,
    tee_pubkey: Option<String>,
    key_requested_at: u64,
    next_id: u64,
    auctions: LookupMap<u64, AuctionRec>,
    /// Active auction id by share token id (set while the share is held).
    token_auction: LookupMap<String, u64>,
    /// mUSDC this contract owes to accounts, paid by `withdraw`.
    owed: LookupMap<AccountId, u128>,
    /// mUSDC deposits of auctions that are not finished yet.
    escrow_total: u128,
    owed_total: u128,
    /// Accepted auctions that are not finished; each holds a storage reserve.
    open_auctions: u32,
    /// Withdrawals sent whose callback has not reported yet.
    pending: LookupMap<u64, Pending>,
    next_pending: u64,
    pending_total: u128,
    /// Last operation id handed out (deliveries and returns).
    next_op: u64,
}

#[near]
impl AuctionContract {
    /// `setup.wasm_url` and `setup.wasm_hash` pin the WASI module OutLayer
    /// runs (hash is 64 lowercase hex). `setup.secrets_account` and
    /// `setup.secrets_profile` name the OutLayer secret row that holds the
    /// TEE master secret.
    #[init]
    pub fn new(registry: AccountId, usdc: AccountId, setup: OutlayerSetup) -> Self {
        // Deploy and init in one transaction signed by this account, so no
        // one else can be first to choose the pinned settings.
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "init must be called by the contract account itself"
        );
        require!(
            setup.wasm_url.starts_with("https://") && setup.wasm_url.len() <= 300,
            "wasm_url must be an https URL"
        );
        require!(
            logic::is_hash64(&setup.wasm_hash),
            "wasm_hash must be 64 lowercase hex characters"
        );
        require!(
            !setup.secrets_profile.is_empty() && setup.secrets_profile.len() <= 64,
            "secrets_profile must be 1 to 64 bytes"
        );
        Self {
            registry,
            usdc,
            outlayer: setup.outlayer,
            wasm_url: setup.wasm_url,
            wasm_hash: setup.wasm_hash,
            secrets_account: setup.secrets_account,
            secrets_profile: setup.secrets_profile,
            limits: Limits::default(),
            tee_pubkey: None,
            key_requested_at: 0,
            next_id: 0,
            auctions: LookupMap::new(StorageKey::Auctions),
            token_auction: LookupMap::new(StorageKey::TokenAuction),
            owed: LookupMap::new(StorageKey::Owed),
            escrow_total: 0,
            owed_total: 0,
            open_auctions: 0,
            pending: LookupMap::new(StorageKey::Pending),
            next_pending: 0,
            pending_total: 0,
            next_op: 0,
        }
    }

    // ---------- views ----------

    pub fn get_config(&self) -> Config {
        Config {
            registry: self.registry.clone(),
            usdc: self.usdc.clone(),
            outlayer: self.outlayer.clone(),
            wasm_url: self.wasm_url.clone(),
            wasm_hash: self.wasm_hash.clone(),
            secrets_account: self.secrets_account.clone(),
            secrets_profile: self.secrets_profile.clone(),
            limits: self.limits.clone(),
            next_id: U64(self.next_id),
            pricing_rule: "second_price_with_reserve".to_string(),
        }
    }

    /// The public key bidders encrypt to (64 hex characters), once published.
    pub fn get_tee_pubkey(&self) -> Option<String> {
        self.tee_pubkey.clone()
    }

    pub fn get_auction(&self, auction_id: U64) -> Option<AuctionView> {
        self.auctions.get(&auction_id.0).map(AuctionView::from)
    }

    /// Live auctions from `from_index`, at most `limit` (50 at most), looking
    /// at no more than `MAX_SCAN` ids. Deleted records leave holes in the id
    /// space, so use `next` to continue, not the page length.
    pub fn get_auctions(&self, from_index: Option<U64>, limit: Option<u64>) -> AuctionPage {
        let from = from_index.map_or(0, |v| v.0);
        let limit = limit.unwrap_or(20).clamp(1, 50) as usize;
        let mut items = Vec::new();
        let mut i = from;
        while i < self.next_id && i < from + MAX_SCAN && items.len() < limit {
            if let Some(a) = self.auctions.get(&i) {
                items.push(AuctionView::from(a));
            }
            i += 1;
        }
        AuctionPage {
            items,
            next: (i < self.next_id).then_some(U64(i)),
        }
    }

    /// The active auction that holds this share, if any.
    pub fn auction_for_token(&self, token_id: String) -> Option<U64> {
        self.token_auction.get(&token_id).copied().map(U64)
    }

    pub fn get_owed(&self, account_id: AccountId) -> U128 {
        U128(self.owed.get(&account_id).copied().unwrap_or(0))
    }

    pub fn get_accounting(&self) -> Accounting {
        Accounting {
            escrow_total: U128(self.escrow_total),
            owed_total: U128(self.owed_total),
            pending_total: U128(self.pending_total),
            open_auctions: self.open_auctions,
            reserve_per_auction: U128(self.reserve_per_auction().as_yoctonear()),
        }
    }

    /// A withdrawal that has not reported back yet, by id (see the `withdraw_sent` event).
    pub fn get_pending_withdrawal(&self, id: U64) -> Option<Pending> {
        self.pending.get(&id.0).cloned()
    }

    /// sha256 hex of the bid list as it stands now: what a settle result must
    /// name in `bids_hash`.
    pub fn get_bids_hash(&self, auction_id: U64) -> String {
        let a = self.auction(auction_id.0);
        logic::bids_hash(&a.id.to_string(), &a.bids)
    }
}

// ---------- internal helpers ----------

impl AuctionContract {
    pub(crate) fn auction(&self, id: u64) -> &AuctionRec {
        self.auctions
            .get(&id)
            .unwrap_or_else(|| env::panic_str("unknown auction"))
    }

    pub(crate) fn auction_mut(&mut self, id: u64) -> &mut AuctionRec {
        self.auctions
            .get_mut(&id)
            .unwrap_or_else(|| env::panic_str("unknown auction"))
    }

    pub(crate) fn credit(&mut self, account: &AccountId, amount: u128) {
        if amount == 0 {
            return;
        }
        let cur = self.owed.get(account).copied().unwrap_or(0);
        self.owed.insert(account.clone(), cur + amount);
        self.owed_total += amount;
    }

    /// NEAR this contract can spend on new storage.
    pub(crate) fn free_balance(&self) -> NearToken {
        let locked = env::storage_byte_cost().saturating_mul(u128::from(env::storage_usage()));
        env::account_balance().saturating_sub(locked)
    }

    /// A new operation id (delivery or return), never 0 and never reused.
    pub(crate) fn new_op(&mut self) -> u64 {
        self.next_op += 1;
        self.next_op
    }

    /// Removes the token index entry only if it still points at this auction
    /// (a newer auction of the same share must keep its entry).
    pub(crate) fn release_token(&mut self, token_id: &str, auction_id: u64) {
        if self.token_auction.get(token_id) == Some(&auction_id) {
            self.token_auction.remove(token_id);
        }
    }

    /// The share is out of this contract's hands for good: stop holding it,
    /// release its index entry, and drop the record of an auction that never
    /// had a bid (nothing is owed to anyone, so nothing needs auditing).
    pub(crate) fn share_out(&mut self, id: u64, event: &str, owner: Option<&str>) {
        let (token_id, reclaim) = {
            let a = self.auction_mut(id);
            a.share_held = false;
            a.returning = false;
            (
                a.token_id.clone(),
                a.status == Status::Cancelled && a.bids.is_empty(),
            )
        };
        self.release_token(&token_id, id);
        emit(event, json!({ "auction_id": U64(id), "owner": owner }));
        if reclaim {
            self.auctions.remove(&id);
        }
    }

    /// NEAR kept free for each accepted auction until it finishes.
    pub(crate) fn reserve_per_auction(&self) -> NearToken {
        env::storage_byte_cost().saturating_mul(u128::from(STORAGE_RESERVE_BYTES))
    }

    /// Free NEAR needed to accept one more auction: the floor plus the
    /// reserve of every open auction and of the new one.
    pub(crate) fn listing_floor(&self) -> NearToken {
        MIN_FREE_BALANCE.saturating_add(
            self.reserve_per_auction()
                .saturating_mul(u128::from(self.open_auctions) + 1),
        )
    }

    /// An auction reached a final state: its reserve is released and the bid
    /// ciphertexts, which are no longer needed, are dropped to free storage.
    pub(crate) fn finish_record(&mut self, id: u64) {
        self.open_auctions = self.open_auctions.saturating_sub(1);
        for b in &mut self.auction_mut(id).bids {
            b.ciphertext = String::new();
        }
    }

    /// Refunds every deposit of the auction into the owed ledger.
    pub(crate) fn credit_all_deposits(&mut self, id: u64) {
        let (max, bidders) = {
            let a = self.auction(id);
            (
                a.max_deposit.0,
                a.bids.iter().map(|b| b.bidder.clone()).collect::<Vec<_>>(),
            )
        };
        for b in &bidders {
            self.credit(b, max);
        }
        self.escrow_total -= max * bidders.len() as u128;
    }

    /// Marks the auction cancelled, credits every deposit back and starts the
    /// share's return to the seller.
    pub(crate) fn cancel_now(&mut self, id: u64, reason: &str) -> Option<Promise> {
        self.credit_all_deposits(id);
        {
            let a = self.auction_mut(id);
            a.status = Status::Cancelled;
            a.cancel_reason = Some(reason.to_string());
        }
        self.finish_record(id);
        emit(
            "auction_cancelled",
            json!({ "auction_id": U64(id), "reason": reason }),
        );
        self.start_return(id)
    }
}

/// Emits a NEP-297 event under the custom standard.
pub(crate) fn emit(event: &str, data: Value) {
    env::log_str(&format!(
        "EVENT_JSON:{}",
        json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_VERSION,
            "event": event,
            "data": [data],
        })
    ));
}

pub(crate) fn json_bytes(v: Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap_or_else(|_| env::panic_str("json"))
}

/// First promise result, if exactly one and it succeeded.
pub(crate) fn callback_result(max_len: usize) -> Option<Vec<u8>> {
    if env::promise_results_count() != 1 {
        return None;
    }
    env::promise_result_checked(0, max_len).ok()
}

#[cfg(test)]
mod tests;
