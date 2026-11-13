//! Mock USDC for testnet and sandbox only. NEP-141 with NEP-145 storage
//! management, 6 decimals. It is NOT Circle USDC and has no value.
//! `mint` is restricted to one minter account fixed at init (the simulated payer).

use near_contract_standards::fungible_token::events::FtMint;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::{
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::U128;
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, PromiseOrValue};

pub const NAME: &str = "Mock USDC (testnet)";
pub const SYMBOL: &str = "mUSDC";
pub const DECIMALS: u8 = 6;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockUsdc {
    token: FungibleToken,
    minter: AccountId,
}

#[near]
impl MockUsdc {
    #[init]
    pub fn new(minter: AccountId) -> Self {
        // Only the token account itself may initialise (audit M1).
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "only the token account itself can initialise"
        );
        Self {
            token: FungibleToken::new(b"t".to_vec()),
            minter,
        }
    }

    /// Mints mock tokens to a registered account. Minter only.
    pub fn mint(&mut self, account_id: AccountId, amount: U128) {
        require!(
            env::predecessor_account_id() == self.minter,
            "only the minter can mint"
        );
        self.token.internal_deposit(&account_id, amount.0);
        FtMint {
            owner_id: &account_id,
            amount,
            memo: Some("mock mint, testnet only"),
        }
        .emit();
    }

    pub fn get_minter(&self) -> AccountId {
        self.minter.clone()
    }
}

#[near]
impl FungibleTokenCore for MockUsdc {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for MockUsdc {
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        let (used, _burned) =
            self.token
                .internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        used.into()
    }
}

#[near]
impl StorageManagement for MockUsdc {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        // Unregistering with a balance would burn mock tokens; keep it simple.
        require!(force != Some(true), "force unregister is disabled");
        self.token.storage_unregister(force)
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenMetadataProvider for MockUsdc {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: NAME.to_string(),
            symbol: SYMBOL.to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: DECIMALS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn setup() -> MockUsdc {
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_millinear(10))
            .build());
        let mut c = MockUsdc::new(accounts(0));
        c.storage_deposit(Some(accounts(1)), Some(true));
        c
    }

    #[test]
    #[should_panic(expected = "only the token account itself can initialise")]
    fn stranger_cannot_initialise() {
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(0))
            .predecessor_account_id(accounts(2))
            .build());
        let _ = MockUsdc::new(accounts(2));
    }

    #[test]
    fn metadata_is_labeled_mock_with_six_decimals() {
        let c = setup();
        let m = c.ft_metadata();
        assert_eq!(m.decimals, 6);
        assert_eq!(m.symbol, "mUSDC");
        assert!(m.name.contains("Mock") && m.name.contains("testnet"));
    }

    #[test]
    fn minter_can_mint() {
        let mut c = setup();
        c.mint(accounts(1), U128(5_000_000));
        assert_eq!(c.ft_balance_of(accounts(1)).0, 5_000_000);
        assert_eq!(c.ft_total_supply().0, 5_000_000);
    }

    #[test]
    #[should_panic(expected = "only the minter can mint")]
    fn others_cannot_mint() {
        let mut c = setup();
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(accounts(1))
            .build());
        c.mint(accounts(1), U128(1));
    }
}
