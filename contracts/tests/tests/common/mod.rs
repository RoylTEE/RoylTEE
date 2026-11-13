//! Shared sandbox setup: mock USDC, registry, members, helpers.
#![allow(dead_code)]

use near_workspaces::network::Sandbox;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{Gas, NearToken};
use near_workspaces::{Account, AccountId, Contract, Worker};
use serde_json::{json, Value};

pub const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);
pub const MUSDC: u128 = 1_000_000; // 6 decimals

pub fn hash(c: char) -> String {
    c.to_string().repeat(64)
}

pub fn wasm(name: &str) -> Vec<u8> {
    let path = format!("{}/../res/{name}.wasm", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("missing {path}: run `npm run build:wasm` first"))
}

pub fn project_config() -> Value {
    let path = format!("{}/../../config/project.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub struct Env {
    pub worker: Worker<Sandbox>,
    pub usdc: Contract,
    pub reg: Contract,
    pub registrar: Account,
    pub writer_a: Account,
    pub writer_b: Account,
    pub buyer: Account,
    pub outsider: Account,
    pub payer: Account,
}

async fn sub(parent: &Account, name: &str, near: u128) -> anyhow::Result<Account> {
    Ok(parent
        .create_subaccount(name)
        .initial_balance(NearToken::from_near(near))
        .transact()
        .await?
        .into_result()?)
}

pub fn ok(r: ExecutionFinalResult) -> ExecutionFinalResult {
    assert!(r.is_success(), "transaction failed: {:#?}", r.failures());
    assert!(
        r.receipt_failures().is_empty(),
        "a receipt failed: {:#?}",
        r.receipt_failures()
    );
    r
}

/// Like `ok`, for calls whose result is not needed.
pub fn check(r: ExecutionFinalResult) {
    let _ = ok(r);
}

pub async fn setup() -> anyhow::Result<Env> {
    setup_with_registry(&wasm("registry")).await
}

/// Same as `setup`, with the given registry wasm (the upgrade test starts
/// from the registry wasm deployed on testnet).
pub async fn setup_with_registry(registry_wasm: &[u8]) -> anyhow::Result<Env> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let dev = sub(&root, "demo", 200).await?;
    let registrar = sub(&dev, "registrar", 10).await?;
    let writer_a = sub(&dev, "writer-a", 10).await?;
    let writer_b = sub(&dev, "writer-b", 10).await?;
    let buyer = sub(&dev, "buyer-pub", 10).await?;
    let outsider = sub(&dev, "outsider", 10).await?;
    let payer = sub(&dev, "payer", 10).await?;

    let usdc_acc = sub(&root, "usdc", 20).await?;
    let usdc = usdc_acc.deploy(&wasm("mock_usdc")).await?.into_result()?;
    check(
        usdc.call("new")
            .args_json(json!({ "minter": payer.id() }))
            .transact()
            .await?,
    );

    let reg_acc = sub(&root, "reg", 50).await?;
    let reg = reg_acc.deploy(registry_wasm).await?.into_result()?;
    let cfg = project_config();
    check(
        reg.call("new")
            .args_json(json!({
                "registrar": registrar.id(),
                "usdc": usdc.id(),
                "metadata": {
                    "spec": "nft-1.0.0",
                    "name": cfg["projectName"],
                    "symbol": cfg["nftSymbol"],
                },
            }))
            .transact()
            .await?,
    );

    let env = Env {
        worker,
        usdc,
        reg,
        registrar,
        writer_a,
        writer_b,
        buyer,
        outsider,
        payer,
    };
    for m in [&env.writer_a, &env.writer_b, &env.buyer] {
        env.grant(m.id()).await?;
    }
    // The registry must be registered with the token to receive payments.
    check(
        env.payer
            .call(env.usdc.id(), "storage_deposit")
            .args_json(json!({ "account_id": env.reg.id(), "registration_only": true }))
            .deposit(NearToken::from_millinear(10))
            .transact()
            .await?,
    );
    // Simulated payer: registers with the mock token and mints mock USDC.
    check(
        env.payer
            .call(env.usdc.id(), "storage_deposit")
            .args_json(json!({ "account_id": env.payer.id(), "registration_only": true }))
            .deposit(NearToken::from_millinear(10))
            .transact()
            .await?,
    );
    check(
        env.payer
            .call(env.usdc.id(), "mint")
            .args_json(
                json!({ "account_id": env.payer.id(), "amount": (1_000 * MUSDC).to_string() }),
            )
            .transact()
            .await?,
    );
    Ok(env)
}

impl Env {
    pub async fn grant(&self, account: &AccountId) -> anyhow::Result<()> {
        check(
            self.registrar
                .call(self.reg.id(), "grant_credential")
                .args_json(json!({
                    "account": account,
                    "org_name_hash": hash('a'),
                    "kyb_ref_hash": hash('b'),
                    "doc_hash": hash('c'),
                }))
                .deposit(NearToken::from_millinear(20))
                .max_gas()
                .transact()
                .await?,
        );
        Ok(())
    }

    pub async fn register_work(&self, code: &str) -> anyhow::Result<u64> {
        let r = ok(self
            .writer_a
            .call(self.reg.id(), "register_work")
            .args_json(json!({
                "code_type": "isrc",
                "code": code,
                "title_hash": hash('d'),
                "provenance": "human",
                "metadata_uri": "",
            }))
            .deposit(NearToken::from_millinear(50))
            .transact()
            .await?);
        Ok(r.json()?)
    }

    /// Registers and mints one publishing share; returns its token id.
    pub async fn mint(&self, work_id: u64, holder: &AccountId, bps: u16) -> anyhow::Result<String> {
        let r = ok(self
            .writer_a
            .call(self.reg.id(), "register_share")
            .args_json(json!({
                "work_id": work_id,
                "right_type": "publishing",
                "territory": "WW",
                "percent_bps": bps,
                "basis": "copyright",
                "holder": holder,
                "evidence_hash": hash('e'),
            }))
            .deposit(NearToken::from_millinear(500))
            .max_gas()
            .transact()
            .await?);
        assert!(r.clone().json::<bool>()?, "mint must succeed");
        let token_id = r
            .logs()
            .iter()
            .find(|l| l.contains("\"share_registered\""))
            .and_then(|l| {
                let v: Value = serde_json::from_str(l.trim_start_matches("EVENT_JSON:")).ok()?;
                v["data"][0]["token_id"].as_str().map(str::to_string)
            })
            .expect("share_registered event");
        Ok(token_id)
    }

    pub fn vault(&self, token_id: &str) -> AccountId {
        format!("{token_id}.{}", self.reg.id()).parse().unwrap()
    }

    pub async fn balance(&self, account: &AccountId) -> anyhow::Result<u128> {
        let v: String = self
            .usdc
            .view("ft_balance_of")
            .args_json(json!({ "account_id": account }))
            .await?
            .json()?;
        Ok(v.parse()?)
    }

    pub async fn ledger(&self, token_id: &str) -> anyhow::Result<Value> {
        Ok(self
            .reg
            .view("get_ledger")
            .args_json(json!({ "token_id": token_id }))
            .await?
            .json()?)
    }

    pub async fn owner(&self, token_id: &str) -> anyhow::Result<String> {
        let t: Value = self
            .reg
            .view("nft_token")
            .args_json(json!({ "token_id": token_id }))
            .await?
            .json()?;
        Ok(t["owner_id"].as_str().unwrap().to_string())
    }

    pub async fn pay_work(
        &self,
        work_id: u64,
        amount: u128,
    ) -> anyhow::Result<ExecutionFinalResult> {
        let msg = json!({ "pay_work": {
            "work_id": work_id, "right_type": "publishing", "territory": "WW",
            "period": "2026-Q3", "statement_hash": hash('f'), "statement_line_id": "line-1",
        }});
        self.ft_call(amount, msg).await
    }

    pub async fn pay_share(
        &self,
        token_id: &str,
        amount: u128,
    ) -> anyhow::Result<ExecutionFinalResult> {
        let msg = json!({ "pay_share": {
            "token_id": token_id, "period": "2026-Q3", "statement_hash": hash('f'),
        }});
        self.ft_call(amount, msg).await
    }

    async fn ft_call(&self, amount: u128, msg: Value) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .payer
            .call(self.usdc.id(), "ft_transfer_call")
            .args_json(json!({ "receiver_id": self.reg.id(), "amount": amount.to_string(), "msg": msg.to_string() }))
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await?)
    }

    pub async fn claim(
        &self,
        who: &Account,
        token_id: &str,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(who
            .call(self.reg.id(), "claim")
            .args_json(json!({ "token_id": token_id }))
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await?)
    }

    pub async fn transfer(
        &self,
        from: &Account,
        to: &AccountId,
        token_id: &str,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(from
            .call(self.reg.id(), "nft_transfer")
            .args_json(json!({ "receiver_id": to, "token_id": token_id }))
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await?)
    }

    /// Zero-keys proof: the vault's access key list must be empty.
    pub async fn assert_keyless(&self, token_id: &str) -> anyhow::Result<()> {
        let vault = self.vault(token_id);
        let keys = self.worker.view_access_keys(&vault).await?;
        println!(
            "zero-keys proof: view_access_key_list({vault}) -> {} keys {:?}",
            keys.len(),
            keys
        );
        assert!(keys.is_empty(), "vault {vault} must have zero access keys");
        Ok(())
    }

    /// Solvency invariant (T31): the registry holds at least what it owes.
    pub async fn assert_solvent(&self) -> anyhow::Result<()> {
        let a: Value = self.reg.view("get_accounting").await?.json()?;
        let owed: u128 = [
            "allowance_total",
            "unallocated_total",
            "escrow_total",
            "undelivered_total",
            "forwards_in_flight_total",
        ]
        .iter()
        .map(|k| a[k].as_str().unwrap().parse::<u128>().unwrap())
        .sum();
        let held = self.balance(self.reg.id()).await?;
        assert!(held >= owed, "registry holds {held} but owes {owed}");
        Ok(())
    }

    pub async fn deploy_receiver(&self) -> anyhow::Result<Contract> {
        let root = self.worker.root_account()?;
        let acc = sub(&root, "receiver", 10).await?;
        let c = acc.deploy(&wasm("test_receiver")).await?.into_result()?;
        check(
            c.call("new")
                .args_json(json!({ "registry": self.reg.id() }))
                .transact()
                .await?,
        );
        self.grant(c.id()).await?;
        Ok(c)
    }
}

pub fn num(v: &Value) -> u128 {
    v.as_str().unwrap().parse().unwrap()
}

pub fn max_gas() -> Gas {
    Gas::from_tgas(300)
}
