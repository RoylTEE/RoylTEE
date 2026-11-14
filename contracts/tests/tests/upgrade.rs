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

// ---------- final fixes: upgrade from the registry deployed after audit F1 ----------

/// Code hash (base58 sha256) of the registry on `reg.royltee-dev.testnet`
/// after the audit F1 redeploy. The fixture is that build, made from commit
/// a409be2 with scripts/build-wasm.sh (sha256 423e3799...).
const DEPLOYED_REGISTRY_CODE_HASH: &str = "5Taw8Syz7g1QxNa4Faw8mz7fnNgoXGj7MwX2VS3obRFo";

fn deployed_registry_wasm() -> Vec<u8> {
    let path = format!(
        "{}/fixtures/registry-deployed-423e3799.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|_| panic!("missing {path}"))
}

/// Borsh `Ledger` as stored: five u128, inbound_pending u32, two empty
/// lock options, epoch u64.
const LEDGER_LEN: usize = 5 * 16 + 4 + 1 + 1 + 8;

fn ledger_key(token_id: &str) -> Vec<u8> {
    // `Key::Ledgers` is the ninth variant of the storage key enum.
    let mut k = vec![8u8];
    k.extend_from_slice(&(token_id.len() as u32).to_le_bytes());
    k.extend_from_slice(token_id.as_bytes());
    k
}

#[tokio::test]
async fn u02_deployed_build_upgrades_in_place_and_a_lost_forward_is_quarantined_and_resolved(
) -> anyhow::Result<()> {
    let e = setup_with_registry(&deployed_registry_wasm()).await?;
    assert_eq!(
        code_hash(&e, e.reg.id()).await?,
        ContractState::LocalHash(DEPLOYED_REGISTRY_CODE_HASH.parse()?),
        "the fixture is the registry wasm deployed on testnet"
    );

    // State built with the deployed code.
    let w0 = e.register_work("USRC12600001").await?;
    let s1 = e.mint(w0, e.writer_a.id(), 5_000).await?;
    let s2 = e.mint(w0, e.writer_b.id(), 5_000).await?;
    check(e.pay_work(w0, 1_000_000).await?);
    check(e.claim(&e.writer_a, &s1).await?);
    check(e.transfer(&e.writer_b, e.buyer.id(), &s2).await?);
    let w1 = e.register_work("USRC12600002").await?;
    let s3 = e.mint(w1, e.writer_a.id(), 10_000).await?;
    e.assert_solvent().await?;

    let acct_before: Value = e.reg.view("get_accounting").await?.json()?;
    let mut before = vec![];
    for t in [&s1, &s2, &s3] {
        before.push(e.ledger(t).await?);
    }

    // In-place redeploy with the branch wasm. No migration call.
    let new_wasm = wasm("registry");
    e.reg.as_account().deploy(&new_wasm).await?.into_result()?;
    assert_eq!(e.worker.view_code(e.reg.id()).await?, new_wasm);
    let acct_after: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(acct_before, acct_after, "counters unchanged");
    for (i, t) in [&s1, &s2, &s3].iter().enumerate() {
        assert_eq!(before[i], e.ledger(t).await?, "{t} ledger unchanged");
    }
    assert_eq!(e.owner(&s2).await?, e.buyer.id().to_string());
    let watch: Value = e
        .reg
        .view("get_forward_watch")
        .args_json(json!({ "token_id": s3 }))
        .await?
        .json()?;
    assert!(
        watch.is_null(),
        "no watch for state from before the upgrade"
    );

    // A payment after the upgrade is watched and settles as usual.
    check(e.pay_share(&s3, 80_000).await?);
    let l = e.ledger(&s3).await?;
    assert_eq!(
        (num(&l["known_balance"]), l["inbound_pending"].as_u64()),
        (80_000, Some(0))
    );
    let watch: Value = e
        .reg
        .view("get_forward_watch")
        .args_json(json!({ "token_id": s3 }))
        .await?
        .json()?;
    assert!(watch.is_null(), "the watch ends with the settled forward");

    // Nothing to release, nothing stale.
    for method in ["release_stale_forward", "release_stale_lock"] {
        let r = e
            .outsider
            .call(e.reg.id(), method)
            .args_json(json!({ "token_id": s3 }))
            .max_gas()
            .transact()
            .await?;
        assert!(r.is_failure(), "{method} with nothing to release");
    }

    // The release window is about a day (150000 blocks), more than a sandbox
    // can fast-forward. The rest of this test runs on the same code built with
    // a 1500 block window (cargo feature `short-stale-forward`, test builds
    // only), deployed over the upgraded registry in place.
    e.reg
        .as_account()
        .deploy(&wasm("registry_short_stale"))
        .await?
        .into_result()?;

    // Lost callback. A real node never loses it here, so the state a lost
    // callback leaves is written by hand: the ledger as it was while the
    // forward was out (nothing credited, one forward pending), the counter
    // for it, and the watch entry. The vault really holds the 80000.
    let key = ledger_key(&s3);
    let state = e.worker.view_state(e.reg.id()).await?;
    let mut raw = state.get(&key).expect("ledger entry").clone();
    assert_eq!(raw.len(), LEDGER_LEN, "ledger layout as expected");
    raw[0..16].copy_from_slice(&0u128.to_le_bytes()); // known_balance
    raw[32..48].copy_from_slice(&0u128.to_le_bytes()); // total_routed_in
    raw[80..84].copy_from_slice(&1u32.to_le_bytes()); // inbound_pending
    e.worker.patch_state(e.reg.id(), &key, &raw).await?;
    let mut root = state.get(&b"STATE"[..]).expect("root state").clone();
    let n = root.len();
    root[n - 40..n - 24].copy_from_slice(&80_000u128.to_le_bytes()); // forwards_in_flight_total
    e.worker.patch_state(e.reg.id(), b"STATE", &root).await?;
    // The forward started "a day ago": block 1 for the watch, so no
    // fast-forward of 150000 blocks is needed. Borsh `ForwardWatch`: since,
    // latest (u64), amount (u128), count (u32), released_through,
    // resolved_through (u64),
    // written_off (u128).
    let mut watch_raw = vec![];
    watch_raw.extend_from_slice(&1u64.to_le_bytes());
    watch_raw.extend_from_slice(&1u64.to_le_bytes());
    watch_raw.extend_from_slice(&80_000u128.to_le_bytes());
    watch_raw.extend_from_slice(&1u32.to_le_bytes());
    watch_raw.extend_from_slice(&0u64.to_le_bytes()); // released_through
    watch_raw.extend_from_slice(&0u64.to_le_bytes()); // resolved_through
    watch_raw.extend_from_slice(&0u128.to_le_bytes()); // written_off
    let mut watch_key = b"fwd:".to_vec();
    watch_key.extend_from_slice(s3.as_bytes());
    e.worker
        .patch_state(e.reg.id(), &watch_key, &watch_raw)
        .await?;
    let l = e.ledger(&s3).await?;
    assert_eq!(
        (num(&l["known_balance"]), l["inbound_pending"].as_u64()),
        (0, Some(1))
    );
    assert_eq!(
        num(&e.reg.view("get_accounting").await?.json::<Value>()?["forwards_in_flight_total"]),
        80_000
    );
    // sync is refused, as the audit reported.
    assert!(e
        .writer_a
        .call(e.reg.id(), "sync")
        .args_json(json!({ "token_id": s3 }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    // Too early on the test build: the watch says block 1, the chain is
    // below block 1501.
    assert!(e
        .outsider
        .call(e.reg.id(), "release_stale_forward")
        .args_json(json!({ "token_id": s3 }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    e.worker.fast_forward(1_600).await?;
    let r = ok(e
        .outsider
        .call(e.reg.id(), "release_stale_forward")
        .args_json(json!({ "token_id": s3 }))
        .max_gas()
        .transact()
        .await?);
    let ev = event(&r, "forward_released_stale");
    assert_eq!(ev["forwards"], 1);
    assert_eq!(ev["quarantined"], "80000");
    println!(
        "GAS release_stale_forward {:.3} Tgas burnt in total (no callback)",
        r.total_gas_burnt.as_gas() as f64 / 1e12
    );
    let watch: Value = e
        .reg
        .view("get_forward_watch")
        .args_json(json!({ "token_id": s3 }))
        .await?
        .json()?;
    assert_eq!(watch["written_off"], "80000");
    assert_eq!(watch["count"], 0);
    // The vault really holds the 80000, yet nothing is credited: a balance
    // proves nothing about where it came from.
    let l = e.ledger(&s3).await?;
    assert_eq!(
        (
            num(&l["known_balance"]),
            num(&l["total_routed_in"]),
            l["inbound_pending"].as_u64()
        ),
        (0, 0, Some(0))
    );
    assert_eq!(e.balance(&e.vault(&s3)).await?, 80_000);
    let acct: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(num(&acct["forwards_in_flight_total"]), 80_000);
    // Not asserted solvent here: the transfer did reach the vault, so the
    // registry holds less than the quarantined counter until it is settled.
    // sync stays refused while the quarantine is open, and nothing resends.
    for (who, method) in [(&e.writer_a, "sync"), (&e.outsider, "retry_forward")] {
        assert!(who
            .call(e.reg.id(), method)
            .args_json(json!({ "token_id": s3 }))
            .max_gas()
            .transact()
            .await?
            .is_failure());
    }
    // The registrar reads the public receipt: the transfer reached the vault.
    let r = ok(e
        .registrar
        .call(e.reg.id(), "resolve_written_off")
        .args_json(json!({ "token_id": s3, "delivered_amount": "80000", "failed_amount": "0", "doc_hash": hash('d') }))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?);
    assert_eq!(
        event(&r, "written_off_resolved")["delivered_amount"],
        "80000"
    );
    let l = e.ledger(&s3).await?;
    assert_eq!(
        (num(&l["known_balance"]), num(&l["total_routed_in"])),
        (80_000, 80_000)
    );
    let acct: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(num(&acct["forwards_in_flight_total"]), 0);
    e.assert_solvent().await?;
    // The holder can claim what the vault holds.
    check(e.claim(&e.writer_a, &s3).await?);
    assert_eq!(e.balance(&e.vault(&s3)).await?, 0);
    e.assert_solvent().await
}

// ---------- final fixes: the deploy and the quiescence check in one transaction ----------

/// Audit RR-4 and R-L3: the redeploy is ONE transaction, a deploy action
/// followed by a call of `assert_quiescent` on the new code (what
/// `near contract deploy ... with-init-call assert_quiescent` sends). Actions
/// of one receipt are atomic, so a busy registry rolls the deploy back. This
/// test deploys the new wasm over the exact deployed one in a single batch:
/// a forward in flight and a payout lock each make the batch fail with the
/// old code still in place and the state untouched; the quiet state passes
/// and the new code is live. It also measures the gas of `assert_quiescent`.
#[tokio::test]
async fn u03_deploy_and_assert_quiescent_in_one_transaction_roll_back_when_busy(
) -> anyhow::Result<()> {
    use near_workspaces::types::Gas;
    let e = setup_with_registry(&deployed_registry_wasm()).await?;
    let w0 = e.register_work("USRC12600001").await?;
    let s1 = e.mint(w0, e.writer_a.id(), 5_000).await?;
    let s2 = e.mint(w0, e.writer_b.id(), 5_000).await?;
    check(e.pay_work(w0, 1_000_000).await?);
    let new_wasm = wasm("registry");
    let old_hash = code_hash(&e, e.reg.id()).await?;
    let batch = |wasm: &Vec<u8>| {
        e.reg
            .as_account()
            .batch(e.reg.id())
            .deploy(wasm)
            .call(
                near_workspaces::operations::Function::new("assert_quiescent")
                    .args_json(json!({}))
                    .gas(Gas::from_tgas(100)),
            )
            .transact()
    };

    // 1. A forward in flight (counter and ledger as an unfinished forward leaves them).
    let key = ledger_key(&s1);
    let state = e.worker.view_state(e.reg.id()).await?;
    let clean_ledger = state.get(&key).expect("ledger entry").clone();
    let clean_root = state.get(&b"STATE"[..]).expect("root").clone();
    let mut busy = clean_ledger.clone();
    busy[80..84].copy_from_slice(&1u32.to_le_bytes()); // inbound_pending
    e.worker.patch_state(e.reg.id(), &key, &busy).await?;
    let mut root = clean_root.clone();
    let n = root.len();
    root[n - 40..n - 24].copy_from_slice(&1_000u128.to_le_bytes());
    e.worker.patch_state(e.reg.id(), b"STATE", &root).await?;
    let r = batch(&new_wasm).await?;
    assert!(r.is_failure(), "a busy registry must reject the batch");
    assert!(format!("{:?}", r.failures()).contains("not quiet: forwards in flight"));
    assert_eq!(
        code_hash(&e, e.reg.id()).await?,
        old_hash,
        "the deploy rolled back with the failed call"
    );
    assert_eq!(
        e.worker.view_code(e.reg.id()).await?,
        deployed_registry_wasm()
    );

    // 2. A payout lock: ledger layout is five u128, inbound u32, then the
    // locks. Write `Some(Lock)` for the payout lock by rebuilding the entry.
    e.worker
        .patch_state(e.reg.id(), &key, &clean_ledger)
        .await?;
    e.worker
        .patch_state(e.reg.id(), b"STATE", &clean_root)
        .await?;
    let mut locked = clean_ledger[..84].to_vec();
    locked.push(1);
    let holder = e.writer_a.id().as_str();
    locked.extend_from_slice(&(holder.len() as u32).to_le_bytes());
    locked.extend_from_slice(holder.as_bytes());
    locked.extend_from_slice(&1u128.to_le_bytes());
    locked.extend_from_slice(&1u64.to_le_bytes());
    locked.push(0);
    locked.extend_from_slice(&clean_ledger[clean_ledger.len() - 8..]);
    e.worker.patch_state(e.reg.id(), &key, &locked).await?;
    assert!(
        batch(&new_wasm).await?.is_failure(),
        "a payout lock rejects the batch"
    );
    assert_eq!(code_hash(&e, e.reg.id()).await?, old_hash);

    // 3. Quiet: the same batch succeeds and the new code is live.
    e.worker
        .patch_state(e.reg.id(), &key, &clean_ledger)
        .await?;
    let r = batch(&new_wasm).await?;
    let g3 = r.total_gas_burnt.as_gas();
    check(r);
    assert_eq!(e.worker.view_code(e.reg.id()).await?, new_wasm);
    println!(
        "GAS deploy + assert_quiescent, 2 tokens: {:.3} Tgas burnt in total",
        g3 as f64 / 1e12
    );
    // The method is private: an outsider cannot call it.
    assert!(e
        .outsider
        .call(e.reg.id(), "assert_quiescent")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    // Gas per token: more tokens, same call from the registry account.
    let call_gas = || async {
        let r = e
            .reg
            .as_account()
            .call(e.reg.id(), "assert_quiescent")
            .args_json(json!({}))
            .gas(Gas::from_tgas(100))
            .transact()
            .await?;
        anyhow::Ok(ok(r).total_gas_burnt.as_gas())
    };
    let a = call_gas().await?;
    let w1 = e.register_work("USRC12600002").await?;
    for _ in 0..4 {
        e.mint(w1, e.writer_a.id(), 2_000).await?;
    }
    let b = call_gas().await?;
    println!(
        "GAS assert_quiescent: {:.3} Tgas at 2 tokens, {:.3} Tgas at 6 tokens, about {:.3} Tgas per token",
        a as f64 / 1e12,
        b as f64 / 1e12,
        (b - a) as f64 / 4.0 / 1e12
    );
    let _ = (&s2, &s1);
    Ok(())
}
