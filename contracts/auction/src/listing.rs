//! Listing: the registry's `nft_transfer_call` lands here.

use crate::*;

#[near(serializers = [json])]
struct ListMsg {
    reserve: U128,
    max_deposit: U128,
    ends_at: U64,
}

#[near]
impl AuctionContract {
    /// NEP-171 receiver. Only the registry can list. Returns `false` to keep
    /// the share (the registry's `nft_resolve_transfer` then leaves it here)
    /// and `true` to refuse the listing (the registry hands it back). A panic
    /// also hands it back, so nothing here can strand a share.
    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: String,
        msg: String,
    ) -> bool {
        require!(
            env::predecessor_account_id() == self.registry,
            "only the registry can list a share"
        );
        let _ = sender_id;
        match self.try_list(previous_owner_id, token_id, &msg) {
            Ok(_) => false,
            Err(why) => {
                emit("listing_refused", json!({ "reason": why }));
                true
            }
        }
    }
}

impl AuctionContract {
    fn try_list(&mut self, seller: AccountId, token_id: String, msg: &str) -> Result<u64, String> {
        let m: ListMsg =
            serde_json::from_str(msg).map_err(|_| "msg is not a listing".to_string())?;
        if self.tee_pubkey.is_none() {
            return Err("the TEE public key is not published yet".into());
        }
        if m.reserve.0 == 0 || m.reserve.0 > m.max_deposit.0 {
            return Err("reserve must be above 0 and at most max_deposit".into());
        }
        let now = env::block_timestamp();
        if m.ends_at.0 < now + MIN_DURATION_NS || m.ends_at.0 > now + MAX_DURATION_NS {
            return Err("ends_at must be 1 minute to 14 days from now".into());
        }
        if m.max_deposit.0 > MAX_TERM {
            return Err("max_deposit must have at most 38 digits".into());
        }
        if self.free_balance() < self.listing_floor() {
            return Err("auction contract is low on NEAR for another auction".into());
        }
        if self.token_auction.contains_key(&token_id) {
            return Err("this share already has an auction".into());
        }
        if seller == env::current_account_id() {
            return Err("seller cannot be the auction itself".into());
        }
        let id = self.next_id;
        self.next_id += 1;
        let rec = AuctionRec {
            id,
            seller: seller.clone(),
            token_id: token_id.clone(),
            reserve: m.reserve,
            max_deposit: m.max_deposit,
            created_at: U64(now),
            ends_at: m.ends_at,
            bids: Vec::new(),
            status: Status::Open,
            attempt: 0,
            failures: 0,
            settling_since: U64(0),
            delivering_since: U64(0),
            returning_since: U64(0),
            share_held: true,
            returning: false,
            winner: None,
            price: None,
            cancel_reason: None,
            last_error: None,
        };
        self.auctions.insert(id, rec);
        self.open_auctions += 1;
        self.token_auction.insert(token_id.clone(), id);
        emit(
            "auction_listed",
            json!({
                "auction_id": U64(id), "seller": seller, "token_id": token_id,
                "reserve": m.reserve, "max_deposit": m.max_deposit, "ends_at": m.ends_at,
            }),
        );
        Ok(id)
    }
}
