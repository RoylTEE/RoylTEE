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
        c.ft_on_transfer(acc("payer.near"), U128(5_000), msg.to_string()),
        PromiseOrValue::Value(U128(0))
    ));
    assert_eq!(ledger(&c, &t).escrowed.0, 1_005_000);
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
    settle(&mut c, &t, 1_005_000);
    yocto(&acc("carol.near"));
    let _ = c.claim(t.clone());
    assert_eq!(ledger(&c, &t).total_paid_out.0, 1_005_000);
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
