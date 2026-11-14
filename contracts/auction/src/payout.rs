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
        // Each pending record is a little larger than the credit it replaces
        // (about 16 bytes). Keep the floor free so that never fails a callback;
        // if it is short the whole call fails and nothing is debited.
        require!(
            self.free_balance() >= MIN_FREE_BALANCE,
            "the auction contract is low on NEAR; top it up before withdrawing"
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

    /// Closes the pending record and, if the token transfer failed, puts the
    /// credit back. Idempotent: a callback whose pending id is gone (already
    /// resolved) changes no money, and the amounts come from the record, not
    /// from the arguments. If this callback is lost the record stays, so the
    /// claim is never erased; it is not refunded automatically, because a
    /// delayed transfer could still land (double payment).
    #[private]
    pub fn on_withdrawn(&mut self, id: u64, account: AccountId, amount: U128) -> bool {
        let _ = (account, amount);
        let Some(rec) = self.pending.remove(&id) else {
            return false;
        };
        self.pending_total = self.pending_total.saturating_sub(rec.amount.0);
        if callback_result(64).is_some() {
            emit(
                "withdrawn",
                json!({ "account": rec.account, "amount": rec.amount }),
            );
            return true;
        }
        self.credit(&rec.account, rec.amount.0);
        emit(
            "withdraw_failed",
            json!({ "account": rec.account, "amount": rec.amount }),
        );
        false
    }

    /// Operator call (this account's own key only): closes a pending
    /// withdrawal whose callback was lost, after the transfer's final receipt
    /// outcome is known. `paid` true: the tokens arrived, the record is
    /// dropped. `paid` false: the transfer failed, the credit is put back.
    /// `evidence_hash` is the sha256 (64 hex) of the receipt evidence.
    pub fn resolve_withdrawal(&mut self, id: U64, paid: bool, evidence_hash: String) {
        self.require_operator(&evidence_hash);
        let rec = self
            .pending
            .remove(&id.0)
            .unwrap_or_else(|| env::panic_str("no such pending withdrawal"));
        self.pending_total = self.pending_total.saturating_sub(rec.amount.0);
        if !paid {
            self.credit(&rec.account, rec.amount.0);
        }
        emit(
            "withdrawal_resolved",
            json!({ "id": id, "account": rec.account, "amount": rec.amount,
                    "paid": paid, "evidence_hash": evidence_hash }),
        );
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
    /// it is no longer this contract, stops holding the share. Only the
    /// auction's own bookkeeping changes, no money moves. Anyone can call.
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
                    .on_share_checked(auction_id.0, a.return_op),
            )
    }

    #[private]
    pub fn on_share_checked(&mut self, auction_id: u64, op: u64) -> bool {
        let owner = registry_owner();
        let Some(owner) = owner else { return false };
        if owner == env::current_account_id().as_str() {
            return false;
        }
        // The same return attempt, still idle: no newer return started.
        let current = self.auctions.get(&auction_id).is_some_and(|a| {
            a.status == Status::Cancelled && a.share_held && !a.returning && a.return_op == op
        });
        if !current {
            return false;
        }
        self.share_out(auction_id, "share_out", Some(&owner));
        true
    }

    /// Looks at a delivery or a share return that has not reported back for 15
    /// minutes. Anyone can call it. It never cancels, refunds or pays because
    /// of how long something took or who holds the share now: elapsed time
    /// does not prove that an earlier transfer will never land.
    ///
    /// It never finishes, cancels, refunds or pays anything. It reads the
    /// share's owner and emits `reconcile_pending` with the observed owner as
    /// a diagnostic. A delivery is settled only by its own `on_delivered`
    /// callback or by the operator's `resolve_delivery`; a return only by its
    /// own `on_returned` callback or by `resolve_return`. The auction
    /// account's own key is kept for this on purpose; it is a stated trust
    /// (README).
    ///
    /// If the payout cannot be written, add NEAR to this account and call it
    /// again; nothing is lost in between.
    pub fn reconcile(&self, auction_id: U64) -> Promise {
        let a = self.auction(auction_id.0);
        let now = env::block_timestamp();
        let (stage, op) = match a.status {
            Status::Delivering if now >= a.delivering_since.0 + RECONCILE_AFTER_NS => {
                (0u8, a.delivery_op)
            }
            Status::Cancelled
                if a.share_held
                    && a.returning
                    && now >= a.returning_since.0 + RECONCILE_AFTER_NS =>
            {
                (1u8, a.return_op)
            }
            _ => env::panic_str("nothing to reconcile yet"),
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
                    .on_reconciled(auction_id.0, stage, op),
            )
    }

    /// Returns true when a diagnostic was emitted, false when the callback
    /// was stale or unreadable. It never changes the auction.
    #[private]
    pub fn on_reconciled(&mut self, auction_id: u64, stage: u8, op: u64) -> bool {
        let Some(owner) = registry_owner() else {
            return false;
        };
        let Some(a) = self.auctions.get(&auction_id) else {
            return false;
        };
        let current = match stage {
            0 => a.status == Status::Delivering && a.delivery_op == op,
            _ => a.status == Status::Cancelled && a.share_held && a.returning && a.return_op == op,
        };
        if !current {
            return false;
        }
        // A diagnostic only. Who holds the share now does not say what this
        // attempt's transfer did, so nothing is settled from it.
        emit(
            "reconcile_pending",
            json!({ "auction_id": U64(auction_id), "op": U64(op), "stage": stage, "owner": owner }),
        );
        true
    }

    /// Operator call (this account's own key only, `predecessor == current
    /// account`). Settles a delivery that is stuck, after the final receipt
    /// outcome of the `nft_transfer` with this `op_id` is known.
    /// `delivered` true: the transfer succeeded, pay the sale. `delivered`
    /// false: it failed, cancel, refund every deposit and send the share back.
    /// `evidence_hash` is the sha256 (64 hex) of the receipt evidence.
    pub fn resolve_delivery(
        &mut self,
        auction_id: U64,
        op_id: U64,
        delivered: bool,
        evidence_hash: String,
    ) -> Option<Promise> {
        self.require_operator(&evidence_hash);
        let ok = self.auctions.get(&auction_id.0).is_some_and(|a| {
            a.status == Status::Delivering
                && a.delivery_op == op_id.0
                && a.winner.is_some()
                && a.price.is_some()
        });
        require!(ok, "no delivery with this id is stuck");
        emit(
            "delivery_resolved",
            json!({ "auction_id": auction_id, "op": op_id, "delivered": delivered,
                    "evidence_hash": evidence_hash }),
        );
        if delivered {
            self.finish_sale(auction_id.0);
            None
        } else {
            self.cancel_now(auction_id.0, "delivery failed (operator)")
        }
    }

    /// Operator call, like `resolve_delivery`, for a share return whose
    /// callback was lost. `returned` true: the share is back with the seller,
    /// stop holding it. `returned` false: the transfer failed, allow
    /// `return_share` again.
    pub fn resolve_return(
        &mut self,
        auction_id: U64,
        op_id: U64,
        returned: bool,
        evidence_hash: String,
    ) {
        self.require_operator(&evidence_hash);
        let ok = self.auctions.get(&auction_id.0).is_some_and(|a| {
            a.status == Status::Cancelled && a.share_held && a.returning && a.return_op == op_id.0
        });
        require!(ok, "no return with this id is stuck");
        emit(
            "return_resolved",
            json!({ "auction_id": auction_id, "op": op_id, "returned": returned,
                    "evidence_hash": evidence_hash }),
        );
        if returned {
            self.share_out(auction_id.0, "share_returned", None);
        } else {
            self.auction_mut(auction_id.0).returning = false;
        }
    }

    #[private]
    pub fn on_returned(&mut self, auction_id: u64, op: u64) -> bool {
        let ok = callback_result(64).is_some();
        // A callback of an older return attempt, or one that an operator has
        // already resolved, changes nothing.
        let current = self
            .auctions
            .get(&auction_id)
            .is_some_and(|a| a.returning && a.return_op == op);
        if !current {
            return false;
        }
        if ok {
            self.share_out(auction_id, "share_returned", None);
        } else {
            self.auction_mut(auction_id).returning = false;
            emit(
                "share_return_failed",
                json!({ "auction_id": U64(auction_id) }),
            );
        }
        ok
    }
}

/// Owner of the share in the answer of the `nft_token` call, if readable.
fn registry_owner() -> Option<String> {
    callback_result(MAX_TOKEN_RETURN)
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("owner_id").and_then(Value::as_str).map(str::to_owned))
}

impl AuctionContract {
    /// Operator calls: this account's own key only, with a receipt evidence hash.
    fn require_operator(&self, evidence_hash: &str) {
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "only the auction account itself can resolve this"
        );
        require!(
            logic::is_hash64(evidence_hash),
            "evidence_hash must be 64 lowercase hex characters"
        );
    }

    pub(crate) fn start_return(&mut self, id: u64) -> Option<Promise> {
        if !self.auction(id).share_held || self.auction(id).returning {
            return None;
        }
        let op = self.new_op();
        let (seller, token_id) = {
            let a = self.auction_mut(id);
            a.returning = true;
            a.returning_since = U64(env::block_timestamp());
            a.return_op = op;
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
                        .on_returned(id, op),
                ),
        )
    }
}
