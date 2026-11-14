//! Paying credits out, and returning a share to its seller.

use crate::*;

#[near]
impl AuctionContract {
    /// Pays each listed account what it is owed in mock USDC, to that same
    /// account only. Anyone can call it. The credit is taken first and put
    /// back if the transfer fails. Up to 9 accounts per call.
    pub fn withdraw(&mut self, accounts: Vec<AccountId>) {
        require!(
            !accounts.is_empty() && accounts.len() <= MAX_WITHDRAW,
            "list 1 to 9 accounts"
        );
        require!(
            env::prepaid_gas().as_gas()
                >= (GAS_FT_TRANSFER.as_gas()
                    + GAS_WITHDRAW_CB.as_gas()
                    + GAS_WITHDRAW_FEES.as_gas())
                    * accounts.len() as u64
                    + Gas::from_tgas(10).as_gas(),
            "not enough gas attached for withdraw"
        );
        let mut seen: Vec<&AccountId> = Vec::new();
        let mut paid = 0usize;
        for acct in &accounts {
            if seen.contains(&acct) {
                continue;
            }
            seen.push(acct);
            let amount = self.owed.get(acct).copied().unwrap_or(0);
            if amount == 0 {
                continue;
            }
            self.owed.remove(acct);
            self.owed_total -= amount;
            paid += 1;
            // A durable record of the money in flight, removed by the callback.
            let pid = self.next_pending;
            self.next_pending += 1;
            self.pending.insert(
                pid,
                Pending {
                    account: acct.clone(),
                    amount: U128(amount),
                    at: U64(env::block_timestamp()),
                },
            );
            self.pending_total += amount;
            emit(
                "withdraw_sent",
                json!({ "id": U64(pid), "account": acct, "amount": U128(amount) }),
            );
            let _ = Promise::new(self.usdc.clone())
                .function_call(
                    "ft_transfer".to_string(),
                    json_bytes(json!({
                        "receiver_id": acct,
                        "amount": U128(amount),
                        "memo": "share auction payout",
                    })),
                    NearToken::from_yoctonear(1),
                    GAS_FT_TRANSFER,
                )
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_WITHDRAW_CB)
                        .on_withdrawn(pid, acct.clone(), U128(amount)),
                );
        }
        require!(paid > 0, "nothing is owed to these accounts");
    }

    /// Closes the pending record. Puts the credit back if the token transfer
    /// failed. If this callback is lost the record stays in `pending`, so the
    /// claim is never erased; it is not refunded automatically, because a
    /// delayed transfer could still land (double payment).
    #[private]
    pub fn on_withdrawn(&mut self, id: u64, account: AccountId, amount: U128) -> bool {
        if self.pending.remove(&id).is_some() {
            self.pending_total = self.pending_total.saturating_sub(amount.0);
        }
        if callback_result(64).is_some() {
            emit("withdrawn", json!({ "account": account, "amount": amount }));
            return true;
        }
        self.credit(&account, amount.0);
        emit(
            "withdraw_failed",
            json!({ "account": account, "amount": amount }),
        );
        false
    }

    /// Retries sending a cancelled auction's share back to its seller. Needed
    /// when the first try failed (for example a dispute froze the share).
    pub fn return_share(&mut self, auction_id: U64) -> Option<Promise> {
        let a = self.auction(auction_id.0);
        require!(a.status == Status::Cancelled, "auction is not cancelled");
        require!(a.share_held, "the share is already out");
        require!(!a.returning, "a return is already in flight");
        self.start_return(auction_id.0)
    }

    /// For a cancelled auction whose share left some other way (for example
    /// the registrar moved it with `registrar_transfer` after this contract's
    /// credential was suspended): reads the owner from the registry and, if
    /// it is no longer this contract, marks the share as out. Anyone can call.
    pub fn refresh_share(&self, auction_id: U64) -> Promise {
        let a = self.auction(auction_id.0);
        require!(a.status == Status::Cancelled, "auction is not cancelled");
        require!(a.share_held, "the share is already out");
        require!(!a.returning, "a return is in flight");
        Promise::new(self.registry.clone())
            .function_call(
                "nft_token".to_string(),
                json_bytes(json!({ "token_id": a.token_id })),
                NearToken::from_yoctonear(0),
                GAS_NFT_TOKEN,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RETURNED_CB)
                    .on_share_checked(auction_id.0),
            )
    }

    #[private]
    pub fn on_share_checked(&mut self, auction_id: u64) -> bool {
        let owner = callback_result(MAX_TOKEN_RETURN)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.get("owner_id").and_then(Value::as_str).map(str::to_owned));
        let Some(owner) = owner else { return false };
        if owner == env::current_account_id().as_str() {
            return false;
        }
        let token_id = {
            let a = self.auction_mut(auction_id);
            if a.status != Status::Cancelled || !a.share_held || a.returning {
                return false;
            }
            a.share_held = false;
            a.token_id.clone()
        };
        self.token_auction.remove(&token_id);
        emit(
            "share_out",
            json!({ "auction_id": U64(auction_id), "owner": owner }),
        );
        true
    }

    /// Recovery when a callback never reported back (for example the sale
    /// payout could not be written because the contract was short of NEAR for
    /// storage, or a return callback was lost). Anyone can call it once the
    /// auction has been Delivering, or Cancelled with a return in flight, for
    /// 15 minutes. It reads who owns the share in the registry and finishes
    /// the job from that fact:
    ///
    /// - Delivering, owner is this contract: the delivery never happened.
    ///   Cancel, refund every deposit, send the share back.
    /// - Delivering, owner is the seller: the share was moved back (for
    ///   example by the registrar). Cancel and refund, the share is out.
    /// - Delivering, owner is anyone else: the winner has it. Pay the sale.
    /// - Cancelled, return in flight, owner is this contract: clear the
    ///   flag so `return_share` can retry.
    /// - Cancelled, return in flight, owner is anyone else: the share is out.
    ///
    /// If the payout still cannot be written, add NEAR to this account and
    /// call it again; nothing is lost in between.
    pub fn reconcile(&self, auction_id: U64) -> Promise {
        let a = self.auction(auction_id.0);
        let now = env::block_timestamp();
        let stuck = match a.status {
            Status::Delivering => now >= a.delivering_since.0 + RECONCILE_AFTER_NS,
            Status::Cancelled => {
                a.share_held && a.returning && now >= a.returning_since.0 + RECONCILE_AFTER_NS
            }
            _ => false,
        };
        require!(stuck, "nothing to reconcile yet");
        let stage = if a.status == Status::Delivering {
            0u8
        } else {
            1u8
        };
        Promise::new(self.registry.clone())
            .function_call(
                "nft_token".to_string(),
                json_bytes(json!({ "token_id": a.token_id })),
                NearToken::from_yoctonear(0),
                GAS_NFT_TOKEN,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RECONCILE_CB)
                    .with_unused_gas_weight(0)
                    .on_reconciled(auction_id.0, stage),
            )
    }

    #[private]
    pub fn on_reconciled(&mut self, auction_id: u64, stage: u8) -> bool {
        let owner = callback_result(MAX_TOKEN_RETURN)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.get("owner_id").and_then(Value::as_str).map(str::to_owned));
        let Some(owner) = owner else { return false };
        let me = env::current_account_id();
        let (status, held, returning, seller, token_id) = {
            let a = self.auction(auction_id);
            (
                a.status,
                a.share_held,
                a.returning,
                a.seller.clone(),
                a.token_id.clone(),
            )
        };
        match (stage, status) {
            // Still the same stuck delivery the call was made for.
            (0, Status::Delivering) => {
                if owner == me.as_str() {
                    let _ = self.cancel_now(auction_id, "delivery did not complete");
                } else if owner == seller.as_str() {
                    self.cancel_share_out(auction_id, "share is back with the seller");
                } else {
                    self.finish_sale(auction_id);
                }
                emit(
                    "reconciled",
                    json!({ "auction_id": U64(auction_id), "owner": owner }),
                );
                true
            }
            (1, Status::Cancelled) if held && returning => {
                if owner == me.as_str() {
                    self.auction_mut(auction_id).returning = false;
                } else {
                    let a = self.auction_mut(auction_id);
                    a.returning = false;
                    a.share_held = false;
                    self.token_auction.remove(&token_id);
                    emit(
                        "share_out",
                        json!({ "auction_id": U64(auction_id), "owner": owner }),
                    );
                }
                emit(
                    "reconciled",
                    json!({ "auction_id": U64(auction_id), "owner": owner }),
                );
                true
            }
            _ => false,
        }
    }

    #[private]
    pub fn on_returned(&mut self, auction_id: u64) -> bool {
        let ok = callback_result(64).is_some();
        let token_id = {
            let a = self.auction_mut(auction_id);
            a.returning = false;
            if ok {
                a.share_held = false;
            }
            a.token_id.clone()
        };
        if ok {
            self.token_auction.remove(&token_id);
            emit("share_returned", json!({ "auction_id": U64(auction_id) }));
            // A cancelled auction that never had a bid owes nobody anything:
            // drop its record so listing and cancelling again cannot grow
            // storage the contract pays for.
            if self.auction(auction_id).bids.is_empty() {
                self.auctions.remove(&auction_id);
            }
        } else {
            emit(
                "share_return_failed",
                json!({ "auction_id": U64(auction_id) }),
            );
        }
        ok
    }
}

impl AuctionContract {
    /// Cancels the auction and refunds every deposit when the share is
    /// already out of this contract's hands (no return is started).
    pub(crate) fn cancel_share_out(&mut self, id: u64, reason: &str) {
        self.credit_all_deposits(id);
        let token_id = {
            let a = self.auction_mut(id);
            a.status = Status::Cancelled;
            a.cancel_reason = Some(reason.to_string());
            a.share_held = false;
            a.token_id.clone()
        };
        self.token_auction.remove(&token_id);
        self.finish_record(id);
        emit(
            "auction_cancelled",
            json!({ "auction_id": U64(id), "reason": reason }),
        );
    }

    pub(crate) fn start_return(&mut self, id: u64) -> Option<Promise> {
        let (seller, token_id) = {
            let a = self.auction_mut(id);
            if !a.share_held || a.returning {
                return None;
            }
            a.returning = true;
            a.returning_since = U64(env::block_timestamp());
            (a.seller.clone(), a.token_id.clone())
        };
        Some(
            Promise::new(self.registry.clone())
                .function_call(
                    "nft_transfer".to_string(),
                    json_bytes(json!({
                        "receiver_id": seller,
                        "token_id": token_id,
                        "memo": format!("auction {id} returned"),
                    })),
                    NearToken::from_yoctonear(1),
                    GAS_NFT_TRANSFER,
                )
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_RETURNED_CB)
                        .on_returned(id),
                ),
        )
    }
}
