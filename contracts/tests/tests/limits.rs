//! Audit checks on the largest allowed group (MAX_SHARES_PER_GROUP = 16):
//! a work payment through ft_transfer_call reaches all 16 vaults, and a
//! resolve that releases escrow to 16 shares fits in one call.

mod common;

use common::*;
use serde_json::json;

#[tokio::test]
async fn a01_full_group_of_16_is_paid_in_one_transfer() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let mut shares = Vec::new();
    for _ in 0..16 {
        shares.push(e.mint(w, e.writer_a.id(), 625).await?);
    }
    let r = e.pay_work(w, 1_600_007).await?;
    println!(
        "16-share payment: gas burnt {} Tgas, success {}",
        r.total_gas_burnt.as_tgas(),
        r.is_success()
    );
    let r = ok(r);
    assert!(
        !r.logs().iter().any(|l| l.contains("payment_refused")),
        "must not be refused"
    );
    let mut sum = 0;
    for s in &shares {
        sum += e.balance(&e.vault(s)).await?;
    }
    assert_eq!(sum, 1_600_007);
    e.assert_solvent().await
}

#[tokio::test]
async fn a02_resolve_releasing_escrow_to_16_shares_fits() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let mut shares = Vec::new();
    for _ in 0..15 {
        shares.push(e.mint(w, e.writer_a.id(), 666).await?);
    }
    // 15 x 666 = 9990. A 10000 bps claimant needs all 15 to cover the
    // 9990 excess, so the minimal contested set is every share (review H2).
    let r = ok(e
        .writer_b
        .call(e.reg.id(), "register_share")
        .args_json(json!({
            "work_id": w, "right_type": "publishing", "territory": "WW",
            "percent_bps": 10_000, "basis": "copyright", "holder": e.buyer.id(),
            "evidence_hash": hash('e'), "contests": shares,
        }))
        .deposit(near_workspaces::types::NearToken::from_millinear(500))
        .max_gas()
        .transact()
        .await?);
    let claimant = format!("s{}", 16);
    assert!(r.logs().iter().any(|l| l.contains("dispute_opened")));
    check(e.pay_work(w, 999_000).await?);
    let mut new_bps: Vec<_> = shares
        .iter()
        .map(|t| json!({ "token_id": t, "bps": 660 }))
        .collect();
    new_bps.push(json!({ "token_id": claimant, "bps": 90 }));
    // The contested total was 9990, so new bps must sum to 9990.
    let r = e
        .registrar
        .call(e.reg.id(), "resolve")
        .args_json(json!({ "dispute_id": 0, "outcome": { "uphold_claimant": { "new_bps": new_bps } }, "doc_hash": hash('7') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    println!(
        "16-way resolve: gas burnt {} Tgas",
        r.total_gas_burnt.as_tgas()
    );
    check(r);
    let mut sum = e.balance(&e.vault(&claimant)).await?;
    for s in &shares {
        sum += e.balance(&e.vault(s)).await?;
    }
    // The group was at 9990 bps: 15 x floor(999000 * 666 / 10000) = 997995
    // went to escrow and is fully released; 1005 is unallocated (decision 4).
    assert_eq!(sum, 997_995, "escrowed amount fully released");
    assert_eq!(e.balance(e.reg.id()).await?, 1_005);
    e.assert_solvent().await
}

/// Audit M1: after deploy, only the contract account itself can call `new`.
#[tokio::test]
async fn a03_stranger_cannot_initialise_a_fresh_deploy() -> anyhow::Result<()> {
    let e = setup().await?;
    let root = e.worker.root_account()?;
    let fresh = root
        .create_subaccount("reg2")
        .initial_balance(near_workspaces::types::NearToken::from_near(20))
        .transact()
        .await?
        .into_result()?;
    let c = fresh.deploy(&wasm("registry")).await?.into_result()?;
    let args = json!({
        "registrar": e.outsider.id(), "usdc": e.usdc.id(),
        "metadata": { "spec": "nft-1.0.0", "name": "x", "symbol": "X" },
    });
    let r = e
        .outsider
        .call(c.id(), "new")
        .args_json(args.clone())
        .transact()
        .await?;
    assert!(r.is_failure(), "stranger init must fail");
    check(c.call("new").args_json(args).transact().await?);
    Ok(())
}

/// Audit F1: a direct work payment to the largest group (16 shares) fits
/// in the 300 Tgas that most wallets and the deploy plan attach to one
/// transaction. Before the fix each forward reserved 8 + 10 Tgas, so 16
/// forwards needed 298 Tgas inside `ft_on_transfer` and the payment was
/// refused. Also measures the gas each forward really burns against the
/// per forward reservations.
#[tokio::test]
async fn a04_full_group_of_16_is_paid_with_300_tgas() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let mut shares = Vec::new();
    for _ in 0..16 {
        shares.push(e.mint(w, e.writer_a.id(), 625).await?);
    }
    let r = e
        .pay_work_with_gas(w, 1_600_007, near_workspaces::types::Gas::from_tgas(300))
        .await?;
    let usdc = e.usdc.id().to_string();
    let reg = e.reg.id().to_string();
    let transfer_gas: Vec<u64> = r
        .receipt_outcomes()
        .iter()
        .filter(|o| {
            o.executor_id.as_str() == usdc
                && o.logs.iter().any(|l| {
                    l.contains("ft_transfer") && l.contains(&format!("\"old_owner_id\":\"{reg}\""))
                })
        })
        .map(|o| o.gas_burnt.as_gas())
        .collect();
    let forward_cb_gas: Vec<u64> = r
        .receipt_outcomes()
        .iter()
        .filter(|o| o.executor_id.as_str() == reg && o.logs.is_empty())
        .map(|o| o.gas_burnt.as_gas())
        .collect();
    let max = |v: &Vec<u64>| v.iter().copied().max().unwrap_or(0) as f64 / 1e12;
    println!(
        "16-share payment at 300 Tgas: gas burnt {} Tgas, success {}, ft_transfer max {:.3} Tgas over {}, on_forward max {:.3} Tgas over {}",
        r.total_gas_burnt.as_tgas(),
        r.is_success(),
        max(&transfer_gas),
        transfer_gas.len(),
        max(&forward_cb_gas),
        forward_cb_gas.len()
    );
    let r = ok(r);
    assert!(
        !r.logs().iter().any(|l| l.contains("payment_refused")),
        "must not be refused at 300 Tgas"
    );
    assert!(!r.logs().iter().any(|l| l.contains("forward_failed")));
    let mut sum = 0;
    for s in &shares {
        sum += e.balance(&e.vault(s)).await?;
    }
    assert_eq!(sum, 1_600_007);
    assert_eq!(transfer_gas.len(), 16);
    assert_eq!(forward_cb_gas.len(), 16);
    // Reservations per forward: GAS_FT_TRANSFER and GAS_FORWARD_CB (lib.rs).
    assert!(
        max(&transfer_gas) < 6.0,
        "ft_transfer within its reservation"
    );
    assert!(
        max(&forward_cb_gas) < 6.0,
        "on_forward within its reservation"
    );
    e.assert_solvent().await
}
