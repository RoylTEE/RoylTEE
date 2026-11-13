//! Rights-share registry.
//!
//! Each rights share of a song is one NEP-171 token. Each token controls its
//! own vault account `s<N>.<registry>`, created here with zero access keys.
//! Only this contract can move money out of a vault, and only to whoever
//! holds the token at that moment. Nothing is handed over on transfer.
//!
//! Contract names and symbols are passed to `new`; the source carries none.

use near_contract_standards::non_fungible_token::metadata::NFTContractMetadata;
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::{self, json, Value};
use near_sdk::store::{IterableMap, LazyOption, LookupMap, LookupSet};
use near_sdk::{
    env, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise,
    StorageUsage,
};

mod agents;
mod disputes;
mod holder;
pub mod logic;
mod membership;
mod money;
mod nft;
mod registration;
pub mod types;

use types::*;

/// Custom event standard. Name-neutral on purpose (DESIGN.md section 13).
pub const EVENT_STANDARD: &str = "share_registry";
pub const EVENT_VERSION: &str = "1.0.0";

/// Vault wasm, embedded so every vault gets its own copy (decision 8).
#[cfg(target_arch = "wasm32")]
pub const VAULT_WASM: &[u8] = include_bytes!("../../res/vault.wasm");
#[cfg(not(target_arch = "wasm32"))]
pub const VAULT_WASM: &[u8] = &[];

/// NEAR sent to each new vault: covers its code storage (about 7.5 KB) and
/// the 1 yoctoNEAR attached to each withdraw.
pub const VAULT_NEAR: NearToken = NearToken::from_millinear(100);
/// NEP-145 registration on the mock token (125 bytes at 1E19 per byte).
pub const FT_STORAGE_DEPOSIT: NearToken = NearToken::from_micronear(1_250);
/// Registry storage reserved at registration for the later NFT mint.
pub const MINT_RESERVE_BYTES: StorageUsage = 1_000;

pub const MAX_SHARES_PER_GROUP: usize = 16;
pub const MAX_GROUPS_PER_WORK: usize = 16;
/// Cap on every entry of a group list, Failed and Void shares included, so
/// loops over a group stay bounded.
pub const MAX_GROUP_ENTRIES: usize = 32;
pub const MAX_URI_LEN: usize = 256;
pub const MAX_NOTE_LEN: usize = 280;
pub const MAX_PERIOD_LEN: usize = 32;
pub const MAX_LINE_ID_LEN: usize = 64;
/// Smallest group payment: 0.01 mock USDC, so 1 bps is at least 1 base unit.
pub const MIN_GROUP_PAYMENT: u128 = 10_000;
/// effective_from may be at most 30 days ahead of the block time.
pub const MAX_EFFECTIVE_FUTURE_NS: u64 = 30 * 24 * 3600 * 1_000_000_000;

/// A lock older than this many blocks may be cleared by anyone (T19).
/// About 20 minutes at one block per second. Long enough under congestion: UNVERIFIED.
pub const STALE_LOCK_BLOCKS: u64 = 1_200;
/// Storage prepaid per statement line id (a 32-byte hash in a set, with margin).
pub const LINE_STORAGE_BYTES: StorageUsage = 100;
pub const MAX_TX_PER_DAY: u32 = 1_000;
pub const NS_PER_HOUR: u64 = 3_600 * 1_000_000_000;

pub const GAS_CREATE_VAULT_CB: Gas = Gas::from_tgas(40);
pub const GAS_STORAGE_DEPOSIT: Gas = Gas::from_tgas(10);
pub const GAS_VAULT_READY_CB: Gas = Gas::from_tgas(20);
pub const GAS_MEMBER_CB: Gas = Gas::from_tgas(5);
pub const GAS_WITHDRAW: Gas = Gas::from_tgas(20);
pub const GAS_CLAIM_CB: Gas = Gas::from_tgas(10);
pub const GAS_FT_TRANSFER: Gas = Gas::from_tgas(8);
pub const GAS_FORWARD_CB: Gas = Gas::from_tgas(5);
pub const GAS_FT_BALANCE: Gas = Gas::from_tgas(5);
pub const GAS_SYNC_CB: Gas = Gas::from_tgas(5);
pub const GAS_NFT_ON_TRANSFER_MIN: Gas = Gas::from_tgas(25);
pub const GAS_RESOLVE_TRANSFER: Gas = Gas::from_tgas(10);

#[derive(BorshStorageKey)]
#[near]
enum Key {
    TokenOwners,
    TokenMetadata,
    TokenEnumeration,
    ContractMetadata,
    Credentials,
    Works,
    CodeIndex,
    Shares,
    Ledgers,
    Groups,
    Disputes,
    Roles,
    Payers,
    StatementLines,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Registry {
    registrar: AccountId,
    usdc: AccountId,
    tokens: NonFungibleToken,
    metadata: LazyOption<NFTContractMetadata>,
    credentials: LookupMap<AccountId, Credential>,
    works: IterableMap<WorkId, Work>,
    code_index: LookupMap<String, WorkId>,
    shares: IterableMap<TokenId, Share>,
    ledgers: LookupMap<TokenId, Ledger>,
    groups: LookupMap<GroupKey, Group>,
    disputes: IterableMap<DisputeId, Dispute>,
    roles: LookupMap<AccountId, AgentRole>,
    payers: LookupSet<AccountId>,
    /// sha256(org || 0x00 || statement_line_id) of every line paid (T9).
    statement_lines: LookupSet<Vec<u8>>,
    /// Solvency counters (T31). Allowance and escrow arrive in blocks 3 and 4.
    allowance_total: u128,
    unallocated_total: u128,
    escrow_total: u128,
    undelivered_total: u128,
    forwards_in_flight_total: u128,
    next_share: u64,
    next_work: WorkId,
    next_dispute: DisputeId,
}

#[near]
impl Registry {
    #[init]
    pub fn new(registrar: AccountId, usdc: AccountId, metadata: NFTContractMetadata) -> Self {
        // Deploy and init are separate actions: only the account itself
        // (its own key, in the deploy transaction) may initialise (audit M1).
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "only the registry account itself can initialise"
        );
        metadata.assert_valid();
        Self {
            registrar,
            usdc,
            // Approvals are disabled (T5), so no approval prefix.
            tokens: NonFungibleToken::new(
                Key::TokenOwners,
                env::current_account_id(),
                Some(Key::TokenMetadata),
                Some(Key::TokenEnumeration),
                None::<Key>,
            ),
            metadata: LazyOption::new(Key::ContractMetadata, Some(metadata)),
            credentials: LookupMap::new(Key::Credentials),
            works: IterableMap::new(Key::Works),
            code_index: LookupMap::new(Key::CodeIndex),
            shares: IterableMap::new(Key::Shares),
            ledgers: LookupMap::new(Key::Ledgers),
            groups: LookupMap::new(Key::Groups),
            disputes: IterableMap::new(Key::Disputes),
            roles: LookupMap::new(Key::Roles),
            payers: LookupSet::new(Key::Payers),
            statement_lines: LookupSet::new(Key::StatementLines),
            allowance_total: 0,
            unallocated_total: 0,
            escrow_total: 0,
            undelivered_total: 0,
            forwards_in_flight_total: 0,
            next_share: 0,
            next_work: 0,
            next_dispute: 0,
        }
    }

    // ---------- views ----------

    pub fn get_config(&self) -> Config {
        Config {
            registrar: self.registrar.clone(),
            usdc: self.usdc.clone(),
        }
    }

    /// Hex sha256 of the embedded vault code. Every vault's on-chain code
    /// hash must match it.
    pub fn get_vault_code_hash(&self) -> String {
        hex(&env::sha256(VAULT_WASM))
    }

    pub fn get_work(&self, work_id: WorkId) -> Option<Work> {
        self.works.get(&work_id).cloned()
    }

    pub fn get_share(&self, token_id: TokenId) -> Option<Share> {
        self.shares.get(&token_id).cloned()
    }

    pub fn get_ledger(&self, token_id: TokenId) -> Option<LedgerView> {
        self.ledgers.get(&token_id).map(ledger_view)
    }

    pub fn get_group(
        &self,
        work_id: WorkId,
        right_type: RightType,
        territory: String,
    ) -> Option<Group> {
        self.groups
            .get(&GroupKey {
                work_id,
                right_type,
                territory,
            })
            .cloned()
    }

    pub fn get_credential(&self, account_id: AccountId) -> Option<Credential> {
        self.credentials.get(&account_id).cloned()
    }

    pub fn is_active_member(&self, account_id: AccountId) -> bool {
        self.is_active(&account_id)
    }

    pub fn get_dispute(&self, dispute_id: DisputeId) -> Option<Dispute> {
        self.disputes.get(&dispute_id).cloned()
    }

    pub fn get_accounting(&self) -> Accounting {
        Accounting {
            allowance_total: U128(self.allowance_total),
            unallocated_total: U128(self.unallocated_total),
            escrow_total: U128(self.escrow_total),
            undelivered_total: U128(self.undelivered_total),
            forwards_in_flight_total: U128(self.forwards_in_flight_total),
        }
    }
}

// ---------- internal helpers ----------

impl Registry {
    fn is_active(&self, account: &AccountId) -> bool {
        matches!(
            self.credentials.get(account),
            Some(Credential {
                status: CredentialStatus::Active,
                ..
            })
        )
    }

    fn is_suspended(&self, account: &AccountId) -> bool {
        matches!(
            self.credentials.get(account),
            Some(Credential {
                status: CredentialStatus::Suspended,
                ..
            })
        )
    }

    pub(crate) fn is_system_account(&self, account: &AccountId) -> bool {
        let me = env::current_account_id();
        account == &me || account.as_str().ends_with(&format!(".{me}")) || account == &self.usdc
    }

    fn assert_active_caller(&self) -> AccountId {
        let caller = env::predecessor_account_id();
        require!(self.is_active(&caller), "caller has no active credential");
        caller
    }

    fn assert_registrar(&self) {
        require!(
            env::predecessor_account_id() == self.registrar,
            "registrar only"
        );
    }

    fn share(&self, token_id: &TokenId) -> &Share {
        self.shares
            .get(token_id)
            .unwrap_or_else(|| env::panic_str("unknown share"))
    }

    /// True if the share is the claimant of an open over-allocation dispute.
    pub(crate) fn is_claimant(&self, share: &Share) -> bool {
        share.dispute.is_some_and(|id| {
            matches!(
                self.disputes.get(&id).map(|d| &d.kind),
                Some(DisputeKind::Overallocation { claimant, .. }) if claimant == &share.token_id
            )
        })
    }

    fn ledger_mut(&mut self, token_id: &TokenId) -> &mut Ledger {
        self.ledgers
            .get_mut(token_id)
            .unwrap_or_else(|| env::panic_str("unknown share"))
    }

    fn owner_of(&self, token_id: &TokenId) -> Option<AccountId> {
        self.tokens.owner_by_id.get(token_id)
    }

    fn lock_here(account: AccountId, amount: u128) -> Lock {
        Lock {
            account,
            amount: U128(amount),
            since_block: U64(env::block_height()),
        }
    }
}

pub(crate) fn ledger_view(l: &Ledger) -> LedgerView {
    LedgerView {
        known_balance: U128(l.known_balance),
        total_paid_out: U128(l.total_paid_out),
        earned: U128(l.known_balance + l.total_paid_out),
        claimable: U128(l.known_balance),
        total_routed_in: U128(l.total_routed_in),
        undelivered: U128(l.undelivered),
        escrowed: U128(l.escrowed),
        inbound_pending: l.inbound_pending,
        payout_in_flight: l.payout_in_flight.clone(),
        transfer_pending: l.transfer_pending.clone(),
        epoch: U64(l.epoch),
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

/// Charges the caller for storage added since `initial` plus `extra_bytes`,
/// plus `forward` NEAR that the call passes on. Refunds the rest.
pub(crate) fn charge_storage(initial: StorageUsage, extra_bytes: StorageUsage, forward: NearToken) {
    let used = env::storage_usage().saturating_sub(initial) + extra_bytes;
    let required = env::storage_byte_cost()
        .saturating_mul(u128::from(used))
        .saturating_add(forward);
    let attached = env::attached_deposit();
    require!(
        attached >= required,
        format!("attach at least {} yoctoNEAR", required.as_yoctonear())
    );
    let refund = attached.saturating_sub(required);
    if !refund.is_zero() {
        Promise::new(env::predecessor_account_id())
            .transfer(refund)
            .detach();
    }
}

/// Reads the single promise result of a callback, bounded in size.
pub(crate) fn callback_result(max_len: usize) -> Option<Vec<u8>> {
    if env::promise_results_count() != 1 {
        return None;
    }
    env::promise_result_checked(0, max_len).ok()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 15) as usize] as char);
    }
    s
}

pub(crate) fn json_bytes(v: Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap_or_else(|_| env::panic_str("json"))
}

#[cfg(test)]
mod tests;
