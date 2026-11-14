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

pub(super) fn assert_panics(expect: &str, f: impl FnOnce()) {
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

// ---------- H3: registrar ----------

/// H3: the registrar cannot grant itself a credential, so it can never be
/// a holder, receive a share through registrar_transfer, or claim.
#[test]
fn h3_registrar_cannot_hold_a_credential() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 100_000);
    call(&registrar(), NearToken::from_near(1));
    assert_panics("the registrar cannot hold a credential", || {
        drop(c.grant_credential(registrar(), h('a'), h('b'), h('c')))
    });
    yocto(&registrar());
    c.suspend_credential(acc("bob.near"), h('9'));
    yocto(&registrar());
    assert_panics("receiver has no active credential", || {
        c.registrar_transfer(t.clone(), registrar(), h('8'))
    });
    yocto(&registrar());
    assert_panics("only the current holder can claim", || {
        drop(c.claim(t.clone()))
    });
}

// ---------- M2: on-chain split sheet ----------

fn register_as(
    c: &mut Registry,
    caller: &str,
    work_id: WorkId,
    holder: &str,
    bps: Bps,
    contests: Option<Vec<TokenId>>,
) -> TokenId {
    call(&acc(caller), NearToken::from_near(1));
    let _ = c.register_share(
        work_id,
        RightType::Publishing,
        "WW".into(),
        bps,
        Basis::Copyright,
        acc(holder),
        h('e'),
        contests,
    );
    format!("s{}", c.next_share)
}

fn finish_mint(c: &mut Registry, t: &TokenId) -> bool {
    callback(ok(b""));
    let _ = c.on_vault_created(t.clone(), acc("alice.near"));
    callback(ok(
        b"{\"total\":\"1250000000000000000000\",\"available\":\"0\"}",
    ));
    c.on_vault_ready(t.clone(), acc("alice.near"))
}

/// M2: a member who did not register the work can no longer take its
/// payments by registering first. The share is held until the work
/// registrant approves it; then the held money goes to its vault.
#[test]
fn m2_share_by_non_registrant_is_held_until_approved() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001"); // registered by alice
    let t = register_as(&mut c, "carol.near", w, "carol.near", 10_000, None);
    assert!(get_logs().join("\n").contains("\"approval\":\"pending\""));
    assert!(finish_mint(&mut c, &t));
    assert_eq!(c.get_share_approval(t.clone()), "pending");
    assert!(matches!(
        pay_work_msg(&mut c, w, 1_000_000),
        PromiseOrValue::Value(U128(0))
    ));
    assert!(get_logs().join("\n").contains("awaiting_approval"));
    assert_eq!(ledger(&c, &t).escrowed.0, 1_000_000);
    assert_eq!(ledger(&c, &t).inbound_pending, 0, "nothing forwarded");
    assert_eq!(c.get_accounting().escrow_total.0, 1_000_000);
    // A direct share payment is held too.
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    assert!(matches!(
        c.ft_on_transfer(acc("payer.near"), U128(MIN_GROUP_PAYMENT), msg.to_string()),
        PromiseOrValue::Value(U128(0))
    ));
    assert_eq!(ledger(&c, &t).escrowed.0, 1_010_000);
    // Held: no claim, no transfer, no sync, no confirmations.
    yocto(&acc("carol.near"));
    assert_panics("nothing to claim", || drop(c.claim(t.clone())));
    yocto(&acc("carol.near"));
    assert_panics("not approved", || {
        c.nft_transfer(acc("bob.near"), t.clone(), None, None, None)
    });
    call(&acc("carol.near"), NearToken::from_yoctonear(0));
    assert_panics("not approved", || drop(c.sync(t.clone())));
    // Only the work registrant (or the registrar) signs.
    yocto(&acc("carol.near"));
    assert_panics("only the work registrant or the registrar", || {
        c.approve_share(t.clone(), None)
    });
    yocto(&acc("bob.near"));
    assert_panics("only the work registrant or the registrar", || {
        c.approve_share(t.clone(), None)
    });
    call(&acc("alice.near"), NearToken::from_yoctonear(0));
    assert_panics("exactly 1 yoctoNEAR", || c.approve_share(t.clone(), None));
    yocto(&acc("alice.near"));
    c.approve_share(t.clone(), None);
    assert!(get_logs().join("\n").contains("share_approved"));
    assert_eq!(c.get_share_approval(t.clone()), "approved");
    assert_eq!(c.get_accounting().escrow_total.0, 0);
    settle(&mut c, &t, 1_010_000);
    yocto(&acc("carol.near"));
    let _ = c.claim(t.clone());
    assert_eq!(ledger(&c, &t).total_paid_out.0, 1_010_000);
    // Approved now: later payments flow straight to the vault.
    let _ = pay_work_msg(&mut c, w, 10_000);
    assert_eq!(ledger(&c, &t).inbound_pending, 1);
}

/// M2: rejection voids the share, frees its bps and moves the held money
/// to the group's unallocated escrow, so the real owner can register and
/// the registrar can release it.
#[test]
fn m2_reject_voids_share_and_keeps_money_in_the_group() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = register_as(&mut c, "carol.near", w, "carol.near", 10_000, None);
    assert!(finish_mint(&mut c, &t));
    let _ = pay_work_msg(&mut c, w, 1_000_000);
    // A pending share cannot be contested; the registrant rejects it instead.
    call(&acc("alice.near"), NearToken::from_near(1));
    assert_panics("pending approval", || {
        drop(c.register_share(
            w,
            RightType::Publishing,
            "WW".into(),
            10_000,
            Basis::Copyright,
            acc("alice.near"),
            h('e'),
            Some(vec![t.clone()]),
        ))
    });
    yocto(&registrar());
    assert_panics("must give a doc_hash", || c.reject_share(t.clone(), None));
    yocto(&registrar());
    c.reject_share(t.clone(), Some(h('6')));
    assert!(get_logs().join("\n").contains("share_rejected"));
    assert_eq!(c.get_share(t.clone()).unwrap().status, ShareStatus::Void);
    assert_eq!(c.get_share_approval(t.clone()), "rejected");
    let g = group_of(&c, w);
    assert_eq!((g.active_bps, g.unallocated.0), (0, 1_000_000));
    assert!(g.shares.is_empty());
    let acct = c.get_accounting();
    assert_eq!(
        (acct.escrow_total.0, acct.unallocated_total.0),
        (0, 1_000_000)
    );
    yocto(&acc("alice.near"));
    assert_panics("not pending approval", || c.approve_share(t.clone(), None));
    call(&acc("carol.near"), NearToken::from_yoctonear(0));
    assert_panics("not approved", || drop(c.sync(t.clone())));
    // The real owner registers; the registrar releases the held money.
    let real = mint(&mut c, w, "alice.near", 10_000);
    assert_eq!(c.get_share_approval(real.clone()), "approved");
    let key = GroupKey {
        work_id: w,
        right_type: RightType::Publishing,
        territory: "WW".into(),
    };
    yocto(&registrar());
    assert_panics("not active", || {
        c.release_unallocated(key.clone(), t.clone(), U128(1), h('5'))
    });
    yocto(&registrar());
    c.release_unallocated(key, real.clone(), U128(1_000_000), h('5'));
    settle(&mut c, &real, 1_000_000);
    assert_eq!(ledger(&c, &real).known_balance.0, 1_000_000);
}

/// M2: shares registered by the work registrant, and every share that
/// existed before the rule (no approval key), are approved.
#[test]
fn m2_registrant_shares_and_old_shares_are_approved() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    assert_eq!(c.get_share_approval(t.clone()), "approved");
    assert!(env::storage_read(b"apv:s1").is_none(), "no key, no state");
    let extra = c.nft_token(t).unwrap().metadata.unwrap().extra.unwrap();
    assert!(extra.contains("\"approval\":\"approved\""));
}

/// M2: a plain deposit sent straight into a rejected share's vault can be
/// sent back by the registrar, and only for a rejected share.
#[test]
fn m2_registrar_recovers_plain_deposits_of_a_rejected_vault() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let ok_share = mint(&mut c, w, "bob.near", 5000);
    let t = register_as(&mut c, "carol.near", w, "carol.near", 5000, None);
    assert!(finish_mint(&mut c, &t));
    yocto(&registrar());
    assert_panics("only a rejected share", || {
        drop(c.recover_rejected_vault(ok_share.clone(), acc("payer.near"), U128(10), h('4')))
    });
    yocto(&acc("alice.near"));
    c.reject_share(t.clone(), None);
    yocto(&acc("alice.near"));
    assert_panics("registrar only", || {
        drop(c.recover_rejected_vault(t.clone(), acc("payer.near"), U128(10), h('4')))
    });
    yocto(&registrar());
    assert_panics("cannot send vault money to itself", || {
        drop(c.recover_rejected_vault(t.clone(), registrar(), U128(10), h('4')))
    });
    yocto(&registrar());
    let _ = c.recover_rejected_vault(t.clone(), acc("payer.near"), U128(10), h('4'));
    callback(ok(b""));
    assert!(c.on_rejected_vault_recovered(t.clone(), acc("payer.near"), U128(10), h('4')));
    assert!(get_logs().join("\n").contains("rejected_vault_recovered"));
}

/// M2: voiding a work moves money held for a pending share to the group's
/// unallocated escrow (then L9 lets the registrar refund it).
#[test]
fn m2_void_work_moves_held_money_to_unallocated() {
    let mut c = setup();
    let w1 = work(&mut c, "USRC12600001");
    let w2 = work(&mut c, "USRC12600001");
    let t = register_as(&mut c, "carol.near", w1, "carol.near", 10_000, None);
    assert!(finish_mint(&mut c, &t));
    let _ = pay_work_msg(&mut c, w1, 50_000);
    assert_eq!(ledger(&c, &t).escrowed.0, 50_000);
    let id = match c.get_work(w2).unwrap().status {
        WorkStatus::DuplicateOpen { dispute_id, .. } => dispute_id,
        s => panic!("{s:?}"),
    };
    resolve(&mut c, id, Outcome::SwapPayable);
    assert_eq!(c.get_share_approval(t.clone()), "rejected");
    assert_eq!(group_of(&c, w1).unallocated.0, 50_000);
    let acct = c.get_accounting();
    assert_eq!((acct.escrow_total.0, acct.unallocated_total.0), (0, 50_000));
}

// ---------- L5, L6 ----------

/// L5: payer_confirmed ends when the payer role is revoked, and a payer
/// account cannot also be an agent.
#[test]
fn l5_attestation_ends_with_the_payer_role() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&registrar());
    c.grant_payer_role(acc("payer.near"), h('1'));
    call(&acc("payer.near"), NearToken::from_millinear(10));
    c.attest_share(t.clone(), h('f'));
    assert_eq!(c.get_evidence_tier(t.clone()), "payer_confirmed");
    yocto(&registrar());
    c.revoke_payer_role(acc("payer.near"), h('2'));
    assert_eq!(c.get_evidence_tier(t), "self_asserted");
    yocto(&registrar());
    c.grant_payer_role(agent(), h('3'));
    assert_panics("payer accounts cannot be agents", || {
        grant(&mut c, 100, 100, 1)
    });
}

/// L6: a holder suspended between registration and mint gets no NFT; the
/// share fails and its bps are released.
#[test]
fn l6_suspended_holder_is_not_minted() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = register_as(&mut c, "alice.near", w, "carol.near", 5000, None);
    yocto(&registrar());
    c.suspend_credential(acc("carol.near"), h('9'));
    assert!(!finish_mint(&mut c, &t));
    assert_eq!(c.owner_of(&t), None);
    assert_eq!(c.get_share(t).unwrap().status, ShareStatus::Failed);
    let g = group_of(&c, w);
    assert_eq!((g.active_bps, g.reserved_bps), (0, 0));
}

// ---------- L8, L9 ----------

/// L8: a share payment below the group minimum is refused (refunded).
#[test]
fn l8_share_payment_has_a_minimum() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    call(&usdc(), NearToken::from_yoctonear(0));
    assert!(matches!(
        c.ft_on_transfer(acc("payer.near"), U128(MIN_GROUP_PAYMENT - 1), msg.to_string()),
        PromiseOrValue::Value(U128(v)) if v == MIN_GROUP_PAYMENT - 1
    ));
    assert!(get_logs()
        .join("\n")
        .contains("below minimum share payment"));
    call(&usdc(), NearToken::from_yoctonear(0));
    assert!(matches!(
        c.ft_on_transfer(acc("payer.near"), U128(MIN_GROUP_PAYMENT), msg.to_string()),
        PromiseOrValue::Value(U128(0))
    ));
}

/// L9: unallocated escrow of a voided work is no longer stranded: the
/// registrar can send it back out, and a failed transfer restores it.
#[test]
fn l9_registrar_refunds_unallocated_of_a_void_work() {
    let mut c = setup();
    let w1 = work(&mut c, "USRC12600001");
    let t1 = mint(&mut c, w1, "bob.near", 6000);
    let _ = pay_work_msg(&mut c, w1, 1_000_000);
    settle(&mut c, &t1, 600_000);
    let key = GroupKey {
        work_id: w1,
        right_type: RightType::Publishing,
        territory: "WW".into(),
    };
    yocto(&registrar());
    assert_panics("work is not void", || {
        drop(c.refund_unallocated(key.clone(), acc("payer.near"), U128(1), h('5')))
    });
    let w2 = work(&mut c, "USRC12600001");
    let id = match c.get_work(w2).unwrap().status {
        WorkStatus::DuplicateOpen { dispute_id, .. } => dispute_id,
        s => panic!("{s:?}"),
    };
    resolve(&mut c, id, Outcome::SwapPayable);
    yocto(&acc("alice.near"));
    assert_panics("registrar only", || {
        drop(c.refund_unallocated(key.clone(), acc("payer.near"), U128(1), h('5')))
    });
    yocto(&registrar());
    assert_panics("cannot send escrow to itself", || {
        drop(c.refund_unallocated(key.clone(), registrar(), U128(1), h('5')))
    });
    yocto(&registrar());
    assert_panics("exceeds unallocated", || {
        drop(c.refund_unallocated(key.clone(), acc("payer.near"), U128(400_001), h('5')))
    });
    // A failed transfer restores the escrow.
    yocto(&registrar());
    let _ = c.refund_unallocated(key.clone(), acc("payer.near"), U128(400_000), h('5'));
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, 400_000);
    callback(PromiseResult::Failed);
    assert!(!c.on_unallocated_refunded(key.clone(), acc("payer.near"), U128(400_000), h('5')));
    assert_eq!(group_of(&c, w1).unallocated.0, 400_000);
    yocto(&registrar());
    let _ = c.refund_unallocated(key.clone(), acc("payer.near"), U128(400_000), h('5'));
    callback(ok(b""));
    assert!(c.on_unallocated_refunded(key, acc("payer.near"), U128(400_000), h('5')));
    let acct = c.get_accounting();
    assert_eq!(
        (acct.unallocated_total.0, acct.forwards_in_flight_total.0),
        (0, 0)
    );
    assert!(get_logs().join("\n").contains("unallocated_refunded"));
}

// ---------- M1 ----------

/// M1: a share whose mint callbacks never committed no longer blocks its
/// group forever: after STALE_MINT_NS anyone fails it, and a late
/// callback mints nothing.
#[test]
fn m1_stale_mint_is_released_and_late_callbacks_mint_nothing() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _a = mint(&mut c, w, "bob.near", 5000);
    let p = register_as(&mut c, "alice.near", w, "carol.near", 5000, None);
    callback(ok(b""));
    let _ = c.on_vault_created(p.clone(), acc("alice.near"));
    // on_vault_ready lost.
    let created = c.get_share(p.clone()).unwrap().created_at.0;
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        created + STALE_MINT_NS - 1,
        10,
    );
    assert_panics("not stale yet", || c.release_stale_mint(p.clone()));
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        created + STALE_MINT_NS,
        10,
    );
    c.release_stale_mint(p.clone());
    assert!(get_logs().join("\n").contains("mint_released_stale"));
    assert_eq!(c.get_share(p.clone()).unwrap().status, ShareStatus::Failed);
    assert_eq!(group_of(&c, w).reserved_bps, 0);
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        created + STALE_MINT_NS,
        10,
    );
    assert_panics("not minting", || c.release_stale_mint(p.clone()));
    // The lost callback arrives late: nothing minted.
    callback(ok(
        b"{\"total\":\"1250000000000000000000\",\"available\":\"0\"}",
    ));
    assert!(!c.on_vault_ready(p.clone(), acc("alice.near")));
    assert!(get_logs().join("\n").contains("late_mint_callback"));
    assert_eq!(c.owner_of(&p), None);
    // The group is usable again.
    let _b = mint(&mut c, w, "carol.near", 5000);
    assert_eq!(group_of(&c, w).active_bps, 10_000);
}

// ---------- L2 ----------

/// L2: the daily limit covers at least 24 full hours. The PoC spent the
/// limit twice 23 hours and 1 minute apart; now the second payment waits
/// until the first payment's hour is more than 24 hours back.
#[test]
fn l2_rolling_limit_covers_a_full_24_hours() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    grant(&mut c, 100_000, 100_000, 10);
    let _ = fund(&mut c, "alice.near", 1_000_000);
    let minute = 60_000_000_000u64;
    let first = 10 * HOUR + 59 * minute;
    assert!(try_pay(&mut c, target(w), 100_000, "a", first));
    assert!(!try_pay(
        &mut c,
        target(w),
        MIN_GROUP_PAYMENT,
        "b",
        34 * HOUR
    ));
    assert!(!try_pay(
        &mut c,
        target(w),
        MIN_GROUP_PAYMENT,
        "c",
        34 * HOUR + 59 * minute
    ));
    at(
        &acc("x.near"),
        NearToken::from_yoctonear(0),
        34 * HOUR + minute,
        1,
    );
    assert_eq!(c.get_role(agent()).unwrap().spent_last_24h.0, 100_000);
    assert!(try_pay(&mut c, target(w), 100_000, "d", 35 * HOUR));
}

/// L2: the tail survives a second payment in the same hour, and the count
/// limit uses the same window.
#[test]
fn l2_tail_counts_within_the_same_hour_and_for_tx_counts() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    grant(&mut c, 100_000, 1_000_000, 2);
    let _ = fund(&mut c, "alice.near", 1_000_000);
    assert!(try_pay(&mut c, target(w), 50_000, "a", 10 * HOUR));
    assert!(try_pay(&mut c, target(w), 50_000, "b", 20 * HOUR));
    // Hour 34: hour 10 is the tail, hour 20 is in the window: 2 tx.
    assert!(!try_pay(&mut c, target(w), 50_000, "c", 34 * HOUR));
    // Hour 35: only hour 20 left.
    assert!(try_pay(&mut c, target(w), 50_000, "d", 35 * HOUR));
    assert!(!try_pay(&mut c, target(w), 50_000, "e", 35 * HOUR + 1));
}

// ---------- Review 03a ----------

/// Review 03a L5: approving a pending share before any payment releases
/// nothing, sends no transfer and leaves no forward counted in flight.
#[test]
fn r03a_l5_approve_before_any_payment_sends_nothing() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001"); // registered by alice
    let t = register_as(&mut c, "carol.near", w, "carol.near", 4000, None);
    assert!(finish_mint(&mut c, &t));
    assert_eq!(c.get_share_approval(t.clone()), "pending");
    // Very little gas: approve must not need the forward reservation.
    testing_env!(ctx(&acc("alice.near"), NearToken::from_yoctonear(1))
        .prepaid_gas(Gas::from_tgas(15))
        .build());
    c.approve_share(t.clone(), None);
    assert_eq!(c.get_share_approval(t.clone()), "approved");
    assert!(
        near_sdk::test_utils::get_created_receipts().is_empty(),
        "no ft_transfer for a zero amount"
    );
    let l = ledger(&c, &t);
    assert_eq!((l.escrowed.0, l.inbound_pending), (0, 0));
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, 0);
    assert!(get_logs().join("\n").contains("\"released\":\"0\""));
}

/// Review 03a L4 and audit F1: each forward reserves GAS_FT_TRANSFER +
/// GAS_FORWARD_CB (5 + 10 Tgas) plus GAS_FORWARD_BASE (10 Tgas) once.
/// With 100 Tgas prepaid (the agent calls in the deploy plan) 6 forwards
/// would need exactly 100 Tgas, so at most 5 fit once any gas is used
/// (sandbox t13b also pays 5). The 16-share maximum fits in what
/// a 300 Tgas payment leaves after PAYMENT_OVERHEAD_GAS.
#[test]
fn r03a_l4_forward_gas_reservation_per_share() {
    let per = GAS_FT_TRANSFER.as_gas() + GAS_FORWARD_CB.as_gas();
    assert_eq!(per, Gas::from_tgas(15).as_gas());
    testing_env!(ctx(&acc("x.near"), NearToken::from_yoctonear(0))
        .prepaid_gas(Gas::from_tgas(100))
        .build());
    assert!(Registry::has_gas_for_forwards(5));
    assert!(!Registry::has_gas_for_forwards(6));
    testing_env!(ctx(&acc("x.near"), NearToken::from_yoctonear(0))
        .prepaid_gas(Gas::from_gas(
            PAYMENT_TX_GAS.as_gas() - PAYMENT_OVERHEAD_GAS.as_gas()
        ))
        .build());
    assert!(Registry::has_gas_for_forwards(MAX_SHARES_PER_GROUP));
}
