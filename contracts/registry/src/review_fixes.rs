//! Regression tests for the external contracts review (01) and the
//! Security Suite sweep of 2026-11-13. Each test started as a proof of
//! concept that passed on the deployed code; each now asserts the fix.

use super::*;

fn panic_msg(f: impl FnOnce()) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(()) => panic!("expected a panic"),
        Err(e) => e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    }
}

fn assert_panics(expect: &str, f: impl FnOnce()) {
    let m = panic_msg(f);
    assert!(
        m.contains(expect),
        "panic {m:?} does not contain {expect:?}"
    );
}

// ---------- H1 and L4: sync ----------

/// H1: only the holder or the registrar may start a sync, so a third party
/// cannot time a balance read against someone else's vault.
#[test]
fn h1_sync_is_holder_or_registrar_only() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("anyone.near"), NearToken::from_yoctonear(0));
    assert_panics("only the holder or the registrar can sync", || {
        drop(c.sync(t.clone()))
    });
    call(&acc("carol.near"), NearToken::from_yoctonear(0));
    assert_panics("only the holder or the registrar can sync", || {
        drop(c.sync(t.clone()))
    });
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    drop(c.sync(t.clone()));
    call(&registrar(), NearToken::from_yoctonear(0));
    drop(c.sync(t));
}

/// L4 (Security Suite F-04): a sync in flight when a dispute opens is
/// discarded, so a plain deposit held by the dispute rule (T24) never
/// becomes claimable during the dispute.
#[test]
fn l4_inflight_sync_across_dispute_open_is_discarded() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _a = mint(&mut c, w, "alice.near", 5000);
    let b = mint(&mut c, w, "bob.near", 5000);
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(b.clone());
    let epoch = ledger(&c, &b).epoch;
    let _cl = mint_with(&mut c, w, "carol.near", 5000, Some(vec![b.clone()]));
    assert_ne!(ledger(&c, &b).epoch, epoch, "dispute open bumps the epoch");
    callback(ok(b"\"77000\""));
    assert_eq!(c.on_sync(b.clone(), epoch), None);
    assert!(get_logs().join("\n").contains("sync_discarded"));
    assert_eq!(ledger(&c, &b).known_balance.0, 0);
    yocto(&acc("bob.near"));
    assert_panics("nothing to claim", || drop(c.claim(b.clone())));
    // Even a read carrying the new epoch is discarded while frozen.
    let now = ledger(&c, &b).epoch;
    callback(ok(b"\"77000\""));
    assert_eq!(c.on_sync(b.clone(), now), None);
}

/// L4: nft_transfer_call bumps the epoch and on_sync refuses while
/// transfer_pending is set.
#[test]
fn l4_sync_does_not_land_during_transfer_pending() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = ledger(&c, &t).epoch;
    yocto(&acc("bob.near"));
    let _ = c.nft_transfer_call(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None,
    );
    assert!(ledger(&c, &t).transfer_pending.is_some());
    assert_ne!(ledger(&c, &t).epoch, epoch);
    callback(ok(b"\"5000\""));
    assert_eq!(c.on_sync(t.clone(), epoch), None);
    let now = ledger(&c, &t).epoch;
    callback(ok(b"\"5000\""));
    assert_eq!(c.on_sync(t.clone(), now), None, "transfer still pending");
    assert_eq!(ledger(&c, &t).known_balance.0, 0);
}
