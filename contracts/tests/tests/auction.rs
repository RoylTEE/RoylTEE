//! Sandbox tests for the sealed-bid share auction.
//!
//! The registry, the vaults and mock USDC are the real built wasm. OutLayer is
//! `mock_outlayer` (Simulated): it answers a settle request with whatever the
//! test set. For the end to end cases the test runs the REAL WASI module on
//! the exact input the auction sent, with bids sealed by the real script,
//! and feeds the module's output back as the answer. The test master secret
//! is a public test value.
//!
//! Needs `npm run build:wasm` first and `node` on the path.

mod common;

use common::*;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{Gas, NearToken};
use near_workspaces::{Account, AccountId, Contract};
use serde_json::{json, Value};
use std::process::Command;

const MAX: u128 = 100 * MUSDC;
const RESERVE: u128 = 10 * MUSDC;

static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A fresh temp file holding `content` (tests run in parallel in one process).
fn temp_json(content: &str) -> std::path::PathBuf {
    let n = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("auction-test-{}-{n}.json", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}

fn helper(args: &[&str]) -> String {
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("node")
        .arg("--no-warnings")
        .arg(format!("{root}/scripts/auction/sandbox-helper.mjs"))
        .args(args)
        .current_dir(&root)
        .output()
        .expect("node must be installed to run the auction sandbox tests");
    assert!(
        out.status.success(),
        "helper failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

struct A {
    e: Env,
    mock: Contract,
    auction: Contract,
    pubkey: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new("sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_string()
}

async fn setup_auction() -> anyhow::Result<A> {
    let e = setup().await?;
    let root = e.worker.root_account()?;
    let mock_acc = root
        .create_subaccount("mockol")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;
    let mock = mock_acc
        .deploy(&wasm("mock_outlayer"))
        .await?
        .into_result()?;
    check(mock.call("new").transact().await?);

    let acc = root
        .create_subaccount("auction")
        .initial_balance(NearToken::from_near(20))
        .transact()
        .await?
        .into_result()?;
    let auction = acc.deploy(&wasm("auction")).await?.into_result()?;
    let wasi_hash = sha256_hex(&wasm("auction_wasi"));
    check(
        auction
            .call("new")
            .args_json(json!({
                "registry": e.reg.id(),
                "usdc": e.usdc.id(),
                "setup": {
                    "outlayer": mock.id(),
                    "wasm_url": format!("https://test.fastfs.io/{}/{}/{wasi_hash}.wasm", auction.id(), mock.id()),
                    "wasm_hash": wasi_hash,
                    "secrets_account": e.writer_a.id(),
                    "secrets_profile": "royltee-auction",
                },
            }))
            .transact()
            .await?,
    );
    // The auction is a plain credentialed member: no registry change.
    e.grant(auction.id()).await?;
    // Publish the TEE key: the answer is the real module's pubkey output.
    let pubkey = helper(&["pubkey", auction.id().as_str()]);
    check(
        mock.call("set_answer")
            .args_json(json!({ "answer": json!({"v":1,"ok":true,"key_version":1,"pubkey":pubkey}).to_string() }))
            .transact()
            .await?,
    );
    check(
        e.outsider
            .call(auction.id(), "publish_key")
            .deposit(NearToken::from_millinear(20))
            .max_gas()
            .transact()
            .await?,
    );
    let got: Option<String> = auction.view("get_tee_pubkey").await?.json()?;
    assert_eq!(got.as_deref(), Some(pubkey.as_str()));
    // The request the contract really sent, run through the real module with
    // the caller the mock saw, gives the same key (so `contract` is in it).
    let rec: Value = mock.view("last_request").await?.json()?;
    assert_eq!(rec["caller"], auction.id().as_str());
    let path = temp_json(rec["input_data"].as_str().unwrap());
    let out = helper(&[
        "run",
        path.to_str().unwrap(),
        rec["caller"].as_str().unwrap(),
    ]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        serde_json::from_str::<Value>(&out)?["pubkey"],
        pubkey.as_str()
    );
    check(
        mock.call("set_answer")
            .args_json(json!({ "answer": null }))
            .transact()
            .await?,
    );
    let a = A {
        e,
        mock,
        auction,
        pubkey,
    };
    // Bidders: mock USDC to buyer-pub and writer-b.
    for who in [a.e.buyer.id(), a.e.writer_b.id()] {
        a.mint_usdc(who, 500 * MUSDC).await?;
    }
    Ok(a)
}

impl A {
    async fn mint_usdc(&self, who: &AccountId, amount: u128) -> anyhow::Result<()> {
        check(
            self.e
                .payer
                .call(self.e.usdc.id(), "mint")
                .args_json(json!({ "account_id": who, "amount": amount.to_string() }))
                .transact()
                .await?,
        );
        Ok(())
    }

    async fn now(&self) -> anyhow::Result<u64> {
        Ok(self.e.worker.view_block().await?.timestamp())
    }

    /// Mints a share to the seller (writer-a) and lists it.
    async fn new_share(&self, code: &str) -> anyhow::Result<String> {
        let w = self.e.register_work(code).await?;
        self.e.mint(w, self.e.writer_a.id(), 5_000).await
    }

    async fn list(&self, token: &str, ends_in_s: u64) -> anyhow::Result<ExecutionFinalResult> {
        let ends = self.now().await? + ends_in_s * 1_000_000_000;
        self.list_msg(
            token,
            json!({"reserve": RESERVE.to_string(), "max_deposit": MAX.to_string(), "ends_at": ends.to_string()}),
        )
        .await
    }

    async fn list_msg(&self, token: &str, msg: Value) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .e
            .writer_a
            .call(self.e.reg.id(), "nft_transfer_call")
            .args_json(json!({
                "receiver_id": self.auction.id(),
                "token_id": token,
                "msg": msg.to_string(),
            }))
            .deposit(ONE_YOCTO)
            .gas(Gas::from_tgas(100))
            .transact()
            .await?)
    }

    fn seal(&self, auction_id: u64, bidder: &AccountId, amount: u128) -> String {
        helper(&[
            "seal",
            &self.pubkey,
            &auction_id.to_string(),
            self.auction.id().as_str(),
            bidder.as_str(),
            &amount.to_string(),
        ])
    }

    async fn bid_with(
        &self,
        who: &Account,
        auction_id: u64,
        deposit: u128,
        ciphertext: &str,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(who
            .call(self.e.usdc.id(), "ft_transfer_call")
            .args_json(json!({
                "receiver_id": self.auction.id(),
                "amount": deposit.to_string(),
                "msg": json!({"auction_id": auction_id.to_string(), "ciphertext": ciphertext}).to_string(),
            }))
            .deposit(ONE_YOCTO)
            .gas(Gas::from_tgas(100))
            .transact()
            .await?)
    }

    async fn bid(&self, who: &Account, auction_id: u64, amount: u128) -> anyhow::Result<()> {
        let ct = self.seal(auction_id, who.id(), amount);
        check(self.bid_with(who, auction_id, MAX, &ct).await?);
        Ok(())
    }

    async fn auction_view(&self, id: u64) -> anyhow::Result<Value> {
        Ok(self
            .auction
            .view("get_auction")
            .args_json(json!({ "auction_id": id.to_string() }))
            .await?
            .json()?)
    }

    async fn wait_until_ended(&self, id: u64) -> anyhow::Result<()> {
        let ends: u64 = self.auction_view(id).await?["ends_at"]
            .as_str()
            .unwrap()
            .parse()?;
        while self.now().await? < ends {
            self.e.worker.fast_forward(100).await?;
        }
        Ok(())
    }

    async fn settle(&self, id: u64) -> anyhow::Result<ExecutionFinalResult> {
        self.settle_gas(id, 300).await
    }

    async fn settle_gas(&self, id: u64, tgas: u64) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .e
            .outsider
            .call(self.auction.id(), "settle")
            .args_json(json!({ "auction_id": id.to_string() }))
            .deposit(NearToken::from_millinear(20))
            .gas(Gas::from_tgas(tgas))
            .transact()
            .await?)
    }

    async fn set_answer(&self, answer: Option<String>) -> anyhow::Result<()> {
        check(
            self.mock
                .call("set_answer")
                .args_json(json!({ "answer": answer }))
                .transact()
                .await?,
        );
        Ok(())
    }

    /// Settle once with no answer set (the mock records the request and
    /// returns null, so the auction reopens), run the real WASI module on the
    /// recorded input, set its output as the answer, settle again.
    async fn settle_with_real_tee(&self, id: u64) -> anyhow::Result<ExecutionFinalResult> {
        self.settle_with_real_tee_gas(id, 300).await
    }

    async fn settle_with_real_tee_gas(
        &self,
        id: u64,
        tgas: u64,
    ) -> anyhow::Result<ExecutionFinalResult> {
        self.set_answer(None).await?;
        let r = self.settle_gas(id, tgas).await?;
        assert!(r.is_success());
        assert_eq!(self.auction_view(id).await?["status"], "open");
        let rec: Value = self.mock.view("last_request").await?.json()?;
        let input = rec["input_data"].as_str().unwrap();
        let path = temp_json(input);
        // The caller OutLayer would report is the account that called it.
        let out = helper(&[
            "run",
            path.to_str().unwrap(),
            rec["caller"].as_str().unwrap(),
        ]);
        let _ = std::fs::remove_file(&path);
        self.set_answer(Some(out)).await?;
        self.settle_gas(id, tgas).await
    }

    async fn withdraw(&self, who: &[&AccountId]) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .e
            .outsider
            .call(self.auction.id(), "withdraw")
            .args_json(json!({ "accounts": who }))
            .gas(Gas::from_tgas(300))
            .transact()
            .await?)
    }

    async fn owed(&self, who: &AccountId) -> anyhow::Result<u128> {
        let v: String = self
            .auction
            .view("get_owed")
            .args_json(json!({ "account_id": who }))
            .await?
            .json()?;
        Ok(v.parse()?)
    }

    async fn accounting(&self) -> anyhow::Result<(u128, u128)> {
        let v: Value = self.auction.view("get_accounting").await?.json()?;
        Ok((num(&v["escrow_total"]), num(&v["owed_total"])))
    }

    /// The auction's mock USDC balance equals deposits held plus credits owed.
    async fn assert_auction_solvent(&self) -> anyhow::Result<()> {
        let (escrow, owed) = self.accounting().await?;
        assert_eq!(self.e.balance(self.auction.id()).await?, escrow + owed);
        Ok(())
    }
}

/// Eight bidders: the two seeded ones plus six new members with mock USDC.
async fn eight_bidders(a: &A) -> anyhow::Result<Vec<Account>> {
    let root = a.e.worker.root_account()?;
    let mut bidders: Vec<Account> = vec![a.e.buyer.clone(), a.e.writer_b.clone()];
    for i in 0..6 {
        let acc = root
            .create_subaccount(&format!("bidder{i}"))
            .initial_balance(NearToken::from_near(5))
            .transact()
            .await?
            .into_result()?;
        a.e.grant(acc.id()).await?;
        a.mint_usdc(acc.id(), 200 * MUSDC).await?;
        bidders.push(acc);
    }
    Ok(bidders)
}

/// Gas burnt by each receipt that ran on the auction account, with the event
/// names it logged, so the callbacks can be told apart. Prints a table.
fn auction_gas(label: &str, auction: &AccountId, r: &ExecutionFinalResult) -> Vec<(String, u64)> {
    let mut rows = Vec::new();
    for o in r.outcomes() {
        if &o.executor_id != auction {
            continue;
        }
        let events: Vec<String> = o
            .logs
            .iter()
            .filter_map(|l| l.split("\"event\":\"").nth(1))
            .filter_map(|t| t.split('"').next())
            .map(str::to_string)
            .collect();
        rows.push((events.join(","), o.gas_burnt.as_gas()));
    }
    for (events, gas) in &rows {
        println!(
            "{label}: auction receipt [{events}] burnt {:.2} Tgas",
            *gas as f64 / 1e12
        );
    }
    println!(
        "{label}: total burnt by the whole transaction {:.2} Tgas",
        r.total_gas_burnt.as_gas() as f64 / 1e12
    );
    rows
}

/// Test A1: the auction can hold a share with only the existing registrar
/// call, and a sub-account of the registry cannot be a member (so the auction
/// lives beside it).
#[tokio::test]
async fn a01_auction_is_a_plain_member_no_registry_change() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let cred: Value =
        a.e.reg
            .view("get_credential")
            .args_json(json!({ "account_id": a.auction.id() }))
            .await?
            .json()?;
    assert_eq!(cred["status"], "active");
    // The registry source and wasm are the audited build.
    assert_eq!(a.e.worker.view_code(a.e.reg.id()).await?, wasm("registry"));
    // A sub-account of the registry is a system account.
    let r =
        a.e.registrar
            .call(a.e.reg.id(), "grant_credential")
            .args_json(json!({
                "account": format!("auction.{}", a.e.reg.id()),
                "org_name_hash": hash('a'), "kyb_ref_hash": hash('b'), "doc_hash": hash('c'),
            }))
            .deposit(NearToken::from_millinear(20))
            .max_gas()
            .transact()
            .await?;
    assert!(r.is_failure());
    Ok(())
}

/// Test A2: full flow with the real WASI module. Second price, share to the
/// winner, seller paid, change and refunds credited, withdraw pays out.
#[tokio::test]
async fn a02_full_sale_with_the_real_module() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let seller = &a.e.writer_a;
    let (b1, b2) = (&a.e.buyer, &a.e.writer_b);
    let token = a.new_share("USRC12600101").await?;
    let before1 = a.e.balance(b1.id()).await?;
    let before2 = a.e.balance(b2.id()).await?;
    let seller_before = a.e.balance(seller.id()).await?;

    let r = a.list(&token, 62).await?;
    check(r);
    assert_eq!(a.e.owner(&token).await?, a.auction.id().as_str());
    assert_eq!(a.auction_view(0).await?["status"], "open");

    a.bid(b1, 0, 90 * MUSDC).await?;
    a.bid(b2, 0, 60 * MUSDC).await?;
    let v = a.auction_view(0).await?;
    assert_eq!(v["bid_count"], 2);
    assert_eq!(a.e.balance(b1.id()).await?, before1 - MAX);
    assert_eq!(a.accounting().await?, (2 * MAX, 0));
    a.assert_auction_solvent().await?;

    a.wait_until_ended(0).await?;
    let settle = a.settle_with_real_tee(0).await?;
    let settle = ok(settle);
    println!("settle logs: {:?}", settle.logs());

    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "sold");
    assert_eq!(v["winner"], b1.id().as_str());
    assert_eq!(num(&v["price"]), 60 * MUSDC, "second price");
    assert_eq!(a.e.owner(&token).await?, b1.id().as_str());

    // What OutLayer saw: the real input, no amounts in the clear.
    let rec: Value = a.mock.view("last_request").await?.json()?;
    let input = rec["input_data"].as_str().unwrap();
    assert!(!input.contains("90000000") && !input.contains("60000000"));
    assert_eq!(rec["caller"], a.auction.id().as_str());
    assert_eq!(rec["payer"], a.e.outsider.id().as_str());
    let src: Value = serde_json::from_str(rec["source"].as_str().unwrap())?;
    assert_eq!(src["WasmUrl"]["build_target"], "wasm32-wasip1");
    let sref: Value = serde_json::from_str(rec["secrets_ref"].as_str().unwrap())?;
    assert_eq!(sref["profile"], "royltee-auction");

    // Credits, then payout.
    assert_eq!(a.owed(seller.id()).await?, 60 * MUSDC);
    assert_eq!(a.owed(b1.id()).await?, MAX - 60 * MUSDC);
    assert_eq!(a.owed(b2.id()).await?, MAX);
    assert_eq!(a.accounting().await?, (0, 2 * MAX));
    a.assert_auction_solvent().await?;
    check(a.withdraw(&[seller.id(), b1.id(), b2.id()]).await?);
    assert_eq!(a.e.balance(seller.id()).await?, seller_before + 60 * MUSDC);
    assert_eq!(a.e.balance(b1.id()).await?, before1 - 60 * MUSDC);
    assert_eq!(a.e.balance(b2.id()).await?, before2);
    assert_eq!(a.accounting().await?, (0, 0));
    assert_eq!(a.e.balance(a.auction.id()).await?, 0);
    // The share is the buyer's now, in the registry's own state.
    assert!(a
        .auction
        .view("auction_for_token")
        .args_json(json!({"token_id": token}))
        .await?
        .json::<Option<String>>()?
        .is_none());
    a.e.assert_solvent().await
}

/// Test A3: money paid into the vault while the auction holds the share
/// travels with the share to the winner (the registry's rule), and the
/// auction cannot claim it.
#[tokio::test]
async fn a03_vault_money_travels_with_the_share() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600102").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 70 * MUSDC).await?;
    a.bid(&a.e.writer_b, 0, 40 * MUSDC).await?;
    // A payment lands in the vault during the auction, and is synced.
    check(a.e.pay_share(&token, 25 * MUSDC).await?);
    // Only the holder or the registrar can sync, and the holder is the
    // auction, which has no such call. The registrar syncs.
    check(
        a.e.registrar
            .call(a.e.reg.id(), "sync")
            .args_json(json!({ "token_id": token }))
            .max_gas()
            .transact()
            .await?,
    );
    // The auction (the holder) has no call that claims, and nobody else can.
    for who in [&a.e.writer_a, &a.e.outsider] {
        let r = a.e.claim(who, &token).await?;
        assert!(r.is_failure(), "only the holder can claim");
    }
    let claimable = num(&a.e.ledger(&token).await?["claimable"]);
    assert_eq!(claimable, 25 * MUSDC);

    a.wait_until_ended(0).await?;
    check(a.settle_with_real_tee(0).await?);
    assert_eq!(a.auction_view(0).await?["status"], "sold");
    // The winner now holds the share and its unclaimed money.
    let before = a.e.balance(a.e.buyer.id()).await?;
    check(a.e.claim(&a.e.buyer, &token).await?);
    assert_eq!(a.e.balance(a.e.buyer.id()).await?, before + 25 * MUSDC);
    a.e.assert_solvent().await
}

/// Test A4: bad bids are refunded in full and change nothing.
#[tokio::test]
async fn a04_bad_bids_are_refunded() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600103").await?;
    check(a.list(&token, 120).await?);
    // Outsider: no credential. Register with mock USDC and fund it first.
    check(
        a.e.payer
            .call(a.e.usdc.id(), "storage_deposit")
            .args_json(json!({ "account_id": a.e.outsider.id(), "registration_only": true }))
            .deposit(NearToken::from_millinear(10))
            .transact()
            .await?,
    );
    a.mint_usdc(a.e.outsider.id(), MAX).await?;
    let ct = a.seal(0, a.e.outsider.id(), 50 * MUSDC);
    let before = a.e.balance(a.e.outsider.id()).await?;
    let r = a.bid_with(&a.e.outsider, 0, MAX, &ct).await?;
    assert!(r.is_success());
    assert_eq!(
        a.e.balance(a.e.outsider.id()).await?,
        before,
        "non-member refunded"
    );

    let (b1, seller) = (&a.e.buyer, &a.e.writer_a);
    let ct1 = a.seal(0, b1.id(), 50 * MUSDC);
    // Wrong deposit amount.
    let bal = a.e.balance(b1.id()).await?;
    check(a.bid_with(b1, 0, MAX - 1, &ct1).await?);
    assert_eq!(a.e.balance(b1.id()).await?, bal);
    // Not base64.
    check(a.bid_with(b1, 0, MAX, "not base64!").await?);
    assert_eq!(a.e.balance(b1.id()).await?, bal);
    // Seller bidding on its own share (the seller is a member with funds).
    a.mint_usdc(seller.id(), MAX).await?;
    let sct = a.seal(0, seller.id(), 50 * MUSDC);
    let sbal = a.e.balance(seller.id()).await?;
    check(a.bid_with(seller, 0, MAX, &sct).await?);
    assert_eq!(a.e.balance(seller.id()).await?, sbal);
    // A good bid, then a second bid from the same account.
    check(a.bid_with(b1, 0, MAX, &ct1).await?);
    assert_eq!(a.e.balance(b1.id()).await?, bal - MAX);
    check(a.bid_with(b1, 0, MAX, &ct1).await?);
    assert_eq!(
        a.e.balance(b1.id()).await?,
        bal - MAX,
        "second bid refunded"
    );
    assert_eq!(a.auction_view(0).await?["bid_count"], 1);
    a.assert_auction_solvent().await
}

/// Test A5: no bid reaches the reserve: the real module finds no winner,
/// the auction cancels, everyone is refunded, the share goes back.
#[tokio::test]
async fn a05_reserve_not_met_cancels_and_returns_the_share() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600104").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 5 * MUSDC).await?; // under the 10 mUSDC reserve
    a.wait_until_ended(0).await?;
    check(a.settle_with_real_tee(0).await?);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "cancelled");
    assert_eq!(v["share_held"], false, "share is back with the seller");
    assert_eq!(a.e.owner(&token).await?, a.e.writer_a.id().as_str());
    assert_eq!(a.owed(a.e.buyer.id()).await?, MAX);
    check(a.withdraw(&[a.e.buyer.id()]).await?);
    assert_eq!(a.accounting().await?, (0, 0));
    assert_eq!(a.e.balance(a.auction.id()).await?, 0);
    Ok(())
}

/// Test A6: the winner is suspended before delivery. The registry refuses the
/// transfer, so the auction cancels, refunds all (the winner too) and the
/// share returns to the seller.
#[tokio::test]
async fn a06_suspended_winner_cancels_and_refunds_everyone() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600105").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 90 * MUSDC).await?;
    a.bid(&a.e.writer_b, 0, 60 * MUSDC).await?;
    a.wait_until_ended(0).await?;
    check(
        a.e.registrar
            .call(a.e.reg.id(), "suspend_credential")
            .args_json(json!({ "account": a.e.buyer.id(), "doc_hash": hash('9') }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    // The registry's refusal is a failed receipt by design.
    let r = a.settle_with_real_tee(0).await?;
    assert!(r.is_success());
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "cancelled");
    assert_eq!(v["share_held"], false);
    assert_eq!(a.e.owner(&token).await?, a.e.writer_a.id().as_str());
    assert_eq!(a.owed(a.e.buyer.id()).await?, MAX);
    assert_eq!(a.owed(a.e.writer_b.id()).await?, MAX);
    assert_eq!(a.owed(a.e.writer_a.id()).await?, 0);
    check(a.withdraw(&[a.e.buyer.id(), a.e.writer_b.id()]).await?);
    assert_eq!(a.accounting().await?, (0, 0));
    Ok(())
}

/// Test A7: answers the contract must not trust. None moves money or the
/// share; each reopens settling.
#[tokio::test]
async fn a07_bad_answers_never_move_money_or_the_share() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600106").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 90 * MUSDC).await?;
    a.bid(&a.e.writer_b, 0, 60 * MUSDC).await?;
    a.wait_until_ended(0).await?;
    let hash: String = a
        .auction
        .view("get_bids_hash")
        .args_json(json!({ "auction_id": "0" }))
        .await?
        .json()?;
    let answers = [
        // Winner who never bid.
        json!({"v":1,"ok":true,"auction":"0","bids_hash":hash,"winner":a.e.outsider.id(),"price":"20000000"}),
        // Price above the deposit, and below the reserve.
        json!({"v":1,"ok":true,"auction":"0","bids_hash":hash,"winner":a.e.buyer.id(),"price":(MAX + 1).to_string()}),
        json!({"v":1,"ok":true,"auction":"0","bids_hash":hash,"winner":a.e.buyer.id(),"price":(RESERVE - 1).to_string()}),
        // A different bid list, a different auction, ok false, junk.
        json!({"v":1,"ok":true,"auction":"0","bids_hash":"00","winner":a.e.buyer.id(),"price":"20000000"}),
        json!({"v":1,"ok":true,"auction":"1","bids_hash":hash,"winner":a.e.buyer.id(),"price":"20000000"}),
        json!({"v":1,"ok":false,"error":"x"}),
        json!("junk"),
    ];
    for (i, ans) in answers.iter().enumerate() {
        a.set_answer(Some(ans.to_string())).await?;
        check(a.settle(0).await?);
        let v = a.auction_view(0).await?;
        assert_eq!(v["status"], "open", "answer {i}");
        assert_eq!(v["failures"], i + 1, "answer {i}");
        assert_eq!(a.accounting().await?, (2 * MAX, 0), "answer {i}");
        assert_eq!(
            a.e.owner(&token).await?,
            a.auction.id().as_str(),
            "answer {i}"
        );
    }
    // A request OutLayer rejects up front also reopens settling.
    check(
        a.mock
            .call("set_reject")
            .args_json(json!({"reject": true}))
            .transact()
            .await?,
    );
    let r = a.settle(0).await?;
    assert!(r.is_success());
    assert_eq!(a.auction_view(0).await?["status"], "open");
    check(
        a.mock
            .call("set_reject")
            .args_json(json!({"reject": false}))
            .transact()
            .await?,
    );
    // Then a real run still settles it.
    check(a.settle_with_real_tee(0).await?);
    assert_eq!(a.auction_view(0).await?["status"], "sold");
    Ok(())
}

/// Test A8: the seller cancels before any bid and gets the share back.
#[tokio::test]
async fn a08_seller_cancels_before_bids() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600107").await?;
    check(a.list(&token, 600).await?);
    assert_eq!(a.e.owner(&token).await?, a.auction.id().as_str());
    // A stranger cannot cancel an open auction.
    let r =
        a.e.outsider
            .call(a.auction.id(), "cancel")
            .args_json(json!({"auction_id":"0"}))
            .max_gas()
            .transact()
            .await?;
    assert!(r.is_failure());
    check(
        a.e.writer_a
            .call(a.auction.id(), "cancel")
            .args_json(json!({"auction_id":"0"}))
            .max_gas()
            .transact()
            .await?,
    );
    assert_eq!(a.e.owner(&token).await?, a.e.writer_a.id().as_str());
    // B-06: an auction that never had a bid leaves no record behind.
    assert!(a.auction_view(0).await?.is_null());
    assert!(a
        .auction
        .view("auction_for_token")
        .args_json(json!({"token_id": token}))
        .await?
        .json::<Value>()?
        .is_null());
    // The share can be listed again.
    check(a.list(&token, 600).await?);
    assert_eq!(a.auction_view(1).await?["status"], "open");
    Ok(())
}

/// Test A9: a refused listing hands the share straight back.
#[tokio::test]
async fn a09_refused_listings_return_the_share() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600108").await?;
    let ends = a.now().await? + 600 * 1_000_000_000;
    for msg in [
        json!("not an object"),
        json!({"reserve":"0","max_deposit":"5","ends_at":ends.to_string()}),
        json!({"reserve":"6","max_deposit":"5","ends_at":ends.to_string()}),
        json!({"reserve":"5","max_deposit":"5","ends_at":"1"}),
    ] {
        let r = a.list_msg(&token, msg).await?;
        assert!(r.is_success());
        assert_eq!(a.e.owner(&token).await?, a.e.writer_a.id().as_str());
    }
    assert_eq!(
        a.auction.view("get_config").await?.json::<Value>()?["next_id"],
        "0"
    );
    // A stranger calling `nft_on_transfer` directly creates nothing.
    let r = a
        .e
        .outsider
        .call(a.auction.id(), "nft_on_transfer")
        .args_json(json!({
            "sender_id": a.e.outsider.id(), "previous_owner_id": a.e.outsider.id(),
            "token_id": "s1", "msg": json!({"reserve":"5","max_deposit":"5","ends_at":ends.to_string()}).to_string(),
        }))
        .max_gas()
        .transact()
        .await?;
    assert!(r.is_failure());
    Ok(())
}

/// Test A10: the registrar suspends the auction while it holds a share. The
/// share cannot leave by the normal path, the registrar rescues it with the
/// existing `registrar_transfer`, and `refresh_share` clears the auction's
/// record. Deposits are never at risk.
#[tokio::test]
async fn a10_suspended_auction_share_is_rescued_by_the_registrar() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600109").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 90 * MUSDC).await?;
    a.wait_until_ended(0).await?;
    check(
        a.e.registrar
            .call(a.e.reg.id(), "suspend_credential")
            .args_json(json!({ "account": a.auction.id(), "doc_hash": hash('9') }))
            .deposit(ONE_YOCTO)
            .transact()
            .await?,
    );
    let r = a.settle_with_real_tee(0).await?;
    assert!(
        r.is_success(),
        "the registry's refusals are failed receipts"
    );
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "cancelled", "delivery failed");
    assert_eq!(v["share_held"], true, "return failed too");
    assert_eq!(a.e.owner(&token).await?, a.auction.id().as_str());
    // Money is safe and can be withdrawn.
    assert_eq!(a.owed(a.e.buyer.id()).await?, MAX);
    check(a.withdraw(&[a.e.buyer.id()]).await?);
    // Registrar rescue.
    check(
        a.e.registrar
            .call(a.e.reg.id(), "registrar_transfer")
            .args_json(json!({ "token_id": token, "receiver_id": a.e.writer_a.id(), "doc_hash": hash('8') }))
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await?,
    );
    assert_eq!(a.e.owner(&token).await?, a.e.writer_a.id().as_str());
    check(
        a.e.outsider
            .call(a.auction.id(), "refresh_share")
            .args_json(json!({"auction_id":"0"}))
            .max_gas()
            .transact()
            .await?,
    );
    assert_eq!(a.auction_view(0).await?["share_held"], false);
    assert_eq!(a.accounting().await?, (0, 0));
    Ok(())
}

/// Test A11: bids made with 100 Tgas fit, and a settle with eight bids fits
/// in 300 Tgas.
#[tokio::test]
async fn a11_eight_bids_settle_within_300_tgas() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600110").await?;
    check(a.list(&token, 62).await?);
    let bidders = eight_bidders(&a).await?;
    for (i, b) in bidders.iter().enumerate() {
        a.bid(b, 0, (20 + 5 * i as u128) * MUSDC).await?;
    }
    assert_eq!(a.auction_view(0).await?["bid_count"], 8);
    a.wait_until_ended(0).await?;
    let r = a.settle_with_real_tee(0).await?;
    let r = ok(r);
    auction_gas("a11 second settle", a.auction.id(), &r);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "sold");
    assert_eq!(v["winner"], bidders[7].id().as_str());
    assert_eq!(num(&v["price"]), 50 * MUSDC, "second highest bid");
    check(
        a.withdraw(&bidders.iter().map(|b| b.id()).take(8).collect::<Vec<_>>()[..])
            .await?,
    );
    check(a.withdraw(&[a.e.writer_a.id()]).await?);
    assert_eq!(a.accounting().await?, (0, 0));
    assert_eq!(a.e.balance(a.auction.id()).await?, 0);
    a.e.assert_solvent().await
}

/// Test A12 (M1): eight valid bids, every ciphertext the largest the contract
/// takes (512 characters), settled with the least gas settle accepts (230
/// Tgas). The callback that reads the answer, hashes the bids and starts the
/// delivery has to fit its 120 Tgas with room to spare, and the sale must
/// finish. Prints the measured gas of each receipt.
#[tokio::test]
async fn a12_eight_max_size_ciphertexts_settle() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600111").await?;
    check(a.list(&token, 62).await?);
    let bidders = eight_bidders(&a).await?;
    for (i, b) in bidders.iter().enumerate() {
        let ct = helper(&[
            "seal-max",
            &a.pubkey,
            "0",
            a.auction.id().as_str(),
            b.id().as_str(),
            &((20 + 5 * i as u128) * MUSDC).to_string(),
        ]);
        assert_eq!(ct.len(), 512);
        check(a.bid_with(b, 0, MAX, &ct).await?);
    }
    let v = a.auction_view(0).await?;
    assert_eq!(v["bid_count"], 8);
    for b in v["bids"].as_array().unwrap() {
        assert_eq!(b["ciphertext"].as_str().unwrap().len(), 512);
    }
    a.wait_until_ended(0).await?;
    // Below the minimum is refused up front.
    a.set_answer(None).await?;
    let r = a.settle_gas(0, 229).await?;
    assert!(r.is_failure(), "229 Tgas is below the minimum");
    let r = a.settle_with_real_tee_gas(0, 230).await?;
    let r = ok(r);
    let rows = auction_gas("a12 second settle at 230 Tgas", a.auction.id(), &r);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "sold");
    assert_eq!(v["winner"], bidders[7].id().as_str());
    assert_eq!(num(&v["price"]), 50 * MUSDC);
    // Every receipt on the auction used well under its allowance. The two
    // callbacks are the ones that logged `winner_chosen` and `auction_sold`.
    let on_settled = rows.iter().find(|(e, _)| e.contains("winner_chosen"));
    let on_delivered = rows.iter().find(|(e, _)| e.contains("auction_sold"));
    let (on_settled, on_delivered) = (on_settled.unwrap().1, on_delivered.unwrap().1);
    assert!(
        on_settled < 120_000_000_000_000 - 85_000_000_000_000,
        "on_settled must leave room for the two promises it creates"
    );
    assert!(on_delivered < 60_000_000_000_000);
    check(
        a.withdraw(&bidders.iter().map(|b| b.id()).collect::<Vec<_>>()[..])
            .await?,
    );
    check(a.withdraw(&[a.e.writer_a.id()]).await?);
    assert_eq!(a.accounting().await?, (0, 0));
    a.e.assert_solvent().await
}

/// Test A13 (C1): a request to OutLayer from any account other than the
/// auction contract is refused by the module, so the secret cannot be used as
/// a decryption oracle. The attacker crafts an input with a victim's real
/// ciphertext and their own bid at the maximum, calls the (mock) OutLayer
/// directly, and the module, given the caller OutLayer reports, prints no
/// winner and no price. The same input from the auction contract is answered.
/// Simulated: OutLayer is the mock; the module, the ciphertexts and the
/// contract are real. What OutLayer's keystore does is not tested here.
#[tokio::test]
async fn a13_a_direct_request_from_another_caller_is_refused() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600112").await?;
    check(a.list(&token, 62).await?);
    let (victim, other) = (&a.e.buyer, &a.e.writer_b);
    a.bid(victim, 0, 55 * MUSDC).await?;
    // The attacker (an outsider, not even a member) seals a bid at the
    // maximum for their own name. All ciphertexts are public chain data.
    let attacker = a.e.outsider.id();
    let atk_ct = a.seal(0, attacker, MAX);
    let v = a.auction_view(0).await?;
    let victim_ct = v["bids"][0]["ciphertext"].as_str().unwrap().to_string();
    let crafted = json!({
        "mode": "settle", "auction_id": "0", "contract": a.auction.id(),
        "pubkey": a.pubkey, "key_version": 1,
        "reserve": RESERVE.to_string(), "max_deposit": MAX.to_string(),
        "bids": [
            {"bidder": victim.id(), "ct": victim_ct},
            {"bidder": attacker, "ct": atk_ct},
        ],
    })
    .to_string();
    // The attacker calls OutLayer directly, so the mock records them as caller.
    check(
        a.e.outsider
            .call(a.mock.id(), "request_execution")
            .args_json(json!({
                "source": {"WasmUrl": {"url": "https://x", "hash": "aa", "build_target": "wasm32-wasip1"}},
                "resource_limits": {"max_instructions": 1_000_000_000u64, "max_memory_mb": 64, "max_execution_seconds": 30},
                "input_data": crafted,
                "secrets_ref": {"profile": "royltee-auction", "account_id": a.e.writer_a.id()},
                "response_format": "Json",
            }))
            .deposit(NearToken::from_millinear(20))
            .transact()
            .await?,
    );
    let rec: Value = a.mock.view("last_request").await?.json()?;
    assert_eq!(rec["caller"], attacker.as_str());
    let run = |caller: &str| {
        let path = temp_json(&crafted);
        let out = helper(&["run", path.to_str().unwrap(), caller]);
        let _ = std::fs::remove_file(&path);
        serde_json::from_str::<Value>(&out).unwrap()
    };
    let refused = run(rec["caller"].as_str().unwrap());
    assert_eq!(refused["ok"], false);
    assert!(refused.get("winner").is_none() && refused.get("price").is_none());
    assert!(!refused.to_string().contains(&(55 * MUSDC).to_string()));
    // Naming the auction as the caller inside the input does not help: the
    // module trusts only the environment OutLayer sets. Naming the attacker's
    // own account as the contract does not help either (B-02): the module
    // serves one account, fixed when it was built.
    let own = crafted.replace(a.auction.id().as_str(), attacker.as_str());
    let path = temp_json(&own);
    let out = helper(&["run", path.to_str().unwrap(), attacker.as_str()]);
    let _ = std::fs::remove_file(&path);
    let own_out: Value = serde_json::from_str(&out)?;
    assert_eq!(own_out["ok"], false);
    assert!(own_out.get("price").is_none());
    // The auction itself is answered, with the price that would leak.
    let answered = run(a.auction.id().as_str());
    assert_eq!(answered["price"], (55 * MUSDC).to_string());

    // Fed to the auction as an answer, a refusal changes nothing.
    a.bid(other, 0, 40 * MUSDC).await?;
    a.wait_until_ended(0).await?;
    a.set_answer(Some(refused.to_string())).await?;
    check(a.settle(0).await?);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "open");
    assert_eq!(v["failures"], 1);
    assert_eq!(a.accounting().await?, (2 * MAX, 0));
    assert_eq!(a.e.owner(&token).await?, a.auction.id().as_str());
    // And the real run through the contract still settles.
    check(a.settle_with_real_tee(0).await?);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "sold");
    assert_eq!(num(&v["price"]), 40 * MUSDC);
    a.e.assert_solvent().await
}

/// Test A14 (B-04): a sale completes when the auction account has almost no
/// spare NEAR. The payout callback writes credits for up to nine accounts, but
/// the ciphertexts it no longer needs are freed in the same call, and each
/// open auction keeps a storage reserve at listing, so the callback fits. (An
/// earlier version of this test drained the account to make the callback fail
/// and then recovered with `reconcile`; with the ciphertexts dropped that
/// failure cannot be produced here, so `reconcile` is covered by unit tests.)
#[tokio::test]
async fn a14_a_sale_completes_with_almost_no_spare_near() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let token = a.new_share("USRC12600113").await?;
    check(a.list(&token, 62).await?);
    a.bid(&a.e.buyer, 0, 90 * MUSDC).await?;
    a.bid(&a.e.writer_b, 0, 60 * MUSDC).await?;
    a.wait_until_ended(0).await?;
    // Leave the auction account 100 bytes of free storage stake (the transfer costs a little gas, and `on_settled` stores the winner and the price before the payout frees the ciphertexts).
    let info = a.auction.view_account().await?;
    let locked = info.storage_usage as u128 * 10u128.pow(19);
    let keep = locked + 100 * 10u128.pow(19);
    let spare = info.balance.as_yoctonear() - keep;
    let root = a.e.worker.root_account()?;
    check(
        a.auction
            .as_account()
            .transfer_near(root.id(), NearToken::from_yoctonear(spare))
            .await?,
    );
    check(a.settle_with_real_tee(0).await?);
    let v = a.auction_view(0).await?;
    assert_eq!(v["status"], "sold", "the payout callback committed");
    assert_eq!(a.e.owner(&token).await?, a.e.buyer.id().as_str());
    assert_eq!(num(&v["price"]), 60 * MUSDC);
    assert!(v["bids"]
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["ciphertext"] == ""));
    assert_eq!(a.accounting().await?, (0, 2 * MAX));
    a.assert_auction_solvent().await?;
    // AR-4: with almost no free NEAR the contract refuses to write pending
    // withdrawal records, and nothing is debited.
    let refused = a
        .withdraw(&[a.e.writer_a.id(), a.e.buyer.id(), a.e.writer_b.id()])
        .await?;
    assert!(
        refused.is_failure(),
        "withdraw needs the free balance floor"
    );
    assert_eq!(a.accounting().await?, (0, 2 * MAX));
    // After a top up the same call works.
    check(
        root.transfer_near(a.auction.id(), NearToken::from_near(1))
            .await?,
    );
    check(
        a.withdraw(&[a.e.writer_a.id(), a.e.buyer.id(), a.e.writer_b.id()])
            .await?,
    );
    assert_eq!(a.accounting().await?, (0, 0));
    a.e.assert_solvent().await
}

/// Test A15 (B-04): admission counts the storage reserve of every open
/// auction. With room for the floor and one and a half reserves, the first
/// listing is accepted and the second is refused (the registry hands the
/// share back).
#[tokio::test]
async fn a15_listing_counts_the_reserve_of_every_open_auction() -> anyhow::Result<()> {
    let a = setup_auction().await?;
    let t1 = a.new_share("USRC12600114").await?;
    let t2 = a.new_share("USRC12600115").await?;
    // Free balance 0.37 NEAR: the floor is 0.25 plus 0.08 for each open
    // auction and the new one, so one listing fits (0.33) and two do not (0.41).
    let info = a.auction.view_account().await?;
    let locked = info.storage_usage as u128 * 10u128.pow(19);
    let keep = locked + 370_000_000_000_000_000_000_000u128;
    let spare = info.balance.as_yoctonear() - keep;
    let root = a.e.worker.root_account()?;
    check(
        a.auction
            .as_account()
            .transfer_near(root.id(), NearToken::from_yoctonear(spare))
            .await?,
    );
    check(a.list(&t1, 600).await?);
    assert_eq!(a.e.owner(&t1).await?, a.auction.id().as_str());
    check(a.list(&t2, 600).await?);
    assert_eq!(
        a.e.owner(&t2).await?,
        a.e.writer_a.id().as_str(),
        "the second listing was refused and the share handed back"
    );
    let acct: Value = a.auction.view("get_accounting").await?.json()?;
    assert_eq!(acct["open_auctions"], 1);
    Ok(())
}
