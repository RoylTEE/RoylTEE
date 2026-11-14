//! Watch records for payment forwards, and quarantine of a forward whose
//! callback never ran (THREAT_MODEL T19b).
//!
//! The ledger only counts forwards in flight (`inbound_pending`). It has no
//! field for when a forward started or how much is still out. Adding one
//! would change the stored `Ledger`, and the deployed registry has no
//! migrate method. So the watch lives in its own storage entry per share,
//! under the key `fwd:<token_id>`. No collection, no field of `Registry` and
//! no stored type changes. Old state has no such entry, and a missing entry
//! simply means "no recorded forward".
//!
//! Design rule (external audits A-01 to A-06): a vault balance can never show
//! where funds came from, and no block count shows that the original token
//! transfer cannot still land. So nothing here credits or resends an
//! amount because of a balance or a timeout. A stale forward is only
//! quarantined. It is settled by the real late `on_forward` callback or by
//! the registrar with the public receipt outcome (`resolve_written_off`).

use crate::*;

const WATCH_PREFIX: &[u8] = b"fwd:";

/// What the registry still waits for on one share's forwards, and what a
/// release quarantined. The format may change until the first entry exists on
/// chain: the registry state does not contain it.
#[near(serializers = [borsh])]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ForwardWatch {
    /// Start block of the oldest forward of the current busy period (set when
    /// the count was zero). Later forwards do not move it, so a steady drip
    /// of payments cannot keep a stuck forward from going stale.
    pub since: u64,
    /// Start block of the newest unsettled forward.
    pub latest: u64,
    /// Sum of the unsettled forwards that were started with a watch.
    pub amount: u128,
    /// Number of those forwards.
    pub count: u32,
    /// Forwards that started at or before this block were quarantined by
    /// `release_stale_forward`. Their callbacks are "late" (see below).
    pub released_through: u64,
    /// Amount of quarantined forwards whose outcome is not known. Still in
    /// `forwards_in_flight_total`, in no other bucket: not credited, not
    /// undelivered, never resent. `sync` is refused while it is above zero.
    pub written_off: u128,
}

fn watch_key(token_id: &TokenId) -> Vec<u8> {
    let mut k = WATCH_PREFIX.to_vec();
    k.extend_from_slice(token_id.as_bytes());
    k
}

/// Panics on bytes that are not a watch: a damaged entry must stop the call,
/// not be read as "no forward" and overwritten.
pub(crate) fn read_watch(token_id: &TokenId) -> Option<ForwardWatch> {
    let bytes = env::storage_read(&watch_key(token_id))?;
    Some(
        near_sdk::borsh::from_slice(&bytes)
            .unwrap_or_else(|_| env::panic_str("malformed forward watch")),
    )
}

/// Quarantined amount of a share, zero if none.
pub(crate) fn written_off(token_id: &TokenId) -> u128 {
    read_watch(token_id).map_or(0, |w| w.written_off)
}

pub(crate) fn write_watch(token_id: &TokenId, w: &ForwardWatch) {
    if w.count == 0 && w.released_through == 0 && w.written_off == 0 {
        env::storage_remove(&watch_key(token_id));
        return;
    }
    let bytes = near_sdk::borsh::to_vec(w).unwrap_or_else(|_| env::panic_str("borsh"));
    env::storage_write(&watch_key(token_id), &bytes);
}

/// True if the share has recorded forwards and the oldest one of the busy
/// period is at least `blocks` old.
pub(crate) fn watch_older_than(token_id: &TokenId, blocks: u64) -> bool {
    read_watch(token_id)
        .is_some_and(|w| w.count > 0 && env::block_height() >= w.since.saturating_add(blocks))
}

/// Records a forward that is about to start and returns its start block, or
/// None if the busy period is already `STALE_FORWARD_BLOCKS` old: then no new
/// forward may start on this share (M1). One read and one write, because
/// every forward of a payment pays for them.
pub(crate) fn watch_begin(token_id: &TokenId, amount: u128) -> Option<u64> {
    let now = env::block_height();
    let mut w = read_watch(token_id).unwrap_or_default();
    if w.count > 0 && now >= w.since.saturating_add(STALE_FORWARD_BLOCKS) {
        return None;
    }
    if w.count == 0 {
        w.since = now;
    }
    w.latest = now;
    w.amount += amount;
    w.count += 1;
    write_watch(token_id, &w);
    Some(now)
}

/// What `on_forward` learns from the watch.
pub(crate) enum Settle {
    /// Settle as usual.
    Normal,
    /// The forward was quarantined by `release_stale_forward`: `settle_late_forward` settles it.
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

impl Registry {
    /// A forward whose start is covered by `released_through` calls back
    /// after `release_stale_forward` quarantined it. This is the real
    /// terminal result of the transfer, so it settles the quarantine:
    ///
    /// - `ok`: the transfer reached the vault. The matched amount is credited
    ///   to the share (`known_balance`, `total_routed_in`) and leaves
    ///   `forwards_in_flight_total`.
    /// - not `ok`: the tokens never left the registry. The matched amount
    ///   leaves `forwards_in_flight_total` and goes to `undelivered`, where
    ///   `retry_forward` can send it again, safely, because the first
    ///   transfer is now known to have failed.
    ///
    /// Only the part still quarantined is matched, so a callback that comes
    /// after `resolve_written_off` is not counted a second time.
    pub(crate) fn settle_late_forward(&mut self, token_id: &TokenId, amount: u128, ok: bool) {
        let mut w = read_watch(token_id).unwrap_or_default();
        let matched = amount.min(w.written_off);
        w.written_off -= matched;
        write_watch(token_id, &w);
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(matched);
        if !ok {
            self.undelivered_total += matched;
        }
        if let Some(l) = self.ledgers.get_mut(token_id) {
            if ok {
                l.known_balance += matched;
                l.total_routed_in += matched;
            } else {
                l.undelivered += matched;
            }
            l.epoch += 1;
        }
        emit(
            "anomaly",
            json!({ "token_id": token_id, "kind": "late_forward_callback", "ok": ok, "amount": U128(amount), "matched": U128(matched), "detail": "settles a quarantined forward" }),
        );
    }
}

#[near]
impl Registry {
    /// What this share is still waiting for from payment forwards, and what
    /// a release quarantined. None if there is no entry.
    pub fn get_forward_watch(&self, token_id: TokenId) -> Option<ForwardWatchView> {
        read_watch(&token_id).map(|w| ForwardWatchView {
            since_block: U64(w.since),
            latest_block: U64(w.latest),
            amount: U128(w.amount),
            count: w.count,
            released_through: U64(w.released_through),
            written_off: U128(w.written_off),
        })
    }

    /// Quarantines a payment forward whose callback never ran (T19b).
    /// Anyone may call it once the oldest recorded forward of the busy period
    /// is `STALE_FORWARD_BLOCKS` (about a day) old. It reads no balance and
    /// credits and resends nothing: the amount moves into the share's
    /// `written_off` bucket (still counted in `forwards_in_flight_total`),
    /// `inbound_pending` goes to zero so new payments to the share flow again,
    /// and the share's `sync` stays refused until the quarantine is settled
    /// by the real late callback or `resolve_written_off`.
    ///
    /// Refused unless every pending forward of the share has a record
    /// (`inbound_pending` equals the recorded count): a forward that began
    /// before the watch existed cannot be told apart, and legacy recovery is
    /// not supported (deploy only when no forward is pending).
    pub fn release_stale_forward(&mut self, token_id: TokenId) {
        let share = self.share(&token_id);
        require!(
            matches!(share.status, ShareStatus::Active | ShareStatus::Void),
            "share has no vault"
        );
        let w = read_watch(&token_id).unwrap_or_default();
        require!(w.count > 0, "no recorded forward");
        require!(
            env::block_height() >= w.since.saturating_add(STALE_FORWARD_BLOCKS),
            "forward is not stale yet"
        );
        let l = self.ledger_mut(&token_id);
        require!(
            l.inbound_pending == w.count,
            "a pending forward has no record: not supported"
        );
        l.inbound_pending = 0;
        l.epoch += 1;
        write_watch(
            &token_id,
            &ForwardWatch {
                since: 0,
                latest: w.latest,
                amount: 0,
                count: 0,
                released_through: w.released_through.max(w.latest),
                written_off: w.written_off + w.amount,
            },
        );
        emit(
            "forward_released_stale",
            json!({ "token_id": token_id, "forwards": w.count, "quarantined": U128(w.amount) }),
        );
        emit(
            "anomaly",
            json!({ "token_id": token_id, "kind": "forward_released_stale", "detail": "forward callback never ran; amount quarantined until its callback or a registrar decision" }),
        );
    }

    /// Registrar only. Settles the amount a release quarantined when its
    /// callback never arrives. `delivered` is whether the token transfer
    /// reached the vault, read from the public receipt outcome off chain.
    /// `true`: the amount is credited to the share and leaves
    /// `forwards_in_flight_total`; refused while the share is in a dispute or
    /// waits for approval, like `sync`. `false`: the registry still holds it,
    /// so it moves to `undelivered` for `retry_forward`. The `doc_hash` ties
    /// the decision to a document. The whole quarantine of the share is
    /// settled at once. A wrong answer is a registrar error, see
    /// THREAT_MODEL T12 and T19b.
    #[payable]
    pub fn resolve_written_off(&mut self, token_id: TokenId, delivered: bool, doc_hash: Hash32) {
        near_sdk::assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let mut w = read_watch(&token_id).unwrap_or_default();
        require!(w.written_off > 0, "nothing written off");
        if delivered {
            require!(
                self.share(&token_id).dispute.is_none(),
                "share is in dispute: resolve after it ends"
            );
            require!(
                self.is_approved(&token_id),
                "share is not approved by the work registrant"
            );
        }
        let amount = w.written_off;
        w.written_off = 0;
        write_watch(&token_id, &w);
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(amount);
        if !delivered {
            self.undelivered_total += amount;
        }
        let l = self.ledger_mut(&token_id);
        l.epoch += 1;
        if delivered {
            l.known_balance += amount;
            l.total_routed_in += amount;
        } else {
            l.undelivered += amount;
        }
        emit(
            "written_off_resolved",
            json!({ "token_id": token_id, "delivered": delivered, "amount": U128(amount), "doc_hash": doc_hash }),
        );
    }
}
