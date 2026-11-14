//! Final contract fixes: the stale payout lock now reads the vault before it
//! is cleared (audit F3, THREAT_MODEL T19). A callback that never ran cannot
//! be produced on a sandbox node, so the state it leaves is written by hand
//! with `patch_state`; the vault, the token and every call are real.

mod common;

use common::*;
use serde_json::{json, Value};

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

#[tokio::test]
async fn f01_stale_payout_that_never_left_the_vault_is_undone() -> anyhow::Result<()> {
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
    assert!(r
        .logs()
        .iter()
        .any(|l| l.contains("withdraw_failed_debit_undone")));
    let l = e.ledger(&s).await?;
    assert_eq!(
        (
            num(&l["known_balance"]),
            num(&l["total_paid_out"]),
            num(&l["earned"])
        ),
        (80_000, 0, 80_000),
        "debit undone: earned is the 80000 that was paid in, no more"
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
async fn f02_stale_payout_that_was_paid_keeps_its_debit() -> anyhow::Result<()> {
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
    assert!(r.logs().iter().any(|l| l.contains("withdraw_happened")));
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
