//! TEST ONLY. A credentialed member contract that behaves like an attacker.
//! Used by the sandbox tests, never deployed to testnet.
//!
//! - `nft_on_transfer` with msg "claim": tries to claim the share it just
//!   received, then asks for the token to be returned (bounce attack, T4).
//! - msg "transfer:<account>": tries to transfer the share onward, then asks
//!   for it to be returned.
//! - msg "keep": keeps the token.
//! - `claim_and_transfer`: as a holder, starts a claim and a transfer in the
//!   same block, so the transfer lands while the payout is in flight (T3).

use near_sdk::serde_json::json;
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue};

const GAS_CLAIM: Gas = Gas::from_tgas(60);
const GAS_TRANSFER: Gas = Gas::from_tgas(30);
const GAS_RECORD: Gas = Gas::from_tgas(5);

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub action: String,
    pub succeeded: bool,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Receiver {
    registry: AccountId,
    attempts: Vec<Attempt>,
}

#[near]
impl Receiver {
    #[init]
    pub fn new(registry: AccountId) -> Self {
        Self {
            registry,
            attempts: Vec::new(),
        }
    }

    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: String,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let _ = (sender_id, previous_owner_id);
        assert_eq!(
            env::predecessor_account_id(),
            self.registry,
            "unknown NFT contract"
        );
        let attack = if msg == "claim" {
            self.claim(&token_id)
        } else if let Some(to) = msg.strip_prefix("transfer:") {
            self.transfer(&token_id, to)
        } else {
            return PromiseOrValue::Value(false);
        };
        attack
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RECORD)
                    .record(msg),
            )
            .into()
    }

    /// Starts a claim and a transfer of the same share in one receipt. Both
    /// land at the registry in the next block, claim first.
    pub fn claim_and_transfer(&mut self, token_id: String, receiver_id: AccountId) {
        self.claim(&token_id)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RECORD)
                    .record("claim".into()),
            )
            .detach();
        self.transfer(&token_id, receiver_id.as_str())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_RECORD)
                    .record(format!("transfer:{receiver_id}")),
            )
            .detach();
    }

    /// Plain claim, for use after the in-flight payout has settled.
    pub fn claim_now(&mut self, token_id: String) -> Promise {
        self.claim(&token_id)
    }

    /// Plain transfer, for use after the in-flight payout has settled.
    pub fn transfer_now(&mut self, token_id: String, receiver_id: AccountId) -> Promise {
        self.transfer(&token_id, receiver_id.as_str())
    }

    /// Records whether the attempted action succeeded. Always asks for the
    /// token to be returned when used as the nft_on_transfer result.
    #[private]
    pub fn record(&mut self, action: String) -> bool {
        let succeeded =
            env::promise_results_count() == 1 && env::promise_result_checked(0, 256).is_ok();
        self.attempts.push(Attempt { action, succeeded });
        true
    }

    pub fn get_attempts(&self) -> Vec<Attempt> {
        self.attempts.clone()
    }
}

impl Receiver {
    fn claim(&self, token_id: &str) -> Promise {
        Promise::new(self.registry.clone()).function_call(
            "claim".to_string(),
            json!({ "token_id": token_id }).to_string().into_bytes(),
            NearToken::from_yoctonear(1),
            GAS_CLAIM,
        )
    }

    fn transfer(&self, token_id: &str, to: &str) -> Promise {
        Promise::new(self.registry.clone()).function_call(
            "nft_transfer".to_string(),
            json!({ "receiver_id": to, "token_id": token_id })
                .to_string()
                .into_bytes(),
            NearToken::from_yoctonear(1),
            GAS_TRANSFER,
        )
    }
}
