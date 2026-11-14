//! Watch records for payment forwards, and recovery of a forward whose
//! callback never ran (audit F1 follow-up, THREAT_MODEL T19b).
//!
//! The ledger only counts forwards in flight (`inbound_pending`). It has no
//! field for when a forward started or how much is still out. Adding one
//! would change the stored `Ledger`, and the deployed registry has no
//! migrate method. So the watch lives in its own storage entry per share,
//! under the key `fwd:<token_id>`. No collection, no field of `Registry` and
//! no stored type changes. Old state has no such entry, and a missing entry
//! simply means "no recorded forward".

use crate::*;

const WATCH_PREFIX: &[u8] = b"fwd:";

/// What the registry still waits for on one share's forwards.
#[near(serializers = [borsh])]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ForwardWatch {
    /// Block height at which the newest unsettled forward started.
    pub since: u64,
    /// Sum of the unsettled forwards that were started with a watch.
    pub amount: u128,
    /// Number of those forwards.
    pub count: u32,
    /// Forwards that started at or before this block were written off by
    /// `release_stale_forward`. A callback of such a forward changes nothing.
    pub released_through: u64,
}

fn watch_key(token_id: &TokenId) -> Vec<u8> {
    let mut k = WATCH_PREFIX.to_vec();
    k.extend_from_slice(token_id.as_bytes());
    k
}

pub(crate) fn read_watch(token_id: &TokenId) -> Option<ForwardWatch> {
    let bytes = env::storage_read(&watch_key(token_id))?;
    near_sdk::borsh::from_slice(&bytes).ok()
}

fn write_watch(token_id: &TokenId, w: &ForwardWatch) {
    if w.count == 0 && w.released_through == 0 {
        env::storage_remove(&watch_key(token_id));
        return;
    }
    let bytes = near_sdk::borsh::to_vec(w).unwrap_or_else(|_| env::panic_str("borsh"));
    env::storage_write(&watch_key(token_id), &bytes);
}

/// Records a forward that is about to start. Returns its start block.
pub(crate) fn watch_start(token_id: &TokenId, amount: u128) -> u64 {
    let now = env::block_height();
    let mut w = read_watch(token_id).unwrap_or_default();
    w.since = now;
    w.amount += amount;
    w.count += 1;
    write_watch(token_id, &w);
    now
}

/// What `on_forward` learns from the watch.
pub(crate) enum Settle {
    /// Settle as usual.
    Normal,
    /// The forward was written off by `release_stale_forward`: change nothing.
    Late,
}

/// Called from `on_forward`. `since` is the start block passed to the
/// callback; callbacks of forwards that began before the upgrade carry none.
pub(crate) fn watch_settle(token_id: &TokenId, amount: u128, since: Option<u64>) -> Settle {
    let Some(since) = since else {
        return Settle::Normal;
    };
    let mut w = read_watch(token_id).unwrap_or_default();
    if w.released_through > 0 && since <= w.released_through {
        return Settle::Late;
    }
    w.amount = w.amount.saturating_sub(amount);
    w.count = w.count.saturating_sub(1);
    if w.count == 0 {
        w.amount = 0;
    }
    write_watch(token_id, &w);
    Settle::Normal
}

#[near]
impl Registry {
    /// What this share is still waiting for from payment forwards: the
    /// newest start block, the amount and the number of forwards. None if
    /// there is no recorded forward.
    pub fn get_forward_watch(&self, token_id: TokenId) -> Option<ForwardWatchView> {
        read_watch(&token_id).map(|w| ForwardWatchView {
            since_block: U64(w.since),
            amount: U128(w.amount),
            count: w.count,
            released_through: U64(w.released_through),
        })
    }

    /// Recovers a share whose payment forward never settled (T19b). Anyone
    /// may call it once the newest recorded forward is `STALE_LOCK_BLOCKS`
    /// old. Without a callback the registry does not know whether the token
    /// transfer reached the vault, so it reads the vault's real balance and
    /// compares it with the ledger: what arrived (at most the recorded
    /// amount) is credited, the rest goes to `undelivered`, where
    /// `retry_forward` sends it again. Nothing is credited that the vault
    /// does not hold. A callback that shows up later changes nothing.
    pub fn release_stale_forward(&mut self, token_id: TokenId) -> Promise {
        require!(
            env::prepaid_gas().as_gas()
                >= GAS_FT_BALANCE.as_gas() + GAS_RELEASE_FORWARD_CB.as_gas() + 5_000_000_000_000,
            "not enough gas attached"
        );
        let share = self.share(&token_id);
        require!(
            matches!(share.status, ShareStatus::Active | ShareStatus::Void),
            "share has no vault"
        );
        let vault = share.vault.clone();
        let w = read_watch(&token_id).unwrap_or_default();
        require!(w.count > 0, "no recorded forward");
        require!(
            env::block_height() >= w.since.saturating_add(STALE_LOCK_BLOCKS),
            "forward is not stale yet"
        );
        let usdc = self.usdc.clone();
        let l = self.ledger_mut(&token_id);
        require!(l.inbound_pending > 0, "no forward in flight");
        require!(
            l.payout_in_flight.is_none() && l.transfer_pending.is_none(),
            "release the stale lock first"
        );
        let epoch = l.epoch;
        Promise::new(usdc)
            .function_call(
                "ft_balance_of".to_string(),
                json_bytes(json!({ "account_id": vault })),
                NearToken::from_yoctonear(0),
                GAS_FT_BALANCE,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RELEASE_FORWARD_CB)
                    .on_release_forward(token_id, U64(epoch), U64(w.since)),
            )
    }

    /// Settles `release_stale_forward`. Discards the reading if anything
    /// changed since the read began. Never panics. Returns true if the
    /// forward was written off.
    #[private]
    pub fn on_release_forward(&mut self, token_id: TokenId, epoch: U64, since: U64) -> bool {
        let observed = callback_result(64).and_then(|v| serde_json::from_slice::<U128>(&v).ok());
        let discard = |reason: &str| {
            emit(
                "sync_discarded",
                json!({ "token_id": token_id, "reason": reason }),
            );
            false
        };
        let Some(observed) = observed else {
            return discard("read failed");
        };
        let w = read_watch(&token_id).unwrap_or_default();
        let Some(l) = self.ledgers.get_mut(&token_id) else {
            return discard("unknown share");
        };
        if l.epoch != epoch.0
            || w.count == 0
            || w.since != since.0
            || l.payout_in_flight.is_some()
            || l.transfer_pending.is_some()
            || l.inbound_pending == 0
        {
            return discard("ledger changed during the read");
        }
        // The vault holds `known_balance` plus whatever arrived since. Count
        // as delivered at most the recorded amount and at most what the vault
        // really holds above the ledger.
        let arrived = observed.0.saturating_sub(l.known_balance);
        let delivered = w.amount.min(arrived);
        let failed = w.amount - delivered;
        l.known_balance += delivered;
        l.total_routed_in += delivered;
        l.undelivered += failed;
        l.inbound_pending = l.inbound_pending.saturating_sub(w.count);
        l.epoch += 1;
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(w.amount);
        self.undelivered_total += failed;
        write_watch(
            &token_id,
            &ForwardWatch {
                since: w.since,
                amount: 0,
                count: 0,
                released_through: w.since,
            },
        );
        emit(
            "forward_released_stale",
            json!({
                "token_id": token_id,
                "forwards": w.count,
                "amount": U128(w.amount),
                "delivered": U128(delivered),
                "undelivered": U128(failed),
            }),
        );
        emit(
            "anomaly",
            json!({ "token_id": token_id, "kind": "forward_released_stale", "detail": "forward callback never ran; ledger set from the vault balance" }),
        );
        true
    }
}
