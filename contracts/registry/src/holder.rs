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
            self.share(&token_id).status == ShareStatus::Active
                && self.share(&token_id).dispute.is_none(),
            "share is not active or is in dispute"
        );
        require!(
            self.is_approved(&token_id),
            "share is not approved by the work registrant"
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

    /// A holder of another share of the same work, right and territory
    /// confirms this share (evidence tier counterparty_confirmed once every
    /// other holder in the group has confirmed). Active member, not the
    /// share's own holder, share active and not in dispute. Needs a deposit
    /// that covers storage (rest refunded), so a function-call key cannot
    /// call it. Confirmations of holders who no longer hold a share in the
    /// group are dropped.
    #[payable]
    pub fn confirm_share(&mut self, token_id: TokenId) {
        require!(
            !env::attached_deposit().is_zero(),
            "attach a deposit of at least 1 yoctoNEAR"
        );
        let caller = self.assert_active_caller();
        let share = self.share(&token_id).clone();
        require!(
            share.status == ShareStatus::Active && share.dispute.is_none(),
            "share is not active or is in dispute"
        );
        require!(
            self.is_approved(&token_id),
            "share is not approved by the work registrant"
        );
        let owner = self.owner_of(&token_id);
        require!(
            owner.as_ref() != Some(&caller),
            "holders cannot confirm their own share"
        );
        let counterparts = self.counterpart_owners(&share);
        require!(
            counterparts.contains(&caller),
            "only holders of other active shares in this group can confirm"
        );
        let initial = env::storage_usage();
        if let Some(s) = self.shares.get_mut(&token_id) {
            s.confirmations.retain(|a| counterparts.contains(a));
            if !s.confirmations.contains(&caller) {
                s.confirmations.push(caller.clone());
            }
        }
        self.shares.flush();
        charge_storage(initial, 0, NearToken::from_yoctonear(0));
        emit(
            "share_confirmed",
            json!({ "token_id": token_id, "by": caller }),
        );
    }

    /// self_asserted, counterparty_confirmed or payer_confirmed.
    pub fn get_evidence_tier(&self, token_id: TokenId) -> String {
        self.evidence_tier(self.share(&token_id)).to_string()
    }
}

impl Registry {
    /// Current owners of the other active shares of the share's group,
    /// excluding the share's own owner.
    fn counterpart_owners(&self, share: &Share) -> Vec<AccountId> {
        let own = self.owner_of(&share.token_id);
        let mut out: Vec<AccountId> = self
            .groups
            .get(&share.group)
            .map(|g| g.shares.clone())
            .unwrap_or_default()
            .iter()
            .filter(|t| **t != share.token_id)
            .filter(|t| {
                self.shares
                    .get(*t)
                    .is_some_and(|s| s.status == ShareStatus::Active)
                    && self.is_approved(t)
            })
            .filter_map(|t| self.owner_of(t))
            .filter(|o| Some(o) != own.as_ref())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn evidence_tier(&self, share: &Share) -> &'static str {
        if share.payer_attestation.is_some() {
            return "payer_confirmed";
        }
        let others = self.counterpart_owners(share);
        if !others.is_empty() && others.iter().all(|o| share.confirmations.contains(o)) {
            "counterparty_confirmed"
        } else {
            "self_asserted"
        }
    }
}
