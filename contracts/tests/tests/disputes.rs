//! Sandbox test 14 from the build plan and the block 4 dispute paths:
//! over-allocation freezes only the contested fraction, plain deposits
//! into a contested vault are held until resolve, resolve releases escrow,
//! duplicate codes can be voided or flipped, and confirm_share. Every test
//! ends with the solvency check (T31) and the zero-keys proof.

mod common;

use common::*;
use near_workspaces::types::NearToken;
use near_workspaces::Account;
use serde_json::{json, Value};

async fn mint_contesting(
    e: &Env,
    work_id: u64,
    holder: &Account,
    bps: u16,
    contests: Vec<&str>,
) -> anyhow::Result<(String, u64)> {
    let r = ok(e
        .writer_a
        .call(e.reg.id(), "register_share")
        .args_json(json!({
            "work_id": work_id, "right_type": "publishing", "territory": "WW",
            "percent_bps": bps, "basis": "copyright", "holder": holder.id(),
            "evidence_hash": hash('e'), "contests": contests,
        }))
        .deposit(NearToken::from_millinear(500))
        .max_gas()
        .transact()
        .await?);
    assert!(r.clone().json::<bool>()?, "claimant mint must succeed");
    let opened = r
        .logs()
        .iter()
        .find(|l| l.contains("\"dispute_opened\""))
        .map(|l| serde_json::from_str::<Value>(l.trim_start_matches("EVENT_JSON:")).unwrap())
        .expect("dispute_opened event");
    let data = &opened["data"][0];
    Ok((
        data["claimant"].as_str().unwrap().to_string(),
        data["dispute_id"].as_u64().unwrap(),
    ))
}

async fn resolve(e: &Env, id: u64, outcome: Value) -> anyhow::Result<()> {
    let r = ok(e
        .registrar
        .call(e.reg.id(), "resolve")
        .args_json(json!({ "dispute_id": id, "outcome": outcome, "doc_hash": hash('7') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r
        .logs()
        .iter()
        .any(|l| l.contains("dispute_resolved") && l.contains(&hash('7'))));
    Ok(())
}

async fn sync(e: &Env, token_id: &str) -> anyhow::Result<bool> {
    Ok(e.outsider
        .call(e.reg.id(), "sync")
        .args_json(json!({ "token_id": token_id }))
        .max_gas()
        .transact()
        .await?
        .is_success())
}

async fn plain_transfer(
    e: &Env,
    to: &near_workspaces::AccountId,
    amount: u128,
) -> anyhow::Result<()> {
    check(
        e.payer
            .call(e.usdc.id(), "ft_transfer")
            .args_json(json!({ "receiver_id": to, "amount": amount.to_string() }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    Ok(())
}

/// Test 14 (T11, T24): shares above 100 percent open a dispute; only the
/// contested fraction freezes; other shares still paid; the frozen share
/// cannot transfer; a plain deposit into the contested vault is held and
/// sync is refused; resolve releases escrow by the new split.
#[tokio::test]
async fn t14_over_allocation_dispute_freezes_only_contested_fraction() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 5_000).await?;
    let b = e.mint(w, e.writer_b.id(), 5_000).await?;
    check(e.pay_work(w, 200_000).await?); // old money: 100000 each

    let (c, id) = mint_contesting(&e, w, &e.buyer, 5_000, vec![&b]).await?;
    let d: Value = e
        .reg
        .view("get_dispute")
        .args_json(json!({ "dispute_id": id }))
        .await?
        .json()?;
    println!("dispute: {d}");
    assert_eq!(d["status"], "open");

    check(e.pay_work(w, 1_000_000).await?);
    assert_eq!(
        e.balance(&e.vault(&a)).await?,
        600_000,
        "uncontested share paid"
    );
    assert_eq!(
        e.balance(&e.vault(&b)).await?,
        100_000,
        "contested share frozen"
    );
    assert_eq!(e.balance(&e.vault(&c)).await?, 0, "claimant not paid");
    assert_eq!(num(&e.ledger(&b).await?["escrowed"]), 500_000);
    assert_eq!(
        e.balance(e.reg.id()).await?,
        500_000,
        "escrow held by the registry"
    );

    // A frozen share cannot transfer; nor can the claimant's.
    assert!(e
        .transfer(&e.writer_b, e.buyer.id(), &b)
        .await?
        .is_failure());
    assert!(e
        .transfer(&e.buyer, e.writer_b.id(), &c)
        .await?
        .is_failure());
    // Plain deposit straight into the contested vault: held, sync refused.
    plain_transfer(&e, &e.vault(&b), 70_000).await?;
    assert!(!sync(&e, &b).await?, "sync refused while disputed");
    check(e.claim(&e.writer_b, &b).await?);
    assert_eq!(
        e.balance(e.writer_b.id()).await?,
        100_000,
        "only old money is claimable"
    );
    assert_eq!(e.balance(&e.vault(&b)).await?, 70_000, "held deposit stays");
    e.assert_solvent().await?;

    resolve(
        &e,
        id,
        json!({ "uphold_claimant": { "new_bps": [
            { "token_id": b, "bps": 2500 }, { "token_id": c, "bps": 2500 },
        ] } }),
    )
    .await?;
    assert_eq!(e.balance(&e.vault(&b)).await?, 70_000 + 250_000);
    assert_eq!(e.balance(&e.vault(&c)).await?, 250_000);
    assert_eq!(e.balance(e.reg.id()).await?, 0);
    // After resolve the held deposit is picked up and claimable.
    assert!(sync(&e, &b).await?);
    check(e.claim(&e.writer_b, &b).await?);
    assert_eq!(e.balance(e.writer_b.id()).await?, 100_000 + 320_000);
    check(e.claim(&e.buyer, &c).await?);
    assert_eq!(e.balance(e.buyer.id()).await?, 250_000);

    // New split 5000 / 2500 / 2500, sum exact.
    check(e.pay_work(w, 1_000_003).await?);
    let sum = e.balance(&e.vault(&a)).await? - 600_000
        + e.balance(&e.vault(&b)).await?
        + e.balance(&e.vault(&c)).await?;
    assert_eq!(sum, 1_000_003);
    check(e.transfer(&e.buyer, e.writer_a.id(), &c).await?);
    for t in [&a, &b, &c] {
        e.assert_keyless(t).await?;
    }
    e.assert_solvent().await
}

/// Test 14, the other outcome: uphold the existing shares. The claimant is
/// voided and cannot move; escrow goes back to the contested share.
#[tokio::test]
async fn t14b_uphold_existing_voids_claimant() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 6_000).await?;
    let b = e.mint(w, e.writer_b.id(), 4_000).await?;
    let (c, id) = mint_contesting(&e, w, &e.buyer, 3_000, vec![&a]).await?;
    check(e.pay_work(w, 1_000_000).await?);
    assert_eq!(e.balance(&e.vault(&b)).await?, 400_000);
    assert_eq!(e.balance(&e.vault(&a)).await?, 0);
    // Only the registrar resolves.
    assert!(e
        .writer_a
        .call(e.reg.id(), "resolve")
        .args_json(json!({ "dispute_id": id, "outcome": "uphold_existing", "doc_hash": hash('7') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?
        .is_failure());
    resolve(&e, id, json!("uphold_existing")).await?;
    assert_eq!(e.balance(&e.vault(&a)).await?, 600_000);
    let s: Value = e
        .reg
        .view("get_share")
        .args_json(json!({ "token_id": c }))
        .await?
        .json()?;
    assert_eq!(s["status"], "void");
    assert!(e
        .transfer(&e.buyer, e.writer_a.id(), &c)
        .await?
        .is_failure());
    check(e.pay_work(w, 10_000).await?);
    assert_eq!(e.balance(&e.vault(&a)).await?, 606_000);
    assert_eq!(e.balance(&e.vault(&c)).await?, 0);
    e.assert_keyless(&c).await?;
    e.assert_solvent().await
}

/// Duplicate ISRC: payments to the duplicate are refused (payer keeps the
/// money); the registrar flips which entry is payable; then the original
/// is refused and the duplicate paid.
#[tokio::test]
async fn t14c_duplicate_code_swap_payable() -> anyhow::Result<()> {
    let e = setup().await?;
    let w1 = e.register_work("USRC12600001").await?;
    let w2 = e.register_work("USRC12600001").await?;
    let t1 = e.mint(w1, e.writer_a.id(), 10_000).await?;
    let t2 = e.mint(w2, e.writer_b.id(), 10_000).await?;
    let work2: Value = e
        .reg
        .view("get_work")
        .args_json(json!({ "work_id": w2 }))
        .await?
        .json()?;
    let id = work2["status"]["duplicate_open"]["dispute_id"]
        .as_u64()
        .unwrap();

    let before = e.balance(e.payer.id()).await?;
    let _ = e.pay_work(w2, 50_000).await?;
    assert_eq!(
        e.balance(e.payer.id()).await?,
        before,
        "refused and refunded"
    );
    check(e.pay_work(w1, 50_000).await?);
    assert_eq!(e.balance(&e.vault(&t1)).await?, 50_000);

    resolve(&e, id, json!("swap_payable")).await?;
    let before = e.balance(e.payer.id()).await?;
    let _ = e.pay_work(w1, 50_000).await?;
    assert_eq!(e.balance(e.payer.id()).await?, before, "old entry refused");
    check(e.pay_work(w2, 50_000).await?);
    assert_eq!(e.balance(&e.vault(&t2)).await?, 50_000);
    // The voided share's holder keeps what it earned before.
    check(e.claim(&e.writer_a, &t1).await?);
    assert_eq!(e.balance(e.writer_a.id()).await?, 50_000);
    assert!(e
        .transfer(&e.writer_a, e.buyer.id(), &t1)
        .await?
        .is_failure());
    e.assert_solvent().await
}

/// confirm_share: counterparty_confirmed once every other holder in the
/// group confirmed; it drops when a confirming holder's share is sold.
#[tokio::test]
async fn b4_confirm_share_tier() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let a = e.mint(w, e.writer_a.id(), 5_000).await?;
    let b = e.mint(w, e.writer_b.id(), 5_000).await?;
    let tier = |t: String| {
        let reg = e.reg.clone();
        async move {
            let v: String = reg
                .view("get_evidence_tier")
                .args_json(json!({ "token_id": t }))
                .await?
                .json()?;
            anyhow::Ok(v)
        }
    };
    assert_eq!(tier(a.clone()).await?, "self_asserted");
    check(
        e.writer_b
            .call(e.reg.id(), "confirm_share")
            .args_json(json!({ "token_id": a }))
            .deposit(NearToken::from_millinear(10))
            .transact()
            .await?,
    );
    assert_eq!(tier(a.clone()).await?, "counterparty_confirmed");
    check(e.transfer(&e.writer_b, e.buyer.id(), &b).await?);
    assert_eq!(tier(a.clone()).await?, "self_asserted");
    e.assert_solvent().await
}
