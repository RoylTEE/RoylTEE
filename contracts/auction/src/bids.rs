//! Sealed bids arrive as mock USDC `ft_transfer_call`.

use crate::*;
use near_sdk::PromiseOrValue;

#[near(serializers = [json])]
struct BidMsg {
    auction_id: String,
    ciphertext: String,
}

#[near]
impl AuctionContract {
    /// NEP-141 receiver. Returns the unused amount: 0 if the bid is taken,
    /// the whole deposit if it is refused (the token contract then refunds).
    /// Membership is read from the registry first, so the answer comes from
    /// a callback. If that callback fails, the token contract refunds all.
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        require!(
            env::predecessor_account_id() == self.usdc,
            "only mock USDC deposits are accepted"
        );
        let Ok(m) = serde_json::from_str::<BidMsg>(&msg) else {
            return PromiseOrValue::Value(amount);
        };
        let Ok(id) = m.auction_id.parse::<u64>() else {
            return PromiseOrValue::Value(amount);
        };
        if self
            .check_bid(&sender_id, amount.0, id, &m.ciphertext)
            .is_err()
        {
            return PromiseOrValue::Value(amount);
        }
        PromiseOrValue::Promise(
            Promise::new(self.registry.clone())
                .function_call(
                    "is_active_member".to_string(),
                    json_bytes(json!({ "account_id": sender_id })),
                    NearToken::from_yoctonear(0),
                    GAS_IS_MEMBER,
                )
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_BID_CB)
                        .on_bid_checked(sender_id, amount, id, m.ciphertext),
                ),
        )
    }

    /// Takes the bid if the sender is an active member and every rule still
    /// holds. Returns the unused amount.
    #[private]
    pub fn on_bid_checked(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        auction_id: u64,
        ciphertext: String,
    ) -> U128 {
        let member = callback_result(64)
            .and_then(|b| serde_json::from_slice::<bool>(&b).ok())
            .unwrap_or(false);
        if !member {
            emit(
                "bid_refused",
                json!({ "auction_id": U64(auction_id), "bidder": sender_id, "reason": "not an active member" }),
            );
            return amount;
        }
        if let Err(why) = self.check_bid(&sender_id, amount.0, auction_id, &ciphertext) {
            emit(
                "bid_refused",
                json!({ "auction_id": U64(auction_id), "bidder": sender_id, "reason": why }),
            );
            return amount;
        }
        let a = self.auction_mut(auction_id);
        a.bids.push(Bid {
            bidder: sender_id.clone(),
            ciphertext,
            at: U64(env::block_timestamp()),
        });
        let n = a.bids.len();
        self.escrow_total += amount.0;
        emit(
            "bid_placed",
            json!({ "auction_id": U64(auction_id), "bidder": sender_id, "bid_count": n }),
        );
        U128(0)
    }
}

impl AuctionContract {
    /// Every rule that does not need the registry.
    fn check_bid(
        &self,
        bidder: &AccountId,
        amount: u128,
        auction_id: u64,
        ciphertext: &str,
    ) -> Result<(), String> {
        let a = self
            .auctions
            .get(&auction_id)
            .ok_or("unknown auction".to_string())?;
        if a.status != Status::Open || env::block_timestamp() >= a.ends_at.0 {
            return Err("bidding is closed".into());
        }
        if amount != a.max_deposit.0 {
            return Err("deposit must equal max_deposit exactly".into());
        }
        if bidder == &a.seller {
            return Err("the seller cannot bid".into());
        }
        if a.bids.iter().any(|b| &b.bidder == bidder) {
            return Err("one bid per account".into());
        }
        if a.bids.len() >= MAX_BIDS {
            return Err("auction is full".into());
        }
        if ciphertext.len() > MAX_CIPHERTEXT || !logic::looks_like_base64(ciphertext) {
            return Err("ciphertext is not base64 or too long".into());
        }
        if self.free_balance() < MIN_FREE_BALANCE {
            return Err("auction contract is low on NEAR".into());
        }
        Ok(())
    }
}
