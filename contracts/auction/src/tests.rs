use super::*;
use logic::Outcome;
use near_sdk::test_utils::{get_logs, VMContextBuilder};
use near_sdk::{test_vm_config, testing_env, PromiseOrValue, PromiseResult, RuntimeFeesConfig};

const SEC: u64 = NS_PER_SEC;
const T0: u64 = 1_000_000 * SEC;

fn acc(s: &str) -> AccountId {
    s.parse().unwrap()
}

fn base(pred: &str) -> VMContextBuilder {
    let mut b = VMContextBuilder::new();

    b.current_account_id(acc("auction.root.testnet"))
        .predecessor_account_id(acc(pred))
        .account_balance(NearToken::from_near(50))
        .block_timestamp(T0)
        .prepaid_gas(Gas::from_tgas(300));
    b
}

fn ctx(pred: &str, ts: u64, deposit: u128) {
    let mut b = base(pred);
    b.block_timestamp(ts)
        .attached_deposit(NearToken::from_yoctonear(deposit));
    testing_env!(b.build());
}

fn ctx_result(pred: &str, ts: u64, result: PromiseResult) {
    let mut b = base(pred);
    b.block_timestamp(ts);
    testing_env!(
        b.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        Default::default(),
        vec![result]
    );
}

fn ok_bytes(v: &str) -> PromiseResult {
    PromiseResult::Successful(v.as_bytes().to_vec())
}

fn new_contract() -> AuctionContract {
    ctx("auction.root.testnet", T0, 0);
    AuctionContract::new(
        acc("reg.root.testnet"),
        acc("usdc.root.testnet"),
        OutlayerSetup {
            outlayer: acc("outlayer.testnet"),
            wasm_url: "https://test.fastfs.io/auction.root.testnet/outlayer.testnet/aa.wasm".into(),
            wasm_hash: "a".repeat(64),
            secrets_account: acc("owner.testnet"),
            secrets_profile: "royltee-auction".into(),
        },
    )
}

const PK: &str = "c99e0b3a826c8f3852bd2c40073f4c3b857f24af0d3ce42561e761402056ea7f";
const MAX: u128 = 100_000_000;
const RESERVE: u128 = 10_000_000;

fn with_key() -> AuctionContract {
    let mut c = new_contract();
    c.tee_pubkey = Some(PK.to_string());
    c
}

fn list_msg(ends_in_s: u64) -> String {
    json!({"reserve": RESERVE.to_string(), "max_deposit": MAX.to_string(),
        "ends_at": (T0 + ends_in_s * SEC).to_string()})
    .to_string()
}

/// Lists share "s1" for seller.testnet and returns the contract.
fn listed() -> AuctionContract {
    let mut c = with_key();
    ctx("reg.root.testnet", T0, 0);
    let keep = c.nft_on_transfer(
        acc("seller.testnet"),
        acc("seller.testnet"),
        "s1".into(),
        list_msg(3600),
    );
    assert!(!keep, "listing must keep the share");
    c
}

fn bid_ct(i: u32) -> String {
    format!("QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo{i:04}")
}

/// Takes a bid straight through the membership callback.
fn place(c: &mut AuctionContract, who: &str, i: u32) -> u128 {
    ctx("usdc.root.testnet", T0 + 10 * SEC, 0);
    let r = c.ft_on_transfer(
        acc(who),
        U128(MAX),
        json!({"auction_id": "0", "ciphertext": bid_ct(i)}).to_string(),
    );
    assert!(
        matches!(r, PromiseOrValue::Promise(_)),
        "bid should go to the member check"
    );
    ctx_result("auction.root.testnet", T0 + 10 * SEC, ok_bytes("true"));
    c.on_bid_checked(acc(who), U128(MAX), 0, bid_ct(i)).0
}

fn bid_hash(c: &AuctionContract) -> String {
    c.get_bids_hash(U64(0))
}

fn answer(c: &AuctionContract, winner: Option<(&str, u128)>) -> String {
    match winner {
        Some((w, p)) => json!({"v":1,"ok":true,"auction":"0","bids_hash":bid_hash(c),"winner":w,"price":p.to_string()}),
        None => json!({"v":1,"ok":true,"auction":"0","bids_hash":bid_hash(c),"winner":null,"price":null}),
    }
    .to_string()
}

fn after_end() -> u64 {
    T0 + 3601 * SEC
}

/// Starts settling with two bids in and returns the contract.
fn settling() -> AuctionContract {
    let mut c = listed();
    assert_eq!(place(&mut c, "bob.testnet", 1), 0);
    assert_eq!(place(&mut c, "carol.testnet", 2), 0);
    ctx(
        "anyone.testnet",
        after_end(),
        20_000_000_000_000_000_000_000,
    );
    assert!(c.settle(U64(0)).is_some());
    assert_eq!(c.auction(0).status, Status::Settling);
    c
}

/// Mocked storage is per thread and kept between calls, so each case of a
/// loop runs on its own thread to start clean.
fn fresh<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::spawn(f).join().expect("case failed");
}

fn panics<F: FnOnce() + std::panic::UnwindSafe>(f: F) -> bool {
    std::panic::catch_unwind(f).is_err()
}

// ---------- logic ----------

#[test]
fn hash_and_base64_checks() {
    assert!(logic::is_hash64(&"0".repeat(64)));
    assert!(!logic::is_hash64(&"A".repeat(64)));
    assert!(!logic::is_hash64("abc"));
    assert!(logic::looks_like_base64("QUJD"));
    assert!(logic::looks_like_base64("QUI="));
    assert!(!logic::looks_like_base64("QU JD"));
    assert!(!logic::looks_like_base64("===="));
    assert!(!logic::looks_like_base64(""));
}

#[test]
fn parse_output_cases() {
    let ok = |extra: &str| format!(r#"{{"v":1,"ok":true,"auction":"0","bids_hash":"h",{extra}}}"#);
    let p = |s: &str| logic::parse_settle_output(s.as_bytes(), "0", "h");
    assert_eq!(
        p(&ok(r#""winner":"a.testnet","price":"5""#)),
        Ok(Outcome::Sale("a.testnet".into(), 5))
    );
    assert_eq!(p(&ok(r#""winner":null,"price":null"#)), Ok(Outcome::NoSale));
    assert!(p(&ok(r#""winner":"a.testnet","price":null"#)).is_err());
    assert!(p(&ok(r#""winner":"a.testnet","price":"-1""#)).is_err());
    assert!(p(&ok(r#""winner":"a.testnet","price":"1e9""#)).is_err());
    assert!(p(&ok(r#""winner":"a.testnet","price":"""#)).is_err());
    assert!(p("null").is_err());
    assert!(p("not json").is_err());
    assert!(p(r#"{"v":1,"ok":false,"error":"x"}"#).is_err());
    assert!(
        p(r#"{"v":2,"ok":true,"auction":"0","bids_hash":"h","winner":null,"price":null}"#).is_err()
    );
    assert!(
        logic::parse_settle_output(ok(r#""winner":null,"price":null"#).as_bytes(), "1", "h")
            .is_err(),
        "other auction"
    );
    assert!(
        logic::parse_settle_output(ok(r#""winner":null,"price":null"#).as_bytes(), "0", "x")
            .is_err(),
        "other bid list"
    );
    // Older wrapper shape.
    let inner = ok(r#""winner":"a.testnet","price":"7""#);
    assert_eq!(
        p(&format!(r#"{{"success":true,"result":{inner}}}"#)),
        Ok(Outcome::Sale("a.testnet".into(), 7))
    );
    assert!(p(&format!(r#"{{"success":false,"result":{inner}}}"#)).is_err());
}

#[test]
fn bids_hash_matches_the_module_golden_value() {
    // Same canonical text as tee/auction-wasi (see its `bids_hash` test).
    let bids = vec![
        Bid {
            bidder: acc("a.testnet"),
            ciphertext: "QUJD".into(),
            at: U64(0),
        },
        Bid {
            bidder: acc("b.testnet"),
            ciphertext: "REVG".into(),
            at: U64(0),
        },
    ];
    ctx("a.testnet", T0, 0);
    assert_eq!(
        logic::bids_text("7", &bids),
        "royltee-auction-bids-v1\n7\na.testnet\nQUJD\nb.testnet\nREVG\n"
    );
    assert_eq!(
        logic::bids_hash("7", &bids),
        include_str!("../../../tee/auction-wasi/fixtures/golden_bids_hash.txt").trim()
    );
}

// ---------- listing ----------

#[test]
fn listing_keeps_the_share_and_records_the_auction() {
    let c = listed();
    let a = c.get_auction(U64(0)).unwrap();
    assert_eq!(a.rec.seller, acc("seller.testnet"));
    assert_eq!(a.rec.token_id, "s1");
    assert_eq!(a.rec.status, Status::Open);
    assert!(a.rec.share_held);
    assert_eq!(c.auction_for_token("s1".into()), Some(U64(0)));
}

#[test]
fn only_the_registry_can_list() {
    let mut c = with_key();
    ctx("mallory.testnet", T0, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.nft_on_transfer(
            acc("mallory.testnet"),
            acc("mallory.testnet"),
            "s1".into(),
            list_msg(3600),
        );
    })));
    assert_eq!(c.next_id, 0);
}

#[test]
fn listing_is_refused_with_true_for_every_bad_input() {
    let refuse = |c: &mut AuctionContract, msg: String| {
        ctx("reg.root.testnet", T0, 0);
        c.nft_on_transfer(
            acc("seller.testnet"),
            acc("seller.testnet"),
            "s9".into(),
            msg,
        )
    };
    let mut c = with_key();
    assert!(refuse(&mut c, "not json".into()));
    assert!(refuse(
        &mut c,
        json!({"reserve":"0","max_deposit":"5","ends_at":(T0+3600*SEC).to_string()}).to_string()
    ));
    assert!(refuse(
        &mut c,
        json!({"reserve":"6","max_deposit":"5","ends_at":(T0+3600*SEC).to_string()}).to_string()
    ));
    assert!(refuse(&mut c, list_msg(10)), "window under a minute");
    assert!(
        refuse(&mut c, list_msg(15 * 24 * 3600)),
        "window over 14 days"
    );
    assert_eq!(c.next_id, 0);
    // No published key yet.
    let mut c = new_contract();
    assert!(refuse(&mut c, list_msg(3600)));
    assert_eq!(c.next_id, 0);
}

#[test]
fn second_listing_of_the_same_share_is_refused() {
    let mut c = listed();
    ctx("reg.root.testnet", T0, 0);
    assert!(c.nft_on_transfer(
        acc("x.testnet"),
        acc("x.testnet"),
        "s1".into(),
        list_msg(3600)
    ));
    assert_eq!(c.next_id, 1);
}

#[test]
fn listing_is_refused_when_the_contract_is_low_on_near() {
    let mut c = with_key();
    let mut b = base("reg.root.testnet");
    b.account_balance(NearToken::from_millinear(10));
    testing_env!(b.build());
    assert!(c.nft_on_transfer(
        acc("s.testnet"),
        acc("s.testnet"),
        "s1".into(),
        list_msg(3600)
    ));
}

// ---------- bids ----------

#[test]
fn only_mock_usdc_can_deposit() {
    let mut c = listed();
    ctx("evil-token.testnet", T0 + SEC, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        let _ = c.ft_on_transfer(acc("bob.testnet"), U128(MAX), "{}".into());
    })));
}

fn refund_of(r: PromiseOrValue<U128>) -> u128 {
    match r {
        PromiseOrValue::Value(v) => v.0,
        PromiseOrValue::Promise(_) => panic!("expected an immediate refund"),
    }
}

#[test]
fn bad_bids_are_refunded_in_full() {
    let mut c = listed();
    let try_bid = |c: &mut AuctionContract, who: &str, amount: u128, msg: String, ts: u64| {
        ctx("usdc.root.testnet", ts, 0);
        refund_of(c.ft_on_transfer(acc(who), U128(amount), msg))
    };
    let good = json!({"auction_id":"0","ciphertext":bid_ct(1)}).to_string();
    assert_eq!(
        try_bid(&mut c, "bob.testnet", MAX, "junk".into(), T0 + SEC),
        MAX
    );
    assert_eq!(
        try_bid(
            &mut c,
            "bob.testnet",
            MAX,
            json!({"auction_id":"x","ciphertext":bid_ct(1)}).to_string(),
            T0 + SEC
        ),
        MAX
    );
    assert_eq!(
        try_bid(
            &mut c,
            "bob.testnet",
            MAX,
            json!({"auction_id":"5","ciphertext":bid_ct(1)}).to_string(),
            T0 + SEC
        ),
        MAX,
        "unknown auction"
    );
    assert_eq!(
        try_bid(&mut c, "bob.testnet", MAX - 1, good.clone(), T0 + SEC),
        MAX - 1,
        "wrong amount"
    );
    assert_eq!(
        try_bid(&mut c, "bob.testnet", MAX + 1, good.clone(), T0 + SEC),
        MAX + 1,
        "wrong amount"
    );
    assert_eq!(
        try_bid(&mut c, "seller.testnet", MAX, good.clone(), T0 + SEC),
        MAX,
        "seller"
    );
    assert_eq!(
        try_bid(&mut c, "bob.testnet", MAX, good.clone(), after_end()),
        MAX,
        "closed"
    );
    assert_eq!(
        try_bid(
            &mut c,
            "bob.testnet",
            MAX,
            json!({"auction_id":"0","ciphertext":"not base64!"}).to_string(),
            T0 + SEC
        ),
        MAX
    );
    let long = "A".repeat(MAX_CIPHERTEXT + 1);
    assert_eq!(
        try_bid(
            &mut c,
            "bob.testnet",
            MAX,
            json!({"auction_id":"0","ciphertext":long}).to_string(),
            T0 + SEC
        ),
        MAX
    );
    assert_eq!(c.auction(0).bids.len(), 0);
    assert_eq!(c.escrow_total, 0);
}

#[test]
fn a_member_bid_is_taken_and_counted() {
    let mut c = listed();
    assert_eq!(place(&mut c, "bob.testnet", 1), 0);
    assert_eq!(c.auction(0).bids.len(), 1);
    assert_eq!(c.escrow_total, MAX);
    assert!(get_logs().iter().any(|l| l.contains("bid_placed")));
    // No plaintext about the bid anywhere in the log.
    assert!(!get_logs().iter().any(|l| l.contains("amount")));
}

#[test]
fn a_second_bid_from_the_same_account_is_refused() {
    let mut c = listed();
    assert_eq!(place(&mut c, "bob.testnet", 1), 0);
    ctx("usdc.root.testnet", T0 + 20 * SEC, 0);
    let r = c.ft_on_transfer(
        acc("bob.testnet"),
        U128(MAX),
        json!({"auction_id":"0","ciphertext":bid_ct(9)}).to_string(),
    );
    assert_eq!(refund_of(r), MAX);
}

#[test]
fn a_non_member_or_a_failed_check_is_refunded() {
    for res in [
        ok_bytes("false"),
        PromiseResult::Failed,
        ok_bytes("garbage"),
    ] {
        fresh(move || {
            let mut c = listed();
            ctx_result("auction.root.testnet", T0 + 10 * SEC, res);
            let unused = c.on_bid_checked(acc("eve.testnet"), U128(MAX), 0, bid_ct(1));
            assert_eq!(unused.0, MAX);
            assert_eq!(c.auction(0).bids.len(), 0);
            assert_eq!(c.escrow_total, 0);
        });
    }
}

#[test]
fn the_callback_rechecks_the_rules() {
    // Two bids race past the sync check; the callback keeps only the first.
    let mut c = listed();
    ctx_result("auction.root.testnet", T0 + 10 * SEC, ok_bytes("true"));
    assert_eq!(
        c.on_bid_checked(acc("bob.testnet"), U128(MAX), 0, bid_ct(1))
            .0,
        0
    );
    ctx_result("auction.root.testnet", T0 + 10 * SEC, ok_bytes("true"));
    assert_eq!(
        c.on_bid_checked(acc("bob.testnet"), U128(MAX), 0, bid_ct(2))
            .0,
        MAX
    );
    // Closed between the two steps.
    ctx_result("auction.root.testnet", after_end(), ok_bytes("true"));
    assert_eq!(
        c.on_bid_checked(acc("dave.testnet"), U128(MAX), 0, bid_ct(3))
            .0,
        MAX
    );
    assert_eq!(c.auction(0).bids.len(), 1);
}

#[test]
fn an_auction_takes_at_most_eight_bids() {
    let mut c = listed();
    for i in 0..MAX_BIDS as u32 {
        assert_eq!(place(&mut c, &format!("bidder{i}.testnet"), i), 0);
    }
    ctx("usdc.root.testnet", T0 + 30 * SEC, 0);
    let r = c.ft_on_transfer(
        acc("late.testnet"),
        U128(MAX),
        json!({"auction_id":"0","ciphertext":bid_ct(99)}).to_string(),
    );
    assert_eq!(refund_of(r), MAX);
}

// ---------- settle ----------

#[test]
fn settle_needs_time_gas_and_deposit() {
    let mut c = listed();
    place(&mut c, "bob.testnet", 1);
    ctx(
        "anyone.testnet",
        T0 + 100 * SEC,
        20_000_000_000_000_000_000_000,
    );
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.settle(U64(0));
        })),
        "too early"
    );
    ctx("anyone.testnet", after_end(), 1);
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.settle(U64(0));
        })),
        "deposit too small"
    );
    let mut b = base("anyone.testnet");
    b.block_timestamp(after_end())
        .attached_deposit(NearToken::from_millinear(20))
        .prepaid_gas(Gas::from_tgas(100));
    testing_env!(b.build());
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.settle(U64(0));
        })),
        "gas too low"
    );
    assert_eq!(c.auction(0).status, Status::Open);
}

#[test]
fn settle_with_no_bids_cancels_and_returns_the_share() {
    let mut c = listed();
    ctx("anyone.testnet", after_end(), 0);
    assert!(c.settle(U64(0)).is_some());
    let a = c.auction(0);
    assert_eq!(a.status, Status::Cancelled);
    assert!(a.returning);
}

#[test]
fn settle_moves_to_settling_and_blocks_a_second_settle() {
    let mut c = settling();
    assert_eq!(c.auction(0).attempt, 1);
    ctx(
        "anyone.testnet",
        after_end(),
        20_000_000_000_000_000_000_000,
    );
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.settle(U64(0));
    })));
}

#[test]
fn a_good_answer_starts_delivery() {
    let mut c = settling();
    let out = answer(&c, Some(("bob.testnet", 30_000_000)));
    ctx_result("auction.root.testnet", after_end(), ok_bytes(&out));
    assert!(c.on_settled(0, 1));
    let a = c.auction(0);
    assert_eq!(a.status, Status::Delivering);
    assert_eq!(a.winner, Some(acc("bob.testnet")));
    assert_eq!(a.price, Some(U128(30_000_000)));
    // Money has not moved yet.
    assert_eq!(c.owed_total, 0);
    assert_eq!(c.escrow_total, 2 * MAX);
}

#[test]
fn bad_answers_reopen_settling_and_never_move_money() {
    type Case = (
        &'static str,
        Box<dyn Fn(&AuctionContract) -> PromiseResult + Send>,
    );
    let cases: Vec<Case> = vec![
        ("promise failed", Box::new(|_| PromiseResult::Failed)),
        ("null", Box::new(|_| ok_bytes("null"))),
        ("garbage", Box::new(|_| ok_bytes("zzz"))),
        (
            "wrong hash",
            Box::new(|_| {
                ok_bytes(
                    r#"{"v":1,"ok":true,"auction":"0","bids_hash":"00","winner":null,"price":null}"#,
                )
            }),
        ),
        (
            "winner not a bidder",
            Box::new(|c| ok_bytes(&answer(c, Some(("mallory.testnet", 20_000_000))))),
        ),
        (
            "price below reserve",
            Box::new(|c| ok_bytes(&answer(c, Some(("bob.testnet", RESERVE - 1))))),
        ),
        (
            "price above max_deposit",
            Box::new(|c| ok_bytes(&answer(c, Some(("bob.testnet", MAX + 1))))),
        ),
    ];
    for (name, mk) in cases {
        fresh(move || {
            let mut c = settling();
            let res = mk(&c);
            ctx_result("auction.root.testnet", after_end(), res);
            assert!(!c.on_settled(0, 1), "{name}");
            let a = c.auction(0);
            assert_eq!(a.status, Status::Open, "{name}");
            assert_eq!(a.failures, 1, "{name}");
            assert!(a.last_error.is_some(), "{name}");
            assert_eq!(c.owed_total, 0, "{name}");
            assert_eq!(c.escrow_total, 2 * MAX, "{name}");
            // And it can be settled again.
            ctx(
                "anyone.testnet",
                after_end() + SEC,
                20_000_000_000_000_000_000_000,
            );
            assert!(c.settle(U64(0)).is_some(), "{name}");
            assert_eq!(c.auction(0).attempt, 2, "{name}");
        });
    }
}

#[test]
fn a_late_answer_for_an_old_attempt_is_ignored() {
    let mut c = settling();
    ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
    c.on_settled(0, 1);
    ctx(
        "anyone.testnet",
        after_end() + SEC,
        20_000_000_000_000_000_000_000,
    );
    c.settle(U64(0));
    let out = answer(&c, Some(("bob.testnet", 30_000_000)));
    ctx_result(
        "auction.root.testnet",
        after_end() + 2 * SEC,
        ok_bytes(&out),
    );
    assert!(!c.on_settled(0, 1), "attempt 1 is stale");
    assert_eq!(c.auction(0).status, Status::Settling);
}

#[test]
fn no_valid_bid_cancels_and_refunds_everyone() {
    let mut c = settling();
    let out = answer(&c, None);
    ctx_result("auction.root.testnet", after_end(), ok_bytes(&out));
    c.on_settled(0, 1);
    let a = c.auction(0);
    assert_eq!(a.status, Status::Cancelled);
    assert!(a.returning);
    assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
    assert_eq!(c.get_owed(acc("carol.testnet")).0, MAX);
    assert_eq!(c.escrow_total, 0);
    assert_eq!(c.owed_total, 2 * MAX);
}

// ---------- delivery ----------

fn delivering() -> AuctionContract {
    let mut c = settling();
    let out = answer(&c, Some(("bob.testnet", 30_000_000)));
    ctx_result("auction.root.testnet", after_end(), ok_bytes(&out));
    assert!(c.on_settled(0, 1));
    c
}

#[test]
fn a_delivered_share_pays_seller_winner_and_losers() {
    let mut c = delivering();
    ctx_result(
        "auction.root.testnet",
        after_end(),
        PromiseResult::Successful(vec![]),
    );
    assert!(c.on_delivered(0));
    let a = c.auction(0);
    assert_eq!(a.status, Status::Sold);
    assert!(!a.share_held);
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 30_000_000);
    assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX - 30_000_000);
    assert_eq!(c.get_owed(acc("carol.testnet")).0, MAX);
    assert_eq!(c.escrow_total, 0);
    assert_eq!(c.owed_total, 2 * MAX, "owed equals the two deposits");
    assert_eq!(c.auction_for_token("s1".into()), None);
}

#[test]
fn a_refused_delivery_cancels_refunds_all_and_returns_the_share() {
    let mut c = delivering();
    ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
    assert!(!c.on_delivered(0));
    let a = c.auction(0);
    assert_eq!(a.status, Status::Cancelled);
    assert!(a.share_held && a.returning);
    assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
    assert_eq!(c.get_owed(acc("carol.testnet")).0, MAX);
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
    assert_eq!(c.owed_total, 2 * MAX);
}

#[test]
fn on_delivered_outside_delivering_does_nothing() {
    let mut c = listed();
    ctx_result(
        "auction.root.testnet",
        T0,
        PromiseResult::Successful(vec![]),
    );
    assert!(!c.on_delivered(0));
    assert_eq!(c.auction(0).status, Status::Open);
    assert_eq!(c.owed_total, 0);
}

#[test]
fn a_failed_share_return_can_be_retried() {
    let mut c = delivering();
    ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
    c.on_delivered(0);
    ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
    assert!(!c.on_returned(0));
    let a = c.auction(0);
    assert!(a.share_held && !a.returning);
    assert_eq!(c.auction_for_token("s1".into()), Some(U64(0)));
    ctx("anyone.testnet", after_end() + 100 * SEC, 0);
    assert!(c.return_share(U64(0)).is_some());
    assert!(c.auction(0).returning);
    // One in flight at a time.
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.return_share(U64(0));
    })));
    ctx_result(
        "auction.root.testnet",
        after_end() + 101 * SEC,
        PromiseResult::Successful(vec![]),
    );
    assert!(c.on_returned(0));
    assert!(!c.auction(0).share_held);
    assert_eq!(c.auction_for_token("s1".into()), None);
}

#[test]
fn return_share_needs_a_cancelled_auction_that_still_holds_it() {
    let mut c = listed();
    ctx("anyone.testnet", T0 + SEC, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.return_share(U64(0));
    })));
}

// ---------- cancel ----------

#[test]
fn seller_cancels_only_before_any_bid() {
    let mut c = listed();
    ctx("bob.testnet", T0 + SEC, 0);
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.cancel(U64(0));
        })),
        "not the seller"
    );
    ctx("seller.testnet", T0 + SEC, 0);
    assert!(c.cancel(U64(0)).is_some());
    assert_eq!(c.auction(0).status, Status::Cancelled);

    let mut c = listed();
    place(&mut c, "bob.testnet", 1);
    ctx("seller.testnet", T0 + 20 * SEC, 0);
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.cancel(U64(0));
        })),
        "a bid is in"
    );
}

/// An open auction whose settle run answered badly once (failures = 1).
fn failed_once() -> AuctionContract {
    let mut c = listed();
    place(&mut c, "bob.testnet", 1);
    ctx(
        "anyone.testnet",
        after_end(),
        20_000_000_000_000_000_000_000,
    );
    assert!(c.settle(U64(0)).is_some());
    ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
    assert!(!c.on_settled(0, 1));
    assert_eq!(c.auction(0).status, Status::Open);
    assert_eq!(c.auction(0).failures, 1);
    c
}

#[test]
fn a_third_party_cannot_void_a_sale_nobody_tried_to_settle() {
    fresh(|| {
        let mut c = listed();
        place(&mut c, "bob.testnet", 1);
        // Six hours and a bit past the end, but no settle run has failed.
        ctx("anyone.testnet", T0 + 3600 * SEC + SETTLE_GRACE_NS + 1, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            c.cancel(U64(0));
        })));
        // Still refused one second before the hard limit.
        ctx("anyone.testnet", T0 + 3600 * SEC + SETTLE_HARD_GRACE_NS, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            c.cancel(U64(0));
        })));
    });
}

#[test]
fn anyone_cancels_after_a_failed_run_and_the_grace_period() {
    fresh(|| {
        let mut c = failed_once();
        let end = after_end() - SEC;
        ctx("anyone.testnet", end + SETTLE_GRACE_NS, 0);
        assert!(
            panics(std::panic::AssertUnwindSafe(|| {
                c.cancel(U64(0));
            })),
            "grace not over"
        );
        ctx("anyone.testnet", end + SETTLE_GRACE_NS + 1, 0);
        assert!(c.cancel(U64(0)).is_some());
        assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
        assert_eq!(c.escrow_total, 0);
    });
}

#[test]
fn anyone_cancels_after_the_hard_limit_even_if_no_run_failed() {
    fresh(|| {
        let mut c = listed();
        place(&mut c, "bob.testnet", 1);
        ctx(
            "anyone.testnet",
            T0 + 3600 * SEC + SETTLE_HARD_GRACE_NS + 1,
            0,
        );
        assert!(c.cancel(U64(0)).is_some());
        assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
        assert_eq!(c.escrow_total, 0);
    });
}

#[test]
fn a_stuck_settle_request_can_be_cancelled_after_an_hour() {
    let mut c = settling();
    ctx("anyone.testnet", after_end() + SETTLING_STALE_NS, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.cancel(U64(0));
    })));
    ctx("anyone.testnet", after_end() + SETTLING_STALE_NS + 1, 0);
    assert!(c.cancel(U64(0)).is_some());
    assert_eq!(c.owed_total, 2 * MAX);
    // The late answer from OutLayer is ignored.
    let out = answer(&c, Some(("bob.testnet", 30_000_000)));
    ctx_result(
        "auction.root.testnet",
        after_end() + SETTLING_STALE_NS + 5,
        ok_bytes(&out),
    );
    assert!(!c.on_settled(0, 1));
    assert_eq!(c.auction(0).status, Status::Cancelled);
}

#[test]
fn a_sold_or_delivering_auction_cannot_be_cancelled() {
    let mut c = delivering();
    ctx("anyone.testnet", T0 + 100 * 3600 * SEC, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.cancel(U64(0));
    })));
}

// ---------- withdraw ----------

fn sold() -> AuctionContract {
    let mut c = delivering();
    ctx_result(
        "auction.root.testnet",
        after_end(),
        PromiseResult::Successful(vec![]),
    );
    c.on_delivered(0);
    c
}

#[test]
fn withdraw_takes_the_credit_first() {
    let mut c = sold();
    ctx("anyone.testnet", after_end() + SEC, 0);
    c.withdraw(vec![
        acc("seller.testnet"),
        acc("seller.testnet"),
        acc("carol.testnet"),
    ]);
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
    assert_eq!(c.get_owed(acc("carol.testnet")).0, 0);
    assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX - 30_000_000);
    assert_eq!(c.owed_total, MAX - 30_000_000);
    // Nothing left for those two: a second call fails.
    ctx("anyone.testnet", after_end() + 2 * SEC, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        c.withdraw(vec![acc("seller.testnet")]);
    })));
}

#[test]
fn a_failed_payout_puts_the_credit_back() {
    let mut c = sold();
    ctx("anyone.testnet", after_end() + SEC, 0);
    c.withdraw(vec![acc("seller.testnet")]);
    ctx_result(
        "auction.root.testnet",
        after_end() + 2 * SEC,
        PromiseResult::Failed,
    );
    assert!(!c.on_withdrawn(0, acc("seller.testnet"), U128(30_000_000)));
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 30_000_000);
    assert_eq!(c.owed_total, 2 * MAX);
}

#[test]
fn withdraw_rejects_empty_and_oversized_lists_and_low_gas() {
    let mut c = sold();
    ctx("anyone.testnet", after_end() + SEC, 0);
    assert!(panics(std::panic::AssertUnwindSafe(|| c.withdraw(vec![]))));
    let many: Vec<AccountId> = (0..10).map(|i| acc(&format!("a{i}.testnet"))).collect();
    assert!(panics(std::panic::AssertUnwindSafe(|| c.withdraw(many))));
    let mut b = base("anyone.testnet");
    b.prepaid_gas(Gas::from_tgas(20));
    testing_env!(b.build());
    assert!(panics(std::panic::AssertUnwindSafe(
        || c.withdraw(vec![acc("seller.testnet")])
    )));
}

/// L2: the gas check counts the promise fees of every account. Nine accounts
/// need (15 + 8 + 3) * 9 + 10 = 244 Tgas; 243 is refused, 244 goes through.
#[test]
fn withdraw_counts_promise_fees_per_account() {
    let nine: Vec<AccountId> = ["seller", "bob", "carol", "d", "e", "f", "g", "h", "i"]
        .iter()
        .map(|n| acc(&format!("{n}.testnet")))
        .collect();
    for (tgas, accepted) in [(243u64, false), (244, true)] {
        let nine = nine.clone();
        fresh(move || {
            let mut c = sold();
            let mut b = base("anyone.testnet");
            b.prepaid_gas(Gas::from_tgas(tgas));
            testing_env!(b.build());
            let r = panics(std::panic::AssertUnwindSafe(|| c.withdraw(nine)));
            assert_eq!(!r, accepted, "{tgas} Tgas");
        });
    }
}

// ---------- the key ----------

#[test]
fn publish_key_stores_a_valid_key_once() {
    let mut c = new_contract();
    ctx(
        "anyone.testnet",
        T0 + 100 * SEC,
        20_000_000_000_000_000_000_000,
    );
    let _ = c.publish_key();
    let good = format!(r#"{{"v":1,"ok":true,"key_version":1,"pubkey":"{PK}"}}"#);
    ctx_result("auction.root.testnet", T0 + 101 * SEC, ok_bytes(&good));
    assert!(c.on_key());
    assert_eq!(c.get_tee_pubkey().as_deref(), Some(PK));
    ctx(
        "anyone.testnet",
        T0 + 300 * SEC,
        20_000_000_000_000_000_000_000,
    );
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.publish_key();
        })),
        "already published"
    );
}

#[test]
fn publish_key_rejects_bad_answers_and_low_deposit() {
    let mut c = new_contract();
    ctx("anyone.testnet", T0 + 100 * SEC, 1);
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        let _ = c.publish_key();
    })));
    ctx(
        "anyone.testnet",
        T0 + 100 * SEC,
        20_000_000_000_000_000_000_000,
    );
    let _ = c.publish_key();
    for bad in [
        ok_bytes("null"),
        PromiseResult::Failed,
        ok_bytes(r#"{"v":1,"ok":true,"key_version":1,"pubkey":"zz"}"#),
        ok_bytes(
            r#"{"v":1,"ok":false,"key_version":1,"pubkey":"c99e0b3a826c8f3852bd2c40073f4c3b857f24af0d3ce42561e761402056ea7f"}"#,
        ),
        ok_bytes(r#"{"v":1,"ok":true}"#),
        // L6: a wrapper that says the run failed, or is malformed.
        ok_bytes(&format!(
            r#"{{"success":false,"result":{{"v":1,"ok":true,"key_version":1,"pubkey":"{PK}"}}}}"#
        )),
        ok_bytes(&format!(
            r#"{{"result":{{"v":1,"ok":true,"key_version":1,"pubkey":"{PK}"}}}}"#
        )),
        ok_bytes(&format!(
            r#"{{"success":"yes","result":{{"v":1,"ok":true,"key_version":1,"pubkey":"{PK}"}}}}"#
        )),
    ] {
        ctx_result("auction.root.testnet", T0 + 101 * SEC, bad);
        assert!(!c.on_key());
        assert_eq!(c.get_tee_pubkey(), None);
    }
    // A second request inside a minute is refused.
    ctx(
        "anyone.testnet",
        T0 + 120 * SEC,
        20_000_000_000_000_000_000_000,
    );
    assert!(panics(std::panic::AssertUnwindSafe(|| {
        let _ = c.publish_key();
    })));
}

#[test]
fn init_checks_its_arguments() {
    ctx("auction.root.testnet", T0, 0);
    let mk = |url: &str, hash: &str, profile: &str| {
        let (u, h, p) = (url.to_string(), hash.to_string(), profile.to_string());
        panics(std::panic::AssertUnwindSafe(move || {
            AuctionContract::new(
                acc("reg.root.testnet"),
                acc("usdc.root.testnet"),
                OutlayerSetup {
                    outlayer: acc("outlayer.testnet"),
                    wasm_url: u,
                    wasm_hash: h,
                    secrets_account: acc("owner.testnet"),
                    secrets_profile: p,
                },
            );
        }))
    };
    assert!(mk("http://x", &"a".repeat(64), "p"));
    assert!(mk("https://x", "short", "p"));
    assert!(mk("https://x", &"a".repeat(64), ""));
    assert!(!mk("https://x", &"a".repeat(64), "p"));
}

// ---------- the request sent to OutLayer ----------

#[test]
fn request_arguments_match_the_outlayer_shape() {
    let c = settling();
    // The request itself is a promise; check the pure pieces it is built from.
    let cfg = c.get_config();
    assert_eq!(cfg.pricing_rule, "second_price_with_reserve");
    assert_eq!(cfg.limits, Limits::default());
    assert_eq!(cfg.secrets_profile, "royltee-auction");
    let a = c.auction(0);
    let input = json!({
        "mode": "settle", "auction_id": "0", "contract": "auction.root.testnet",
        "reserve": a.reserve, "max_deposit": a.max_deposit,
        "bids": a.bids.iter().map(|b| json!({"bidder": b.bidder, "ct": b.ciphertext})).collect::<Vec<_>>(),
    });
    assert_eq!(input["reserve"], json!(RESERVE.to_string()));
    assert_eq!(input["bids"][0]["bidder"], json!("bob.testnet"));
    assert!(get_logs().iter().any(|l| l.contains("settle_requested")));
}

/// L6 and M4: the key and the settle answer are read from the shapes OutLayer
/// may return (the guest JSON, a `{success, result}` wrapper, a JSON string),
/// and a wrapper that did not succeed never counts.
#[test]
fn answers_are_read_from_every_known_shape_and_only_when_successful() {
    let inner = format!(r#"{{"v":1,"ok":true,"key_version":1,"pubkey":"{PK}"}}"#);
    let shapes = [
        inner.clone(),
        format!(r#"{{"success":true,"result":{inner}}}"#),
        serde_json::to_string(&inner).unwrap(),
        serde_json::to_string(&format!(r#"{{"success":true,"result":{inner}}}"#)).unwrap(),
    ];
    for shape in shapes {
        fresh(move || {
            let mut c = new_contract();
            ctx(
                "anyone.testnet",
                T0 + 100 * SEC,
                20_000_000_000_000_000_000_000,
            );
            let _ = c.publish_key();
            ctx_result("auction.root.testnet", T0 + 101 * SEC, ok_bytes(&shape));
            assert!(c.on_key(), "{shape}");
            assert_eq!(c.get_tee_pubkey().as_deref(), Some(PK));
        });
    }
    // Nested strings are not unwrapped without end.
    let deep = serde_json::to_string(&serde_json::to_string(&inner).unwrap()).unwrap();
    fresh(move || {
        let mut c = new_contract();
        ctx(
            "anyone.testnet",
            T0 + 100 * SEC,
            20_000_000_000_000_000_000_000,
        );
        let _ = c.publish_key();
        ctx_result("auction.root.testnet", T0 + 101 * SEC, ok_bytes(&deep));
        assert!(!c.on_key());
    });
    // The settle answer goes through the same reader.
    let c = settling();
    let plain = answer(&c, Some(("bob.testnet", 30_000_000)));
    let wrapped =
        json!({"success": true, "result": serde_json::from_str::<Value>(&plain).unwrap()})
            .to_string();
    let failed =
        json!({"success": false, "result": serde_json::from_str::<Value>(&plain).unwrap()})
            .to_string();
    let stringified = serde_json::to_string(&plain).unwrap();
    let hash = bid_hash(&c);
    for ok_shape in [&plain, &wrapped, &stringified] {
        assert!(matches!(
            logic::parse_settle_output(ok_shape.as_bytes(), "0", &hash),
            Ok(Outcome::Sale(..))
        ));
    }
    assert!(logic::parse_settle_output(failed.as_bytes(), "0", &hash).is_err());
}

// ---------- reconcile (H1) ----------

fn nft_token_owner(owner: &str) -> PromiseResult {
    ok_bytes(&json!({"token_id": "s1", "owner_id": owner}).to_string())
}

#[test]
fn reconcile_waits_fifteen_minutes_and_only_for_stuck_states() {
    fresh(|| {
        let c = delivering();
        let t = c.auction(0).delivering_since.0;
        ctx("anyone.testnet", t + RECONCILE_AFTER_NS - 1, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.reconcile(U64(0));
        })));
        ctx("anyone.testnet", t + RECONCILE_AFTER_NS, 0);
        let _ = c.reconcile(U64(0));
    });
    fresh(|| {
        // An open, a settling and a sold auction are not reconcilable.
        let c = listed();
        ctx("anyone.testnet", T0 + 100 * 3600 * SEC, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.reconcile(U64(0));
        })));
        let c = settling();
        ctx("anyone.testnet", after_end() + 100 * 3600 * SEC, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.reconcile(U64(0));
        })));
    });
    fresh(|| {
        let c = sold();
        ctx("anyone.testnet", after_end() + 100 * 3600 * SEC, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.reconcile(U64(0));
        })));
    });
}

/// The delivery callback was lost and the registry says the share never left.
#[test]
fn a_lost_delivery_callback_is_reconciled_from_the_registry() {
    fresh(|| {
        let mut c = delivering();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("auction.root.testnet"),
        );
        assert!(c.on_reconciled(0, 0));
        let a = c.auction(0);
        assert_eq!(a.status, Status::Cancelled);
        assert!(a.share_held && a.returning, "the share is sent back");
        assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
        assert_eq!(c.get_owed(acc("carol.testnet")).0, MAX);
        assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
        assert_eq!(c.escrow_total, 0);
        assert_eq!(c.owed_total, 2 * MAX);
    });
    // The registry says the winner holds it: the sale is paid.
    fresh(|| {
        let mut c = delivering();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("bob.testnet"),
        );
        assert!(c.on_reconciled(0, 0));
        assert_eq!(c.auction(0).status, Status::Sold);
        assert_eq!(c.get_owed(acc("seller.testnet")).0, 30_000_000);
        assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX - 30_000_000);
        assert_eq!(c.get_owed(acc("carol.testnet")).0, MAX);
        assert_eq!(c.owed_total, 2 * MAX);
        assert_eq!(c.escrow_total, 0);
        assert_eq!(c.auction_for_token("s1".into()), None);
        // A late on_delivered after this does nothing.
        ctx_result(
            "auction.root.testnet",
            after_end() + 21 * 60 * SEC,
            PromiseResult::Successful(vec![]),
        );
        assert!(!c.on_delivered(0));
        assert_eq!(c.owed_total, 2 * MAX);
    });
    // The share went on from the winner to someone else: still a paid sale.
    fresh(|| {
        let mut c = delivering();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("third.testnet"),
        );
        assert!(c.on_reconciled(0, 0));
        assert_eq!(c.auction(0).status, Status::Sold);
    });
    // The share is back with the seller: refund everyone, nothing to return.
    fresh(|| {
        let mut c = delivering();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("seller.testnet"),
        );
        assert!(c.on_reconciled(0, 0));
        let a = c.auction(0);
        assert_eq!(a.status, Status::Cancelled);
        assert!(!a.share_held && !a.returning);
        assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
        assert_eq!(c.owed_total, 2 * MAX);
        assert_eq!(c.auction_for_token("s1".into()), None);
    });
    // An unreadable registry answer changes nothing.
    fresh(|| {
        let mut c = delivering();
        for bad in [PromiseResult::Failed, ok_bytes("null"), ok_bytes("{}")] {
            ctx_result("auction.root.testnet", after_end() + 20 * 60 * SEC, bad);
            assert!(!c.on_reconciled(0, 0));
            assert_eq!(c.auction(0).status, Status::Delivering);
            assert_eq!(c.owed_total, 0);
        }
    });
}

/// The return callback was lost: `returning` would stay true forever.
#[test]
fn a_lost_return_callback_is_reconciled() {
    fn returning() -> AuctionContract {
        let mut c = delivering();
        ctx_result("auction.root.testnet", after_end(), PromiseResult::Failed);
        c.on_delivered(0);
        let a = c.auction(0);
        assert!(a.share_held && a.returning);
        c
    }
    fresh(|| {
        let c = returning();
        let t = c.auction(0).returning_since.0;
        ctx("anyone.testnet", t + RECONCILE_AFTER_NS - 1, 0);
        assert!(panics(std::panic::AssertUnwindSafe(|| {
            let _ = c.reconcile(U64(0));
        })));
        ctx("anyone.testnet", t + RECONCILE_AFTER_NS, 0);
        let _ = c.reconcile(U64(0));
    });
    // Still held by the auction: the flag is cleared, so `return_share` works.
    fresh(|| {
        let mut c = returning();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("auction.root.testnet"),
        );
        assert!(c.on_reconciled(0, 1));
        let a = c.auction(0);
        assert!(a.share_held && !a.returning);
        ctx("anyone.testnet", after_end() + 21 * 60 * SEC, 0);
        assert!(c.return_share(U64(0)).is_some());
    });
    // The share is elsewhere (the lost callback had succeeded): marked out.
    fresh(|| {
        let mut c = returning();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("seller.testnet"),
        );
        assert!(c.on_reconciled(0, 1));
        let a = c.auction(0);
        assert!(!a.share_held && !a.returning);
        assert_eq!(c.auction_for_token("s1".into()), None);
        // Money is untouched by this path.
        assert_eq!(c.owed_total, 2 * MAX);
    });
    // The wrong stage for the state does nothing.
    fresh(|| {
        let mut c = returning();
        ctx_result(
            "auction.root.testnet",
            after_end() + 20 * 60 * SEC,
            nft_token_owner("seller.testnet"),
        );
        assert!(!c.on_reconciled(0, 0));
        assert!(c.auction(0).returning);
    });
}

// ---------- audit B: init, key, terms, storage, pending withdrawals ----------

/// B-07: `new` only works when the contract account itself calls it, which is
/// what a deploy plus init batch signed by that account does.
#[test]
fn init_only_works_from_the_contract_account() {
    ctx("attacker.testnet", T0, 0);
    assert!(panics(|| {
        AuctionContract::new(
            acc("reg.root.testnet"),
            acc("usdc.root.testnet"),
            OutlayerSetup {
                outlayer: acc("outlayer.testnet"),
                wasm_url: "https://x".into(),
                wasm_hash: "a".repeat(64),
                secrets_account: acc("owner.testnet"),
                secrets_profile: "p".into(),
            },
        );
    }));
}

/// B-08: a published key must not be a low order point, whatever the top bit.
#[test]
fn low_order_and_malformed_keys_are_never_usable() {
    assert!(logic::is_usable_pubkey(PK));
    let bad = [
        "0".repeat(64),
        format!("01{}", "0".repeat(62)),
        "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800".to_string(),
        // top bit of the last byte set: X25519 ignores it
        "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b880".to_string(),
        "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157".to_string(),
        format!("ec{}7f", "ff".repeat(30)),
        format!("ed{}ff", "ff".repeat(30)),
        format!("ee{}7f", "ff".repeat(30)),
        "A".repeat(64),
        "ab".to_string(),
    ];
    for k in bad {
        assert!(!logic::is_usable_pubkey(&k), "{k}");
    }
}

#[test]
fn the_key_answer_needs_a_usable_key_and_the_known_version() {
    let zero = "0".repeat(64);
    let answers = [
        format!(r#"{{"v":1,"ok":true,"key_version":1,"pubkey":"{zero}"}}"#),
        format!(r#"{{"v":1,"ok":true,"pubkey":"{PK}"}}"#),
        format!(r#"{{"v":1,"ok":true,"key_version":2,"pubkey":"{PK}"}}"#),
        format!(r#"{{"v":1,"ok":true,"key_version":"1","pubkey":"{PK}"}}"#),
    ];
    for a in answers {
        fresh(move || {
            let mut c = new_contract();
            ctx(
                "anyone.testnet",
                T0 + 100 * SEC,
                20_000_000_000_000_000_000_000,
            );
            let _ = c.publish_key();
            ctx_result("auction.root.testnet", T0 + 101 * SEC, ok_bytes(&a));
            assert!(!c.on_key(), "{a}");
            assert_eq!(c.get_tee_pubkey(), None);
        });
    }
}

/// B-09: terms above 38 digits are refused at listing, so the module, the
/// script and the callback all work on the same range.
#[test]
fn listing_terms_are_capped_at_38_digits() {
    assert_eq!(MAX_TERM.to_string().len(), 38);
    assert_eq!((MAX_TERM + 1).to_string().len(), 39);
    for (max, kept) in [(MAX_TERM, true), (MAX_TERM + 1, false), (u128::MAX, false)] {
        fresh(move || {
            let mut c = with_key();
            ctx("reg.root.testnet", T0, 0);
            let msg = json!({"reserve": "1", "max_deposit": max.to_string(),
                "ends_at": (T0 + 3600 * SEC).to_string()})
            .to_string();
            let keep = c.nft_on_transfer(
                acc("seller.testnet"),
                acc("seller.testnet"),
                "s1".into(),
                msg,
            );
            assert_eq!(!keep, kept, "max_deposit {max}");
        });
    }
}

fn list_n(c: &mut AuctionContract, token: &str, balance: NearToken) -> bool {
    let mut b = base("reg.root.testnet");
    b.account_balance(balance);
    testing_env!(b.build());
    !c.nft_on_transfer(
        acc("seller.testnet"),
        acc("seller.testnet"),
        token.into(),
        list_msg(3600),
    )
}

/// B-04: the reserve of every open auction is subtracted from what the next
/// listing may use, so accepted auctions can always complete.
#[test]
fn admission_counts_the_reserve_of_every_open_auction() {
    let mut c = with_key();
    // Free balance is balance minus the storage already used. Start from a
    // known point: room for the floor plus two and a half reserves.
    ctx("reg.root.testnet", T0, 0);
    let locked = env::storage_byte_cost().saturating_mul(u128::from(env::storage_usage()));
    let balance = locked
        .saturating_add(MIN_FREE_BALANCE)
        .saturating_add(c.reserve_per_auction().saturating_mul(5).saturating_div(2));
    assert!(list_n(&mut c, "s1", balance));
    assert!(list_n(&mut c, "s2", balance));
    assert!(!list_n(&mut c, "s3", balance), "third listing must not fit");
    assert_eq!(c.open_auctions, 2);
    assert_eq!(c.get_accounting().open_auctions, 2);
}

/// B-04 and B-06: a finished auction gives its reserve back and drops the
/// ciphertexts.
#[test]
fn a_finished_auction_releases_its_reserve_and_ciphertexts() {
    let c = sold();
    assert_eq!(c.open_auctions, 0);
    let a = c.auction(0);
    assert_eq!(a.bids.len(), 2);
    assert!(a.bids.iter().all(|b| b.ciphertext.is_empty()));
    let mut c = settling();
    assert_eq!(c.open_auctions, 1);
    ctx("anyone.testnet", after_end() + SETTLING_STALE_NS + 1, 0);
    c.cancel(U64(0));
    assert_eq!(c.open_auctions, 0);
    assert!(c.auction(0).bids.iter().all(|b| b.ciphertext.is_empty()));
}

/// B-06: listing and cancelling with no bids leaves nothing behind.
#[test]
fn a_cancelled_auction_with_no_bids_leaves_no_record() {
    let mut c = listed();
    ctx("seller.testnet", T0 + 20 * SEC, 0);
    assert!(c.cancel(U64(0)).is_some());
    assert_eq!(c.open_auctions, 0);
    ctx_result(
        "auction.root.testnet",
        T0 + 21 * SEC,
        PromiseResult::Successful(vec![]),
    );
    assert!(c.on_returned(0));
    assert!(c.get_auction(U64(0)).is_none());
    assert_eq!(c.auction_for_token("s1".into()), None);
    // Listing the share again works and costs one record only.
    ctx("reg.root.testnet", T0 + 30 * SEC, 0);
    let msg = list_msg(3600);
    assert!(!c.nft_on_transfer(
        acc("seller.testnet"),
        acc("seller.testnet"),
        "s1".into(),
        msg
    ));
    assert_eq!(c.open_auctions, 1);
    assert_eq!(c.auction_for_token("s1".into()), Some(U64(1)));
}

fn long_name(prefix: char, i: usize) -> String {
    // 64 characters, the longest account id.
    format!("{}{i}.testnet", prefix.to_string().repeat(55))
}

/// Bytes the contract's map entries take on a real node: key plus value plus
/// 40 bytes of record overhead each. The mock does not track storage use, so
/// the stored entries are counted directly (this empties the mock).
fn stored_bytes() -> u64 {
    near_sdk::mock::with_mocked_blockchain(|b| {
        b.take_storage()
            .iter()
            .map(|(k, v)| (k.len() + v.len() + 40) as u64)
            .sum()
    })
}

/// Runs the worst case flow and returns the stored bytes after the bids
/// (`stop` 0) or after the sale (`stop` 1).
fn worst_case(stop: u8) -> u64 {
    let r = std::thread::spawn(move || {
        let mut c = with_key();
        let seller = long_name('s', 0);
        ctx("reg.root.testnet", T0, 0);
        assert!(!c.nft_on_transfer(acc(&seller), acc(&seller), "s123456".into(), list_msg(3600)));
        let bidders: Vec<String> = (1..=8).map(|i| long_name('b', i)).collect();
        for who in &bidders {
            ctx("usdc.root.testnet", T0 + 10 * SEC, 0);
            let ct = "A".repeat(MAX_CIPHERTEXT);
            let _ = c.ft_on_transfer(
                acc(who),
                U128(MAX),
                json!({"auction_id": "0", "ciphertext": ct}).to_string(),
            );
            ctx_result("auction.root.testnet", T0 + 10 * SEC, ok_bytes("true"));
            assert_eq!(c.on_bid_checked(acc(who), U128(MAX), 0, ct).0, 0);
        }
        if stop == 1 {
            ctx(
                "anyone.testnet",
                after_end(),
                20_000_000_000_000_000_000_000,
            );
            assert!(c.settle(U64(0)).is_some());
            let out = answer(&c, Some((&bidders[0], 30_000_000)));
            ctx_result("auction.root.testnet", after_end() + SEC, ok_bytes(&out));
            assert!(c.on_settled(0, 1));
            ctx_result(
                "auction.root.testnet",
                after_end() + 2 * SEC,
                PromiseResult::Successful(vec![]),
            );
            assert!(c.on_delivered(0));
            assert_eq!(c.auction(0).status, Status::Sold);
            assert_eq!(c.owed_total, 8 * MAX);
        }
        // The store collections write on flush (normally when the contract
        // value is dropped at the end of a call).
        c.auctions.flush();
        c.owed.flush();
        c.token_auction.flush();
        stored_bytes()
    })
    .join()
    .expect("worst case run failed");
    r
}

/// B-04: measures the worst case against `STORAGE_RESERVE_BYTES`: a seller and
/// eight bidders with 64 character names, eight 512 character ciphertexts,
/// nine owed entries after the sale.
#[test]
fn the_storage_reserve_covers_the_worst_case() {
    let after_bids = worst_case(0);
    let after_sale = worst_case(1);
    println!("worst case stored bytes: after bids {after_bids}, after sale {after_sale}");
    assert!(after_bids <= STORAGE_RESERVE_BYTES, "bids use {after_bids}");
    assert!(
        after_sale <= STORAGE_RESERVE_BYTES,
        "sale uses {after_sale}"
    );
}

/// B-05: a withdrawal leaves a durable record until its callback closes it.
/// A lost callback keeps the claim visible; it is never refunded blindly.
#[test]
fn a_withdrawal_leaves_a_pending_record_until_its_callback() {
    let mut c = sold();
    ctx("anyone.testnet", after_end() + SEC, 0);
    c.withdraw(vec![acc("seller.testnet")]);
    let p = c.get_pending_withdrawal(U64(0)).expect("pending record");
    assert_eq!(p.account, acc("seller.testnet"));
    assert_eq!(p.amount.0, 30_000_000);
    assert_eq!(c.get_accounting().pending_total.0, 30_000_000);
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
    // The callback never runs: the record and the total stay, no refund.
    ctx("anyone.testnet", after_end() + 100 * 3600 * SEC, 0);
    assert!(c.get_pending_withdrawal(U64(0)).is_some());
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 0);
    // A normal callback closes it.
    ctx_result(
        "auction.root.testnet",
        after_end() + 2 * SEC,
        PromiseResult::Successful(vec![]),
    );
    assert!(c.on_withdrawn(0, acc("seller.testnet"), U128(30_000_000)));
    assert!(c.get_pending_withdrawal(U64(0)).is_none());
    assert_eq!(c.get_accounting().pending_total.0, 0);
}

#[test]
fn a_failed_withdrawal_closes_its_record_and_restores_the_credit() {
    let mut c = sold();
    ctx("anyone.testnet", after_end() + SEC, 0);
    c.withdraw(vec![acc("seller.testnet")]);
    ctx_result(
        "auction.root.testnet",
        after_end() + 2 * SEC,
        PromiseResult::Failed,
    );
    assert!(!c.on_withdrawn(0, acc("seller.testnet"), U128(30_000_000)));
    assert!(c.get_pending_withdrawal(U64(0)).is_none());
    assert_eq!(c.get_accounting().pending_total.0, 0);
    assert_eq!(c.get_owed(acc("seller.testnet")).0, 30_000_000);
}
