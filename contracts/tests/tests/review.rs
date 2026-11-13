//! Regression tests for the contract review fixes (H1, M1, M2), ported
//! from the review's sandbox proofs of concept.

mod common;

use common::*;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::NearToken;
use near_workspaces::Account;
use serde_json::{json, Value};

async fn mint_tokens(e: &Env, to: &Account, amount: u128) -> anyhow::Result<()> {
    check(
        e.payer
            .call(e.usdc.id(), "mint")
            .args_json(json!({ "account_id": to.id(), "amount": amount.to_string() }))
            .transact()
            .await?,
    );
    Ok(())
}

async fn known(e: &Env, t: &str) -> anyhow::Result<u128> {
    Ok(num(&e.ledger(t).await?["known_balance"]))
}

async fn sync(e: &Env, who: &Account, t: &str) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.reg.id(), "sync")
        .args_json(json!({ "token_id": t }))
        .max_gas()
        .transact()
        .await?)
}

/// H1: the phantom balance PoC needed an outsider sync timed against an
/// ft_transfer_call refund. Now only the holder or the registrar may sync,
/// and a new vault keeps tokens sent with ft_transfer_call, so there is no
/// refund to race.
#[tokio::test]
async fn r01_h1_outsider_cannot_sync_and_new_vaults_keep_transfer_call() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let b = e.mint(w, e.writer_b.id(), 10_000).await?;
    check(e.pay_work(w, 100_000).await?);
    let r = sync(&e, &e.outsider, &b).await?;
    assert!(!r.is_success(), "outsider sync must fail");
    assert!(format!("{:?}", r.failures()).contains("only the holder or the registrar"));

    let x = 500_000u128;
    mint_tokens(&e, &e.writer_b, x).await?;
    let r = ok(e
        .writer_b
        .call(e.usdc.id(), "ft_transfer_call")
        .args_json(json!({ "receiver_id": e.vault(&b), "amount": x.to_string(), "msg": "" }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert_eq!(
        r.json::<String>()?,
        x.to_string(),
        "all of x used, none refunded"
    );
    assert_eq!(e.balance(&e.vault(&b)).await?, 100_000 + x);
    assert_eq!(e.balance(e.writer_b.id()).await?, 0);
    check(sync(&e, &e.writer_b, &b).await?);
    assert_eq!(
        known(&e, &b).await?,
        100_000 + x,
        "ledger matches the vault"
    );
    check(e.claim(&e.writer_b, &b).await?);
    assert_eq!(e.balance(&e.vault(&b)).await?, 0);
    e.assert_solvent().await
}

async fn register_by(
    e: &Env,
    who: &Account,
    work_id: u64,
    bps: u16,
) -> anyhow::Result<(String, ExecutionFinalResult)> {
    let r = ok(who
        .call(e.reg.id(), "register_share")
        .args_json(json!({
            "work_id": work_id, "right_type": "publishing", "territory": "WW",
            "percent_bps": bps, "basis": "copyright", "holder": who.id(),
            "evidence_hash": hash('e'),
        }))
        .deposit(NearToken::from_millinear(500))
        .max_gas()
        .transact()
        .await?);
    let t = r
        .logs()
        .iter()
        .find(|l| l.contains("\"share_registered\""))
        .and_then(|l| {
            let v: Value = serde_json::from_str(l.trim_start_matches("EVENT_JSON:")).ok()?;
            v["data"][0]["token_id"].as_str().map(str::to_string)
        })
        .expect("share_registered event");
    Ok((t, r))
}

async fn approval(e: &Env, t: &str) -> anyhow::Result<String> {
    Ok(e.reg
        .view("get_share_approval")
        .args_json(json!({ "token_id": t }))
        .await?
        .json()?)
}

/// M2: a share registered by someone other than the work registrant is
/// held until the registrant approves it; a rejected one sends its held
/// money to the group's unallocated escrow.
#[tokio::test]
async fn r02_m2_split_sheet_approval_and_rejection() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?; // writer_a registers
    let (t, r) = register_by(&e, &e.writer_b, w, 6_000).await?;
    assert!(r
        .logs()
        .iter()
        .any(|l| l.contains("\"approval\":\"pending\"")));
    assert_eq!(approval(&e, &t).await?, "pending");
    let a = e.mint(w, e.writer_a.id(), 4_000).await?;
    check(e.pay_work(w, 100_000).await?);
    assert_eq!(e.balance(&e.vault(&t)).await?, 0, "held, not forwarded");
    assert_eq!(e.balance(&e.vault(&a)).await?, 40_000);
    assert_eq!(num(&e.ledger(&t).await?["escrowed"]), 60_000);
    assert!(!sync(&e, &e.writer_b, &t).await?.is_success());
    assert!(!e
        .transfer(&e.writer_b, e.buyer.id(), &t)
        .await?
        .is_success());
    // The claimant cannot approve itself.
    let self_ok = e
        .writer_b
        .call(e.reg.id(), "approve_share")
        .args_json(json!({ "token_id": t }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(!self_ok.is_success());
    let r = ok(e
        .writer_a
        .call(e.reg.id(), "approve_share")
        .args_json(json!({ "token_id": t }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r.logs().iter().any(|l| l.contains("share_approved")));
    assert_eq!(
        e.balance(&e.vault(&t)).await?,
        60_000,
        "held money released"
    );
    check(e.claim(&e.writer_b, &t).await?);
    assert_eq!(e.balance(e.writer_b.id()).await?, 60_000);

    // Rejection on a second work.
    let w2 = e.register_work("USRC12600002").await?;
    let (t2, _) = register_by(&e, &e.writer_b, w2, 10_000).await?;
    check(e.pay_work(w2, 50_000).await?);
    let r = ok(e
        .writer_a
        .call(e.reg.id(), "reject_share")
        .args_json(json!({ "token_id": t2 }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r.logs().iter().any(|l| l.contains("share_rejected")));
    assert_eq!(approval(&e, &t2).await?, "rejected");
    let acct: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(num(&acct["unallocated_total"]), 50_000);
    assert_eq!(num(&acct["escrow_total"]), 0);
    e.assert_solvent().await
}

fn callback_gas(r: &ExecutionFinalResult, reg: &str, tag: &str) -> Vec<u64> {
    r.receipt_outcomes()
        .iter()
        .filter(|o| o.executor_id.as_str() == reg && o.logs.iter().any(|l| l.contains(tag)))
        .map(|o| o.gas_burnt.as_gas())
        .collect()
}

fn silent_reg_gas(r: &ExecutionFinalResult, reg: &str) -> Vec<u64> {
    r.receipt_outcomes()
        .iter()
        .filter(|o| o.executor_id.as_str() == reg && o.logs.is_empty())
        .map(|o| o.gas_burnt.as_gas())
        .collect()
}

/// M1: measured callback gas against the static reservations
/// (GAS_FORWARD_CB 10, GAS_VAULT_READY_CB 20, GAS_SYNC_CB 5, GAS_CLAIM_CB
/// 10 Tgas), with a 16-share fan-out.
#[tokio::test]
async fn r03_m1_callback_gas_fits_the_reservations() -> anyhow::Result<()> {
    let e = setup().await?;
    let reg = e.reg.id().to_string();
    let w = e.register_work("USRC12600001").await?;
    let r = ok(e
        .writer_a
        .call(e.reg.id(), "register_share")
        .args_json(json!({
            "work_id": w, "right_type": "publishing", "territory": "WW",
            "percent_bps": 1000, "basis": "copyright", "holder": e.writer_a.id(),
            "evidence_hash": hash('e'),
        }))
        .deposit(NearToken::from_millinear(500))
        .max_gas()
        .transact()
        .await?);
    let ready = callback_gas(&r, &reg, "vault_ready");
    let mut shares = vec![];
    for _ in 0..15 {
        shares.push(e.mint(w, e.writer_a.id(), 600).await?);
    }
    let r = ok(e.pay_work(w, 1_000_000).await?);
    let fwd = silent_reg_gas(&r, &reg);
    let t = &shares[0];
    let r = ok(sync(&e, &e.writer_a, t).await?);
    let sync_gas = callback_gas(&r, &reg, "\"sync\"");
    let r = ok(e.claim(&e.writer_a, t).await?);
    let claim = callback_gas(&r, &reg, "claim_settled");
    let max = |v: &Vec<u64>| v.iter().copied().max().unwrap_or(0) as f64 / 1e12;
    println!(
        "GAS on_vault_ready {:.3} (20), on_forward max {:.3} over {} (10), on_sync {:.3} (5), on_claim {:.3} (10) Tgas",
        max(&ready),
        max(&fwd),
        fwd.len(),
        max(&sync_gas),
        max(&claim)
    );
    assert_eq!(fwd.len(), 16);
    assert!(max(&fwd) < 10.0 && max(&ready) < 20.0 && max(&sync_gas) < 5.0 && max(&claim) < 10.0);
    e.assert_solvent().await
}
