//! On-chain split sheet (review M2).
//!
//! A share registered by anyone other than the work's registrant stays
//! pending until the registrant approves it, or the registrar as a backup
//! with a document hash. A pending share is minted and has its vault, but
//! it is never paid: its part of every payment is held in dispute-style
//! escrow in the registry (`Ledger::escrowed`), it cannot be transferred,
//! synced, confirmed, attested or contested. Approval releases the held
//! money to its vault. Rejection voids the share and moves the held money
//! to the group's unallocated escrow, which only the registrar releases to
//! a share of the same group (decision 4). No money is lost or stuck.
//!
//! A share that opens an over-allocation dispute is not put through this
//! path: the registrar's `resolve` is its approval.
//!
//! The status lives in its own storage keys (`apv:` + token id), outside
//! every Borsh-serialised struct, so the deployed state needs no migration.
//! A share with no key is approved; that covers every share registered
//! before this rule existed.

use crate::*;
use near_sdk::assert_one_yocto;

const APPROVAL_PREFIX: &[u8] = b"apv:";
const PENDING: u8 = 1;
const REJECTED: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Approval {
    Approved,
    Pending,
    Rejected,
}

impl Approval {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Approval::Approved => "approved",
            Approval::Pending => "pending",
            Approval::Rejected => "rejected",
        }
    }
}

fn approval_key(token_id: &str) -> Vec<u8> {
    [APPROVAL_PREFIX, token_id.as_bytes()].concat()
}

#[near]
impl Registry {
    /// The work registrant (or the registrar, with a document hash) signs
    /// the split sheet for one pending share. Held money goes to its vault.
    #[payable]
    pub fn approve_share(&mut self, token_id: TokenId, doc_hash: Option<Hash32>) {
        assert_one_yocto();
        let by = self.assert_split_sheet_signer(&token_id, doc_hash.as_ref());
        self.require_pending_minted(&token_id);
        let amount = self.take_escrow(&token_id);
        if amount > 0 {
            require!(Self::has_gas_for_forwards(1), "not enough gas to forward");
        }
        self.set_approval(&token_id, Approval::Approved);
        self.forward(&token_id, amount);
        emit(
            "share_approved",
            json!({ "token_id": token_id, "by": by, "doc_hash": doc_hash, "released": U128(amount) }),
        );
    }

    /// The work registrant (or the registrar, with a document hash) refuses
    /// a pending share. The share becomes Void, its bps are freed, and its
    /// held money moves to the group's unallocated escrow.
    #[payable]
    pub fn reject_share(&mut self, token_id: TokenId, doc_hash: Option<Hash32>) {
        assert_one_yocto();
        let by = self.assert_split_sheet_signer(&token_id, doc_hash.as_ref());
        self.require_pending_minted(&token_id);
        let share = self.share(&token_id).clone();
        let amount = self.take_escrow(&token_id);
        if let Some(s) = self.shares.get_mut(&token_id) {
            s.status = ShareStatus::Void;
        }
        if let Some(l) = self.ledgers.get_mut(&token_id) {
            l.epoch += 1;
        }
        if let Some(g) = self.groups.get_mut(&share.group) {
            g.active_bps -= u32::from(share.percent_bps);
            g.unallocated = U128(g.unallocated.0 + amount);
        }
        self.unallocated_total += amount;
        self.prune_from_group(&share.group, &token_id);
        self.set_approval(&token_id, Approval::Rejected);
        emit(
            "share_rejected",
            json!({ "token_id": token_id, "by": by, "doc_hash": doc_hash, "to_unallocated": U128(amount) }),
        );
    }

    /// Sends a plain deposit that a payer sent straight into a rejected
    /// share's vault to `receiver` (for example back to that payer).
    /// Registrar only, 1 yoctoNEAR, document hash. A rejected share was
    /// never paid and never synced, so its ledger knows nothing in the
    /// vault and no ledger value changes.
    #[payable]
    pub fn recover_rejected_vault(
        &mut self,
        token_id: TokenId,
        receiver: AccountId,
        amount: U128,
        doc_hash: Hash32,
    ) -> Promise {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        require!(
            self.approval(&token_id) == Approval::Rejected,
            "only a rejected share's vault can be recovered"
        );
        require!(amount.0 > 0, "amount must be positive");
        let l = self
            .ledgers
            .get(&token_id)
            .unwrap_or_else(|| env::panic_str("unknown share"));
        require!(
            l.known_balance == 0 && l.payout_in_flight.is_none(),
            "ledger is not empty"
        );
        let vault = self.share(&token_id).vault.clone();
        let args =
            near_sdk::borsh::to_vec(&(self.usdc.to_string(), receiver.to_string(), amount.0))
                .unwrap_or_else(|_| env::panic_str("borsh"));
        Promise::new(vault)
            .function_call(
                "withdraw".to_string(),
                args,
                NearToken::from_yoctonear(0),
                GAS_WITHDRAW,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CLAIM_CB)
                    .on_rejected_vault_recovered(token_id, receiver, amount, doc_hash),
            )
    }

    #[private]
    pub fn on_rejected_vault_recovered(
        &mut self,
        token_id: TokenId,
        receiver: AccountId,
        amount: U128,
        doc_hash: Hash32,
    ) -> bool {
        let ok = callback_result(64).is_some();
        emit(
            "rejected_vault_recovered",
            json!({ "token_id": token_id, "receiver": receiver, "amount": amount, "doc_hash": doc_hash, "ok": ok }),
        );
        ok
    }

    /// "approved", "pending" or "rejected".
    pub fn get_share_approval(&self, token_id: TokenId) -> String {
        self.share(&token_id);
        self.approval(&token_id).as_str().to_string()
    }
}

impl Registry {
    pub(crate) fn approval(&self, token_id: &str) -> Approval {
        match env::storage_read(&approval_key(token_id)).as_deref() {
            Some([PENDING]) => Approval::Pending,
            Some([REJECTED]) => Approval::Rejected,
            _ => Approval::Approved,
        }
    }

    pub(crate) fn set_approval(&mut self, token_id: &str, a: Approval) {
        let key = approval_key(token_id);
        match a {
            Approval::Approved => {
                env::storage_remove(&key);
            }
            Approval::Pending => {
                env::storage_write(&key, &[PENDING]);
            }
            Approval::Rejected => {
                env::storage_write(&key, &[REJECTED]);
            }
        }
    }

    pub(crate) fn is_approved(&self, token_id: &str) -> bool {
        self.approval(token_id) == Approval::Approved
    }

    /// Caller must be the work's registrant (active) or the registrar with
    /// a document hash. Returns the caller.
    fn assert_split_sheet_signer(
        &self,
        token_id: &TokenId,
        doc_hash: Option<&Hash32>,
    ) -> AccountId {
        let caller = env::predecessor_account_id();
        if let Some(d) = doc_hash {
            require!(
                logic::is_hash32(d),
                "doc_hash must be 64 lowercase hex characters"
            );
        }
        if caller == self.registrar {
            require!(doc_hash.is_some(), "the registrar must give a doc_hash");
            return caller;
        }
        let work_id = self.share(token_id).group.work_id;
        let registrant = self
            .works
            .get(&work_id)
            .map(|w| w.registered_by.clone())
            .unwrap_or_else(|| env::panic_str("unknown work"));
        require!(
            caller == registrant,
            "only the work registrant or the registrar can approve or reject"
        );
        require!(self.is_active(&caller), "caller has no active credential");
        caller
    }

    fn require_pending_minted(&self, token_id: &TokenId) {
        require!(
            self.approval(token_id) == Approval::Pending,
            "share is not pending approval"
        );
        require!(
            self.share(token_id).status == ShareStatus::Active,
            "share is still minting or has no vault"
        );
    }
}
