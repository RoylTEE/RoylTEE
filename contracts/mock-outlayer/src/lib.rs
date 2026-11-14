//! TEST ONLY. Simulated OutLayer, used by the sandbox tests, never deployed.
//!
//! The real `outlayer.testnet` runs a WASI module in a TEE and answers
//! through yield and resume. This mock has the same `request_execution`
//! signature (the fields the auction sends) and answers at once:
//!
//! - if a canned answer was set with `set_answer`, it returns that raw JSON;
//! - otherwise it records the request and returns `null`, the answer
//!   OutLayer gives for a failed run. The test then reads `last_request`,
//!   runs the real WASI module on that exact input, sets the output as the
//!   canned answer and settles again.
//!
//! It keeps a fixed fee and refunds the rest of the deposit to
//! `payer_account_id`, as OutLayer does, so the tests can check refunds.

use near_sdk::serde_json::{self, Value};
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, Promise};

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Recorded {
    pub input_data: String,
    pub payer: AccountId,
    pub caller: AccountId,
    pub signer: AccountId,
    pub deposit: String,
    pub source: String,
    pub secrets_ref: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockOutlayer {
    fee: NearToken,
    answer: Option<String>,
    reject: bool,
    last: Option<Recorded>,
    calls: u64,
}

#[near]
impl MockOutlayer {
    #[init]
    pub fn new() -> Self {
        Self {
            fee: NearToken::from_millinear(1),
            answer: None,
            reject: false,
            last: None,
            calls: 0,
        }
    }

    pub fn set_answer(&mut self, answer: Option<String>) {
        self.answer = answer;
    }

    /// When true, `request_execution` panics (a request rejected up front).
    pub fn set_reject(&mut self, reject: bool) {
        self.reject = reject;
    }

    #[payable]
    #[allow(clippy::too_many_arguments)]
    pub fn request_execution(
        &mut self,
        source: Value,
        resource_limits: Value,
        input_data: Option<String>,
        secrets_ref: Option<Value>,
        response_format: Option<String>,
        payer_account_id: Option<AccountId>,
    ) -> Value {
        require!(!self.reject, "request rejected");
        require!(
            resource_limits.get("max_instructions").is_some(),
            "resource_limits is required"
        );
        require!(
            response_format.as_deref() == Some("Json"),
            "the auction asks for Json"
        );
        let deposit = env::attached_deposit();
        require!(deposit >= self.fee, "deposit below the fee");
        let payer = payer_account_id.unwrap_or_else(env::predecessor_account_id);
        self.calls += 1;
        self.last = Some(Recorded {
            input_data: input_data.unwrap_or_default(),
            payer: payer.clone(),
            caller: env::predecessor_account_id(),
            signer: env::signer_account_id(),
            deposit: deposit.as_yoctonear().to_string(),
            source: source.to_string(),
            secrets_ref: secrets_ref.map(|s| s.to_string()).unwrap_or_default(),
        });
        let refund = deposit.saturating_sub(self.fee);
        if !refund.is_zero() {
            let _ = Promise::new(payer).transfer(refund);
        }
        match &self.answer {
            Some(a) => serde_json::from_str(a).unwrap_or(Value::Null),
            None => Value::Null,
        }
    }

    pub fn last_request(&self) -> Option<Recorded> {
        self.last.clone()
    }

    pub fn calls(&self) -> u64 {
        self.calls
    }
}
