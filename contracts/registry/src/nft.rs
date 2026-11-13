//! NEP-171 core, NEP-177 metadata, NEP-181 enumeration, with transfer locks.
//!
//! Transfers need 1 yoctoNEAR, go only to accounts with an active
//! credential (T10), and fail while a payout is in flight (T3) or another
//! transfer_call is pending (T4). Approvals are disabled (T5).

use crate::*;
use near_contract_standards::non_fungible_token::core::NonFungibleTokenCore;
use near_contract_standards::non_fungible_token::enumeration::NonFungibleTokenEnumeration;
use near_contract_standards::non_fungible_token::events::NftTransfer;
use near_contract_standards::non_fungible_token::metadata::NFTContractMetadata;
use near_contract_standards::non_fungible_token::Token;
use near_sdk::{assert_one_yocto, GasWeight, PromiseOrValue};
use std::collections::HashMap;

const APPROVALS_DISABLED: &str = "approvals are disabled; holders sign transfers themselves";

#[near]
impl Registry {
    /// NEP-171 transfer with an extra optional `effective_from` (nanoseconds),
    /// recorded in history only. Payout logic ignores it: whoever holds the
    /// token at claim time gets the whole vault balance.
    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        effective_from: Option<U64>,
    ) {
        assert_one_yocto();
        let sender = env::predecessor_account_id();
        let eff = self.check_transfer(
            &sender,
            &receiver_id,
            &token_id,
            approval_id,
            effective_from,
        );
        self.move_token(&sender, &receiver_id, &token_id, memo, eff, false);
    }

    /// NEP-171 transfer_call. Sets `transfer_pending` until
    /// `nft_resolve_transfer` runs, so the receiver cannot claim, transfer
    /// onward or sync during `nft_on_transfer` (T4).
    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
        effective_from: Option<U64>,
    ) -> PromiseOrValue<bool> {
        assert_one_yocto();
        require!(
            env::prepaid_gas().as_gas()
                >= GAS_NFT_ON_TRANSFER_MIN.as_gas()
                    + GAS_RESOLVE_TRANSFER.as_gas()
                    + Gas::from_tgas(15).as_gas(),
            "not enough gas attached for nft_transfer_call"
        );
        let sender = env::predecessor_account_id();
        let eff = self.check_transfer(
            &sender,
            &receiver_id,
            &token_id,
            approval_id,
            effective_from,
        );
        self.ledger_mut(&token_id).transfer_pending = Some(Self::lock_here(receiver_id.clone(), 0));
        self.move_token(&sender, &receiver_id, &token_id, memo, eff, true);
        Promise::new(receiver_id.clone())
            .function_call_weight(
                "nft_on_transfer".to_string(),
                json_bytes(json!({
                    "sender_id": sender,
                    "previous_owner_id": sender,
                    "token_id": token_id,
                    "msg": msg,
                })),
                NearToken::from_yoctonear(0),
                GAS_NFT_ON_TRANSFER_MIN,
                GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RESOLVE_TRANSFER)
                    .with_unused_gas_weight(0)
                    .nft_resolve_transfer(
                        sender,
                        receiver_id,
                        token_id,
                        None,
                        Some(U64(env::block_height())),
                    ),
            )
            .into()
    }

    /// Clears `transfer_pending` first, then returns the token to the
    /// previous owner if the receiver asked for it (returned true) or failed,
    /// and still owns it. The return path skips the credential check: it
    /// restores the prior state. Never panics. Returns true if the receiver
    /// kept the token.
    #[private]
    pub fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        approved_account_ids: Option<HashMap<AccountId, u64>>,
        lock_block: Option<U64>,
    ) -> bool {
        let _ = approved_account_ids;
        // Act only if the pending lock is still ours. If it was released as
        // stale, the current owner keeps the token and a newer lock stays.
        let mine = self.ledgers.get(&token_id).is_some_and(|l| {
            l.transfer_pending
                .as_ref()
                .is_some_and(|k| Some(k.since_block) == lock_block && k.account == receiver_id)
        });
        if !mine {
            return true;
        }
        if let Some(l) = self.ledgers.get_mut(&token_id) {
            l.transfer_pending = None;
        }
        let must_return = match callback_result(16) {
            Some(v) => serde_json::from_slice::<bool>(&v).unwrap_or(true),
            None => true,
        };
        if !must_return || self.owner_of(&token_id).as_ref() != Some(&receiver_id) {
            return true;
        }
        self.tokens
            .internal_transfer_unguarded(&token_id, &receiver_id, &previous_owner_id);
        NftTransfer {
            old_owner_id: &receiver_id,
            new_owner_id: &previous_owner_id,
            token_ids: &[&token_id],
            authorized_id: None,
            memo: None,
        }
        .emit();
        emit(
            "transfer_returned",
            json!({ "token_id": token_id, "from": receiver_id, "to": previous_owner_id }),
        );
        false
    }

    /// Moves a suspended holder's share to an active member (decision 3).
    /// Registrar only, 1 yoctoNEAR, with a document hash. Unclaimed money
    /// travels with the token as in any transfer.
    #[payable]
    pub fn registrar_transfer(
        &mut self,
        token_id: TokenId,
        receiver_id: AccountId,
        doc_hash: Hash32,
    ) {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let owner = self
            .owner_of(&token_id)
            .unwrap_or_else(|| env::panic_str("token not found"));
        require!(
            self.is_suspended(&owner),
            "registrar can only move shares of suspended holders"
        );
        require!(receiver_id != owner, "receiver is already the holder");
        require!(
            self.share(&token_id).status == ShareStatus::Active,
            "share is not transferable"
        );
        require!(
            self.share(&token_id).dispute.is_none(),
            "share is frozen by a dispute"
        );
        let ledger = self
            .ledgers
            .get(&token_id)
            .unwrap_or_else(|| env::panic_str("unknown share"));
        require!(
            ledger.payout_in_flight.is_none(),
            "payout in flight: transfer after it settles"
        );
        require!(
            ledger.transfer_pending.is_none(),
            "another transfer is pending"
        );
        require!(
            self.is_active(&receiver_id),
            "receiver has no active credential"
        );
        let now = env::block_timestamp().max(self.share(&token_id).last_effective_from.0);
        self.move_token(&owner, &receiver_id, &token_id, None, now, false);
        emit(
            "registrar_transfer",
            json!({
                "token_id": token_id,
                "from": owner,
                "to": receiver_id,
                "doc_hash": doc_hash,
                "by": env::predecessor_account_id(),
            }),
        );
    }

    /// Token with live fields in `metadata.extra` (decision 7): earned, paid
    /// out, claimable, locks, status, provenance, basis, split.
    pub fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        self.tokens
            .nft_token(token_id)
            .map(|t| self.with_live_fields(t))
    }

    #[payable]
    pub fn nft_approve(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        let _ = (token_id, account_id, msg);
        env::panic_str(APPROVALS_DISABLED)
    }

    #[payable]
    pub fn nft_revoke(&mut self, token_id: TokenId, account_id: AccountId) {
        let _ = (token_id, account_id);
        env::panic_str(APPROVALS_DISABLED)
    }

    #[payable]
    pub fn nft_revoke_all(&mut self, token_id: TokenId) {
        let _ = token_id;
        env::panic_str(APPROVALS_DISABLED)
    }

    pub fn nft_is_approved(
        &self,
        token_id: TokenId,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        let _ = (token_id, approved_account_id, approval_id);
        false
    }

    pub fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata
            .get()
            .clone()
            .unwrap_or_else(|| env::panic_str("metadata missing"))
    }

    pub fn nft_total_supply(&self) -> U128 {
        self.tokens.nft_total_supply()
    }

    pub fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.tokens
            .nft_tokens(from_index, limit)
            .into_iter()
            .map(|t| self.with_live_fields(t))
            .collect()
    }

    pub fn nft_supply_for_owner(&self, account_id: AccountId) -> U128 {
        self.tokens.nft_supply_for_owner(account_id)
    }

    pub fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        self.tokens
            .nft_tokens_for_owner(account_id, from_index, limit)
            .into_iter()
            .map(|t| self.with_live_fields(t))
            .collect()
    }
}

impl Registry {
    /// All transfer checks. Returns the effective_from to record.
    fn check_transfer(
        &self,
        sender: &AccountId,
        receiver: &AccountId,
        token_id: &TokenId,
        approval_id: Option<u64>,
        effective_from: Option<U64>,
    ) -> u64 {
        require!(approval_id.is_none(), APPROVALS_DISABLED);
        let owner = self
            .owner_of(token_id)
            .unwrap_or_else(|| env::panic_str("token not found"));
        require!(&owner == sender, "only the current holder can transfer");
        require!(receiver != sender, "receiver is already the holder");
        let share = self.share(token_id);
        require!(
            share.status == ShareStatus::Active,
            "share is not transferable"
        );
        require!(share.dispute.is_none(), "share is frozen by a dispute");
        let ledger = self
            .ledgers
            .get(token_id)
            .unwrap_or_else(|| env::panic_str("unknown share"));
        require!(
            ledger.payout_in_flight.is_none(),
            "payout in flight: transfer after it settles"
        );
        require!(
            ledger.transfer_pending.is_none(),
            "another transfer is pending"
        );
        // Decision 3: a suspended holder can neither send nor receive.
        require!(!self.is_suspended(sender), "holder credential is suspended");
        require!(
            self.is_active(receiver),
            "receiver has no active credential"
        );
        let now = env::block_timestamp();
        let eff = effective_from.map_or(now, |e| e.0);
        require!(
            eff >= share.last_effective_from.0
                && eff <= now.saturating_add(MAX_EFFECTIVE_FUTURE_NS),
            "effective_from out of range"
        );
        eff
    }

    fn move_token(
        &mut self,
        sender: &AccountId,
        receiver: &AccountId,
        token_id: &TokenId,
        memo: Option<String>,
        effective_from: u64,
        via_call: bool,
    ) {
        // Emits the standard nep171 nft_transfer event.
        self.tokens
            .internal_transfer(sender, receiver, token_id, None, memo);
        if let Some(s) = self.shares.get_mut(token_id) {
            s.last_effective_from = U64(effective_from);
        }
        emit(
            "share_transfer",
            json!({
                "token_id": token_id,
                "from": sender,
                "to": receiver,
                "effective_from": U64(effective_from),
                "via_call": via_call,
            }),
        );
    }

    fn with_live_fields(&self, mut token: Token) -> Token {
        let (Some(share), Some(ledger)) = (
            self.shares.get(&token.token_id),
            self.ledgers.get(&token.token_id),
        ) else {
            return token;
        };
        let provenance = self.works.get(&share.group.work_id).map(|w| w.provenance);
        let v = ledger_view(ledger);
        let extra = json!({
            "kind": "share",
            "work_id": share.group.work_id,
            "right_type": share.group.right_type,
            "territory": share.group.territory,
            "percent_bps": share.percent_bps,
            "basis": share.basis,
            "provenance": provenance,
            "evidence_tier": self.evidence_tier(share),
            "confirmations": share.confirmations,
            "dispute": share.dispute,
            "escrowed": v.escrowed,
            "payer_attestation": share.payer_attestation,
            "note": share.note,
            "status": share.status,
            "vault": share.vault,
            "earned": v.earned,
            "paid_out": v.total_paid_out,
            "claimable": v.claimable,
            "payout_in_flight": v.payout_in_flight.is_some(),
            "transfer_pending": v.transfer_pending.is_some(),
            "last_effective_from": share.last_effective_from,
        });
        if let Some(m) = token.metadata.as_mut() {
            m.extra = Some(extra.to_string());
        }
        token
    }
}
