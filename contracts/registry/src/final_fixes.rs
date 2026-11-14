//! Final contract fixes before the registry key is deleted: quarantine of a
//! payment forward whose callback never ran (THREAT_MODEL T19b) and the
//! handling of a stale payout lock (audit F3, A-02, A-03, T19). Nothing here
//! credits or resends an amount because of a balance or a timeout. Callbacks
//! are driven by hand with mocked promise results; block heights are set
//! explicitly.

use super::review_fixes::assert_panics;
use super::*;

fn anyone() -> AccountId {
    acc("anyone.near")
}

/// Pays one share directly at `height` and does NOT settle the forward.
fn pay_open(c: &mut Registry, t: &TokenId, amount: u128, height: u64) {
    at(&usdc(), NearToken::from_yoctonear(0), 0, height);
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    let r = c.ft_on_transfer(acc("payer.near"), U128(amount), msg.to_string());
    assert!(matches!(r, PromiseOrValue::Value(U128(0))));
}

fn settle_at(c: &mut Registry, t: &TokenId, amount: u128, since: u64, good: bool) -> bool {
    callback(if good { ok(b"") } else { PromiseResult::Failed });
    c.on_forward(t.clone(), U128(amount), Some(U64(since)))
}

fn release_fwd(c: &mut Registry, t: &TokenId, height: u64) -> u64 {
    at(&anyone(), NearToken::from_yoctonear(0), 0, height);
    c.release_stale_forward(t.clone());
    c.ledgers.get(t).unwrap().epoch
}

fn acct(c: &Registry) -> (u128, u128) {
    let a = c.get_accounting();
    (a.forwards_in_flight_total.0, a.undelivered_total.0)
}

fn fresh() -> (Registry, TokenId) {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    (c, t)
}

const STALE_F: u64 = STALE_FORWARD_BLOCKS;

fn watch(c: &Registry, t: &TokenId) -> ForwardWatchView {
    c.get_forward_watch(t.clone()).unwrap()
}

fn resolve_amounts(c: &mut Registry, t: &TokenId, delivered: u128, failed: u128) {
    call(&registrar(), NearToken::from_yoctonear(1));
    c.resolve_written_off(t.clone(), U128(delivered), U128(failed), h('d'));
}

/// Settles the whole quarantine of the share in one direction.
fn resolve_wo(c: &mut Registry, t: &TokenId, delivered: bool) {
    let all = watch(c, t).written_off.0;
    if delivered {
        resolve_amounts(c, t, all, 0);
    } else {
        resolve_amounts(c, t, 0, all);
    }
}

fn resolve_payout(c: &mut Registry, t: &TokenId, paid: bool, height: u64) {
    at(&registrar(), NearToken::from_yoctonear(1), 0, height);
    c.resolve_stale_payout(t.clone(), paid, h('e'));
}

/// Share with one lost forward of `amount` started at block 500, released.
fn quarantined(amount: u128) -> (Registry, TokenId) {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, amount, 500);
    release_fwd(&mut c, &t, 500 + STALE_F);
    (c, t)
}

fn sync_with(c: &mut Registry, t: &TokenId, vault: u128) -> bool {
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = c.ledgers.get(t).unwrap().epoch;
    callback(ok(format!("\"{vault}\"").as_bytes()));
    c.on_sync(t.clone(), U64(epoch)).is_some()
}

#[test]
fn a_forward_is_watched_and_the_watch_ends_when_it_settles() {
    let (mut c, t) = fresh();
    assert!(c.get_forward_watch(t.clone()).is_none());
    pay_open(&mut c, &t, 80_000, 500);
    let w = watch(&c, &t);
    assert_eq!(
        (w.since_block.0, w.latest_block.0, w.amount.0, w.count),
        (500, 500, 80_000, 1)
    );
    assert_eq!(w.released_through.0, 0);
    // A second forward joins the busy period: `since` stays at its start.
    pay_open(&mut c, &t, 20_000, 520);
    let w = watch(&c, &t);
    assert_eq!(
        (w.since_block.0, w.latest_block.0, w.amount.0, w.count),
        (500, 520, 100_000, 2)
    );
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(!settle_at(&mut c, &t, 20_000, 520, false));
    assert!(c.get_forward_watch(t.clone()).is_none(), "entry removed");
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (80_000, 20_000, 0)
    );
    // A new busy period starts again.
    pay_open(&mut c, &t, 10_000, 9_000);
    assert_eq!(watch(&c, &t).since_block.0, 9_000);
}

#[test]
fn callbacks_from_before_the_upgrade_still_settle() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    callback(ok(b""));
    assert!(c.on_forward(t.clone(), U128(80_000), None));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.inbound_pending), (80_000, 0));
}

#[test]
fn release_needs_a_stale_recorded_forward() {
    let (mut c, t) = fresh();
    at(&anyone(), NearToken::from_yoctonear(0), 0, 500);
    assert_panics("no recorded forward", || c.release_stale_forward(t.clone()));
    pay_open(&mut c, &t, 80_000, 500);
    // The lock window is not enough: a release waits about a day.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_LOCK_BLOCKS,
    );
    assert_panics("not stale yet", || c.release_stale_forward(t.clone()));
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F - 1,
    );
    assert_panics("not stale yet", || c.release_stale_forward(t.clone()));
    at(&anyone(), NearToken::from_yoctonear(0), 0, 500 + STALE_F);
    c.release_stale_forward(t.clone());
    assert!(get_logs().join("\n").contains("forward_released_stale"));
}

/// Audit A-01: a release only quarantines. It reads no balance, credits
/// nothing and resends nothing, so a transfer that is only late cannot be
/// paid twice.
#[test]
fn release_only_quarantines_and_credits_and_resends_nothing() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    assert_eq!(acct(&c), (80_000, 0));
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("payment forward in flight", || drop(c.sync(t.clone())));
    release_fwd(&mut c, &t, 500 + STALE_F);
    let l = ledger(&c, &t);
    assert_eq!(
        (
            l.known_balance.0,
            l.total_routed_in.0,
            l.undelivered.0,
            l.inbound_pending
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(acct(&c), (80_000, 0), "still counted as in flight");
    let w = watch(&c, &t);
    assert_eq!(
        (w.count, w.amount.0, w.released_through.0, w.written_off.0),
        (0, 0, 500, 80_000)
    );
    // Nothing to resend: undelivered is empty.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 1,
    );
    assert_panics("nothing undelivered", || c.retry_forward(t.clone()));
    // sync stays refused while the quarantine is open.
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("quarantined forward is unresolved", || {
        drop(c.sync(t.clone()))
    });
    // New payments flow again.
    pay_open(&mut c, &t, 20_000, 500 + STALE_F + 2);
    assert_eq!(acct(&c), (100_000, 0));
    assert_eq!(ledger(&c, &t).inbound_pending, 1);
}

/// Audit A-01 and review H1: the transfer was only late. Its real callback
/// settles the quarantine, and the amount is never resent.
#[test]
fn late_successful_transfer_after_release_is_never_resent() {
    let (mut c, t) = quarantined(80_000);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 1,
    );
    assert_panics("nothing undelivered", || c.retry_forward(t.clone()));
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(get_logs().join("\n").contains("late_forward_callback"));
    let l = ledger(&c, &t);
    assert_eq!(
        (
            l.known_balance.0,
            l.total_routed_in.0,
            l.undelivered.0,
            l.inbound_pending
        ),
        (80_000, 80_000, 0, 0)
    );
    assert_eq!(acct(&c), (0, 0));
    assert_eq!(watch(&c, &t).written_off.0, 0);
    // A replayed callback changes nothing.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.total_routed_in.0), (80_000, 80_000));
    assert_eq!(acct(&c), (0, 0));
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 2,
    );
    assert_panics("nothing undelivered", || c.retry_forward(t.clone()));
    // sync works again.
    assert!(sync_with(&mut c, &t, 80_000));
    assert_eq!(ledger(&c, &t).claimable.0, 80_000);
}

#[test]
fn late_failed_transfer_after_release_goes_to_undelivered_once() {
    let (mut c, t) = quarantined(80_000);
    assert!(!settle_at(&mut c, &t, 80_000, 500, false));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (0, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 80_000));
    // A replay does not move it again.
    assert!(!settle_at(&mut c, &t, 80_000, 500, false));
    assert_eq!(ledger(&c, &t).undelivered.0, 80_000);
    assert_eq!(acct(&c), (0, 80_000));
    // The first transfer is known to have failed, so a resend is safe.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 1,
    );
    c.retry_forward(t.clone());
    assert_eq!(acct(&c), (80_000, 0));
}

/// Audit A-01: the reviewer's harness. The delayed transfer lands after the
/// release. Nothing is resent, so another liability is not spent.
#[test]
fn delayed_forward_retry_must_not_spend_other_liabilities() {
    let (mut c, t) = fresh();
    c.allowance_total = 100_000;
    pay_open(&mut c, &t, 80_000, 500);
    let mut registry_tokens = 180_000u128;
    let mut vault_tokens = 0u128;
    release_fwd(&mut c, &t, 500 + STALE_F);
    // The original transfer now lands and its callback runs.
    registry_tokens -= 80_000;
    vault_tokens += 80_000;
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    // A retry has nothing to resend.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 1,
    );
    assert_panics("nothing undelivered", || c.retry_forward(t.clone()));
    assert_eq!(vault_tokens, ledger(&c, &t).known_balance.0);
    assert!(registry_tokens >= c.allowance_total);
}

#[test]
fn the_registrar_settles_a_write_off_when_the_callback_never_comes() {
    // The transfer reached the vault.
    let (mut c, t) = quarantined(80_000);
    resolve_wo(&mut c, &t, true);
    assert!(get_logs().join("\n").contains("written_off_resolved"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_routed_in.0, l.undelivered.0),
        (80_000, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 0));
    assert_eq!(watch(&c, &t).written_off.0, 0);
    // A callback after the decision is not counted a second time.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
    assert_eq!(acct(&c), (0, 0));
    assert_panics("nothing written off", || resolve_wo(&mut c, &t, true));
}

#[test]
fn a_write_off_settled_as_not_delivered_moves_to_undelivered_once() {
    let (mut c, t) = quarantined(80_000);
    resolve_wo(&mut c, &t, false);
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.undelivered.0), (0, 80_000));
    assert_eq!(acct(&c), (0, 80_000));
    // A late failure callback does not move it again.
    assert!(!settle_at(&mut c, &t, 80_000, 500, false));
    assert_eq!(ledger(&c, &t).undelivered.0, 80_000);
    assert_eq!(acct(&c), (0, 80_000));
}

#[test]
fn only_the_registrar_with_one_yocto_and_a_hash_can_resolve() {
    let (mut c, t) = quarantined(80_000);
    let r = |c: &mut Registry, d: u128, f: u128, hash: &str| {
        c.resolve_written_off(t.clone(), U128(d), U128(f), hash.to_string())
    };
    call(&anyone(), NearToken::from_yoctonear(1));
    assert_panics("registrar", || r(&mut c, 80_000, 0, &h('d')));
    call(&registrar(), NearToken::from_yoctonear(0));
    assert_panics("1 yocto", || r(&mut c, 80_000, 0, &h('d')));
    call(&registrar(), NearToken::from_yoctonear(1));
    assert_panics("doc_hash", || r(&mut c, 80_000, 0, "xyz"));
    assert_panics("more than is written off", || r(&mut c, 80_000, 1, &h('d')));
    assert_panics("nothing to resolve", || r(&mut c, 0, 0, &h('d')));
    assert_eq!(watch(&c, &t).written_off.0, 80_000);
}

/// Audit A-05: a recovery must not credit a share in a dispute or without
/// approval, like `sync`.
#[test]
fn resolve_as_delivered_is_held_for_a_disputed_or_unapproved_share() {
    let (mut c, t) = quarantined(80_000);
    c.shares.get_mut(&t).unwrap().dispute = Some(0);
    assert_panics("in dispute", || resolve_wo(&mut c, &t, true));
    assert_eq!(ledger(&c, &t).known_balance.0, 0);
    // Not delivered only moves the amount back to the registry's own bucket.
    resolve_wo(&mut c, &t, false);
    assert_eq!(ledger(&c, &t).undelivered.0, 80_000);
}

#[test]
fn resolve_as_delivered_is_held_for_an_unapproved_share() {
    let (mut c, t) = quarantined(80_000);
    c.set_approval(&t, approval::Approval::Pending);
    assert_panics("not approved", || resolve_wo(&mut c, &t, true));
    assert_eq!(ledger(&c, &t).known_balance.0, 0);
}

/// Audit A-05: the reviewer's harness. A pre-dispute forward was lost, the
/// share is disputed, then a plain deposit arrives. Recovery must not make
/// that deposit claimable.
#[test]
fn disputed_plain_deposit_must_not_become_claimable_via_recovery() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    c.shares.get_mut(&t).unwrap().dispute = Some(0);
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("in dispute", || drop(c.sync(t.clone())));
    release_fwd(&mut c, &t, 500 + STALE_F);
    // Even with a plain deposit of 80000 in the vault: no read, no credit.
    assert_eq!(ledger(&c, &t).claimable.0, 0);
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("in dispute", || drop(c.sync(t.clone())));
    assert_panics("in dispute", || resolve_wo(&mut c, &t, true));
    assert_eq!(ledger(&c, &t).claimable.0, 0);
}

/// Audit A-04 and A-06: a pending forward without a record cannot be told
/// apart, so a release is refused. The reviewer's legacy harness: the legacy
/// callback keeps settling normally and no balance is attributed twice.
#[test]
fn audit_legacy_none_callback_cannot_double_attribute_a_vault_balance() {
    let (mut c, t) = fresh();
    // A genuine pre-upgrade forward: counted, no watch entry, already delivered.
    c.ledgers.get_mut(&t).unwrap().inbound_pending = 1;
    c.forwards_in_flight_total = 80_000;
    pay_open(&mut c, &t, 50_000, 100);
    assert_eq!(ledger(&c, &t).inbound_pending, 2);
    assert_eq!(watch(&c, &t).count, 1);
    at(&anyone(), NearToken::from_yoctonear(0), 0, 100 + STALE_F);
    assert_panics("has no record", || c.release_stale_forward(t.clone()));
    // Nothing changed: no balance was read, nothing was credited.
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.inbound_pending), (0, 2));
    // The legacy callback and the new one settle normally, once each.
    callback(ok(b""));
    assert!(c.on_forward(t.clone(), U128(80_000), None));
    assert!(!settle_at(&mut c, &t, 50_000, 100, false));
    let l = ledger(&c, &t);
    assert_eq!(
        (
            l.known_balance.0,
            l.total_routed_in.0,
            l.undelivered.0,
            l.inbound_pending
        ),
        (80_000, 80_000, 50_000, 0)
    );
}

/// Review M2, redesigned by audit A-02: a forward pending without a watch
/// entry must not block a stale payout lock for good. It is never cleared
/// blindly, but the registrar settles it, so the share is not stuck.
#[test]
fn unwatched_pending_forward_does_not_block_stale_payout_release() {
    let (mut c, t) = fresh();
    pay_share(&mut c, &t, 700_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    c.ledgers.get_mut(&t).unwrap().inbound_pending = 1;
    assert!(c.get_forward_watch(t.clone()).is_none());
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        100 + STALE_LOCK_BLOCKS,
    );
    let _ = c.release_stale_lock(t.clone());
    assert!(
        ledger(&c, &t).payout_in_flight.is_some(),
        "no automatic clear"
    );
    resolve_payout(&mut c, &t, true, 100 + STALE_LOCK_BLOCKS);
    let l = ledger(&c, &t);
    assert!(l.payout_in_flight.is_none());
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
}

/// Audit A-06: state from a deployment with a forward in flight and no
/// record has no automatic path. The registrar can still settle its payout
/// lock, and the deploy precondition (no pending forward) keeps it away.
#[test]
fn legacy_missing_watch_and_stale_payout_is_settled_by_the_registrar() {
    let (mut c, t) = fresh();
    {
        let l = c.ledgers.get_mut(&t).unwrap();
        l.inbound_pending = 1;
        l.total_paid_out = 100_000;
        l.payout_in_flight = Some(Lock {
            account: acc("bob.near"),
            amount: U128(100_000),
            since_block: U64(500),
        });
    }
    c.forwards_in_flight_total = 80_000;
    at(&anyone(), NearToken::from_yoctonear(0), 0, 1_700);
    let _ = c.release_stale_lock(t.clone());
    assert!(
        ledger(&c, &t).payout_in_flight.is_some(),
        "no automatic clear"
    );
    assert_panics("no recorded forward", || c.release_stale_forward(t.clone()));
    resolve_payout(&mut c, &t, false, 1_700);
    let l = ledger(&c, &t);
    assert!(l.payout_in_flight.is_none());
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (100_000, 0));
}

#[test]
fn a_malformed_watch_entry_stops_the_call_instead_of_reading_as_absent() {
    let (mut c, t) = fresh();
    let mut key = b"fwd:".to_vec();
    key.extend_from_slice(t.as_bytes());
    env::storage_write(&key, &[1, 2, 3]);
    assert_panics("malformed forward watch", || {
        let _ = c.get_forward_watch(t.clone());
    });
    assert_panics("malformed forward watch", || {
        pay_open(&mut c, &t, 10_000, 500)
    });
    at(&anyone(), NearToken::from_yoctonear(0), 0, 500 + STALE_F);
    assert_panics("malformed forward watch", || {
        c.release_stale_forward(t.clone())
    });
}

#[test]
fn a_sync_started_before_a_release_is_discarded_at_commit() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = c.ledgers.get(&t).unwrap().epoch;
    // A quarantine appears while the read is out (the epoch moves too).
    let w = forwards::ForwardWatch {
        written_off: 1,
        ..Default::default()
    };
    forwards::write_watch(&t, &w);
    callback(ok(b"\"90000\""));
    assert!(c.on_sync(t.clone(), U64(epoch)).is_none());
    assert!(get_logs().join("\n").contains("sync_discarded"));
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
}

#[test]
fn a_second_release_after_the_first_is_refused() {
    let (mut c, t) = quarantined(80_000);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 5,
    );
    assert_panics("no recorded forward", || c.release_stale_forward(t.clone()));
    assert_eq!(watch(&c, &t).written_off.0, 80_000);
    assert_eq!(acct(&c), (80_000, 0));
}

/// A second lost forward after a release adds to the quarantine.
#[test]
fn a_later_lost_forward_adds_to_the_quarantine() {
    let (mut c, t) = quarantined(80_000);
    pay_open(&mut c, &t, 30_000, 600 + STALE_F);
    release_fwd(&mut c, &t, 600 + 2 * STALE_F);
    let w = watch(&c, &t);
    assert_eq!(
        (w.written_off.0, w.released_through.0),
        (110_000, 600 + STALE_F)
    );
    // The late callback of the first one settles its own part only.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    let w = watch(&c, &t);
    assert_eq!(w.written_off.0, 30_000);
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
    assert_eq!(acct(&c), (30_000, 0));
}

/// Review M1: a drip of new payments must not postpone a release.
#[test]
fn a_drip_of_payments_cannot_postpone_a_stale_release() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500); // its callback is lost
    for (i, blk) in [40_000u64, 90_000, 140_000].into_iter().enumerate() {
        pay_open(&mut c, &t, 10_000, blk);
        assert!(settle_at(&mut c, &t, 10_000, blk, true), "drip {i}");
        assert_eq!(
            watch(&c, &t).since_block.0,
            500,
            "busy period start unchanged"
        );
    }
    // Once the oldest forward is a day old, new payments are held, not sent.
    pay_open(&mut c, &t, 10_000, 500 + STALE_F);
    assert!(get_logs().join("\n").contains("forward_held"));
    assert_eq!(acct(&c), (80_000, 10_000));
    assert_eq!(
        watch(&c, &t).latest_block.0,
        140_000,
        "nothing new was recorded"
    );
    at(&anyone(), NearToken::from_yoctonear(0), 0, 500 + STALE_F);
    assert_panics("stale forward is stuck", || c.retry_forward(t.clone()));
    // And the release is open, because `since` did not move.
    release_fwd(&mut c, &t, 500 + STALE_F);
    assert_eq!(watch(&c, &t).written_off.0, 80_000);
    assert_eq!(ledger(&c, &t).known_balance.0, 30_000);
    assert_eq!(acct(&c), (80_000, 10_000));
}

// ---------- audit F3, A-02, A-03, RR-2: stale payout lock ----------

/// A claim of `amount` at `block` whose callback is lost, then the stale
/// release call. The lock must stay.
fn claim_and_go_stale(c: &mut Registry, t: &TokenId, amount: u128, block: u64) {
    pay_share(c, t, amount);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, block);
    let _ = c.claim(t.clone());
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        block + STALE_LOCK_BLOCKS,
    );
    let _ = c.release_stale_lock(t.clone());
}

fn diagnose(c: &mut Registry, t: &TokenId, block: u64, amount: u128, vault: &str) {
    callback(ok(format!("\"{vault}\"").as_bytes()));
    c.on_release_payout(t.clone(), U64(block), U128(amount));
}

fn lock_and_debit(c: &Registry, t: &TokenId) -> (bool, u128, u128) {
    let l = ledger(c, t);
    (
        l.payout_in_flight.is_some(),
        l.known_balance.0,
        l.total_paid_out.0,
    )
}

/// Audit RR-2: the release never decides a payout from a balance. Whatever
/// the vault shows, the lock and the debit stay as they are.
fn stale_lock_stays_for(vault: &str) {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 700_000, 100);
    diagnose(&mut c, &t, 100, 700_000, vault);
    assert!(
        get_logs().join("\n").contains("payout_uncertain"),
        "{vault}"
    );
    assert_eq!(lock_and_debit(&c, &t), (true, 0, 700_000), "{vault}");
}

#[test]
fn a_stale_payout_lock_stays_when_the_vault_looks_paid() {
    stale_lock_stays_for("0");
}

#[test]
fn a_stale_payout_lock_stays_when_the_vault_looks_failed() {
    stale_lock_stays_for("700000");
}

#[test]
fn a_stale_payout_lock_stays_for_any_other_balance() {
    stale_lock_stays_for("900000");
}

/// Audit RR-2, the reviewer's equal-deposit counterexample: a claim of
/// 100000 really succeeded and lost its callback, then a deposit of 100000
/// restored the vault to the failure balance. The old exact match undid the
/// debit and erased the paid-out history. Now nothing changes, and the late
/// real callback still settles the lock.
#[test]
fn an_equal_deposit_cannot_turn_a_paid_payout_into_a_failed_one() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 100_000, 100);
    diagnose(&mut c, &t, 100, 100_000, "100000");
    assert_eq!(lock_and_debit(&c, &t), (true, 0, 100_000));
    // The real success callback arrives late and settles the lock.
    callback(ok(b""));
    assert!(c.on_claim(t.clone(), acc("bob.near"), U128(100_000), U64(100)));
    assert_eq!(lock_and_debit(&c, &t), (false, 0, 100_000));
    assert!(sync_with(&mut c, &t, 100_000));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_paid_out.0, l.earned.0),
        (100_000, 100_000, 200_000),
        "received 200000, paid 100000"
    );
}

/// Audit RR-2, the ownership consequence: while the payout lock stays, the
/// share cannot be transferred, so an old withdraw cannot empty a new
/// holder's vault.
#[test]
fn a_share_with_a_stale_payout_lock_cannot_be_sold_before_it_is_settled() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 100_000, 100);
    diagnose(&mut c, &t, 100, 100_000, "100000");
    at(
        &acc("bob.near"),
        NearToken::from_yoctonear(1),
        0,
        100 + STALE_LOCK_BLOCKS,
    );
    assert_panics("", || {
        c.nft_transfer(acc("carol.near"), t.clone(), None, None, None);
    });
    assert_eq!(c.owner_of(&t), Some(acc("bob.near")));
}

#[test]
fn a_registrar_decision_that_the_payout_failed_undoes_the_debit_once() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 700_000, 100);
    resolve_payout(&mut c, &t, false, 100 + STALE_LOCK_BLOCKS);
    assert_eq!(lock_and_debit(&c, &t), (false, 700_000, 0));
    assert!(get_logs().join("\n").contains("payout_resolved"));
    // The late real callback finds no lock and changes nothing.
    callback(PromiseResult::Failed);
    assert!(!c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(100)));
    assert_eq!(lock_and_debit(&c, &t), (false, 700_000, 0));
    assert!(sync_with(&mut c, &t, 700_000));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.earned.0, l.total_paid_out.0, l.claimable.0),
        (700_000, 0, 700_000)
    );
}

#[test]
fn a_registrar_decision_that_the_payout_was_paid_keeps_the_debit() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 700_000, 100);
    resolve_payout(&mut c, &t, true, 100 + STALE_LOCK_BLOCKS);
    assert_eq!(lock_and_debit(&c, &t), (false, 0, 700_000));
}

/// Audit A-03: the reviewer's harness. A failed payout plus a plain deposit:
/// the lock stays, no history is published, and the registrar's decision
/// gives a true `earned` after the next sync.
#[test]
fn a_failed_payout_plus_a_deposit_keeps_the_lock_and_never_inflates_earned() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 100_000, 100);
    diagnose(&mut c, &t, 100, 100_000, "120000");
    assert!(get_logs().join("\n").contains("payout_uncertain"));
    assert_eq!(lock_and_debit(&c, &t), (true, 0, 100_000));
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("", || drop(c.sync(t.clone())));
    assert_panics("registrar", || {
        call(&anyone(), NearToken::from_yoctonear(1));
        c.resolve_stale_payout(t.clone(), false, h('e'));
    });
    resolve_payout(&mut c, &t, false, 100 + STALE_LOCK_BLOCKS);
    assert!(sync_with(&mut c, &t, 120_000));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_paid_out.0, l.earned.0),
        (120_000, 0, 120_000)
    );
}

/// Audit A-02: the reviewer's harness. A failed payout and a failed forward
/// are not conflated: the payout lock is never cleared by the release.
#[test]
fn a_failed_payout_and_a_failed_forward_are_not_conflated() {
    let (mut c, t) = fresh();
    pay_share(&mut c, &t, 100_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    pay_open(&mut c, &t, 50_000, 200);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        200 + STALE_LOCK_BLOCKS,
    );
    let _ = c.release_stale_lock(t.clone());
    assert!(ledger(&c, &t).payout_in_flight.is_some());
    release_fwd(&mut c, &t, 200 + STALE_F);
    let _ = c.release_stale_lock(t.clone());
    assert!(ledger(&c, &t).payout_in_flight.is_some());
    // Both are settled from the public outcome: both failed.
    resolve_payout(&mut c, &t, false, 200 + STALE_F);
    resolve_wo(&mut c, &t, false);
    let a = c.get_accounting();
    assert_eq!(
        (a.forwards_in_flight_total.0, a.undelivered_total.0),
        (0, 50_000)
    );
    assert!(sync_with(&mut c, &t, 100_000));
    let l = ledger(&c, &t);
    assert_eq!(
        (
            l.known_balance.0,
            l.total_paid_out.0,
            l.total_routed_in.0,
            l.earned.0
        ),
        (100_000, 0, 100_000, 100_000)
    );
    assert_eq!(l.undelivered.0, 50_000);
}

#[test]
fn a_failed_balance_read_or_a_moved_lock_changes_nothing() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 700_000, 100);
    callback(PromiseResult::Failed);
    c.on_release_payout(t.clone(), U64(100), U128(700_000));
    assert!(get_logs().join("\n").contains("release_discarded"));
    assert_eq!(lock_and_debit(&c, &t), (true, 0, 700_000));
    // The real claim callback settles the lock; a later read finds none.
    callback(ok(b""));
    assert!(c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(100)));
    diagnose(&mut c, &t, 100, 700_000, "700000");
    assert!(get_logs().join("\n").contains("release_discarded"));
    assert_eq!(lock_and_debit(&c, &t), (false, 0, 700_000));
}

#[test]
fn a_payout_lock_with_a_pending_forward_is_never_cleared() {
    let (mut c, t) = fresh();
    pay_share(&mut c, &t, 700_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    pay_open(&mut c, &t, 20_000, 100 + STALE_LOCK_BLOCKS - 5);
    for h in [100 + STALE_LOCK_BLOCKS, 100 + 2 * STALE_LOCK_BLOCKS] {
        at(&anyone(), NearToken::from_yoctonear(0), 0, h);
        let _ = c.release_stale_lock(t.clone());
    }
    assert_eq!(lock_and_debit(&c, &t), (true, 0, 700_000));
}

#[test]
fn resolve_stale_payout_needs_a_stale_lock() {
    let (mut c, t) = fresh();
    pay_share(&mut c, &t, 700_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    at(
        &registrar(),
        NearToken::from_yoctonear(1),
        0,
        100 + STALE_LOCK_BLOCKS - 1,
    );
    assert_panics("no stale payout lock", || {
        c.resolve_stale_payout(t.clone(), true, h('e'))
    });
    at(
        &registrar(),
        NearToken::from_yoctonear(1),
        0,
        100 + STALE_LOCK_BLOCKS,
    );
    c.resolve_stale_payout(t.clone(), true, h('e'));
    assert_eq!(lock_and_debit(&c, &t), (false, 0, 700_000));
    assert_panics("no stale payout lock", || {
        c.resolve_stale_payout(t.clone(), true, h('e'))
    });
}

/// Review L4: a stale transfer lock is released even when a stale payout
/// lock sits beside it, and the payout lock stays.
#[test]
fn a_stale_transfer_lock_is_released_even_if_the_payout_lock_must_wait() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 20_000, 1_000 + STALE_LOCK_BLOCKS);
    {
        let l = c.ledgers.get_mut(&t).unwrap();
        l.payout_in_flight = Some(Lock {
            account: acc("bob.near"),
            amount: U128(5),
            since_block: U64(1_000),
        });
        l.transfer_pending = Some(Lock {
            account: acc("bob.near"),
            amount: U128(0),
            since_block: U64(1_000),
        });
    }
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        1_000 + STALE_LOCK_BLOCKS,
    );
    let _ = c.release_stale_lock(t.clone());
    let l = ledger(&c, &t);
    assert!(l.transfer_pending.is_none(), "transfer lock released");
    assert!(l.payout_in_flight.is_some(), "payout lock stays");
}

// ---------- audit RR-1, R-M1: a resolved quarantine fences its callbacks ----------

/// RR-1, success case. A (80000) reaches the vault, its callback is delayed,
/// the registrar truthfully credits it. Later B (50000) fails and is
/// quarantined. A's late success callback must not consume B's quarantine.
#[test]
fn a_late_success_callback_of_a_resolved_quarantine_never_settles_a_later_one() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    release_fwd(&mut c, &t, 500 + STALE_F);
    resolve_wo(&mut c, &t, true);
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
    let b_start = 600 + STALE_F;
    pay_open(&mut c, &t, 50_000, b_start);
    release_fwd(&mut c, &t, b_start + STALE_F);
    assert_eq!(watch(&c, &t).written_off.0, 50_000);
    // A's callback finally runs.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(get_logs()
        .join("\n")
        .contains("late_forward_callback_fenced"));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.total_routed_in.0), (80_000, 80_000));
    assert_eq!(watch(&c, &t).written_off.0, 50_000, "B untouched");
    assert_eq!(acct(&c), (50_000, 0));
    // B's real callback (failed) still settles B, once.
    assert!(!settle_at(&mut c, &t, 50_000, b_start, false));
    assert_eq!(watch(&c, &t).written_off.0, 0);
    assert_eq!(ledger(&c, &t).undelivered.0, 50_000);
    assert_eq!(acct(&c), (0, 50_000));
}

/// RR-1, failure case. A (80000) failed and was truthfully resolved as
/// failed, then retried and succeeded. B (50000) reaches the vault and is
/// quarantined. A's late failed callback must not make B retryable.
#[test]
fn a_late_failure_callback_of_a_resolved_quarantine_never_settles_a_later_one() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    release_fwd(&mut c, &t, 500 + STALE_F);
    resolve_wo(&mut c, &t, false);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_F + 1,
    );
    c.retry_forward(t.clone());
    assert!(settle_at(&mut c, &t, 80_000, 500 + STALE_F + 1, true));
    let b_start = 600 + 2 * STALE_F;
    pay_open(&mut c, &t, 50_000, b_start);
    release_fwd(&mut c, &t, b_start + STALE_F);
    // A's late failed callback.
    assert!(!settle_at(&mut c, &t, 80_000, 500, false));
    assert!(get_logs()
        .join("\n")
        .contains("late_forward_callback_fenced"));
    assert_eq!(ledger(&c, &t).undelivered.0, 0, "B is not retryable");
    assert_eq!(watch(&c, &t).written_off.0, 50_000);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        b_start + STALE_F + 1,
    );
    assert_panics("nothing undelivered", || c.retry_forward(t.clone()));
    assert_eq!(acct(&c), (50_000, 0));
}

// ---------- audit RR-3: mixed outcomes ----------

#[test]
fn a_mixed_quarantine_is_settled_with_separate_delivered_and_failed_amounts() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 40_000, 500);
    pay_open(&mut c, &t, 60_000, 510);
    release_fwd(&mut c, &t, 500 + STALE_F);
    assert_eq!(watch(&c, &t).written_off.0, 100_000);
    resolve_amounts(&mut c, &t, 40_000, 60_000);
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_routed_in.0, l.undelivered.0),
        (40_000, 40_000, 60_000)
    );
    assert_eq!(acct(&c), (0, 60_000));
    assert_eq!(watch(&c, &t).written_off.0, 0);
    assert!(get_logs().join("\n").contains("written_off_resolved"));
}

#[test]
fn a_remainder_stays_quarantined_and_its_callbacks_stay_fenced() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 40_000, 500);
    pay_open(&mut c, &t, 60_000, 510);
    release_fwd(&mut c, &t, 500 + STALE_F);
    resolve_amounts(&mut c, &t, 40_000, 0);
    assert_eq!(watch(&c, &t).written_off.0, 60_000, "remainder");
    assert_eq!(acct(&c), (60_000, 0));
    // No callback of that quarantine may settle the remainder by amount.
    assert!(settle_at(&mut c, &t, 40_000, 500, true));
    assert_eq!(watch(&c, &t).written_off.0, 60_000);
    assert_eq!(ledger(&c, &t).known_balance.0, 40_000);
    // The registrar settles the rest.
    resolve_amounts(&mut c, &t, 0, 60_000);
    assert_eq!(acct(&c), (0, 60_000));
    assert_eq!(watch(&c, &t).written_off.0, 0);
}

#[test]
fn a_delivered_part_is_held_for_a_disputed_share_but_a_failed_part_is_not() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 40_000, 500);
    pay_open(&mut c, &t, 60_000, 510);
    release_fwd(&mut c, &t, 500 + STALE_F);
    c.shares.get_mut(&t).unwrap().dispute = Some(0);
    assert_panics("in dispute", || resolve_amounts(&mut c, &t, 40_000, 60_000));
    resolve_amounts(&mut c, &t, 0, 60_000);
    assert_eq!(ledger(&c, &t).undelivered.0, 60_000);
    assert_eq!(watch(&c, &t).written_off.0, 40_000);
}

// ---------- audit RR-4, R-L3: the deploy barrier ----------

fn self_call() {
    at(&acc("reg.near"), NearToken::from_yoctonear(0), 0, 10);
}

#[test]
fn assert_quiescent_passes_on_a_quiet_registry() {
    let (c, _t) = fresh();
    self_call();
    c.assert_quiescent();
}

#[test]
fn assert_quiescent_refuses_a_forward_in_flight_and_passes_once_it_settles() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 20_000, 500);
    self_call();
    assert_panics("not quiet", || c.assert_quiescent());
    assert!(settle_at(&mut c, &t, 20_000, 500, true));
    self_call();
    c.assert_quiescent();
}

#[test]
fn assert_quiescent_refuses_a_payout_lock() {
    let (mut c, t) = fresh();
    claim_and_go_stale(&mut c, &t, 700_000, 100);
    self_call();
    assert_panics("payout lock", || c.assert_quiescent());
}

#[test]
fn assert_quiescent_refuses_a_transfer_lock() {
    let (mut c, t) = fresh();
    c.ledgers.get_mut(&t).unwrap().transfer_pending = Some(Lock {
        account: acc("bob.near"),
        amount: U128(0),
        since_block: U64(1),
    });
    self_call();
    assert_panics("transfer lock", || c.assert_quiescent());
}

#[test]
fn assert_quiescent_refuses_a_quarantine_until_it_is_resolved() {
    let (mut c, t) = quarantined(80_000);
    self_call();
    assert_panics("not quiet", || c.assert_quiescent());
    resolve_wo(&mut c, &t, false);
    // The amount sits in undelivered, which is not a forward.
    self_call();
    c.assert_quiescent();
}

#[test]
fn assert_quiescent_refuses_a_watch_in_another_format() {
    let (c, t) = fresh();
    let mut key = b"fwd:".to_vec();
    key.extend_from_slice(t.as_bytes());
    env::storage_write(&key, &[0u8; 36]);
    self_call();
    assert_panics("malformed forward watch", || c.assert_quiescent());
}
