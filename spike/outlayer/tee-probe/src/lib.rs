//! Throwaway testnet contract for the OutLayer call-path check.
//!
//! `probe(input)` forwards the attached NEAR to `request_execution` on the
//! OutLayer contract with a small resource limit and chains `on_result`,
//! which stores what came back and logs it. Nothing here holds user money
//! beyond one call: the full deposit goes to OutLayer, which refunds unused
//! funds to `payer_account_id`, set to the caller of `probe`.
//!
//! Argument shapes follow the OutLayer contract source
//! (fastnear/near-outlayer, `contract/src/lib.rs` `ExecutionSource`,
//! `ResourceLimits`, `ResponseFormat`; `contract/src/execution.rs`
//! `request_execution`) and the NEAR integration docs. The callback reads the
//! raw return value: the current OutLayer callback returns the guest output as
//! JSON (an object for `Json`) or `null` when the run failed. The older
//! `{success, result, error}` wrapper used by the private DAO example is also
//! understood.
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, json, Value};
use near_sdk::{
    env, near, require, AccountId, Gas, GasWeight, NearToken, PanicOnDefault, Promise, PromiseError,
};

/// Longest `input` accepted, in bytes. Keeps the request under OutLayer's
/// free input size and the event log limit.
pub const MAX_INPUT: usize = 1024;
/// Longest return value read in the callback, in bytes.
pub const MAX_RETURN: usize = 16 * 1024;
/// Longest output stored in state, in bytes.
pub const MAX_STORED: usize = 2048;
/// Gas for the `request_execution` call itself. Unused gas also flows there
/// (weight 1), as in the private DAO example.
pub const REQUEST_GAS: Gas = Gas::from_tgas(50);
/// Static gas for `on_result`.
pub const CALLBACK_GAS: Gas = Gas::from_tgas(15);

/// Where the WASI code comes from. Serialises to the same externally tagged
/// JSON as OutLayer's `ExecutionSource`.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    WasmUrl {
        url: String,
        hash: String,
        build_target: Option<String>,
    },
    Project {
        project_id: String,
        version_key: Option<String>,
    },
}

/// Mirrors OutLayer's `ResourceLimits`.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        // Small on purpose: an echo needs far less than this.
        Self {
            max_instructions: 100_000_000,
            max_memory_mb: 64,
            max_execution_seconds: 10,
        }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeResult {
    pub probe_id: u64,
    pub caller: AccountId,
    pub ok: bool,
    /// Output as returned by OutLayer, as compact JSON text, cut to `MAX_STORED`.
    pub output: Option<String>,
    pub error: Option<String>,
    pub block_height: u64,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct TeeProbe {
    owner: AccountId,
    outlayer: AccountId,
    source: Source,
    limits: Limits,
    next_probe: u64,
    last: Option<ProbeResult>,
}

#[near(serializers = [json])]
pub struct Config {
    pub owner: AccountId,
    pub outlayer: AccountId,
    pub source: Source,
    pub limits: Limits,
    pub next_probe: u64,
}

/// Builds the `request_execution` arguments. Pure, so it is unit tested.
pub fn request_args(source: &Source, limits: &Limits, input: &str, payer: &AccountId) -> Value {
    json!({
        "source": source,
        "resource_limits": limits,
        "input_data": input,
        "response_format": "Json",
        "payer_account_id": payer,
    })
}

/// Turns the callback's promise result into (ok, output, error). Pure.
pub fn classify(result: Option<&[u8]>) -> (bool, Option<String>, Option<String>) {
    let bytes = match result {
        None => {
            return (
                false,
                None,
                Some("promise failed (OutLayer call failed or timed out)".into()),
            )
        }
        Some(b) => b,
    };
    if bytes.is_empty() {
        return (false, None, Some("empty return value".into()));
    }
    let v: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => {
            let text = String::from_utf8_lossy(bytes).into_owned();
            return (
                false,
                Some(cut(text)),
                Some("return value is not JSON".into()),
            );
        }
    };
    if v.is_null() {
        return (
            false,
            None,
            Some("null output: the run failed or printed nothing".into()),
        );
    }
    // Older wrapper shape: {"success": bool, "result": ..., "error": ...}.
    if let (Some(success), true) = (
        v.get("success").and_then(Value::as_bool),
        v.get("ok").is_none(),
    ) {
        let err = v.get("error").and_then(Value::as_str).map(str::to_owned);
        let out = v
            .get("result")
            .or_else(|| v.get("output"))
            .map(|r| cut(r.to_string()));
        return (
            success,
            out,
            if success {
                None
            } else {
                err.or(Some("unknown error".into()))
            },
        );
    }
    (true, Some(cut(v.to_string())), None)
}

fn cut(mut s: String) -> String {
    if s.len() > MAX_STORED {
        let mut end = MAX_STORED;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

#[near]
impl TeeProbe {
    #[init]
    pub fn new(
        owner: AccountId,
        outlayer: AccountId,
        source: Source,
        limits: Option<Limits>,
    ) -> Self {
        Self {
            owner,
            outlayer,
            source,
            limits: limits.unwrap_or_default(),
            next_probe: 0,
            last: None,
        }
    }

    /// Owner only: switch between WasmUrl and Project, or change limits.
    pub fn configure(&mut self, source: Option<Source>, limits: Option<Limits>) {
        require!(env::predecessor_account_id() == self.owner, "owner only");
        if let Some(s) = source {
            self.source = s;
        }
        if let Some(l) = limits {
            self.limits = l;
        }
    }

    /// Requests one OutLayer execution. Attach at least
    /// `estimate_execution_cost` on the OutLayer contract.
    #[payable]
    pub fn probe(&mut self, input: String) -> Promise {
        require!(input.len() <= MAX_INPUT, "input too long");
        let deposit = env::attached_deposit();
        require!(
            deposit > NearToken::from_yoctonear(0),
            "attach the OutLayer fee"
        );
        let caller = env::predecessor_account_id();
        let probe_id = self.next_probe;
        self.next_probe += 1;
        let args = request_args(&self.source, &self.limits, &input, &caller);
        env::log_str(&format!(
            "tee_probe request probe_id={probe_id} caller={caller} deposit={}",
            deposit.as_yoctonear()
        ));
        Promise::new(self.outlayer.clone())
            .function_call_weight(
                "request_execution".to_string(),
                serde_json::to_vec(&args).unwrap_or_default(),
                deposit,
                REQUEST_GAS,
                GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_result(probe_id, caller),
            )
    }

    #[private]
    pub fn on_result(&mut self, probe_id: u64, caller: AccountId) -> bool {
        let (ok, output, error) = match env::promise_result_checked(0, MAX_RETURN) {
            Ok(b) => classify(Some(&b)),
            Err(PromiseError::TooLong(n)) => (
                false,
                None,
                Some(format!("return value too long ({n} bytes)")),
            ),
            Err(_) => classify(None),
        };
        env::log_str(&format!(
            "tee_probe result probe_id={probe_id} ok={ok} output={} error={}",
            output.as_deref().unwrap_or("none"),
            error.as_deref().unwrap_or("none")
        ));
        self.last = Some(ProbeResult {
            probe_id,
            caller,
            ok,
            output,
            error,
            block_height: env::block_height(),
        });
        ok
    }

    pub fn last_result(&self) -> Option<ProbeResult> {
        self.last.clone()
    }

    pub fn get_config(&self) -> Config {
        Config {
            owner: self.owner.clone(),
            outlayer: self.outlayer.clone(),
            source: self.source.clone(),
            limits: self.limits.clone(),
            next_probe: self.next_probe,
        }
    }

    /// Contract NEAR balance, to check that nothing is left behind.
    pub fn balance(&self) -> U128 {
        U128(env::account_balance().as_yoctonear())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{get_logs, VMContextBuilder};
    use near_sdk::{test_vm_config, PromiseResult};
    use near_sdk::{testing_env, RuntimeFeesConfig};

    fn acc(s: &str) -> AccountId {
        s.parse().unwrap()
    }

    fn wasm_url() -> Source {
        Source::WasmUrl {
            url: "https://test.fastfs.io/probe.example.testnet/outlayer.testnet/aa.wasm".into(),
            hash: "a".repeat(64),
            build_target: Some("wasm32-wasip1".into()),
        }
    }

    fn setup(pred: &str, deposit: u128) -> TeeProbe {
        let ctx = VMContextBuilder::new()
            .current_account_id(acc("probe.example.testnet"))
            .predecessor_account_id(acc(pred))
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .build();
        testing_env!(ctx);
        TeeProbe::new(
            acc("owner.testnet"),
            acc("outlayer.testnet"),
            wasm_url(),
            None,
        )
    }

    #[test]
    fn wasm_url_args_match_outlayer_shape() {
        let v = request_args(
            &wasm_url(),
            &Limits::default(),
            "{\"a\":1}",
            &acc("alice.testnet"),
        );
        assert_eq!(
            v,
            json!({
                "source": {"WasmUrl": {
                    "url": "https://test.fastfs.io/probe.example.testnet/outlayer.testnet/aa.wasm",
                    "hash": "a".repeat(64),
                    "build_target": "wasm32-wasip1"
                }},
                "resource_limits": {"max_instructions": 100_000_000u64, "max_memory_mb": 64, "max_execution_seconds": 10},
                "input_data": "{\"a\":1}",
                "response_format": "Json",
                "payer_account_id": "alice.testnet"
            })
        );
    }

    #[test]
    fn project_args_match_outlayer_shape() {
        let src = Source::Project {
            project_id: "p.testnet/echo".into(),
            version_key: None,
        };
        let v = request_args(&src, &Limits::default(), "", &acc("bob.testnet"));
        assert_eq!(
            v["source"],
            json!({"Project": {"project_id": "p.testnet/echo", "version_key": null}})
        );
        assert_eq!(v["payer_account_id"], json!("bob.testnet"));
    }

    #[test]
    fn classify_json_object_is_ok() {
        let (ok, out, err) = classify(Some(br#"{"ok":true,"echo":"x","len":1}"#));
        assert!(ok);
        assert_eq!(out.unwrap(), r#"{"ok":true,"echo":"x","len":1}"#);
        assert!(err.is_none());
    }

    #[test]
    fn classify_null_is_failure() {
        let (ok, out, err) = classify(Some(b"null"));
        assert!(!ok && out.is_none());
        assert!(err.unwrap().contains("null output"));
    }

    #[test]
    fn classify_promise_failure() {
        let (ok, _, err) = classify(None);
        assert!(!ok);
        assert!(err.unwrap().contains("promise failed"));
    }

    #[test]
    fn classify_legacy_wrapper() {
        let (ok, _, err) = classify(Some(
            br#"{"success":false,"result":null,"error":"hash mismatch"}"#,
        ));
        assert!(!ok);
        assert_eq!(err.unwrap(), "hash mismatch");
        let (ok, out, _) = classify(Some(br#"{"success":true,"result":{"x":1},"error":null}"#));
        assert!(ok);
        assert_eq!(out.unwrap(), r#"{"x":1}"#);
    }

    #[test]
    fn classify_text_and_garbage() {
        let (ok, out, _) = classify(Some(br#""plain text""#));
        assert!(ok);
        assert_eq!(out.unwrap(), r#""plain text""#);
        let (ok, _, err) = classify(Some(b"\xff\xfe"));
        assert!(!ok);
        assert!(err.unwrap().contains("not JSON"));
    }

    #[test]
    fn output_is_cut() {
        let big = format!("\"{}\"", "x".repeat(5000));
        let (_, out, _) = classify(Some(big.as_bytes()));
        assert_eq!(out.unwrap().len(), MAX_STORED);
    }

    #[test]
    fn probe_needs_deposit() {
        let mut c = setup("alice.testnet", 0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = c.probe("{}".into());
        }));
        assert!(r.is_err());
    }

    #[test]
    fn probe_rejects_long_input() {
        let mut c = setup("alice.testnet", 1);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = c.probe("x".repeat(MAX_INPUT + 1));
        }));
        assert!(r.is_err());
    }

    #[test]
    fn probe_counts_and_logs() {
        let mut c = setup("alice.testnet", 10u128.pow(22));
        let _ = c.probe("{\"hello\":\"tee\"}".into());
        assert_eq!(c.get_config().next_probe, 1);
        assert!(get_logs()[0].starts_with("tee_probe request probe_id=0 caller=alice.testnet"));
    }

    #[test]
    fn configure_is_owner_only() {
        let mut c = setup("mallory.testnet", 0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.configure(None, Some(Limits::default()));
        }));
        assert!(r.is_err());
        let mut c = setup("owner.testnet", 0);
        let p = Source::Project {
            project_id: "p.testnet/echo".into(),
            version_key: None,
        };
        c.configure(Some(p.clone()), None);
        assert_eq!(c.get_config().source, p);
    }

    fn callback_with(result: PromiseResult) -> TeeProbe {
        let mut c = setup("alice.testnet", 0);
        let ctx = VMContextBuilder::new()
            .current_account_id(acc("probe.example.testnet"))
            .predecessor_account_id(acc("probe.example.testnet"))
            .build();
        testing_env!(
            ctx,
            test_vm_config(),
            RuntimeFeesConfig::test(),
            Default::default(),
            vec![result]
        );
        let ok = c.on_result(7, acc("alice.testnet"));
        assert_eq!(ok, c.last_result().unwrap().ok);
        c
    }

    #[test]
    fn callback_stores_success() {
        let c = callback_with(PromiseResult::Successful(
            br#"{"ok":true,"echo":"hi","len":2}"#.to_vec(),
        ));
        let r = c.last_result().unwrap();
        assert!(r.ok);
        assert_eq!(r.probe_id, 7);
        assert_eq!(r.caller, acc("alice.testnet"));
        assert!(get_logs()
            .iter()
            .any(|l| l.contains("tee_probe result probe_id=7 ok=true")));
    }

    #[test]
    fn callback_stores_failure() {
        let c = callback_with(PromiseResult::Failed);
        let r = c.last_result().unwrap();
        assert!(!r.ok);
        assert!(r.error.unwrap().contains("promise failed"));
    }
}
