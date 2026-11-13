//! Unit tests. Callbacks are driven by hand with mocked promise results.

use super::*;
use near_contract_standards::non_fungible_token::metadata::NFT_METADATA_SPEC;
use near_sdk::test_utils::{get_logs, VMContextBuilder};
use near_sdk::{testing_env, PromiseOrValue, PromiseResult, RuntimeFeesConfig};

fn acc(s: &str) -> AccountId {
    s.parse().unwrap()
}
fn reg() -> AccountId {
    acc("reg.near")
}
fn usdc() -> AccountId {
    acc("usdc.near")
}
fn registrar() -> AccountId {
    acc("registrar.near")
}
fn h(c: char) -> String {
    c.to_string().repeat(64)
}

fn ctx(predecessor: &AccountId, deposit: NearToken) -> VMContextBuilder {
    let mut b = VMContextBuilder::new();
    b.current_account_id(reg())
        .predecessor_account_id(predecessor.clone())
        .signer_account_id(predecessor.clone())
        .attached_deposit(deposit)
        .prepaid_gas(Gas::from_tgas(300))
        .account_balance(NearToken::from_near(100));
    b
}

fn call(predecessor: &AccountId, deposit: NearToken) {
    testing_env!(ctx(predecessor, deposit).build());
}

fn yocto(predecessor: &AccountId) {
    call(predecessor, NearToken::from_yoctonear(1));
}

fn callback(result: PromiseResult) {
    testing_env!(
        ctx(&reg(), NearToken::from_yoctonear(0)).build(),
        near_sdk::test_vm_config(),
        RuntimeFeesConfig::test(),
        Default::default(),
        vec![result],
    );
}

fn ok(bytes: &[u8]) -> PromiseResult {
    PromiseResult::Successful(bytes.to_vec())
}

fn setup() -> Registry {
    call(&registrar(), NearToken::from_yoctonear(0));
    let mut c = Registry::new(
        registrar(),
        usdc(),
        NFTContractMetadata {
            spec: NFT_METADATA_SPEC.to_string(),
            name: "test".to_string(),
            symbol: "TST".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        },
    );
    for m in ["alice.near", "bob.near", "carol.near"] {
        call(&registrar(), NearToken::from_near(1));
        let _ = c.grant_credential(acc(m), h('a'), h('b'), h('c'));
    }
    c
}

fn work(c: &mut Registry, code: &str) -> WorkId {
    call(&acc("alice.near"), NearToken::from_near(1));
    c.register_work(
        CodeType::Isrc,
        code.to_string(),
        h('d'),
        Provenance::Human,
        String::new(),
    )
}

/// Registers and fully mints one share; returns its token id.
fn mint(c: &mut Registry, work_id: WorkId, holder: &str, bps: Bps) -> TokenId {
    call(&acc("alice.near"), NearToken::from_near(1));
    let _ = c.register_share(
        work_id,
        RightType::Publishing,
        "WW".into(),
        bps,
        Basis::Copyright,
        acc(holder),
        h('e'),
    );
    let token_id = format!("s{}", c.next_share);
    callback(ok(b""));
    let _ = c.on_vault_created(token_id.clone(), acc("alice.near"));
    callback(ok(
        b"{\"total\":\"1250000000000000000000\",\"available\":\"0\"}",
    ));
    assert!(c.on_vault_ready(token_id.clone(), acc("alice.near")));
    token_id
}

/// Routes a direct payment to one share and settles the forward.
fn pay_share(c: &mut Registry, token_id: &TokenId, amount: u128) {
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg = json!({ "pay_share": { "token_id": token_id, "period": "2026-Q3", "statement_hash": h('f') } });
    let r = c.ft_on_transfer(acc("payer.near"), U128(amount), msg.to_string());
    assert!(matches!(r, PromiseOrValue::Value(U128(0))));
    callback(ok(b""));
    assert!(c.on_forward(token_id.clone(), U128(amount)));
}

fn ledger(c: &Registry, t: &TokenId) -> LedgerView {
    c.get_ledger(t.clone()).unwrap()
}

#[test]
fn mint_creates_active_share_owned_by_holder() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    assert_eq!(c.owner_of(&t), Some(acc("bob.near")));
    let s = c.get_share(t.clone()).unwrap();
    assert_eq!(s.status, ShareStatus::Active);
    assert_eq!(s.vault, acc("s1.reg.near"));
    let g = c.get_group(w, RightType::Publishing, "WW".into()).unwrap();
    assert_eq!((g.active_bps, g.reserved_bps), (10_000, 0));
}

#[test]
fn failed_vault_creation_mints_nothing_and_releases_bps() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    call(&acc("alice.near"), NearToken::from_near(1));
    let _ = c.register_share(
        w,
        RightType::Publishing,
        "WW".into(),
        5000,
        Basis::Copyright,
        acc("bob.near"),
        h('e'),
    );
    callback(PromiseResult::Failed);
    let r = c.on_vault_created("s1".into(), acc("alice.near"));
    assert!(matches!(r, PromiseOrValue::Value(false)));
    assert_eq!(
        c.get_share("s1".into()).unwrap().status,
        ShareStatus::Failed
    );
    assert!(c.nft_token("s1".into()).is_none());
    let g = c.get_group(w, RightType::Publishing, "WW".into()).unwrap();
    assert_eq!((g.active_bps, g.reserved_bps), (0, 0));
}

#[test]
fn failed_token_registration_mints_nothing() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    call(&acc("alice.near"), NearToken::from_near(1));
    let _ = c.register_share(
        w,
        RightType::Publishing,
        "WW".into(),
        5000,
        Basis::Copyright,
        acc("bob.near"),
        h('e'),
    );
    callback(ok(b""));
    let _ = c.on_vault_created("s1".into(), acc("alice.near"));
    callback(PromiseResult::Failed);
    assert!(!c.on_vault_ready("s1".into(), acc("alice.near")));
    assert_eq!(
        c.get_share("s1".into()).unwrap().status,
        ShareStatus::Failed
    );
    assert_eq!(c.nft_total_supply().0, 0);
}

#[test]
#[should_panic(expected = "would exceed 100 percent")]
fn over_allocation_is_refused_in_block_2() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    mint(&mut c, w, "bob.near", 6000);
    call(&acc("alice.near"), NearToken::from_near(1));
    let _ = c.register_share(
        w,
        RightType::Publishing,
        "WW".into(),
        5000,
        Basis::Copyright,
        acc("bob.near"),
        h('e'),
    );
}

#[test]
#[should_panic(expected = "holder has no active credential")]
fn cannot_mint_to_outsider() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    mint(&mut c, w, "outsider.near", 10_000);
}

#[test]
#[should_panic(expected = "attach at least")]
fn register_share_needs_deposit() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    call(&acc("alice.near"), NearToken::from_millinear(1));
    let _ = c.register_share(
        w,
        RightType::Publishing,
        "WW".into(),
        5000,
        Basis::Copyright,
        acc("bob.near"),
        h('e'),
    );
}

#[test]
fn duplicate_code_opens_dispute_and_payments_are_refused() {
    let mut c = setup();
    let w1 = work(&mut c, "USRC12600001");
    let w2 = work(&mut c, "us-rc1-26-00001");
    assert_eq!(c.get_work(w1).unwrap().status, WorkStatus::Active);
    assert!(
        matches!(c.get_work(w2).unwrap().status, WorkStatus::DuplicateOpen { original, .. } if original == w1)
    );
    let t = mint(&mut c, w2, "bob.near", 10_000);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    let r = c.ft_on_transfer(acc("payer.near"), U128(50_000), msg.to_string());
    assert!(matches!(r, PromiseOrValue::Value(U128(50_000))));
}

#[test]
fn claim_debits_before_call_and_settles() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    assert_eq!(ledger(&c, &t).claimable.0, 700_000);

    yocto(&acc("bob.near"));
    let _ = c.claim(t.clone());
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.total_paid_out.0, l.earned.0),
        (0, 700_000, 700_000)
    );
    assert!(l.payout_in_flight.is_some());

    callback(ok(b""));
    assert!(c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(0)));
    let l = ledger(&c, &t);
    assert!(l.payout_in_flight.is_none());
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
}

#[test]
fn failed_claim_rolls_back_exactly() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    yocto(&acc("bob.near"));
    let _ = c.claim(t.clone());
    callback(PromiseResult::Failed);
    assert!(!c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(0)));
    let l = ledger(&c, &t);
    assert!(l.payout_in_flight.is_none());
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (700_000, 0));
}

#[test]
#[should_panic(expected = "only the current holder can claim")]
fn non_owner_cannot_claim() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    yocto(&acc("alice.near"));
    let _ = c.claim(t);
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn claim_needs_one_yocto() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    let _ = c.claim(t);
}

#[test]
#[should_panic(expected = "payout already in flight")]
fn second_claim_while_in_flight_fails() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    yocto(&acc("bob.near"));
    let _ = c.claim(t.clone());
    yocto(&acc("bob.near"));
    let _ = c.claim(t);
}

#[test]
fn transfer_blocked_while_payout_in_flight_then_allowed() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    yocto(&acc("bob.near"));
    let _ = c.claim(t.clone());

    yocto(&acc("bob.near"));
    let blocked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.nft_transfer(acc("carol.near"), t.clone(), None, None, None)
    }));
    assert!(blocked.is_err());

    callback(ok(b""));
    c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(0));
    yocto(&acc("bob.near"));
    c.nft_transfer(acc("carol.near"), t.clone(), None, None, None);
    assert_eq!(c.owner_of(&t), Some(acc("carol.near")));
    // No double pay: the buyer has nothing to claim from the old payment.
    assert_eq!(ledger(&c, &t).claimable.0, 0);
}

#[test]
fn balance_travels_with_the_token() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 300_000);
    yocto(&acc("bob.near"));
    c.nft_transfer(acc("carol.near"), t.clone(), None, None, None);
    pay_share(&mut c, &t, 200_000);
    yocto(&acc("bob.near"));
    let seller = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = c.claim(t.clone());
    }));
    assert!(seller.is_err());
    yocto(&acc("carol.near"));
    let _ = c.claim(t.clone());
    assert_eq!(ledger(&c, &t).payout_in_flight.unwrap().amount.0, 500_000);
}

#[test]
#[should_panic(expected = "receiver has no active credential")]
fn transfer_to_outsider_fails() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&acc("bob.near"));
    c.nft_transfer(acc("outsider.near"), t, None, None, None);
}

#[test]
#[should_panic(expected = "only the current holder can transfer")]
fn non_owner_cannot_transfer() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&acc("alice.near"));
    c.nft_transfer(acc("carol.near"), t, None, None, None);
}

#[test]
#[should_panic(expected = "approvals are disabled")]
fn nft_approve_panics() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("bob.near"), NearToken::from_millinear(10));
    let _ = c.nft_approve(t, acc("carol.near"), None);
}

#[test]
#[should_panic(expected = "approvals are disabled")]
fn transfer_with_approval_id_panics() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&acc("bob.near"));
    c.nft_transfer(acc("carol.near"), t, Some(0), None, None);
}

#[test]
fn transfer_call_lock_blocks_claim_and_resolve_returns_token() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 400_000);
    yocto(&acc("bob.near"));
    let _ = c.nft_transfer_call(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None,
    );
    assert_eq!(c.owner_of(&t), Some(acc("carol.near")));
    assert!(ledger(&c, &t).transfer_pending.is_some());

    for attempt in 0..3 {
        yocto(&acc("carol.near"));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match attempt {
            0 => drop(c.claim(t.clone())),
            1 => c.nft_transfer(acc("alice.near"), t.clone(), None, None, None),
            _ => drop(c.sync(t.clone())),
        }));
        assert!(
            r.is_err(),
            "attempt {attempt} must fail while transfer is pending"
        );
    }

    callback(ok(b"true"));
    assert!(!c.nft_resolve_transfer(
        acc("bob.near"),
        acc("carol.near"),
        t.clone(),
        None,
        Some(U64(0))
    ));
    assert_eq!(c.owner_of(&t), Some(acc("bob.near")));
    let l = ledger(&c, &t);
    assert!(l.transfer_pending.is_none());
    assert_eq!(l.known_balance.0, 400_000);
}

#[test]
fn resolve_keeps_token_when_receiver_returns_false() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&acc("bob.near"));
    let _ = c.nft_transfer_call(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None,
    );
    callback(ok(b"false"));
    assert!(c.nft_resolve_transfer(
        acc("bob.near"),
        acc("carol.near"),
        t.clone(),
        None,
        Some(U64(0))
    ));
    assert_eq!(c.owner_of(&t), Some(acc("carol.near")));
}

#[test]
fn resolve_returns_token_when_receiver_fails() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&acc("bob.near"));
    let _ = c.nft_transfer_call(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None,
    );
    callback(PromiseResult::Failed);
    assert!(!c.nft_resolve_transfer(
        acc("bob.near"),
        acc("carol.near"),
        t.clone(),
        None,
        Some(U64(0))
    ));
    assert_eq!(c.owner_of(&t), Some(acc("bob.near")));
}

#[test]
fn work_payment_splits_exactly_and_remainder_goes_to_largest() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let a = mint(&mut c, w, "alice.near", 3333);
    let b = mint(&mut c, w, "bob.near", 3334);
    let d = mint(&mut c, w, "carol.near", 3333);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg = json!({ "pay_work": { "work_id": w, "right_type": "publishing", "territory": "WW",
        "period": "2026-Q3", "statement_hash": h('f'), "statement_line_id": "line-1" } });
    let amount = 1_000_007u128;
    let r = c.ft_on_transfer(acc("payer.near"), U128(amount), msg.to_string());
    assert!(matches!(r, PromiseOrValue::Value(U128(0))));
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, amount);
    let log = get_logs()
        .into_iter()
        .find(|l| l.contains("payment_in"))
        .unwrap();
    let v: Value = serde_json::from_str(log.trim_start_matches("EVENT_JSON:")).unwrap();
    let credits: Vec<(String, u128)> = v["data"][0]["credits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["token_id"].as_str().unwrap().to_string(),
                x["amount"].as_str().unwrap().parse().unwrap(),
            )
        })
        .collect();
    assert_eq!(credits.iter().map(|x| x.1).sum::<u128>(), amount);
    // floor: 333302, 333402, 333302 = 1000006; remainder 1 to the 3334 share.
    assert_eq!(
        credits,
        vec![(a, 333_302), (b.clone(), 333_403), (d, 333_302)]
    );
    assert_eq!(ledger(&c, &b).inbound_pending, 1);
}

#[test]
fn refused_payments_return_the_full_amount() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    mint(&mut c, w, "alice.near", 5000);
    let pay = |c: &mut Registry, from: &AccountId, amount: u128, msg: String| {
        call(from, NearToken::from_yoctonear(0));
        match c.ft_on_transfer(acc("payer.near"), U128(amount), msg) {
            PromiseOrValue::Value(v) => v.0,
            PromiseOrValue::Promise(_) => panic!("unexpected promise"),
        }
    };
    let good = json!({ "pay_work": { "work_id": w, "right_type": "publishing", "territory": "WW",
        "period": "2026-Q3", "statement_hash": h('f') } })
    .to_string();
    // Unknown group: refused.
    let other = json!({ "pay_work": { "work_id": w, "right_type": "master", "territory": "WW",
        "period": "2026-Q3", "statement_hash": h('f') } })
    .to_string();
    assert_eq!(pay(&mut c, &usdc(), 50_000, other), 50_000);
    // Wrong token contract (T18).
    assert_eq!(pay(&mut c, &acc("fake.near"), 50_000, good), 50_000);
    // Malformed message.
    assert_eq!(
        pay(&mut c, &usdc(), 50_000, "{\"pay_work\":1}".into()),
        50_000
    );
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, 0);
}

fn pay_group(c: &mut Registry, w: WorkId, amount: u128) -> PromiseOrValue<U128> {
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg = json!({ "pay_work": { "work_id": w, "right_type": "publishing", "territory": "WW",
        "period": "2026-Q3", "statement_hash": h('f') } });
    c.ft_on_transfer(acc("payer.near"), U128(amount), msg.to_string())
}

#[test]
fn under_allocated_group_pays_exact_bps_and_escrows_the_rest() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let a = mint(&mut c, w, "alice.near", 5000);
    let b = mint(&mut c, w, "bob.near", 2500);
    assert!(matches!(
        pay_group(&mut c, w, 1_000_003),
        PromiseOrValue::Value(U128(0))
    ));
    let group = c.get_group(w, RightType::Publishing, "WW".into()).unwrap();
    // 500001 + 250000 credited; 250002 unassigned (25 percent plus dust).
    assert_eq!(group.unallocated.0, 250_002);
    assert_eq!(c.get_accounting().unallocated_total.0, 250_002);
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, 750_001);
    for (t, amt) in [(&a, 500_001u128), (&b, 250_000)] {
        callback(ok(b""));
        c.on_forward(t.clone(), U128(amt));
    }
    assert_eq!(ledger(&c, &a).known_balance.0, 500_001);
    assert_eq!(ledger(&c, &b).known_balance.0, 250_000);

    // The missing writer registers; the registrar releases the escrow to them.
    let d = mint(&mut c, w, "carol.near", 2500);
    let key = GroupKey {
        work_id: w,
        right_type: RightType::Publishing,
        territory: "WW".into(),
    };
    yocto(&acc("alice.near"));
    assert!(
        panics(|| c.release_unallocated(key.clone(), d.clone(), U128(1), h('6'))),
        "registrar only"
    );
    yocto(&registrar());
    assert!(
        panics(|| c.release_unallocated(key.clone(), d.clone(), U128(250_003), h('6'))),
        "over escrow"
    );
    yocto(&registrar());
    c.release_unallocated(key.clone(), d.clone(), U128(250_002), h('6'));
    assert!(get_logs()
        .iter()
        .any(|l| l.contains("unallocated_released") && l.contains(&h('6'))));
    assert_eq!(
        c.get_group(w, RightType::Publishing, "WW".into())
            .unwrap()
            .unallocated
            .0,
        0
    );
    assert_eq!(c.get_accounting().unallocated_total.0, 0);
    callback(ok(b""));
    c.on_forward(d.clone(), U128(250_002));
    assert_eq!(ledger(&c, &d).known_balance.0, 250_002);

    // Now full: the next payment leaves nothing unallocated.
    assert!(matches!(
        pay_group(&mut c, w, 1_000_000),
        PromiseOrValue::Value(U128(0))
    ));
    assert_eq!(
        c.get_group(w, RightType::Publishing, "WW".into())
            .unwrap()
            .unallocated
            .0,
        0
    );
}

#[test]
#[should_panic(expected = "share is not in this group")]
fn unallocated_release_must_stay_in_its_group() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    mint(&mut c, w, "alice.near", 5000);
    let _ = pay_group(&mut c, w, 1_000_000);
    let w2 = work(&mut c, "USRC12600002");
    let other = mint(&mut c, w2, "bob.near", 10_000);
    let key = GroupKey {
        work_id: w,
        right_type: RightType::Publishing,
        territory: "WW".into(),
    };
    yocto(&registrar());
    c.release_unallocated(key, other, U128(1), h('6'));
}

#[test]
fn below_minimum_group_payment_is_refused() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    mint(&mut c, w, "alice.near", 10_000);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg = json!({ "pay_work": { "work_id": w, "right_type": "publishing", "territory": "WW",
        "period": "2026-Q3", "statement_hash": h('f') } });
    let r = c.ft_on_transfer(acc("payer.near"), U128(9_999), msg.to_string());
    assert!(matches!(r, PromiseOrValue::Value(U128(9_999))));
}

#[test]
fn failed_forward_goes_to_undelivered() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "alice.near", 10_000);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    let _ = c.ft_on_transfer(acc("payer.near"), U128(50_000), msg.to_string());
    callback(PromiseResult::Failed);
    assert!(!c.on_forward(t.clone(), U128(50_000)));
    let l = ledger(&c, &t);
    assert_eq!(
        (l.known_balance.0, l.undelivered.0, l.inbound_pending),
        (0, 50_000, 0)
    );
    assert_eq!(c.get_accounting().undelivered_total.0, 50_000);
}

#[test]
fn sync_records_plain_transfers_and_discards_stale_reads() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "alice.near", 10_000);
    call(&acc("anyone.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t.clone());
    let epoch = ledger(&c, &t).epoch;
    callback(ok(b"\"250000\""));
    assert_eq!(c.on_sync(t.clone(), epoch), Some(U128(250_000)));
    assert_eq!(ledger(&c, &t).earned.0, 250_000);

    // A read that began before a ledger change is discarded.
    let stale = ledger(&c, &t).epoch;
    pay_share(&mut c, &t, 10_000);
    callback(ok(b"\"999999\""));
    assert_eq!(c.on_sync(t.clone(), stale), None);
    assert_eq!(ledger(&c, &t).known_balance.0, 260_000);
}

#[test]
#[should_panic(expected = "payment forward in flight")]
fn sync_refused_during_forward() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "alice.near", 10_000);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    let _ = c.ft_on_transfer(acc("payer.near"), U128(50_000), msg.to_string());
    call(&acc("anyone.near"), NearToken::from_yoctonear(0));
    let _ = c.sync(t);
}

fn panics(f: impl FnOnce()) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err()
}

#[test]
fn suspended_holder_accrues_but_cannot_claim_send_or_receive() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 5000);
    let t2 = mint(&mut c, w, "carol.near", 5000);
    yocto(&registrar());
    c.suspend_credential(acc("bob.near"), h('9'));

    // Money keeps arriving.
    pay_share(&mut c, &t, 100_000);
    assert_eq!(ledger(&c, &t).known_balance.0, 100_000);
    assert_eq!(c.owner_of(&t), Some(acc("bob.near")));

    yocto(&acc("bob.near"));
    assert!(
        panics(|| drop(c.claim(t.clone()))),
        "suspended holder cannot claim"
    );
    yocto(&acc("bob.near"));
    assert!(
        panics(|| c.nft_transfer(acc("alice.near"), t.clone(), None, None, None)),
        "cannot send"
    );
    yocto(&acc("bob.near"));
    assert!(panics(|| drop(c.nft_transfer_call(
        acc("alice.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None
    ))));
    yocto(&acc("carol.near"));
    assert!(
        panics(|| c.nft_transfer(acc("bob.near"), t2.clone(), None, None, None)),
        "cannot receive"
    );

    // Only the registrar can move it, with a document hash and an event.
    yocto(&acc("alice.near"));
    assert!(panics(|| c.registrar_transfer(
        t.clone(),
        acc("alice.near"),
        h('8')
    )));
    yocto(&registrar());
    assert!(panics(|| c.registrar_transfer(
        t.clone(),
        acc("outsider.near"),
        h('8')
    )));
    yocto(&registrar());
    c.registrar_transfer(t.clone(), acc("alice.near"), h('8'));
    assert_eq!(c.owner_of(&t), Some(acc("alice.near")));
    assert!(get_logs()
        .iter()
        .any(|l| l.contains("\"registrar_transfer\"") && l.contains(&h('8'))));
    // The balance travelled with the token.
    yocto(&acc("alice.near"));
    let _ = c.claim(t.clone());
    assert_eq!(ledger(&c, &t).payout_in_flight.unwrap().amount.0, 100_000);
}

#[test]
#[should_panic(expected = "registrar can only move shares of suspended holders")]
fn registrar_cannot_move_active_holders_share() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    yocto(&registrar());
    c.registrar_transfer(t, acc("alice.near"), h('8'));
}

#[test]
fn reinstated_holder_can_claim_again() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 50_000);
    yocto(&registrar());
    c.suspend_credential(acc("bob.near"), h('9'));
    yocto(&registrar());
    c.reinstate_credential(acc("bob.near"), h('7'));
    yocto(&acc("bob.near"));
    let _ = c.claim(t.clone());
    assert!(ledger(&c, &t).payout_in_flight.is_some());
}

#[test]
fn effective_from_bounds() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    let far = U64(env::block_timestamp() + MAX_EFFECTIVE_FUTURE_NS + 1);
    yocto(&acc("bob.near"));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.nft_transfer(acc("carol.near"), t.clone(), None, None, Some(far))
    }));
    assert!(r.is_err());
    yocto(&acc("bob.near"));
    c.nft_transfer(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        Some(U64(env::block_timestamp() + 5)),
    );
    assert_eq!(
        c.get_share(t).unwrap().last_effective_from.0,
        env::block_timestamp() + 5
    );
}

#[test]
fn nft_token_carries_live_fields() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 123_456);
    let tok = c.nft_token(t).unwrap();
    let extra: Value =
        serde_json::from_str(tok.metadata.unwrap().extra.as_deref().unwrap()).unwrap();
    assert_eq!(extra["earned"], "123456");
    assert_eq!(extra["claimable"], "123456");
    assert_eq!(extra["paid_out"], "0");
    assert_eq!(extra["provenance"], "human");
    assert_eq!(extra["vault"], "s1.reg.near");
}

#[test]
fn system_accounts_cannot_hold_credentials() {
    let mut c = setup();
    for a in ["reg.near", "s1.reg.near", "usdc.near"] {
        call(&registrar(), NearToken::from_near(1));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(c.grant_credential(acc(a), h('a'), h('b'), h('c')))
        }));
        assert!(r.is_err(), "{a} must be refused");
    }
}

#[test]
#[should_panic(expected = "registrar only")]
fn only_registrar_grants_credentials() {
    let mut c = setup();
    call(&acc("alice.near"), NearToken::from_near(1));
    let _ = c.grant_credential(acc("dave.near"), h('a'), h('b'), h('c'));
}

#[test]
fn failed_member_registration_removes_credential() {
    let mut c = setup();
    call(&registrar(), NearToken::from_near(1));
    let _ = c.grant_credential(acc("dave.near"), h('a'), h('b'), h('c'));
    assert!(c.is_active_member(acc("dave.near")));
    callback(PromiseResult::Failed);
    assert!(!c.on_member_registered(acc("dave.near")));
    assert!(!c.is_active_member(acc("dave.near")));
}

// ---------- block 3: agents, allowances, holder note, payer role, locks ----------

const FAR: u64 = 4_000_000_000_000_000_000;
const HOUR: u64 = 3_600_000_000_000;

fn agent() -> AccountId {
    acc("pay.alice.near")
}

fn at(predecessor: &AccountId, deposit: NearToken, ts: u64, height: u64) {
    testing_env!(ctx(predecessor, deposit)
        .block_timestamp(ts)
        .block_height(height)
        .build());
}

fn grant(c: &mut Registry, per_tx: u128, daily: u128, max_tx: u32) {
    call(&acc("alice.near"), NearToken::from_millinear(100));
    c.grant_role(
        agent(),
        vec![AgentMethod::PayFromAllowance, AgentMethod::Report],
        U128(per_tx),
        U128(daily),
        max_tx,
        U64(FAR),
    );
}

fn fund(c: &mut Registry, from: &str, amount: u128) -> PromiseOrValue<U128> {
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg = json!({ "fund_allowance": agent() });
    c.ft_on_transfer(acc(from), U128(amount), msg.to_string())
}

fn target(w: WorkId) -> PayTarget {
    PayTarget::Work {
        work_id: w,
        right_type: RightType::Publishing,
        territory: "WW".into(),
    }
}

fn pay_at(c: &mut Registry, t: PayTarget, amount: u128, line: &str, ts: u64) {
    at(&agent(), NearToken::from_yoctonear(0), ts, 0);
    c.pay_from_allowance(t, U128(amount), line.to_string(), "2026-Q3".into(), h('f'));
}

fn try_pay(c: &mut Registry, t: PayTarget, amount: u128, line: &str, ts: u64) -> bool {
    !panics(|| pay_at(c, t, amount, line, ts))
}

#[test]
fn grant_role_requires_direct_sub_account_and_credits_lines() {
    let mut c = setup();
    call(&acc("alice.near"), NearToken::from_millinear(100));
    assert!(panics(|| c.grant_role(
        acc("pay.bob.near"),
        vec![AgentMethod::PayFromAllowance],
        U128(1),
        U128(1),
        1,
        U64(FAR)
    )));
    call(&acc("alice.near"), NearToken::from_millinear(100));
    assert!(panics(|| c.grant_role(
        acc("a.b.alice.near"),
        vec![AgentMethod::PayFromAllowance],
        U128(1),
        U128(1),
        1,
        U64(FAR)
    )));
    call(&acc("alice.near"), NearToken::from_millinear(100));
    assert!(
        panics(|| c.grant_role(
            agent(),
            vec![AgentMethod::PayFromAllowance],
            U128(2),
            U128(1),
            1,
            U64(FAR)
        )),
        "per_tx above daily is refused"
    );
    call(&acc("outsider.near"), NearToken::from_millinear(100));
    assert!(
        panics(|| c.grant_role(
            acc("pay.outsider.near"),
            vec![AgentMethod::PayFromAllowance],
            U128(1),
            U128(1),
            1,
            U64(FAR)
        )),
        "non members cannot grant roles"
    );
    grant(&mut c, 100_000, 300_000, 5);
    let r = c.get_role(agent()).unwrap();
    assert!(
        r.line_credits > 50 && r.line_credits < 100,
        "{}",
        r.line_credits
    );
    assert_eq!(
        (r.org, r.allowance.0, r.revoked),
        (acc("alice.near"), 0, false)
    );
    // A second grant while active is refused.
    assert!(panics(|| grant(&mut c, 1, 1, 1)));
    // An agent cannot later receive a credential.
    call(&registrar(), NearToken::from_near(1));
    assert!(panics(|| drop(c.grant_credential(
        agent(),
        h('a'),
        h('b'),
        h('c')
    ))));
}

#[test]
fn only_the_org_can_fund_and_allowance_is_counted() {
    let mut c = setup();
    grant(&mut c, 100_000, 300_000, 5);
    assert!(matches!(
        fund(&mut c, "bob.near", 50_000),
        PromiseOrValue::Value(U128(50_000))
    ));
    assert!(matches!(
        fund(&mut c, "alice.near", 500_000),
        PromiseOrValue::Value(U128(0))
    ));
    assert_eq!(c.get_role(agent()).unwrap().allowance.0, 500_000);
    assert_eq!(c.get_accounting().allowance_total.0, 500_000);
}

#[test]
fn agent_pays_registered_shares_only_and_once_per_line() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t1 = mint(&mut c, w, "bob.near", 6000);
    let t2 = mint(&mut c, w, "carol.near", 4000);
    grant(&mut c, 100_000, 300_000, 5);
    let _ = fund(&mut c, "alice.near", 500_000);
    let credits = c.get_role(agent()).unwrap().line_credits;

    pay_at(&mut c, target(w), 100_000, "L-1", 0);
    let logs = get_logs().join("\n");
    assert!(logs.contains("\"source\":\"allowance\""));
    assert!(logs.contains("allowance_spent"));
    callback(ok(b""));
    c.on_forward(t1.clone(), U128(60_000));
    callback(ok(b""));
    c.on_forward(t2.clone(), U128(40_000));
    assert_eq!(ledger(&c, &t1).known_balance.0, 60_000);
    assert_eq!(ledger(&c, &t2).known_balance.0, 40_000);
    let r = c.get_role(agent()).unwrap();
    assert_eq!((r.allowance.0, r.line_credits), (400_000, credits - 1));
    assert_eq!(c.get_accounting().allowance_total.0, 400_000);
    assert!(c.is_statement_line_used(acc("alice.near"), "L-1".into()));

    // Same line again: refused, nothing moves.
    assert!(!try_pay(&mut c, target(w), 100_000, "L-1", 0));
    assert_eq!(c.get_role(agent()).unwrap().allowance.0, 400_000);
    // Pay one share directly by token id.
    assert!(try_pay(
        &mut c,
        PayTarget::Share { token_id: t2 },
        1,
        "L-2",
        0
    ));
}

#[test]
fn per_tx_daily_and_count_limits_with_rolling_window() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    grant(&mut c, 100_000, 250_000, 3);
    let _ = fund(&mut c, "alice.near", 1_000_000);
    let t0 = 10 * HOUR;

    assert!(!try_pay(&mut c, target(w), 100_001, "a", t0), "per tx");
    assert!(try_pay(&mut c, target(w), 100_000, "b", t0));
    assert!(try_pay(&mut c, target(w), 100_000, "c", t0 + HOUR));
    assert!(
        !try_pay(&mut c, target(w), 60_000, "d", t0 + 2 * HOUR),
        "daily"
    );
    assert!(try_pay(&mut c, target(w), 50_000, "e", t0 + 2 * HOUR));
    assert!(
        !try_pay(&mut c, target(w), 10_000, "f", t0 + 3 * HOUR),
        "daily or count"
    );
    // 24 hours after the first payment its hour drops out of the window.
    assert!(try_pay(&mut c, target(w), 100_000, "g", t0 + 24 * HOUR));
    let r = c.get_role(agent()).unwrap();
    assert_eq!((r.spent_last_24h.0, r.tx_last_24h), (250_000, 3));
    assert_eq!(r.allowance.0, 650_000);
}

#[test]
fn count_limit_alone_blocks() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    grant(&mut c, 100_000, 1_000_000, 2);
    let _ = fund(&mut c, "alice.near", 1_000_000);
    assert!(try_pay(&mut c, target(w), 10_000, "a", 0));
    assert!(try_pay(&mut c, target(w), 10_000, "b", 0));
    assert!(!try_pay(&mut c, target(w), 10_000, "c", 0));
}

#[test]
fn allowance_and_minimum_payment_are_enforced() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    grant(&mut c, 100_000, 1_000_000, 10);
    let _ = fund(&mut c, "alice.near", 50_000);
    assert!(
        !try_pay(&mut c, target(w), 60_000, "a", 0),
        "over allowance"
    );
    assert!(
        !try_pay(&mut c, target(w), MIN_GROUP_PAYMENT - 1, "b", 0),
        "below minimum"
    );
    assert!(!c.is_statement_line_used(acc("alice.near"), "b".into()));
    assert_eq!(c.get_role(agent()).unwrap().allowance.0, 50_000);
}

#[test]
fn revoked_expired_suspended_and_wrong_method_are_blocked() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("alice.near"), NearToken::from_millinear(100));
    c.grant_role(
        agent(),
        vec![AgentMethod::Report],
        U128(100_000),
        U128(100_000),
        5,
        U64(5 * HOUR),
    );
    let _ = fund(&mut c, "alice.near", 100_000);
    assert!(
        !try_pay(&mut c, target(w), 10_000, "a", 0),
        "method not allowed"
    );
    at(&agent(), NearToken::from_yoctonear(0), 0, 0);
    c.report("a".into(), h('1'));
    assert!(
        panics(|| {
            at(&agent(), NearToken::from_yoctonear(0), 5 * HOUR, 0);
            c.report("a".into(), h('1'));
        }),
        "expired"
    );

    // Re-grant after revoke keeps the allowance.
    yocto(&registrar());
    c.revoke_role(agent());
    assert!(matches!(
        fund(&mut c, "alice.near", 1),
        PromiseOrValue::Value(U128(1))
    ));
    grant(&mut c, 100_000, 100_000, 5);
    assert_eq!(c.get_role(agent()).unwrap().allowance.0, 100_000);
    assert!(try_pay(&mut c, target(w), 10_000, "b", 0));
    yocto(&acc("alice.near"));
    c.revoke_role(agent());
    assert!(!try_pay(&mut c, target(w), 10_000, "c", 0), "revoked");

    grant(&mut c, 100_000, 100_000, 5);
    yocto(&registrar());
    c.suspend_credential(acc("alice.near"), h('9'));
    assert!(!try_pay(&mut c, target(w), 10_000, "d", 0), "org suspended");
    yocto(&acc("alice.near"));
    assert!(panics(|| drop(c.withdraw_allowance(agent(), U128(1)))));
}

#[test]
fn agent_cannot_use_owner_methods() {
    let mut c = setup();
    grant(&mut c, 100_000, 100_000, 5);
    let _ = fund(&mut c, "alice.near", 100_000);
    yocto(&agent());
    assert!(panics(|| c.revoke_role(agent())));
    yocto(&agent());
    assert!(panics(|| drop(c.withdraw_allowance(agent(), U128(1)))));
    call(&agent(), NearToken::from_millinear(10));
    assert!(panics(|| c.add_line_credits(agent())));
    call(&agent(), NearToken::from_millinear(100));
    assert!(panics(|| c.grant_role(
        acc("x.pay.alice.near"),
        vec![AgentMethod::Report],
        U128(1),
        U128(1),
        1,
        U64(FAR)
    )));
}

#[test]
fn line_credits_run_out_and_can_be_topped_up() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let _ = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("alice.near"), NearToken::from_millinear(10));
    c.grant_role(
        agent(),
        vec![AgentMethod::PayFromAllowance],
        U128(100_000),
        U128(1_000_000),
        100,
        U64(FAR),
    );
    let _ = fund(&mut c, "alice.near", 1_000_000);
    let n = c.get_role(agent()).unwrap().line_credits;
    for i in 0..n {
        assert!(try_pay(&mut c, target(w), 10_000, &format!("l{i}"), 0));
    }
    assert!(!try_pay(&mut c, target(w), 10_000, "more", 0));
    call(&acc("alice.near"), NearToken::from_millinear(1));
    c.add_line_credits(agent());
    assert!(try_pay(&mut c, target(w), 10_000, "more", 0));
}

#[test]
fn failed_allowance_withdrawal_restores() {
    let mut c = setup();
    grant(&mut c, 100_000, 100_000, 5);
    let _ = fund(&mut c, "alice.near", 100_000);
    yocto(&acc("alice.near"));
    let _ = c.withdraw_allowance(agent(), U128(70_000));
    assert_eq!(c.get_accounting().allowance_total.0, 30_000);
    callback(PromiseResult::Failed);
    assert!(!c.on_allowance_withdrawn(agent(), acc("alice.near"), U128(70_000)));
    assert_eq!(c.get_role(agent()).unwrap().allowance.0, 100_000);
    assert_eq!(c.get_accounting().allowance_total.0, 100_000);
}

#[test]
fn note_is_holder_only_and_never_changes_payouts() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("alice.near"), NearToken::from_millinear(10));
    assert!(panics(|| c.set_share_note(t.clone(), "x".into())));
    call(&acc("bob.near"), NearToken::from_yoctonear(0));
    assert!(panics(|| c.set_share_note(t.clone(), "x".into())));
    call(&acc("bob.near"), NearToken::from_millinear(10));
    assert!(panics(|| c.set_share_note(t.clone(), "n".repeat(281))));
    call(&acc("bob.near"), NearToken::from_millinear(10));
    c.set_share_note(t.clone(), "pay carol.near instead".into());
    assert_eq!(
        c.get_share(t.clone()).unwrap().note.as_deref(),
        Some("pay carol.near instead")
    );
    let vault_before = c.get_share(t.clone()).unwrap().vault;
    pay_share(&mut c, &t, 50_000);
    assert_eq!(ledger(&c, &t).known_balance.0, 50_000);
    assert_eq!(c.get_share(t.clone()).unwrap().vault, vault_before);
    call(&acc("bob.near"), NearToken::from_yoctonear(1));
    c.set_share_note(t.clone(), String::new());
    assert!(c.get_share(t).unwrap().note.is_none());
}

#[test]
fn payer_role_attests_evidence_tier() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&acc("payer.near"), NearToken::from_millinear(10));
    assert!(panics(|| c.attest_share(t.clone(), h('7'))));
    yocto(&acc("alice.near"));
    assert!(panics(|| c.grant_payer_role(acc("payer.near"), h('6'))));
    yocto(&registrar());
    c.grant_payer_role(acc("payer.near"), h('6'));
    assert!(c.is_payer(acc("payer.near")));
    call(&acc("payer.near"), NearToken::from_millinear(10));
    c.attest_share(t.clone(), h('7'));
    let extra = c
        .nft_token(t.clone())
        .unwrap()
        .metadata
        .unwrap()
        .extra
        .unwrap();
    assert!(extra.contains("payer_confirmed"), "{extra}");
    yocto(&registrar());
    c.revoke_payer_role(acc("payer.near"), h('6'));
    assert!(!c.is_payer(acc("payer.near")));
}

#[test]
fn retry_forward_resends_undelivered() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    call(&usdc(), NearToken::from_yoctonear(0));
    let msg =
        json!({ "pay_share": { "token_id": t, "period": "2026-Q3", "statement_hash": h('f') } });
    let _ = c.ft_on_transfer(acc("payer.near"), U128(80_000), msg.to_string());
    callback(PromiseResult::Failed);
    assert!(!c.on_forward(t.clone(), U128(80_000)));
    assert_eq!(c.get_accounting().undelivered_total.0, 80_000);
    call(&acc("anyone.near"), NearToken::from_yoctonear(0));
    c.retry_forward(t.clone());
    assert_eq!(c.get_accounting().undelivered_total.0, 0);
    assert_eq!(c.get_accounting().forwards_in_flight_total.0, 80_000);
    callback(ok(b""));
    assert!(c.on_forward(t.clone(), U128(80_000)));
    assert_eq!(ledger(&c, &t).known_balance.0, 80_000);
    call(&acc("anyone.near"), NearToken::from_yoctonear(0));
    assert!(panics(|| c.retry_forward(t)), "nothing left to retry");
}

#[test]
fn stale_payout_lock_released_only_after_timeout_and_late_callback_is_safe() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    pay_share(&mut c, &t, 700_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 100);
    let _ = c.claim(t.clone());
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        0,
        100 + STALE_LOCK_BLOCKS - 1,
    );
    assert!(panics(|| c.release_stale_lock(t.clone())), "too early");
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        0,
        100 + STALE_LOCK_BLOCKS,
    );
    c.release_stale_lock(t.clone());
    assert!(ledger(&c, &t).payout_in_flight.is_none());
    assert!(get_logs().join("\n").contains("lock_released_stale"));

    // A new claim cannot start (nothing claimable), but a late success of
    // the old claim must not touch state beyond the epoch.
    callback(ok(b""));
    assert!(c.on_claim(t.clone(), acc("bob.near"), U128(700_000), U64(100)));
    let l = ledger(&c, &t);
    assert_eq!((l.known_balance.0, l.total_paid_out.0), (0, 700_000));
}

#[test]
fn late_resolve_after_stale_release_keeps_current_owner() {
    let mut c = setup();
    let w = work(&mut c, "USRC12600001");
    let t = mint(&mut c, w, "bob.near", 10_000);
    at(&acc("bob.near"), NearToken::from_yoctonear(1), 0, 10);
    let _ = c.nft_transfer_call(
        acc("carol.near"),
        t.clone(),
        None,
        None,
        String::new(),
        None,
    );
    at(
        &acc("anyone.near"),
        NearToken::from_yoctonear(0),
        0,
        10 + STALE_LOCK_BLOCKS,
    );
    c.release_stale_lock(t.clone());
    assert!(ledger(&c, &t).transfer_pending.is_none());
    callback(PromiseResult::Failed);
    assert!(c.nft_resolve_transfer(
        acc("bob.near"),
        acc("carol.near"),
        t.clone(),
        None,
        Some(U64(10))
    ));
    assert_eq!(c.owner_of(&t), Some(acc("carol.near")));
}
