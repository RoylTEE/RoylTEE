//! Final contract fixes: the stale payout lock now reads the vault before it
//! is cleared (audit F3, THREAT_MODEL T19). A callback that never ran cannot
//! be produced on a sandbox node, so the state it leaves is written by hand
//! with `patch_state`; the vault, the token and every call are real.

mod common;

use common::*;
use near_workspaces::types::NearToken;
use serde_json::{json, Value};

fn event(r: &near_workspaces::result::ExecutionFinalResult, name: &str) -> Value {
    r.logs()
        .iter()
        .find(|l| l.contains(&format!("\"{name}\"")))
        .map(|l| serde_json::from_str::<Value>(l.trim_start_matches("EVENT_JSON:")).unwrap())
        .unwrap_or_else(|| panic!("{name} event"))["data"][0]
        .clone()
}

fn u128_le(v: u128) -> [u8; 16] {
    v.to_le_bytes()
}

/// Borsh `Ledger` with a payout lock: five u128, inbound_pending u32,
/// `Some(Lock { account, amount, since_block })`, no transfer lock, epoch.
fn ledger_with_payout_lock(
    known: u128,
    paid_out: u128,
    routed_in: u128,
    holder: &str,
    amount: u128,
    since_block: u64,
    epoch: u64,
) -> Vec<u8> {
    let mut raw = vec![];
    for v in [known, paid_out, routed_in, 0, 0] {
        raw.extend_from_slice(&u128_le(v));
    }
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.push(1);
    raw.extend_from_slice(&(holder.len() as u32).to_le_bytes());
    raw.extend_from_slice(holder.as_bytes());
    raw.extend_from_slice(&u128_le(amount));
    raw.extend_from_slice(&since_block.to_le_bytes());
    raw.push(0);
    raw.extend_from_slice(&epoch.to_le_bytes());
    raw
}

fn ledger_key(token_id: &str) -> Vec<u8> {
    // `Key::Ledgers` is the ninth variant of the storage key enum.
    let mut k = vec![8u8];
    k.extend_from_slice(&(token_id.len() as u32).to_le_bytes());
    k.extend_from_slice(token_id.as_bytes());
    k
}

/// Leaves share `s` with 80000 in its vault and a payout lock of 80000 held
/// by its holder, as if `claim` had run and neither the withdraw nor its
/// callback had been seen. The vault really holds the money, so this is the
/// "withdraw never happened" case.
async fn stuck_failed_payout(e: &Env, s: &str) -> anyhow::Result<()> {
    check(e.pay_share(s, 80_000).await?);
    let l = e.ledger(s).await?;
    let since = e.worker.view_block().await?.height();
    let epoch = num(&l["epoch"]) as u64;
    let raw = ledger_with_payout_lock(
        0,
        80_000,
        num(&l["total_routed_in"]),
        e.writer_a.id().as_str(),
        80_000,
        since,
        epoch + 1,
    );
    e.worker
        .patch_state(e.reg.id(), &ledger_key(s), &raw)
        .await?;
    let l = e.ledger(s).await?;
    assert_eq!(num(&l["known_balance"]), 0);
    assert_eq!(num(&l["total_paid_out"]), 80_000);
    assert!(l["payout_in_flight"].is_object());
    Ok(())
}

/// Calls `release_stale_lock` and returns the result of the diagnostic
/// callback's gas, so the test can measure it.
fn diag_gas(r: &near_workspaces::result::ExecutionFinalResult, reg: &str) -> u64 {
    let g: Vec<u64> = r
        .receipt_outcomes()
        .iter()
        .filter(|o| {
            o.executor_id.as_str() == reg && o.logs.iter().any(|l| l.contains("payout_uncertain"))
        })
        .map(|o| o.gas_burnt.as_gas())
        .collect();
    assert_eq!(g.len(), 1);
    g[0]
}

async fn resolve_payout(
    e: &Env,
    who: &near_workspaces::Account,
    s: &str,
    paid: bool,
    deposit: NearToken,
) -> anyhow::Result<near_workspaces::result::ExecutionFinalResult> {
    // The registrar names the lock it reviewed.
    let l = e.ledger(s).await?;
    let expected_since = l["payout_in_flight"]["since_block"].clone();
    Ok(who
        .call(e.reg.id(), "resolve_stale_payout")
        .args_json(json!({ "token_id": s, "paid": paid, "expected_since_block": expected_since, "doc_hash": hash('e') }))
        .deposit(deposit)
        .max_gas()
        .transact()
        .await?)
}

/// Audit RR-2: a stale payout lock whose withdraw never left the vault is
/// not undone from the balance. The release only reports the balance. The
/// registrar settles it from the receipt outcome.
#[tokio::test]
async fn f01_stale_payout_lock_stays_until_the_registrar_settles_it() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let s = e.mint(w, e.writer_a.id(), 10_000).await?;
    stuck_failed_payout(&e, &s).await?;

    let call = |who: &near_workspaces::Account| {
        who.call(e.reg.id(), "release_stale_lock")
            .args_json(json!({ "token_id": s }))
            .max_gas()
            .transact()
    };
    assert!(call(&e.outsider).await?.is_failure(), "too early");
    e.worker.fast_forward(1_300).await?;
    let r = ok(call(&e.outsider).await?);
    let ev = event(&r, "payout_uncertain");
    assert_eq!(ev["observed"], "80000", "the balance is a diagnostic");
    let gas = diag_gas(&r, e.reg.id().as_str());
    println!(
        "GAS on_release_payout {:.3} Tgas (reserved 10)",
        gas as f64 / 1e12
    );
    assert!(gas < 10_000_000_000_000);
    let l = e.ledger(&s).await?;
    assert_eq!(
        (num(&l["known_balance"]), num(&l["total_paid_out"])),
        (0, 80_000),
        "nothing changed"
    );
    assert!(l["payout_in_flight"].is_object(), "the lock stays");
    // The holder cannot claim or sell past the lock.
    assert!(e.claim(&e.writer_a, &s).await?.is_failure());
    // Only the registrar, with 1 yocto, settles it.
    assert!(resolve_payout(&e, &e.outsider, &s, false, ONE_YOCTO)
        .await?
        .is_failure());
    assert!(
        resolve_payout(&e, &e.registrar, &s, false, NearToken::from_yoctonear(0))
            .await?
            .is_failure()
    );
    let r = ok(resolve_payout(&e, &e.registrar, &s, false, ONE_YOCTO).await?);
    assert_eq!(event(&r, "payout_resolved")["paid"], false);
    let l = e.ledger(&s).await?;
    assert_eq!(
        (
            num(&l["known_balance"]),
            num(&l["total_paid_out"]),
            num(&l["earned"])
        ),
        (80_000, 0, 80_000)
    );
    assert!(l["payout_in_flight"].is_null());
    // The holder claims it, once.
    check(e.claim(&e.writer_a, &s).await?);
    assert_eq!(e.balance(e.writer_a.id()).await?, 80_000);
    let l = e.ledger(&s).await?;
    assert_eq!(
        (
            num(&l["known_balance"]),
            num(&l["total_paid_out"]),
            num(&l["earned"])
        ),
        (0, 80_000, 80_000)
    );
    e.assert_solvent().await
}

#[tokio::test]
async fn f02_stale_payout_that_was_paid_keeps_its_debit_when_the_registrar_says_so(
) -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let s = e.mint(w, e.writer_a.id(), 10_000).await?;
    // The money really left: a real claim ran.
    check(e.pay_share(&s, 80_000).await?);
    check(e.claim(&e.writer_a, &s).await?);
    assert_eq!(e.balance(&e.vault(&s)).await?, 0);
    let l = e.ledger(&s).await?;
    let since = e.worker.view_block().await?.height();
    let raw = ledger_with_payout_lock(
        0,
        80_000,
        num(&l["total_routed_in"]),
        e.writer_a.id().as_str(),
        80_000,
        since,
        num(&l["epoch"]) as u64 + 1,
    );
    e.worker
        .patch_state(e.reg.id(), &ledger_key(&s), &raw)
        .await?;
    e.worker.fast_forward(1_300).await?;
    let r = ok(e
        .outsider
        .call(e.reg.id(), "release_stale_lock")
        .args_json(json!({ "token_id": s }))
        .max_gas()
        .transact()
        .await?);
    assert_eq!(event(&r, "payout_uncertain")["observed"], "0");
    let l: Value = e.ledger(&s).await?;
    assert!(l["payout_in_flight"].is_object(), "the lock stays");
    let _ = ok(resolve_payout(&e, &e.registrar, &s, true, ONE_YOCTO).await?);
    let l: Value = e.ledger(&s).await?;
    assert_eq!(
        (
            num(&l["known_balance"]),
            num(&l["total_paid_out"]),
            num(&l["earned"])
        ),
        (0, 80_000, 80_000)
    );
    assert!(l["payout_in_flight"].is_null());
    e.assert_solvent().await
}

/// External review L6: the registry account calls `on_forward` with the two
/// argument JSON that receipts created before the upgrade carry (no `since`).
/// The method must accept it. Called directly it has no promise result, so it
/// settles as a failed forward: the amount goes to `undelivered`.
#[tokio::test]
async fn f03_on_forward_accepts_the_two_argument_json_of_old_receipts() -> anyhow::Result<()> {
    let e = setup().await?;
    let w = e.register_work("USRC12600001").await?;
    let s = e.mint(w, e.writer_a.id(), 10_000).await?;
    let r = ok(e
        .reg
        .as_account()
        .call(e.reg.id(), "on_forward")
        .args_json(json!({ "token_id": s, "amount": "1000" }))
        .max_gas()
        .transact()
        .await?);
    assert!(r.logs().iter().any(|l| l.contains("forward_failed")));
    let l = e.ledger(&s).await?;
    assert_eq!(num(&l["undelivered"]), 1_000);
    // The same call with the new third argument also works.
    check(
        e.reg
            .as_account()
            .call(e.reg.id(), "on_forward")
            .args_json(json!({ "token_id": s, "amount": "500", "since": "7" }))
            .max_gas()
            .transact()
            .await?,
    );
    assert_eq!(num(&e.ledger(&s).await?["undelivered"]), 1_500);
    // Nobody else can call it.
    assert!(e
        .outsider
        .call(e.reg.id(), "on_forward")
        .args_json(json!({ "token_id": s, "amount": "1" }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    Ok(())
}

/// External review H1, end to end on the test build with the short release
/// window: a forward whose transfer the vault does not show is quarantined,
/// never resent; the registrar settles it as not delivered; only then does
/// `retry_forward` send it, once, and the registry stays solvent throughout.
/// The lost callback is written with `patch_state` (the registry holds the
/// 80000 as a plain deposit, which is what a failed transfer leaves).
#[tokio::test]
async fn f04_a_written_off_forward_is_quarantined_then_resolved_and_resent_once(
) -> anyhow::Result<()> {
    let e = setup().await?;
    e.reg
        .as_account()
        .deploy(&wasm("registry_short_stale"))
        .await?
        .into_result()?;
    let w = e.register_work("USRC12600001").await?;
    let s = e.mint(w, e.writer_a.id(), 10_000).await?;
    check(
        e.payer
            .call(e.usdc.id(), "ft_transfer")
            .args_json(json!({ "receiver_id": e.reg.id(), "amount": "80000" }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    let key = ledger_key(&s);
    let state = e.worker.view_state(e.reg.id()).await?;
    let mut raw = state.get(&key).expect("ledger entry").clone();
    raw[80..84].copy_from_slice(&1u32.to_le_bytes()); // inbound_pending
    e.worker.patch_state(e.reg.id(), &key, &raw).await?;
    let mut root = state.get(&b"STATE"[..]).expect("root state").clone();
    let n = root.len();
    root[n - 40..n - 24].copy_from_slice(&80_000u128.to_le_bytes()); // forwards_in_flight_total
    e.worker.patch_state(e.reg.id(), b"STATE", &root).await?;
    let mut watch = vec![];
    watch.extend_from_slice(&1u64.to_le_bytes()); // since
    watch.extend_from_slice(&1u64.to_le_bytes()); // latest
    watch.extend_from_slice(&80_000u128.to_le_bytes());
    watch.extend_from_slice(&1u32.to_le_bytes());
    watch.extend_from_slice(&0u64.to_le_bytes()); // released_through
    watch.extend_from_slice(&0u64.to_le_bytes()); // resolved_through
    watch.extend_from_slice(&0u128.to_le_bytes()); // written_off
    watch.extend_from_slice(&0u64.to_le_bytes()); // settle_seq
    let mut watch_key = b"fwd:".to_vec();
    watch_key.extend_from_slice(s.as_bytes());
    e.worker.patch_state(e.reg.id(), &watch_key, &watch).await?;
    e.assert_solvent().await?;

    e.worker.fast_forward(1_600).await?;
    let r = ok(e
        .outsider
        .call(e.reg.id(), "release_stale_forward")
        .args_json(json!({ "token_id": s }))
        .max_gas()
        .transact()
        .await?);
    let ev = event(&r, "forward_released_stale");
    assert_eq!(
        (ev["forwards"].as_u64(), ev["quarantined"].as_str()),
        (Some(1), Some("80000"))
    );
    // No balance was read and nothing was credited.
    let l = e.ledger(&s).await?;
    assert_eq!(
        (num(&l["known_balance"]), l["inbound_pending"].as_u64()),
        (0, Some(0))
    );
    // sync stays refused while the quarantine is open.
    assert!(e
        .writer_a
        .call(e.reg.id(), "sync")
        .args_json(json!({ "token_id": s }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    let acct: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(
        num(&acct["forwards_in_flight_total"]),
        80_000,
        "quarantined"
    );
    assert_eq!(num(&acct["undelivered_total"]), 0);
    e.assert_solvent().await?;
    // Nothing can be resent while it is quarantined.
    assert!(e
        .outsider
        .call(e.reg.id(), "retry_forward")
        .args_json(json!({ "token_id": s }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    // Only the registrar settles it, with 1 yocto and a document hash.
    let seq = |n: &str| json!(n);
    let resolve_seq = |who: &near_workspaces::Account, deposit, expected_seq: Value| {
        who.call(e.reg.id(), "resolve_written_off")
            .args_json(json!({ "token_id": s, "delivered_amount": "0", "failed_amount": "80000", "expected_seq": expected_seq, "doc_hash": hash('d') }))
            .deposit(deposit)
            .max_gas()
            .transact()
    };
    let resolve = |who: &near_workspaces::Account, deposit| resolve_seq(who, deposit, seq("1"));
    // The release moved the settlement sequence to 1; an older review fails.
    let w: Value = e
        .reg
        .view("get_forward_watch")
        .args_json(json!({ "token_id": s }))
        .await?
        .json()?;
    assert_eq!(w["settle_seq"], "1");
    assert!(resolve_seq(&e.registrar, ONE_YOCTO, seq("0"))
        .await?
        .is_failure());
    assert!(resolve(&e.outsider, ONE_YOCTO).await?.is_failure());
    assert!(resolve(&e.registrar, NearToken::from_yoctonear(0))
        .await?
        .is_failure());
    let r = ok(resolve(&e.registrar, ONE_YOCTO).await?);
    assert_eq!(event(&r, "written_off_resolved")["failed_amount"], "80000");
    let acct: Value = e.reg.view("get_accounting").await?.json()?;
    assert_eq!(
        (
            num(&acct["forwards_in_flight_total"]),
            num(&acct["undelivered_total"])
        ),
        (0, 80_000)
    );
    e.assert_solvent().await?;
    // Now the retry sends it, once.
    check(
        e.outsider
            .call(e.reg.id(), "retry_forward")
            .args_json(json!({ "token_id": s }))
            .max_gas()
            .transact()
            .await?,
    );
    let l = e.ledger(&s).await?;
    assert_eq!(num(&l["known_balance"]), 80_000);
    assert_eq!(e.balance(&e.vault(&s)).await?, 80_000);
    assert!(e
        .outsider
        .call(e.reg.id(), "retry_forward")
        .args_json(json!({ "token_id": s }))
        .max_gas()
        .transact()
        .await?
        .is_failure());
    e.assert_solvent().await?;
    check(e.claim(&e.writer_a, &s).await?);
    e.assert_solvent().await
}
