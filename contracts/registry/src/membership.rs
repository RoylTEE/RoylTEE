//! Membership credentials (minimal block 2 version). Registrar only.
//! A credential is non-transferable. KYB behind it is SIMULATED in the demo.

use crate::*;
use near_sdk::assert_one_yocto;

#[near]
impl Registry {
    /// Grants an active credential and registers the member with the token
    /// contract, so claims to it can land (T17). Attach the registry storage
    /// cost plus `FT_STORAGE_DEPOSIT`; the rest is refunded.
    #[payable]
    pub fn grant_credential(
        &mut self,
        account: AccountId,
        org_name_hash: Hash32,
        kyb_ref_hash: Hash32,
        doc_hash: Hash32,
    ) -> Promise {
        self.assert_registrar();
        require!(
            logic::is_hash32(&org_name_hash)
                && logic::is_hash32(&kyb_ref_hash)
                && logic::is_hash32(&doc_hash),
            "hashes must be 64 lowercase hex characters"
        );
        require!(
            !self.is_system_account(&account),
            "system accounts cannot hold a credential"
        );
        // The registrar never holds, receives or claims a share (review H3).
        require!(
            account != self.registrar,
            "the registrar cannot hold a credential"
        );
        require!(
            self.roles.get(&account).is_none(),
            "agent accounts cannot hold a credential"
        );
        require!(
            self.credentials.get(&account).is_none(),
            "credential already exists"
        );
        let initial = env::storage_usage();
        let now = U64(env::block_timestamp());
        self.credentials.insert(
            account.clone(),
            Credential {
                org_name_hash: org_name_hash.clone(),
                kyb_ref_hash: kyb_ref_hash.clone(),
                status: CredentialStatus::Active,
                granted_at: now,
                updated_at: now,
            },
        );
        self.credentials.flush();
        charge_storage(initial, 0, FT_STORAGE_DEPOSIT);
        emit(
            "credential_granted",
            json!({
                "account": account,
                "org_name_hash": org_name_hash,
                "kyb_ref_hash": kyb_ref_hash,
                "doc_hash": doc_hash,
            }),
        );
        Promise::new(self.usdc.clone())
            .function_call(
                "storage_deposit".to_string(),
                json_bytes(json!({ "account_id": account, "registration_only": true })),
                FT_STORAGE_DEPOSIT,
                GAS_STORAGE_DEPOSIT,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_MEMBER_CB)
                    .on_member_registered(account),
            )
    }

    /// Removes the credential again if the token registration failed.
    #[private]
    pub fn on_member_registered(&mut self, account: AccountId) -> bool {
        if callback_result(512).is_some() {
            return true;
        }
        self.credentials.remove(&account);
        emit(
            "credential_grant_failed",
            json!({ "account": account, "reason": "token registration failed" }),
        );
        false
    }

    /// Suspends the credential (decision 3). Shares stay put and keep
    /// accruing, but a suspended holder cannot claim, transfer out or
    /// receive. Only the registrar can move a suspended holder's share, with
    /// `registrar_transfer` and a document hash.
    #[payable]
    pub fn suspend_credential(&mut self, account: AccountId, doc_hash: Hash32) {
        assert_one_yocto();
        self.set_status(
            account,
            doc_hash,
            CredentialStatus::Suspended,
            "credential_suspended",
        );
    }

    #[payable]
    pub fn reinstate_credential(&mut self, account: AccountId, doc_hash: Hash32) {
        assert_one_yocto();
        self.set_status(
            account,
            doc_hash,
            CredentialStatus::Active,
            "credential_reinstated",
        );
    }
}

impl Registry {
    fn set_status(
        &mut self,
        account: AccountId,
        doc_hash: Hash32,
        status: CredentialStatus,
        event: &str,
    ) {
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let c = self
            .credentials
            .get_mut(&account)
            .unwrap_or_else(|| env::panic_str("no credential"));
        require!(c.status != status, "credential already in that status");
        c.status = status;
        c.updated_at = U64(env::block_timestamp());
        emit(event, json!({ "account": account, "doc_hash": doc_hash }));
    }
}
