//! Holder note (a holder power) and the payer role (evidence tier).

use crate::*;
use near_sdk::assert_one_yocto;

#[near]
impl Registry {
    /// Holder power: set a short note on the share (T32). Holder only, not
    /// while suspended or while a transfer is pending. Needs a deposit (so a
    /// function-call key cannot call it) that covers any added storage; the
    /// rest is refunded. An empty note clears it. The note is never read by
    /// payout logic: it cannot redirect money (T8).
    #[payable]
    pub fn set_share_note(&mut self, token_id: TokenId, note: String) {
        require!(
            !env::attached_deposit().is_zero(),
            "attach a deposit of at least 1 yoctoNEAR"
        );
        let caller = env::predecessor_account_id();
        require!(
            self.owner_of(&token_id).as_ref() == Some(&caller),
            "only the current holder can set the note"
        );
        require!(
            !self.is_suspended(&caller),
            "holder credential is suspended"
        );
        require!(note.len() <= MAX_NOTE_LEN, "note longer than 280 bytes");
        require!(
            self.ledgers
                .get(&token_id)
                .is_some_and(|l| l.transfer_pending.is_none()),
            "transfer pending"
        );
        let initial = env::storage_usage();
        let note_hash = hex(&env::sha256(note.as_bytes()));
        if let Some(s) = self.shares.get_mut(&token_id) {
            s.note = (!note.is_empty()).then_some(note);
        }
        self.shares.flush();
        charge_storage(initial, 0, NearToken::from_yoctonear(0));
        emit(
            "note_set",
            json!({ "token_id": token_id, "by": caller, "note_hash": note_hash }),
        );
    }

    #[payable]
    pub fn grant_payer_role(&mut self, account: AccountId, doc_hash: Hash32) {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        require!(
            !self.is_system_account(&account),
            "system accounts cannot be payers"
        );
        require!(
            self.roles.get(&account).is_none(),
            "agent accounts cannot be payers"
        );
        require!(self.payers.insert(account.clone()), "already a payer");
        emit(
            "payer_role_granted",
            json!({ "account": account, "doc_hash": doc_hash }),
        );
    }

    #[payable]
    pub fn revoke_payer_role(&mut self, account: AccountId, doc_hash: Hash32) {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        require!(self.payers.remove(&account), "not a payer");
        emit(
            "payer_role_revoked",
            json!({ "account": account, "doc_hash": doc_hash }),
        );
    }

    /// A payer attests that it pays this share (evidence tier
    /// payer_confirmed). Payer role only; the deposit covers storage.
    #[payable]
    pub fn attest_share(&mut self, token_id: TokenId, statement_hash: Hash32) {
        require!(
            !env::attached_deposit().is_zero(),
            "attach a deposit of at least 1 yoctoNEAR"
        );
        let payer = env::predecessor_account_id();
        require!(self.payers.contains(&payer), "payer role only");
        require!(
            logic::is_hash32(&statement_hash),
            "statement_hash must be 64 lowercase hex characters"
        );
        require!(
            self.share(&token_id).status == ShareStatus::Active,
            "share is not active"
        );
        let initial = env::storage_usage();
        if let Some(s) = self.shares.get_mut(&token_id) {
            s.payer_attestation = Some(Attestation {
                payer: payer.clone(),
                statement_hash: statement_hash.clone(),
                at: U64(env::block_timestamp()),
            });
        }
        self.shares.flush();
        charge_storage(initial, 0, NearToken::from_yoctonear(0));
        emit(
            "share_attested",
            json!({ "token_id": token_id, "by": payer, "statement_hash": statement_hash }),
        );
    }

    pub fn is_payer(&self, account_id: AccountId) -> bool {
        self.payers.contains(&account_id)
    }
}
