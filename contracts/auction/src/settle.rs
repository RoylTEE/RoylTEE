//! Publishing the TEE key, settling through OutLayer, delivery, cancelling.

use crate::*;

#[near]
impl AuctionContract {
    /// Asks OutLayer to run the module in `pubkey` mode and stores the public
    /// key it prints. Anyone can call it once; it cannot be changed after.
    /// Attach NEAR for OutLayer (0.02 is plenty; OutLayer refunds the rest
    /// to the caller).
    #[payable]
    pub fn publish_key(&mut self) -> Promise {
        require!(self.tee_pubkey.is_none(), "the key is already published");
        require!(
            env::attached_deposit() >= NearToken::from_millinear(5),
            "attach at least 0.005 NEAR for OutLayer"
        );
        let now = env::block_timestamp();
        require!(
            now >= self.key_requested_at + 60 * NS_PER_SEC,
            "a key request was made in the last minute"
        );
        self.key_requested_at = now;
        // The module refuses any request whose `contract` is not the account
        // that called OutLayer, so the request names this contract.
        let input = json!({ "mode": "pubkey", "contract": env::current_account_id() }).to_string();
        self.request_execution(
            &input,
            env::attached_deposit(),
            &env::predecessor_account_id(),
        )
        .then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_KEY_CB)
                .on_key(),
        )
    }

    #[private]
    pub fn on_key(&mut self) -> bool {
        if self.tee_pubkey.is_some() {
            return true;
        }
        let key = callback_result(MAX_RETURN)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| logic::unwrap_answer(v).ok())
            .and_then(|v| {
                (v.get("ok").and_then(Value::as_bool) == Some(true)
                    && v.get("v").and_then(Value::as_u64) == Some(1))
                .then(|| v.get("pubkey").and_then(Value::as_str).map(str::to_owned))
                .flatten()
            })
            .filter(|k| logic::is_pubkey_hex(k));
        match key {
            Some(k) => {
                emit("tee_key_published", json!({ "pubkey": k }));
                self.tee_pubkey = Some(k);
                true
            }
            None => {
                emit("tee_key_failed", json!({}));
                false
            }
        }
    }

    /// Anyone, after `ends_at`. Attach at least 0.01 NEAR and 230 Tgas.
    /// OutLayer refunds what it does not use to the caller.
    #[payable]
    pub fn settle(&mut self, auction_id: U64) -> Option<Promise> {
        let id = auction_id.0;
        require!(
            env::prepaid_gas() >= GAS_SETTLE_MIN,
            "attach at least 230 Tgas to settle"
        );
        let caller = env::predecessor_account_id();
        let deposit = env::attached_deposit();
        let (status, ends_at, n_bids) = {
            let a = self.auction(id);
            (a.status, a.ends_at.0, a.bids.len())
        };
        require!(status == Status::Open, "auction is not open for settling");
        require!(env::block_timestamp() >= ends_at, "bidding is still open");
        if n_bids == 0 {
            // Nothing to decide and nothing to pay OutLayer for.
            if !deposit.is_zero() {
                let _ = Promise::new(caller).transfer(deposit);
            }
            return self.cancel_now(id, "no bids");
        }
        require!(
            deposit >= MIN_SETTLE_DEPOSIT,
            "attach at least 0.01 NEAR for OutLayer"
        );
        let input = {
            let a = self.auction(id);
            json!({
                "mode": "settle",
                "auction_id": id.to_string(),
                "contract": env::current_account_id(),
                "reserve": a.reserve,
                "max_deposit": a.max_deposit,
                "bids": a.bids.iter().map(|b| json!({"bidder": b.bidder, "ct": b.ciphertext})).collect::<Vec<_>>(),
            })
            .to_string()
        };
        let attempt = {
            let a = self.auction_mut(id);
            a.status = Status::Settling;
            a.attempt += 1;
            a.settling_since = U64(env::block_timestamp());
            a.attempt
        };
        emit(
            "settle_requested",
            json!({ "auction_id": auction_id, "attempt": attempt, "caller": caller }),
        );
        Some(
            self.request_execution(&input, deposit, &caller).then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_SETTLED_CB)
                    .with_unused_gas_weight(0)
                    .on_settled(id, attempt),
            ),
        )
    }

    /// OutLayer answered (or failed). Validates the answer, then starts the
    /// delivery of the share. A bad answer reopens settling; it never moves
    /// money.
    #[private]
    pub fn on_settled(&mut self, auction_id: u64, attempt: u32) -> bool {
        let live = self
            .auctions
            .get(&auction_id)
            .is_some_and(|a| a.status == Status::Settling && a.attempt == attempt);
        if !live {
            return false;
        }
        let expected = logic::bids_hash(&auction_id.to_string(), &self.auction(auction_id).bids);
        let parsed = match callback_result(MAX_RETURN) {
            Some(raw) => logic::parse_settle_output(&raw, &auction_id.to_string(), &expected),
            None => Err("OutLayer call failed or timed out".to_string()),
        };
        let outcome = match parsed.and_then(|o| self.check_outcome(auction_id, o)) {
            Ok(o) => o,
            Err(why) => {
                let a = self.auction_mut(auction_id);
                a.status = Status::Open;
                a.failures += 1;
                a.last_error = Some(why.clone());
                emit(
                    "settle_failed",
                    json!({ "auction_id": U64(auction_id), "attempt": attempt, "reason": why }),
                );
                return false;
            }
        };
        match outcome {
            logic::Outcome::NoSale => {
                let _ = self.cancel_now(auction_id, "no valid bid at or above the reserve");
                false
            }
            logic::Outcome::Sale(winner, price) => {
                let winner: AccountId = winner
                    .parse()
                    .unwrap_or_else(|_| env::panic_str("winner checked above"));
                let token_id = {
                    let a = self.auction_mut(auction_id);
                    a.status = Status::Delivering;
                    a.delivering_since = U64(env::block_timestamp());
                    a.winner = Some(winner.clone());
                    a.price = Some(U128(price));
                    a.token_id.clone()
                };
                emit(
                    "winner_chosen",
                    json!({ "auction_id": U64(auction_id), "winner": winner, "price": U128(price) }),
                );
                let _ = Promise::new(self.registry.clone())
                    .function_call(
                        "nft_transfer".to_string(),
                        json_bytes(json!({
                            "receiver_id": winner,
                            "token_id": token_id,
                            "memo": format!("auction {auction_id}"),
                        })),
                        NearToken::from_yoctonear(1),
                        GAS_NFT_TRANSFER,
                    )
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_DELIVERED_CB)
                            .with_unused_gas_weight(0)
                            .on_delivered(auction_id),
                    );
                true
            }
        }
    }

    /// The share transfer to the winner finished. Success: pay out the sale.
    /// Failure (winner not an active member any more, dispute freeze, ...):
    /// cancel, refund every deposit and send the share back.
    #[private]
    pub fn on_delivered(&mut self, auction_id: u64) -> bool {
        let live = self.auctions.get(&auction_id).is_some_and(|a| {
            a.status == Status::Delivering && a.winner.is_some() && a.price.is_some()
        });
        if !live {
            return false;
        }
        let delivered = callback_result(64).is_some();
        if !delivered {
            let _ = self.cancel_now(auction_id, "the share could not be delivered to the winner");
            return false;
        }
        self.finish_sale(auction_id);
        true
    }

    /// Cancels an auction that cannot finish. Allowed for the seller while
    /// there are no bids; for anyone 6 hours after `ends_at` once at least one
    /// settle run has failed (so a third party cannot void a sale that nobody
    /// tried to settle); for anyone 7 days after `ends_at` whatever happened;
    /// and for anyone 1 hour after a settle request that never answered.
    /// Credits every deposit back and returns the share.
    pub fn cancel(&mut self, auction_id: U64) -> Option<Promise> {
        let id = auction_id.0;
        let now = env::block_timestamp();
        let caller = env::predecessor_account_id();
        let a = self.auction(id);
        let allowed = match a.status {
            Status::Open => {
                (caller == a.seller && a.bids.is_empty())
                    || (a.failures >= 1 && now > a.ends_at.0 + SETTLE_GRACE_NS)
                    || now > a.ends_at.0 + SETTLE_HARD_GRACE_NS
            }
            Status::Settling => now > a.settling_since.0 + SETTLING_STALE_NS,
            _ => false,
        };
        require!(allowed, "this auction cannot be cancelled now");
        self.cancel_now(id, "cancelled")
    }
}

impl AuctionContract {
    /// Pays out a delivered sale: the seller gets the price, the winner the
    /// change, every loser the whole deposit. The caller has checked that the
    /// auction is Delivering with a winner and a price.
    pub(crate) fn finish_sale(&mut self, auction_id: u64) {
        let (winner, price, max, seller, bidders) = {
            let a = self.auction(auction_id);
            (
                a.winner
                    .clone()
                    .unwrap_or_else(|| env::panic_str("no winner")),
                a.price.unwrap_or_else(|| env::panic_str("no price")),
                a.max_deposit.0,
                a.seller.clone(),
                a.bids.iter().map(|b| b.bidder.clone()).collect::<Vec<_>>(),
            )
        };
        for b in &bidders {
            if b == &winner {
                self.credit(b, max - price.0);
            } else {
                self.credit(b, max);
            }
        }
        self.credit(&seller, price.0);
        self.escrow_total -= max * bidders.len() as u128;
        let token_id = {
            let a = self.auction_mut(auction_id);
            a.status = Status::Sold;
            a.share_held = false;
            a.token_id.clone()
        };
        self.token_auction.remove(&token_id);
        emit(
            "auction_sold",
            json!({ "auction_id": U64(auction_id), "winner": winner, "price": price, "seller": seller }),
        );
    }

    /// Checks the TEE answer against what the contract knows. Output the
    /// contract can bound (not verify): winner is a bidder, price is between
    /// the reserve and `max_deposit`.
    fn check_outcome(&self, id: u64, o: logic::Outcome) -> Result<logic::Outcome, String> {
        if let logic::Outcome::Sale(w, p) = &o {
            let a = self.auction(id);
            if !a.bids.iter().any(|b| b.bidder.as_str() == w) {
                return Err("winner is not a bidder".into());
            }
            if *p < a.reserve.0 || *p > a.max_deposit.0 {
                return Err("price is outside the reserve and max_deposit range".into());
            }
        }
        Ok(o)
    }

    pub(crate) fn request_execution(
        &self,
        input: &str,
        deposit: NearToken,
        payer: &AccountId,
    ) -> Promise {
        let args = json!({
            "source": { "WasmUrl": {
                "url": self.wasm_url,
                "hash": self.wasm_hash,
                "build_target": "wasm32-wasip1",
            }},
            "resource_limits": self.limits,
            "input_data": input,
            "secrets_ref": {
                "profile": self.secrets_profile,
                "account_id": self.secrets_account,
            },
            "response_format": "Json",
            "payer_account_id": payer,
        });
        Promise::new(self.outlayer.clone()).function_call_weight(
            "request_execution".to_string(),
            json_bytes(args),
            deposit,
            GAS_REQUEST,
            near_sdk::GasWeight(1),
        )
    }
}
