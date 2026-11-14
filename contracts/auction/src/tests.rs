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
        acc("outlayer.testnet"),
        "https://test.fastfs.io/auction.root.testnet/outlayer.testnet/aa.wasm".into(),
        "a".repeat(64),
        acc("owner.testnet"),
        "royltee-auction".into(),
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
        c.ft_on_transfer(acc("bob.testnet"), U128(MAX), "{}".into());
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

#[test]
fn anyone_cancels_an_unsettled_auction_after_the_grace_period() {
    let mut c = listed();
    place(&mut c, "bob.testnet", 1);
    ctx("anyone.testnet", T0 + 3600 * SEC + SETTLE_GRACE_NS, 0);
    assert!(
        panics(std::panic::AssertUnwindSafe(|| {
            c.cancel(U64(0));
        })),
        "grace not over"
    );
    ctx("anyone.testnet", T0 + 3600 * SEC + SETTLE_GRACE_NS + 1, 0);
    assert!(c.cancel(U64(0)).is_some());
    assert_eq!(c.get_owed(acc("bob.testnet")).0, MAX);
    assert_eq!(c.escrow_total, 0);
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
    assert!(!c.on_withdrawn(acc("seller.testnet"), U128(30_000_000)));
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
    let good = format!(r#"{{"v":1,"ok":true,"pubkey":"{PK}"}}"#);
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
        ok_bytes(r#"{"v":1,"ok":true,"pubkey":"zz"}"#),
        ok_bytes(
            r#"{"v":1,"ok":false,"pubkey":"c99e0b3a826c8f3852bd2c40073f4c3b857f24af0d3ce42561e761402056ea7f"}"#,
        ),
        ok_bytes(r#"{"v":1,"ok":true}"#),
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
                acc("outlayer.testnet"),
                u,
                h,
                acc("owner.testnet"),
                p,
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
