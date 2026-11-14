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
/// Raw storage key of the consumed evidence of one share: `evd:<token_id>`.
/// First byte 0x65: no clash with `Key` (0 to 13), `STATE` (0x53) or `fwd:` (0x66).
const EVIDENCE_PREFIX: &[u8] = b"evd:";

/// Most evidence hashes one share can ever consume (registrar settlements
/// of quarantined forwards and of stale payout locks together). Storage:
/// 4 bytes of length plus 32 bytes per hash, so at most 2052 bytes (about
/// 0.021 NEAR) for each share, paid by the registry, and nothing for a share
/// that never needed a registrar settlement. At the limit the registrar can
/// not settle that share again (stated in THREAT_MODEL T19b).
pub const MAX_EVIDENCE_PER_SHARE: usize = 64;

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
    /// Forwards that started at or before this block belong to a quarantine
    /// the registrar already touched (`resolve_written_off`). Their
    /// callbacks change nothing and emit an anomaly, so an old callback can
    /// never settle a later, unrelated quarantine (audit RR-1, R-M1). Never
    /// above `released_through`.
    pub resolved_through: u64,
    /// Amount of quarantined forwards whose outcome is not known. Still in
    /// `forwards_in_flight_total`, in no other bucket: not credited, not
    /// undelivered, never resent. `sync` is refused while it is above zero.
    pub written_off: u128,
    /// Settlement sequence of the quarantine. Bumped by every change to it:
    /// a release, a late callback that settles part of it, a registrar
    /// resolution. The registrar passes the value it reviewed as
    /// `expected_seq`, so a callback or another settlement that lands between
    /// its review and its transaction makes the transaction fail instead of
    /// counting the same receipt twice (audit RF-H1). An entry with a
    /// nonzero sequence is never removed, so the sequence never restarts.
    pub settle_seq: u64,
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
    if w.count == 0
        && w.released_through == 0
        && w.resolved_through == 0
        && w.written_off == 0
        && w.settle_seq == 0
    {
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
    /// The registrar already resolved the quarantine this forward belonged
    /// to: change nothing, emit an anomaly.
    Fenced,
}

/// Called from `on_forward`. `since` is the start block passed to the
/// callback; callbacks of forwards that began before the upgrade carry none.
pub(crate) fn watch_settle(token_id: &TokenId, amount: u128, since: Option<u64>) -> Settle {
    let Some(since) = since else {
        return Settle::Normal;
    };
    let mut w = read_watch(token_id).unwrap_or_default();
    if w.resolved_through > 0 && since <= w.resolved_through {
        return Settle::Fenced;
    }
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

fn evidence_key(token_id: &TokenId) -> Vec<u8> {
    let mut k = EVIDENCE_PREFIX.to_vec();
    k.extend_from_slice(token_id.as_bytes());
    k
}

fn read_evidence(token_id: &TokenId) -> Vec<[u8; 32]> {
    env::storage_read(&evidence_key(token_id)).map_or_else(Vec::new, |b| {
        near_sdk::borsh::from_slice(&b)
            .unwrap_or_else(|_| env::panic_str("malformed evidence list"))
    })
}

fn hex32(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// `doc_hash` is validated as 64 lowercase hex characters first.
fn unhex32(s: &str) -> [u8; 32] {
    let v = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
    let b = s.as_bytes();
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = v(b[2 * i]) * 16 + v(b[2 * i + 1]);
    }
    out
}

/// Records `doc_hash` as consumed evidence of this share. Panics if it was
/// used before or if the share has used up `MAX_EVIDENCE_PER_SHARE`. A
/// registrar settlement cites one receipt, so the same receipt can never
/// settle twice (audit RF-H1).
pub(crate) fn consume_evidence(token_id: &TokenId, doc_hash: &str) {
    let h = unhex32(doc_hash);
    let mut list = read_evidence(token_id);
    require!(
        !list.contains(&h),
        "this evidence was already used for this share"
    );
    require!(
        list.len() < MAX_EVIDENCE_PER_SHARE,
        "evidence list of this share is full"
    );
    list.push(h);
    let bytes = near_sdk::borsh::to_vec(&list).unwrap_or_else(|_| env::panic_str("borsh"));
    env::storage_write(&evidence_key(token_id), &bytes);
}

/// A late callback of a quarantine the registrar already resolved: nothing
/// changes (its amount left `forwards_in_flight_total` at the decision).
pub(crate) fn emit_fenced(token_id: &TokenId, amount: u128, ok: bool) {
    emit(
        "anomaly",
        json!({ "token_id": token_id, "kind": "late_forward_callback_fenced", "ok": ok, "amount": U128(amount), "detail": "its quarantine was already resolved by the registrar; nothing changed" }),
    );
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
        if matched > 0 {
            w.settle_seq += 1;
        }
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
            resolved_through: U64(w.resolved_through),
            written_off: U128(w.written_off),
            settle_seq: U64(w.settle_seq),
        })
    }

    /// Evidence hashes this share has consumed (registrar settlements). A
    /// hash in the list is refused for this share.
    pub fn get_used_evidence(&self, token_id: TokenId) -> Vec<Hash32> {
        read_evidence(&token_id).iter().map(hex32).collect()
    }

    /// Deploy barrier (audit RR-4, R-L3). The redeploy runs it in the same
    /// transaction as the code deploy (`with-init-call assert_quiescent`), so
    /// a busy registry rolls the whole deploy back. Panics unless the whole
    /// registry is quiet: `forwards_in_flight_total` is 0 and every share
    /// has `inbound_pending` 0, no payout lock, no transfer lock and no
    /// forward watch with anything in flight or quarantined. A watch entry
    /// in a damaged or older format panics too (`read_watch`), so two watch
    /// formats can never meet. Changes nothing. It iterates over ALL tokens,
    /// so its gas grows with the token count (about 0.21 Tgas per
    /// token measured, THREAT_MODEL T19b): fine for the 13 tokens on
    /// testnet at 100 Tgas, and a registry with several hundred tokens
    /// would need more gas than one call can carry.
    #[private]
    pub fn assert_quiescent(&self) {
        require!(
            self.forwards_in_flight_total == 0,
            "not quiet: forwards in flight"
        );
        for token_id in self.shares.keys() {
            if let Some(l) = self.ledgers.get(token_id) {
                require!(l.inbound_pending == 0, "not quiet: pending forward");
                require!(l.payout_in_flight.is_none(), "not quiet: payout lock");
                require!(l.transfer_pending.is_none(), "not quiet: transfer lock");
            }
            if let Some(w) = read_watch(token_id) {
                require!(
                    w.count == 0 && w.amount == 0 && w.written_off == 0,
                    "not quiet: forward watch"
                );
            }
        }
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
                resolved_through: w.resolved_through,
                written_off: w.written_off + w.amount,
                settle_seq: w.settle_seq + 1,
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

    /// Registrar only. Settles amounts a release quarantined when their
    /// callbacks never arrive, from the final receipt outcome of the original
    /// transfers read off chain (a missing or pending receipt is not
    /// evidence; THREAT_MODEL T19b has the procedure). `delivered_amount` is
    /// what reached the vault: it is credited to the share and leaves
    /// `forwards_in_flight_total`; refused above zero for a share in a
    /// dispute or waiting for approval, like `sync`. `failed_amount` is what
    /// failed: the registry still holds it, so it leaves
    /// `forwards_in_flight_total` and moves to `undelivered` for
    /// `retry_forward`. The two must sum to more than zero and at most the
    /// quarantined amount; any remainder stays quarantined for another call.
    /// Whatever the amounts, the callbacks of everything quarantined so far
    /// are fenced (`resolved_through`): a late callback changes nothing, so
    /// it can never settle a later quarantine (audit RR-1). The `doc_hash`
    /// is a reference to the evidence (the receipt id). A wrong answer is a
    /// registrar error, see THREAT_MODEL T12 and T19b.
    #[payable]
    pub fn resolve_written_off(
        &mut self,
        token_id: TokenId,
        delivered_amount: U128,
        failed_amount: U128,
        expected_seq: U64,
        doc_hash: Hash32,
    ) {
        near_sdk::assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let (delivered, failed) = (delivered_amount.0, failed_amount.0);
        let mut w = read_watch(&token_id).unwrap_or_default();
        require!(w.written_off > 0, "nothing written off");
        require!(
            w.settle_seq == expected_seq.0,
            "the quarantine changed since it was reviewed: read get_forward_watch again"
        );
        let total = delivered
            .checked_add(failed)
            .unwrap_or_else(|| env::panic_str("amounts overflow"));
        require!(total > 0, "nothing to resolve");
        require!(total <= w.written_off, "more than is written off");
        if delivered > 0 {
            require!(
                self.share(&token_id).dispute.is_none(),
                "share is in dispute: resolve after it ends"
            );
            require!(
                self.is_approved(&token_id),
                "share is not approved by the work registrant"
            );
        }
        consume_evidence(&token_id, &doc_hash);
        w.written_off -= total;
        w.resolved_through = w.released_through;
        w.settle_seq += 1;
        write_watch(&token_id, &w);
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(total);
        self.undelivered_total += failed;
        let l = self.ledger_mut(&token_id);
        l.epoch += 1;
        l.known_balance += delivered;
        l.total_routed_in += delivered;
        l.undelivered += failed;
        emit(
            "written_off_resolved",
            json!({ "token_id": token_id, "delivered_amount": delivered_amount, "failed_amount": failed_amount, "remaining": U128(w.written_off), "settle_seq": U64(w.settle_seq), "doc_hash": doc_hash }),
        );
    }
}
