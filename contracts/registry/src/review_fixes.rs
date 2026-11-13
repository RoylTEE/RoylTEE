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

// ---------- H2: contests ----------

/// H2: a 1 bps claim can no longer default to contesting every share; a
/// named minimal set freezes only what the excess needs.
#[test]
fn h2_one_bps_claim_freezes_only_one_named_share() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let a = mint(&mut c, w, "bob.near", 5000);
    let b = mint(&mut c, w, "carol.near", 5000);
    call(&acc("alice.near"), NearToken::from_near(1));
    assert_panics("name the contested shares", || {
        drop(c.register_share(
            w,
            RightType::Publishing,
            "WW".into(),
            1,
            Basis::Copyright,
            acc("alice.near"),
            h('e'),
            None,
        ))
    });
    call(&acc("alice.near"), NearToken::from_near(1));
    assert_panics("more shares than the excess needs", || {
        drop(c.register_share(
            w,
            RightType::Publishing,
            "WW".into(),
            1,
            Basis::Copyright,
            acc("alice.near"),
            h('e'),
            Some(vec![a.clone(), b.clone()]),
        ))
    });
    let _cl = mint_with(&mut c, w, "alice.near", 1, Some(vec![a.clone()]));
    assert!(c.get_share(a.clone()).unwrap().dispute.is_some());
    assert!(c.get_share(b.clone()).unwrap().dispute.is_none());
    let _ = pay_work_msg(&mut c, w, 1_000_000);
    assert_eq!(ledger(&c, &a).escrowed.0, 500_000);
    assert_eq!(ledger(&c, &b).inbound_pending, 1, "b still paid");
}

/// H2: repeated claims plus uphold_existing no longer fill the group list;
/// each voided claimant is removed, so the group never closes.
#[test]
fn h2_repeat_claims_do_not_close_the_group() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let a = mint(&mut c, w, "bob.near", 5000);
    let _b = mint(&mut c, w, "carol.near", 5000);
    for _ in 0..(MAX_GROUP_ENTRIES + 5) {
        let cl = mint_with(&mut c, w, "alice.near", 1, Some(vec![a.clone()]));
        let id = c.get_share(cl.clone()).unwrap().dispute.unwrap();
        resolve(&mut c, id, Outcome::UpholdExisting);
        assert_eq!(c.get_share(cl).unwrap().status, ShareStatus::Void);
        assert_eq!(group_of(&c, w).shares.len(), 2);
    }
    // A genuine claimant can still register.
    let cl = mint_with(&mut c, w, "alice.near", 2500, Some(vec![a.clone()]));
    assert!(c.get_share(cl).unwrap().dispute.is_some());
}

/// H2: an incumbent cannot open a sham dispute against its own dummy share
/// to hold the group's one dispute slot.
#[test]
fn h2_incumbent_cannot_dispute_its_own_share() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _main = mint(&mut c, w, "bob.near", 9999);
    let dummy = mint(&mut c, w, "bob.near", 1);
    call(&acc("alice.near"), NearToken::from_near(1));
    assert_panics("held by the claimant or the caller", || {
        drop(c.register_share(
            w,
            RightType::Publishing,
            "WW".into(),
            1,
            Basis::Copyright,
            acc("bob.near"),
            h('e'),
            Some(vec![dummy.clone()]),
        ))
    });
    // The caller cannot contest a share it holds either.
    call(&acc("bob.near"), NearToken::from_near(1));
    assert_panics("held by the claimant or the caller", || {
        drop(c.register_share(
            w,
            RightType::Publishing,
            "WW".into(),
            1,
            Basis::Copyright,
            acc("carol.near"),
            h('e'),
            Some(vec![dummy.clone()]),
        ))
    });
    assert!(group_of(&c, w).open_dispute.is_none());
}

/// H2: uphold_claimant with a 0 bps result also prunes the voided share.
#[test]
fn h2_zero_bps_outcome_is_pruned() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let a = mint(&mut c, w, "bob.near", 5000);
    let b = mint(&mut c, w, "carol.near", 5000);
    let cl = mint_with(&mut c, w, "alice.near", 5000, Some(vec![b.clone()]));
    let id = c.get_share(cl.clone()).unwrap().dispute.unwrap();
    resolve(
        &mut c,
        id,
        Outcome::UpholdClaimant {
            new_bps: vec![
                BpsEntry {
                    token_id: b.clone(),
                    bps: 0,
                },
                BpsEntry {
                    token_id: cl.clone(),
                    bps: 5000,
                },
            ],
        },
    );
    assert_eq!(c.get_share(b.clone()).unwrap().status, ShareStatus::Void);
    assert_eq!(group_of(&c, w).shares, vec![a, cl]);
}
