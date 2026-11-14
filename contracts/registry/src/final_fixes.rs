//! Final contract fixes before the registry key is deleted: recovery of a
//! payment forward whose callback never ran (THREAT_MODEL T19b) and the
//! accounting of a stale payout lock (audit F3, T19). Callbacks are driven
//! by hand with mocked promise results; block heights are set explicitly.

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
    let _ = c.release_stale_forward(t.clone());
    c.ledgers.get(t).unwrap().epoch
}

fn finish_release(
    c: &mut Registry,
    t: &TokenId,
    epoch: u64,
    since: u64,
    vault_balance: &str,
) -> bool {
    callback(ok(format!("\"{vault_balance}\"").as_bytes()));
    c.on_release_forward(t.clone(), U64(epoch), U64(since))
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

#[test]
fn a_forward_is_watched_and_the_watch_ends_when_it_settles() {
    let (mut c, t) = fresh();
    assert!(c.get_forward_watch(t.clone()).is_none());
    pay_open(&mut c, &t, 80_000, 500);
    let w = c.get_forward_watch(t.clone()).unwrap();
    assert_eq!((w.since_block.0, w.amount.0, w.count), (500, 80_000, 1));
    assert_eq!(w.released_through.0, 0);
    pay_open(&mut c, &t, 20_000, 520);
    let w = c.get_forward_watch(t.clone()).unwrap();
    assert_eq!((w.since_block.0, w.amount.0, w.count), (520, 100_000, 2));
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(!settle_at(&mut c, &t, 20_000, 520, false));
    assert!(c.get_forward_watch(t.clone()).is_none(), "entry removed");
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (80_000, 20_000, 0)
    );
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
fn release_needs_a_stale_recorded_forward_and_no_lock() {
    let (mut c, t) = fresh();
    at(&anyone(), NearToken::from_yoctonear(0), 0, 500);
    assert_panics("no recorded forward", || {
        drop(c.release_stale_forward(t.clone()))
    });
    pay_open(&mut c, &t, 80_000, 500);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_LOCK_BLOCKS - 1,
    );
    assert_panics("not stale yet", || drop(c.release_stale_forward(t.clone())));
    testing_env!(ctx(&anyone(), NearToken::from_yoctonear(0))
        .block_height(500 + STALE_LOCK_BLOCKS)
        .prepaid_gas(Gas::from_tgas(15))
        .build());
    assert_panics("not enough gas", || {
        drop(c.release_stale_forward(t.clone()))
    });
    // A payout lock blocks it: the balance would be ambiguous.
    c.ledgers.get_mut(&t).unwrap().payout_in_flight = Some(Lock {
        account: acc("bob.near"),
        amount: U128(1),
        since_block: U64(1),
    });
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_LOCK_BLOCKS,
    );
    assert_panics("release the stale lock first", || {
        drop(c.release_stale_forward(t.clone()))
    });
}

#[test]
fn release_credits_what_the_vault_holds_when_the_callback_was_lost() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    assert_eq!(acct(&c), (80_000, 0));
    // sync is refused while the forward is out.
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert_panics("payment forward in flight", || drop(c.sync(t.clone())));
    let epoch = release_fwd(&mut c, &t, 500 + STALE_LOCK_BLOCKS);
    // The transfer reached the vault: it holds the 80000.
    assert!(finish_release(&mut c, &t, epoch, 500, "80000"));
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
    assert_eq!(l.earned.0, 80_000);
    assert_eq!(acct(&c), (0, 0));
    let w = c.get_forward_watch(t.clone()).unwrap();
    assert_eq!((w.count, w.amount.0, w.released_through.0), (0, 0, 500));
    assert!(get_logs().join("\n").contains("forward_released_stale"));
    // The share is usable again: sync works and the holder can claim.
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = c.ledgers.get(&t).unwrap().epoch;
    callback(ok(b"\"80000\""));
    assert!(c.on_sync(t.clone(), U64(epoch)).is_some());
    assert_eq!(ledger(&c, &t).claimable.0, 80_000);
    // The late callback shows up: it changes no balance and no counter.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(get_logs().join("\n").contains("late_forward_callback"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_routed_in.0, l.inbound_pending),
        (80_000, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 0));
}

#[test]
fn release_moves_an_undelivered_forward_to_undelivered_and_retry_works() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    let epoch = release_fwd(&mut c, &t, 500 + STALE_LOCK_BLOCKS);
    // The transfer never reached the vault.
    assert!(finish_release(&mut c, &t, epoch, 500, "0"));
    let l = ledger(&c, &t);
    assert_eq!(
        (
            l.known_balance.0,
            l.total_routed_in.0,
            l.undelivered.0,
            l.inbound_pending
        ),
        (0, 0, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 80_000));
    // The amount is still in the registry, so retry_forward sends it again.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_LOCK_BLOCKS + 5,
    );
    c.retry_forward(t.clone());
    assert_eq!(acct(&c), (80_000, 0));
    let w = c.get_forward_watch(t.clone()).unwrap();
    assert_eq!(
        (w.since_block.0, w.amount.0, w.count),
        (500 + STALE_LOCK_BLOCKS + 5, 80_000, 1)
    );
    // The old callback is late and ignored; the new one settles normally.
    assert!(!settle_at(&mut c, &t, 80_000, 500, false));
    assert_eq!(acct(&c), (80_000, 0));
    assert_eq!(ledger(&c, &t).inbound_pending, 1);
    assert!(settle_at(
        &mut c,
        &t,
        80_000,
        500 + STALE_LOCK_BLOCKS + 5,
        true
    ));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (80_000, 0, 0)
    );
    assert_eq!(acct(&c), (0, 0));
}

#[test]
fn release_never_credits_more_than_the_recorded_forwards_or_the_vault_holds() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    pay_open(&mut c, &t, 50_000, 540);
    assert_eq!(ledger(&c, &t).inbound_pending, 2);
    // Only the newest start counts for staleness.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + STALE_LOCK_BLOCKS,
    );
    assert_panics("not stale yet", || drop(c.release_stale_forward(t.clone())));
    let epoch = release_fwd(&mut c, &t, 540 + STALE_LOCK_BLOCKS);
    // The vault holds 50000 above the ledger, so only that much is credited.
    assert!(finish_release(&mut c, &t, epoch, 540, "50000"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (50_000, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 80_000));

    // Extra money in the vault (a plain deposit) is not claimed by the
    // release: at most the recorded amount is credited.
    pay_open(&mut c, &t, 10_000, 3_000);
    let epoch = release_fwd(&mut c, &t, 3_000 + STALE_LOCK_BLOCKS);
    assert!(finish_release(&mut c, &t, epoch, 3_000, "1000000"));
    let l = ledger(&c, &t);
    assert_eq!(l.known_balance.0, 60_000);
    assert_eq!(acct(&c), (0, 80_000));
}

#[test]
fn a_release_is_discarded_if_the_ledger_moved_during_the_read() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    let epoch = release_fwd(&mut c, &t, 500 + STALE_LOCK_BLOCKS);
    // The real callback arrives while the balance read is out.
    assert!(settle_at(&mut c, &t, 80_000, 500, true));
    assert!(!finish_release(&mut c, &t, epoch, 500, "80000"));
    assert!(get_logs()
        .join("\n")
        .contains("ledger changed during the read"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_routed_in.0, l.inbound_pending),
        (80_000, 80_000, 0)
    );
    assert_eq!(acct(&c), (0, 0));
}

#[test]
fn a_failed_balance_read_changes_nothing() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    let epoch = release_fwd(&mut c, &t, 500 + STALE_LOCK_BLOCKS);
    callback(PromiseResult::Failed);
    assert!(!c.on_release_forward(t.clone(), U64(epoch), U64(500)));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.inbound_pending), (0, 1));
    assert_eq!(acct(&c), (80_000, 0));
    assert_eq!(c.get_forward_watch(t.clone()).unwrap().count, 1);
}

#[test]
fn a_second_release_after_the_first_is_refused() {
    let (mut c, t) = fresh();
    pay_open(&mut c, &t, 80_000, 500);
    let epoch = release_fwd(&mut c, &t, 500 + STALE_LOCK_BLOCKS);
    assert!(finish_release(&mut c, &t, epoch, 500, "80000"));
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        500 + 3 * STALE_LOCK_BLOCKS,
    );
    assert_panics("no recorded forward", || {
        drop(c.release_stale_forward(t.clone()))
    });
    // A replay of the same callback does nothing.
    assert!(!finish_release(&mut c, &t, epoch, 500, "80000"));
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
}

// ---------- audit F3: stale payout lock ----------

fn claim_and_go_stale(c: &mut Registry, t: &TokenId, amount: u128, block: u64) -> u64 {
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
    c.ledgers.get(t).unwrap().epoch
}

fn finish_payout(
    c: &mut Registry,
    t: &TokenId,
    epoch: u64,
    block: u64,
    amount: u128,
    vault: &str,
) -> bool {
    callback(ok(format!("\"{vault}\"").as_bytes()));
    c.on_release_payout(t.clone(), U64(epoch), U64(block), U128(amount))
}

#[test]
fn f3_failed_stale_payout_is_undone_so_earned_stays_true() {
    let (mut c, t) = fresh();
    let epoch = claim_and_go_stale(&mut c, &t, 700_000, 100);
    assert!(finish_payout(&mut c, &t, epoch, 100, 700_000, "700000"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_paid_out.0, l.earned.0),
        (700_000, 0, 700_000)
    );
    assert!(l.payout_in_flight.is_none());
    // The audit's scenario: sync afterwards no longer doubles earned.
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = c.ledgers.get(&t).unwrap().epoch;
    callback(ok(b"\"700000\""));
    let _ = c.on_sync(t.clone(), U64(epoch));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.earned.0, l.total_paid_out.0, l.claimable.0),
        (700_000, 0, 700_000)
    );
}

#[test]
fn f3_paid_stale_payout_keeps_its_debit() {
    let (mut c, t) = fresh();
    let epoch = claim_and_go_stale(&mut c, &t, 700_000, 100);
    assert!(finish_payout(&mut c, &t, epoch, 100, 700_000, "0"));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_paid_out.0, l.earned.0),
        (0, 700_000, 700_000)
    );
    assert!(!get_logs().join("\n").contains("payout_lock_released_stale"));
}

#[test]
fn f3_unclear_balance_clears_the_lock_without_undoing_the_debit() {
    let (mut c, t) = fresh();
    let epoch = claim_and_go_stale(&mut c, &t, 700_000, 100);
    // Neither "empty" nor "still full": a plain deposit is mixed in.
    assert!(finish_payout(&mut c, &t, epoch, 100, 700_000, "900000"));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
    assert!(l.payout_in_flight.is_none());
    assert!(get_logs().join("\n").contains("payout_lock_released_stale"));
}

#[test]
fn f3_failed_read_or_a_moved_ledger_keeps_the_lock() {
    let (mut c, t) = fresh();
    let epoch = claim_and_go_stale(&mut c, &t, 700_000, 100);
    callback(PromiseResult::Failed);
    assert!(!c.on_release_payout(t.clone(), U64(epoch), U64(100), U128(700_000)));
    assert!(ledger(&c, &t).payout_in_flight.is_some());
    // The real claim callback arrives during a later read.
    callback(ok(b"\"0\""));
    assert!(c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(100)));
    assert!(!finish_payout(&mut c, &t, epoch, 100, 700_000, "700000"));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
}

#[test]
fn f3_a_payout_lock_waits_for_a_normal_forward_and_skips_the_read_for_a_stuck_one() {
    let (mut c, t) = fresh();
    pay_share(&mut c, &t, 700_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    // A new payment is forwarding when the payout lock goes stale.
    pay_open(&mut c, &t, 20_000, 100 + STALE_LOCK_BLOCKS - 5);
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        100 + STALE_LOCK_BLOCKS,
    );
    assert_panics("payment forward in flight", || {
        drop(c.release_stale_lock(t.clone()))
    });
    // Once that forward is stale too, the lock is cleared as before F3:
    // no read, the debit stays, and the share is marked.
    at(
        &anyone(),
        NearToken::from_yoctonear(0),
        0,
        100 + 2 * STALE_LOCK_BLOCKS,
    );
    let _ = c.release_stale_lock(t.clone());
    let l = ledger(&c, &t);
    assert!(l.payout_in_flight.is_none());
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
    assert!(get_logs().join("\n").contains("payout_lock_released_stale"));
    // Then the forward can be released.
    let epoch = release_fwd(&mut c, &t, 100 + 2 * STALE_LOCK_BLOCKS);
    assert!(finish_release(
        &mut c,
        &t,
        epoch,
        100 + STALE_LOCK_BLOCKS - 5,
        "20000"
    ));
    assert_eq!(ledger(&c, &t).known_balance.0, 20_000);
}
