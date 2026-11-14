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
                >= (GAS_FT_TRANSFER.as_gas() + GAS_WITHDRAW_CB.as_gas()) * accounts.len() as u64
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
                        .on_withdrawn(acct.clone(), U128(amount)),
                );
        }
        require!(paid > 0, "nothing is owed to these accounts");
    }

    /// Puts the credit back if the token transfer failed. Never panics.
    #[private]
    pub fn on_withdrawn(&mut self, account: AccountId, amount: U128) -> bool {
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
    pub(crate) fn start_return(&mut self, id: u64) -> Option<Promise> {
        let (seller, token_id) = {
            let a = self.auction_mut(id);
            if !a.share_held || a.returning {
                return None;
            }
            a.returning = true;
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
