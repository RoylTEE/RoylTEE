//! Review 03a M3: the in-place redeploy planned in DEPLOY_PLAN step 15.
//!
//! Starts from the exact registry wasm deployed on testnet (fixture
//! `fixtures/registry-testnet-e7439e12.wasm`, read with the `view_code` RPC
//! from `reg.royltee-dev.testnet`; its sha256 is checked below), builds
//! state with it that mirrors the testnet demo (work payment, plain deposit
//! and sync, claims, the catalog sale, an open over-allocation dispute with
//! money in escrow), deploys the branch registry wasm on the same account
//! with no migration call, and checks that every record still reads back
//! and that payments, claims, the dispute resolve and new mints still work.

mod common;

use common::*;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, ContractState};
use serde_json::{json, Value};

/// Code hash of `reg.royltee-dev.testnet` (base58 sha256, as in
/// `deployments/testnet.json`). The sandbox computes it from the fixture.
const TESTNET_REGISTRY_CODE_HASH: &str = "GZkyojcHYDip7pZsDYvkgkjEtgGakPZZcX28cgSyAVhq";

fn testnet_registry_wasm() -> Vec<u8> {
    let path = format!(
        "{}/fixtures/registry-testnet-e7439e12.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|_| panic!("missing {path}"))
}

async fn code_hash(e: &Env, id: &near_workspaces::AccountId) -> anyhow::Result<ContractState> {
    Ok(e.worker.view_account(id).await?.contract_state)
}

async fn register_as(
    e: &Env,
    who: &Account,
    work_id: u64,
    holder: &Account,
    bps: u16,
    contests: Option<Vec<&str>>,
) -> anyhow::Result<near_workspaces::result::ExecutionFinalResult> {
    let mut args = json!({
        "work_id": work_id, "right_type": "publishing", "territory": "WW",
        "percent_bps": bps, "basis": "copyright", "holder": holder.id(),
        "evidence_hash": hash('e'),
    });
    if let Some(c) = contests {
        args["contests"] = json!(c);
    }
    Ok(who
        .call(e.reg.id(), "register_share")
        .args_json(args)
        .deposit(NearToken::from_millinear(500))
        .max_gas()
        .transact()
        .await?)
}

fn event(r: &near_workspaces::result::ExecutionFinalResult, name: &str) -> Value {
    r.logs()
        .iter()
        .find(|l| l.contains(&format!("\"{name}\"")))
        .map(|l| serde_json::from_str::<Value>(l.trim_start_matches("EVENT_JSON:")).unwrap())
        .unwrap_or_else(|| panic!("{name} event"))["data"][0]
        .clone()
}

#[tokio::test]
async fn u01_testnet_wasm_upgrades_in_place_without_migration() -> anyhow::Result<()> {
    let e = setup_with_registry(&testnet_registry_wasm()).await?;
    assert_eq!(
        code_hash(&e, e.reg.id()).await?,
        ContractState::LocalHash(TESTNET_REGISTRY_CODE_HASH.parse()?),
        "the fixture is the registry wasm deployed on testnet"
    );

    // State built with the deployed code, in the order of the testnet demo.
    let w0 = e.register_work("USRC12600001").await?;
    let s1 = e.mint(w0, e.writer_a.id(), 5_000).await?;
    let s2 = e.mint(w0, e.writer_b.id(), 5_000).await?;
    check(e.pay_work(w0, 1_000_000).await?);
    let w1 = e.register_work("USRC12600002").await?;
    let s3 = e.mint(w1, e.writer_a.id(), 10_000).await?;
    check(
        e.payer
            .call(e.usdc.id(), "ft_transfer")
            .args_json(json!({ "receiver_id": e.vault(&s3), "amount": "200000" }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    check(
        e.writer_a
            .call(e.reg.id(), "sync")
            .args_json(json!({ "token_id": s3 }))
            .max_gas()
            .transact()
            .await?,
    );
    check(e.claim(&e.writer_a, &s1).await?);
    check(e.transfer(&e.writer_b, e.buyer.id(), &s2).await?);
    let w2 = e.register_work("USRC12600003").await?;
    let s5 = e.mint(w2, e.writer_a.id(), 7_000).await?;
    let s6 = e.mint(w2, e.writer_b.id(), 3_000).await?;
    let r = ok(register_as(&e, &e.writer_b, w2, &e.writer_b, 2_000, Some(vec![&s5])).await?);
    let opened = event(&r, "dispute_opened");
    let claimant = opened["claimant"].as_str().unwrap().to_string();
    let dispute_id = opened["dispute_id"].as_u64().unwrap();
    check(e.pay_work(w2, 1_000_000).await?);
    e.assert_solvent().await?;

    let tokens_before: Value = e
        .reg
        .view("nft_tokens")
        .args_json(json!({ "from_index": "0", "limit": 100 }))
        .await?
        .json()?;
    let acct_before: Value = e.reg.view("get_accounting").await?.json()?;
    let mut ledgers_before = vec![];
    for t in [&s1, &s2, &s3, &s5, &s6, &claimant] {
        ledgers_before.push(e.ledger(t).await?);
    }
    assert_eq!(num(&acct_before["escrow_total"]), 700_000, "s5's part held");

    // In-place redeploy with the branch wasm. No migration call.
    let new_wasm = wasm("registry");
    e.reg.as_account().deploy(&new_wasm).await?.into_result()?;
    assert_eq!(
        e.worker.view_code(e.reg.id()).await?,
        new_wasm,
        "the branch registry code is live"
    );

    // Every record reads back unchanged.
    let tokens_after: Value = e
        .reg
        .view("nft_tokens")
        .args_json(json!({ "from_index": "0", "limit": 100 }))
        .await?
        .json()?;
    assert_eq!(tokens_after.as_array().unwrap().len(), 6);
    for (a, b) in tokens_before
        .as_array()
        .unwrap()
        .iter()
        .zip(tokens_after.as_array().unwrap())
    {
        assert_eq!(a["token_id"], b["token_id"]);
        assert_eq!(a["owner_id"], b["owner_id"]);
    }
    assert_eq!(
        e.owner(&s2).await?,
        e.buyer.id().to_string(),
        "catalog sale kept"
    );
    let acct_after: Value = e.reg.view("get_accounting").await?.json()?;
    for k in [
        "allowance_total",
        "unallocated_total",
        "escrow_total",
        "undelivered_total",
        "forwards_in_flight_total",
    ] {
        assert_eq!(acct_before[k], acct_after[k], "{k} unchanged");
    }
    for (i, t) in [&s1, &s2, &s3, &s5, &s6, &claimant].iter().enumerate() {
        let after = e.ledger(t).await?;
        for k in ["known_balance", "total_paid_out", "escrowed", "undelivered"] {
            assert_eq!(ledgers_before[i][k], after[k], "{t} {k} unchanged");
        }
        let approval: String = e
            .reg
            .view("get_share_approval")
            .args_json(json!({ "token_id": t }))
            .await?
            .json()?;
        assert_eq!(
            approval, "approved",
            "{t}: shares from before the rule are approved"
        );
    }

    // New rules apply to the old state.
    let r = e
        .outsider
        .call(e.reg.id(), "sync")
        .args_json(json!({ "token_id": s3 }))
        .max_gas()
        .transact()
        .await?;
    assert!(
        !r.is_success(),
        "review H1: an outsider cannot sync after the upgrade"
    );

    // Old records keep working with the new code.
    check(e.claim(&e.buyer, &s2).await?);
    assert_eq!(
        e.balance(e.buyer.id()).await?,
        500_000,
        "s2 paid to the buyer"
    );
    check(e.claim(&e.writer_a, &s3).await?);
    check(e.pay_work(w0, 100_000).await?);
    check(e.claim(&e.writer_a, &s1).await?);
    let r = ok(e
        .registrar
        .call(e.reg.id(), "resolve")
        .args_json(json!({ "dispute_id": dispute_id, "outcome": "uphold_existing", "doc_hash": hash('7') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert!(r.logs().iter().any(|l| l.contains("dispute_resolved")));
    assert_eq!(
        e.balance(&e.vault(&s5)).await?,
        700_000,
        "escrow opened before the upgrade is released after it"
    );

    // A share minted after the upgrade gets the new vault code and the
    // split sheet rule.
    let w3 = e.register_work("USRC12600004").await?;
    let r = ok(register_as(&e, &e.writer_b, w3, &e.writer_b, 4_000, None).await?);
    let reg_ev = event(&r, "share_registered");
    assert_eq!(reg_ev["approval"], "pending");
    let new_token = reg_ev["token_id"].as_str().unwrap().to_string();
    assert_eq!(
        e.worker.view_code(&e.vault(&new_token)).await?,
        wasm("vault"),
        "a vault made after the upgrade runs the new vault code"
    );
    assert_ne!(
        code_hash(&e, &e.vault(&s1)).await?,
        code_hash(&e, &e.vault(&new_token)).await?,
        "vaults made before the upgrade keep their old code (T20)"
    );
    for t in [&s1, &s2, &s3, &s5, &s6, &new_token] {
        e.assert_keyless(t).await?;
    }
    e.assert_solvent().await
}
