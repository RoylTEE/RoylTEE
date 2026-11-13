//! Sandbox tests 1 to 7 and 16 from the build plan, plus the zero-keys
//! proof, T16 (sum of credits equals the payment) and decisions 3 and 4.

mod common;

use common::*;
use near_workspaces::types::NearToken;
use serde_json::{json, Value};

/// Test 1 (T1, T15): mint creates a vault with zero access keys, running
/// the embedded vault code, registered with mock USDC.
#[tokio::test]
async fn t01_mint_creates_keyless_registered_vault() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    assert_eq!(e.owner(&t).await?, e.writer_a.id().as_str());

    e.assert_keyless(&t).await?;
    let code = e.worker.view_code(&e.vault(&t)).await?;
    assert_eq!(
        code,
        wasm("vault"),
        "vault runs exactly the embedded vault code"
    );
    let reg_storage: Value = e
        .usdc
        .view("storage_balance_of")
        .args_json(json!({ "account_id": e.vault(&t) }))
        .await?
        .json()?;
    assert!(
        !reg_storage.is_null(),
        "vault is storage-registered with mock USDC"
    );
    let share: Value = e
        .reg
        .view("get_share")
        .args_json(json!({ "token_id": t }))
        .await?
        .json()?;
    assert_eq!(share["status"], "active");
    let meta: Value = e.reg.view("nft_metadata").await?.json()?;
    assert_eq!(meta["symbol"], project_config()["nftSymbol"]);
    let ft_meta: Value = e.usdc.view("ft_metadata").await?.json()?;
    assert_eq!(ft_meta["decimals"], 6);
    assert_eq!(ft_meta["name"], project_config()["mockTokenName"]);
    assert_eq!(ft_meta["symbol"], project_config()["mockTokenSymbol"]);

    let vault_acc = e.worker.view_account(&e.vault(&t)).await?;
    println!(
        "vault {} balance {} storage {} bytes",
        e.vault(&t),
        vault_acc.balance,
        vault_acc.storage_usage
    );
    e.assert_solvent().await
}

/// Test 2: a plain ft_transfer straight into a vault, then sync: earned updates.
#[tokio::test]
async fn t02_plain_transfer_then_sync_updates_earned() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    check(
        e.payer
            .call(e.usdc.id(), "ft_transfer")
            .args_json(json!({ "receiver_id": e.vault(&t), "amount": "250000" }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    assert_eq!(
        num(&e.ledger(&t).await?["earned"]),
        0,
        "not seen before sync"
    );
    // Anyone can sync.
    check(
        e.outsider
            .call(e.reg.id(), "sync")
            .args_json(json!({ "token_id": t }))
            .max_gas()
            .transact()
            .await?,
    );
    let l = e.ledger(&t).await?;
    assert_eq!(num(&l["earned"]), 250_000);
    assert_eq!(num(&l["claimable"]), 250_000);
    let tok: Value = e
        .reg
        .view("nft_token")
        .args_json(json!({ "token_id": t }))
        .await?
        .json()?;
    let extra: Value = serde_json::from_str(tok["metadata"]["extra"].as_str().unwrap())?;
    assert_eq!(extra["earned"], "250000");
    e.assert_solvent().await
}

/// Test 3 (T2): owner claim pays the current owner; non-owner claim fails.
#[tokio::test]
async fn t03_owner_claims_non_owner_fails() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    check(e.pay_share(&t, 400_000).await?);
    assert_eq!(e.balance(&e.vault(&t)).await?, 400_000);

    let r = e.claim(&e.writer_b, &t).await?;
    assert!(r.is_failure(), "non-owner claim must fail");
    let r = e.claim(&e.outsider, &t).await?;
    assert!(r.is_failure(), "outsider claim must fail");
    assert_eq!(e.balance(&e.vault(&t)).await?, 400_000);

    let r = ok(e.claim(&e.writer_a, &t).await?);
    assert!(r.json::<bool>()?, "claim settled ok");
    assert_eq!(e.balance(e.writer_a.id()).await?, 400_000);
    assert_eq!(e.balance(&e.vault(&t)).await?, 0);
    let l = e.ledger(&t).await?;
    assert_eq!(
        (
            num(&l["claimable"]),
            num(&l["total_paid_out"]),
            num(&l["earned"])
        ),
        (0, 400_000, 400_000)
    );
    assert!(
        e.claim(&e.writer_a, &t).await?.is_failure(),
        "nothing left to claim"
    );
    e.assert_solvent().await
}

/// Test 4 (T2, the catalog-sale scene): pay, transfer, pay again. The seller
/// cannot claim anything; the buyer claims both. Zero keys before and after.
#[tokio::test]
async fn t04_catalog_sale_balance_travels_with_token() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 5_000).await?;
    let b = e.mint(w, e.writer_b.id(), 5_000).await?;
    e.assert_keyless(&a).await?;

    check(e.pay_work(w, 1_000_000).await?);
    check(e.transfer(&e.writer_a, e.buyer.id(), &a).await?);
    assert_eq!(e.owner(&a).await?, e.buyer.id().as_str());
    check(e.pay_work(w, 1_000_000).await?);

    let r = e.claim(&e.writer_a, &a).await?;
    assert!(r.is_failure(), "seller cannot claim after the sale");
    assert_eq!(e.balance(e.writer_a.id()).await?, 0);

    check(e.claim(&e.buyer, &a).await?);
    assert_eq!(
        e.balance(e.buyer.id()).await?,
        1_000_000,
        "buyer gets both payments"
    );
    check(e.claim(&e.writer_b, &b).await?);
    assert_eq!(e.balance(e.writer_b.id()).await?, 1_000_000);
    e.assert_keyless(&a).await?;
    e.assert_keyless(&b).await?;
    e.assert_solvent().await
}

/// Test 5 (T10): transfer to a non-credentialed account fails.
#[tokio::test]
async fn t05_transfer_to_outsider_fails() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    let r = e.transfer(&e.writer_a, e.outsider.id(), &t).await?;
    assert!(r.is_failure());
    assert!(format!("{:?}", r.failures()).contains("receiver has no active credential"));
    let r = e
        .writer_a
        .call(e.reg.id(), "nft_transfer_call")
        .args_json(json!({ "receiver_id": e.outsider.id(), "token_id": t, "msg": "" }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_failure());
    // Without 1 yoctoNEAR (what a function-call key can send) it fails too.
    let r = e
        .writer_a
        .call(e.reg.id(), "nft_transfer")
        .args_json(json!({ "receiver_id": e.buyer.id(), "token_id": t }))
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_failure());
    assert_eq!(e.owner(&t).await?, e.writer_a.id().as_str());
    Ok(())
}

/// Test 6 (T3): a transfer that lands while a payout is in flight fails;
/// after the callback it succeeds; nobody is paid twice.
#[tokio::test]
async fn t06_transfer_during_inflight_payout_fails() -> anyhow::Result<()> {
    let e = setup().await?;
    let holder = e.deploy_receiver().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, holder.id(), 10_000).await?;
    check(e.pay_share(&t, 600_000).await?);

    // claim and nft_transfer leave the holder in one receipt, claim first.
    let r = holder
        .call("claim_and_transfer")
        .args_json(json!({ "token_id": t, "receiver_id": e.buyer.id() }))
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_success());
    let attempts: Value = holder.view("get_attempts").await?.json()?;
    println!("attempts: {attempts}");
    let claim_ok = attempts
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"] == "claim")
        .unwrap()["succeeded"]
        .clone();
    let transfer_ok = attempts
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"].as_str().unwrap().starts_with("transfer"))
        .unwrap()["succeeded"]
        .clone();
    assert_eq!(claim_ok, true, "claim went through");
    assert_eq!(
        transfer_ok, false,
        "transfer during in-flight payout must fail"
    );
    assert!(
        r.logs().iter().any(|l| l.contains("payout in flight"))
            || format!("{:?}", r.receipt_failures()).contains("payout in flight"),
        "failure reason is the in-flight lock"
    );
    assert_eq!(e.owner(&t).await?, holder.id().as_str());
    assert_eq!(e.balance(holder.id()).await?, 600_000);

    // After the callback cleared the lock, the transfer succeeds.
    assert!(e.ledger(&t).await?["payout_in_flight"].is_null());
    check(
        holder
            .call("transfer_now")
            .args_json(json!({ "token_id": t, "receiver_id": e.buyer.id() }))
            .max_gas()
            .transact()
            .await?,
    );
    assert_eq!(e.owner(&t).await?, e.buyer.id().as_str());

    // No double pay: the buyer has nothing to claim; the vault is empty.
    assert!(e.claim(&e.buyer, &t).await?.is_failure());
    assert_eq!(e.balance(&e.vault(&t)).await?, 0);
    assert_eq!(e.balance(e.buyer.id()).await?, 0);
    let l = e.ledger(&t).await?;
    assert_eq!(
        (num(&l["total_paid_out"]), num(&l["claimable"])),
        (600_000, 0)
    );
    e.assert_solvent().await
}

/// Test 7 (T4): nft_transfer_call to a member contract that tries to claim
/// (and, in a second round, transfer onward) during nft_on_transfer and then
/// returns the token. Both attempts fail, the token comes back, funds intact.
#[tokio::test]
async fn t07_transfer_call_bounce_cannot_claim() -> anyhow::Result<()> {
    let e = setup().await?;
    let attacker = e.deploy_receiver().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    check(e.pay_share(&t, 900_000).await?);

    for msg in ["claim".to_string(), format!("transfer:{}", e.buyer.id())] {
        let r = e
            .writer_a
            .call(e.reg.id(), "nft_transfer_call")
            .args_json(json!({ "receiver_id": attacker.id(), "token_id": t, "msg": msg }))
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await?;
        assert!(r.is_success());
        assert!(!r.clone().json::<bool>()?, "token was returned, not kept");
        assert!(r.logs().iter().any(|l| l.contains("transfer_returned")));
        assert_eq!(
            e.owner(&t).await?,
            e.writer_a.id().as_str(),
            "token back with the seller"
        );
        let l = e.ledger(&t).await?;
        assert!(l["transfer_pending"].is_null(), "lock cleared");
        assert!(l["payout_in_flight"].is_null());
        assert_eq!(num(&l["claimable"]), 900_000, "ledger intact");
        assert_eq!(e.balance(&e.vault(&t)).await?, 900_000, "vault intact");
        assert_eq!(e.balance(attacker.id()).await?, 0, "attacker got nothing");
    }
    let attempts: Value = attacker.view("get_attempts").await?.json()?;
    println!("attacker attempts: {attempts}");
    assert_eq!(attempts.as_array().unwrap().len(), 2);
    assert!(attempts
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["succeeded"] == false));

    // The rightful holder can still claim afterwards.
    check(e.claim(&e.writer_a, &t).await?);
    assert_eq!(e.balance(e.writer_a.id()).await?, 900_000);

    // A receiver that keeps the token: transfer completes normally.
    let r = ok(e
        .writer_a
        .call(e.reg.id(), "nft_transfer_call")
        .args_json(json!({ "receiver_id": attacker.id(), "token_id": t, "msg": "keep" }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r.json::<bool>()?);
    assert_eq!(e.owner(&t).await?, attacker.id().as_str());
    e.assert_solvent().await
}

/// Test 16 (T5): nft_approve panics; a transfer with approval_id fails.
#[tokio::test]
async fn t16_approvals_are_disabled() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let t = e.mint(w, e.writer_a.id(), 10_000).await?;
    let r = e
        .writer_a
        .call(e.reg.id(), "nft_approve")
        .args_json(json!({ "token_id": t, "account_id": e.buyer.id() }))
        .deposit(NearToken::from_millinear(10))
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_failure());
    assert!(format!("{:?}", r.failures()).contains("approvals are disabled"));
    let r = e
        .writer_a
        .call(e.reg.id(), "nft_transfer")
        .args_json(json!({ "receiver_id": e.buyer.id(), "token_id": t, "approval_id": 0 }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(r.is_failure());
    let approved: bool = e
        .reg
        .view("nft_is_approved")
        .args_json(json!({ "token_id": t, "approved_account_id": e.buyer.id() }))
        .await?
        .json()?;
    assert!(!approved);
    Ok(())
}

/// T16: a work payment splits by bps; credits sum to the payment to the
/// last unit; the remainder lands on the largest share.
#[tokio::test]
async fn t16b_split_sums_to_payment_exactly() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let s1 = e.mint(w, e.writer_a.id(), 3_333).await?;
    let s2 = e.mint(w, e.writer_b.id(), 3_334).await?;
    let s3 = e.mint(w, e.buyer.id(), 3_333).await?;
    let mut total = 0u128;
    for amount in [1_000_007u128, 10_001, 123_456_789] {
        let r = ok(e.pay_work(w, amount).await?);
        assert!(r.logs().iter().any(|l| l.contains("payment_in")));
        total += amount;
        let sum = e.balance(&e.vault(&s1)).await?
            + e.balance(&e.vault(&s2)).await?
            + e.balance(&e.vault(&s3)).await?;
        assert_eq!(sum, total, "vault balances must equal total paid exactly");
        if total == 1_000_007 {
            // floors 333302 + 333402 + 333302, remainder 1 to s2 (3334 bps).
            assert_eq!(e.balance(&e.vault(&s1)).await?, 333_302);
            assert_eq!(e.balance(&e.vault(&s2)).await?, 333_403);
            assert_eq!(e.balance(&e.vault(&s3)).await?, 333_302);
        }
    }
    let [a, b, c] = [
        e.balance(&e.vault(&s1)).await?,
        e.balance(&e.vault(&s2)).await?,
        e.balance(&e.vault(&s3)).await?,
    ];
    println!("T16 vault balances after 3 payments: {a} {b} {c}, total {total}");
    assert!(b > a && a == c, "largest share took the remainders");
    assert_eq!(
        e.balance(e.reg.id()).await?,
        0,
        "nothing left in the registry"
    );
    e.assert_solvent().await
}

/// Decision 4: a group under 100 percent pays registered shares their exact
/// bps; the rest is held as unallocated escrow until the registrar releases it.
#[tokio::test]
async fn d4_under_allocated_payment_escrows_unassigned_part() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 5_000).await?;
    check(e.pay_work(w, 1_000_000).await?);
    assert_eq!(e.balance(&e.vault(&a)).await?, 500_000);
    assert_eq!(
        e.balance(e.reg.id()).await?,
        500_000,
        "unassigned half held by the registry"
    );
    let g: Value = e
        .reg
        .view("get_group")
        .args_json(json!({ "work_id": w, "right_type": "publishing", "territory": "WW" }))
        .await?
        .json()?;
    assert_eq!(g["unallocated"], "500000");
    e.assert_solvent().await?;

    let b = e.mint(w, e.writer_b.id(), 5_000).await?;
    let args = json!({
        "group": { "work_id": w, "right_type": "publishing", "territory": "WW" },
        "token_id": b, "amount": "500000", "doc_hash": hash('6'),
    });
    let r = e
        .writer_a
        .call(e.reg.id(), "release_unallocated")
        .args_json(&args)
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_failure(), "only the registrar can release");
    let r = ok(e
        .registrar
        .call(e.reg.id(), "release_unallocated")
        .args_json(&args)
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r.logs().iter().any(|l| l.contains("unallocated_released")));
    assert_eq!(e.balance(&e.vault(&b)).await?, 500_000);
    assert_eq!(e.balance(e.reg.id()).await?, 0);
    e.assert_solvent().await
}

/// Decision 3: a suspended holder keeps accruing but cannot claim, send or
/// receive; only the registrar can move the share, with a document hash.
#[tokio::test]
async fn d3_suspended_holder_frozen_registrar_can_move() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 5_000).await?;
    let b = e.mint(w, e.writer_b.id(), 5_000).await?;
    check(
        e.registrar
            .call(e.reg.id(), "suspend_credential")
            .args_json(json!({ "account": e.writer_a.id(), "doc_hash": hash('9') }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    check(e.pay_work(w, 1_000_000).await?);
    assert_eq!(e.balance(&e.vault(&a)).await?, 500_000, "keeps accruing");
    assert!(e.claim(&e.writer_a, &a).await?.is_failure(), "cannot claim");
    assert!(
        e.transfer(&e.writer_a, e.buyer.id(), &a)
            .await?
            .is_failure(),
        "cannot send"
    );
    assert!(
        e.transfer(&e.writer_b, e.writer_a.id(), &b)
            .await?
            .is_failure(),
        "cannot receive"
    );

    let r = ok(e
        .registrar
        .call(e.reg.id(), "registrar_transfer")
        .args_json(json!({ "token_id": a, "receiver_id": e.buyer.id(), "doc_hash": hash('8') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r
        .logs()
        .iter()
        .any(|l| l.contains("registrar_transfer") && l.contains(&hash('8'))));
    assert_eq!(e.owner(&a).await?, e.buyer.id().as_str());
    check(e.claim(&e.buyer, &a).await?);
    assert_eq!(e.balance(e.buyer.id()).await?, 500_000);
    e.assert_keyless(&a).await?;
    e.assert_solvent().await
}
