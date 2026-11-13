//! Disputes: over-allocation (a share pushed its group above 100 percent)
//! and duplicate codes (the same ISRC or ISWC registered twice).
//!
//! Over-allocation freezes only the contested fraction and only new money
//! (T11): payments for contested shares go to escrow in the registry, other
//! shares keep being paid, frozen shares cannot move, and `sync` is refused
//! on them, so plain transfers straight into a contested vault are held,
//! unattributed and unclaimable, until resolve (T24). Money the ledger
//! already knew stays claimable.
//!
//! Only the registrar resolves, with 1 yoctoNEAR and a document hash.

use crate::*;
use near_sdk::assert_one_yocto;

#[near]
impl Registry {
    /// Registrar decision on an open dispute. The outcome must fit the kind.
    /// Releases escrow with the usual exact split; a failed forward lands
    /// in the share's undelivered balance (retry_forward).
    #[payable]
    pub fn resolve(&mut self, dispute_id: DisputeId, outcome: Outcome, doc_hash: Hash32) {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let dispute = self
            .disputes
            .get(&dispute_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("unknown dispute"));
        require!(dispute.status == DisputeStatus::Open, "dispute is not open");
        let mut released: Vec<Value> = Vec::new();
        match (&dispute.kind, &outcome) {
            (
                DisputeKind::Overallocation {
                    group,
                    claimant,
                    contested,
                },
                Outcome::UpholdExisting,
            ) => {
                self.require_claimant_minted(claimant);
                require!(
                    Self::has_gas_for_forwards(contested.len()),
                    "not enough gas to release escrow"
                );
                for t in contested {
                    self.unfreeze(t);
                    let amount = self.take_escrow(t);
                    self.forward(t, amount);
                    released.push(json!({ "token_id": t, "amount": U128(amount) }));
                }
                let bps = u32::from(self.share(claimant).percent_bps);
                self.unfreeze(claimant);
                self.set_void(claimant, dispute_id);
                if let Some(g) = self.groups.get_mut(group) {
                    g.claimant_bps -= bps;
                    g.open_dispute = None;
                }
            }
            (
                DisputeKind::Overallocation {
                    group,
                    claimant,
                    contested,
                },
                Outcome::UpholdClaimant { new_bps },
            ) => {
                self.require_claimant_minted(claimant);
                let mut members: Vec<TokenId> = contested.clone();
                members.push(claimant.clone());
                members.sort();
                let mut named: Vec<TokenId> = new_bps.iter().map(|e| e.token_id.clone()).collect();
                named.sort();
                named.dedup();
                require!(
                    named == members && new_bps.len() == members.len(),
                    "new_bps must name each contested share and the claimant exactly once"
                );
                let old_total: u32 = contested
                    .iter()
                    .map(|t| u32::from(self.share(t).percent_bps))
                    .sum();
                let new_total: u32 = new_bps.iter().map(|e| u32::from(e.bps)).sum();
                require!(
                    new_total == old_total,
                    "new_bps must sum to the contested shares' current total"
                );
                require!(
                    Self::has_gas_for_forwards(new_bps.len()),
                    "not enough gas to release escrow"
                );
                let pool: u128 = contested.iter().map(|t| self.take_escrow(t)).sum();
                let parts: Vec<logic::SplitPart> = new_bps
                    .iter()
                    .map(|e| (self.share(&e.token_id).number, e.bps))
                    .collect();
                let credits = logic::split_by_total(pool, &parts)
                    .unwrap_or_else(|| env::panic_str("new_bps must not all be zero"));
                let claimant_old = u32::from(self.share(claimant).percent_bps);
                for (e, credit) in new_bps.iter().zip(credits) {
                    self.unfreeze(&e.token_id);
                    if let Some(s) = self.shares.get_mut(&e.token_id) {
                        s.percent_bps = e.bps;
                    }
                    if e.bps == 0 {
                        self.set_void(&e.token_id, dispute_id);
                    }
                    self.forward(&e.token_id, credit);
                    released.push(
                        json!({ "token_id": e.token_id, "amount": U128(credit), "bps": e.bps }),
                    );
                }
                // active_bps is unchanged: the contested total is now shared
                // with the claimant.
                if let Some(g) = self.groups.get_mut(group) {
                    g.claimant_bps -= claimant_old;
                    g.open_dispute = None;
                }
            }
            (DisputeKind::DuplicateCode { duplicate, .. }, Outcome::VoidDuplicate) => {
                self.void_work(*duplicate, dispute_id);
            }
            (DisputeKind::DuplicateCode { duplicate, .. }, Outcome::KeepBoth) => {
                if let Some(w) = self.works.get_mut(duplicate) {
                    w.status = WorkStatus::Active;
                }
            }
            (
                DisputeKind::DuplicateCode {
                    original,
                    duplicate,
                },
                Outcome::SwapPayable,
            ) => {
                let orig = self
                    .works
                    .get(original)
                    .cloned()
                    .unwrap_or_else(|| env::panic_str("unknown work"));
                require!(
                    orig.status == WorkStatus::Active,
                    "the original is no longer the payable entry"
                );
                self.void_work(*original, dispute_id);
                if let Some(w) = self.works.get_mut(duplicate) {
                    w.status = WorkStatus::Active;
                }
                self.code_index
                    .insert(format!("{:?}:{}", orig.code_type, orig.code), *duplicate);
            }
            _ => env::panic_str("outcome does not fit this dispute kind"),
        }
        if let Some(d) = self.disputes.get_mut(&dispute_id) {
            d.status = DisputeStatus::Resolved {
                outcome: outcome.clone(),
                doc_hash: doc_hash.clone(),
                at: U64(env::block_timestamp()),
            };
        }
        emit(
            "dispute_resolved",
            json!({
                "dispute_id": dispute_id,
                "outcome": outcome,
                "doc_hash": doc_hash,
                "released": released,
                "by": env::predecessor_account_id(),
            }),
        );
    }

    pub fn list_disputes(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<Dispute> {
        let from = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(50).min(100);
        (from..self.next_dispute.min(from.saturating_add(limit)))
            .filter_map(|id| self.disputes.get(&id).cloned())
            .collect()
    }
}

impl Registry {
    /// Opens an over-allocation dispute at registration: the claimant and
    /// the contested shares are frozen from now on.
    pub(crate) fn open_overallocation(
        &mut self,
        opened_by: &AccountId,
        group: &GroupKey,
        claimant: &TokenId,
        contested: Vec<TokenId>,
    ) {
        let id = self.next_dispute;
        self.next_dispute += 1;
        for t in contested.iter().chain(std::iter::once(claimant)) {
            if let Some(s) = self.shares.get_mut(t) {
                s.dispute = Some(id);
            }
            // A sync read that started before the freeze must not land
            // after it (review L4, Security Suite F-04).
            if let Some(l) = self.ledgers.get_mut(t) {
                l.epoch += 1;
            }
        }
        for t in &contested {
            let snapshot = self.ledgers.get(t).map_or(0, |l| l.known_balance);
            emit(
                "dispute_frozen",
                json!({ "dispute_id": id, "token_id": t, "claimable_snapshot": U128(snapshot) }),
            );
        }
        if let Some(g) = self.groups.get_mut(group) {
            g.open_dispute = Some(id);
        }
        emit(
            "dispute_opened",
            json!({
                "dispute_id": id,
                "kind": "overallocation",
                "group": group,
                "claimant": claimant,
                "contested": contested,
                "opened_by": opened_by,
            }),
        );
        self.disputes.insert(
            id,
            Dispute {
                id,
                kind: DisputeKind::Overallocation {
                    group: group.clone(),
                    claimant: claimant.clone(),
                    contested,
                },
                opened_by: opened_by.clone(),
                opened_at: U64(env::block_timestamp()),
                status: DisputeStatus::Open,
            },
        );
    }

    /// The claimant's mint failed: close the dispute, unfreeze, and move
    /// the escrow to each share's undelivered balance (a mint callback has
    /// no gas for forwards; anyone can call retry_forward).
    pub(crate) fn cancel_dispute(&mut self, dispute_id: DisputeId) {
        let Some(d) = self.disputes.get(&dispute_id).cloned() else {
            return;
        };
        let DisputeKind::Overallocation {
            group, contested, ..
        } = d.kind
        else {
            return;
        };
        for t in &contested {
            self.unfreeze(t);
            let amount = self.take_escrow(t);
            if let Some(l) = self.ledgers.get_mut(t) {
                l.undelivered += amount;
            }
            self.undelivered_total += amount;
        }
        if let Some(g) = self.groups.get_mut(&group) {
            g.open_dispute = None;
        }
        if let Some(d) = self.disputes.get_mut(&dispute_id) {
            d.status = DisputeStatus::Cancelled;
        }
        emit(
            "dispute_cancelled",
            json!({ "dispute_id": dispute_id, "reason": "claimant mint failed" }),
        );
    }

    /// Adds a contested share's credit to dispute escrow.
    pub(crate) fn escrow(&mut self, token_id: &TokenId, amount: u128) {
        if let Some(l) = self.ledgers.get_mut(token_id) {
            l.escrowed += amount;
        }
        self.escrow_total += amount;
    }

    fn take_escrow(&mut self, token_id: &TokenId) -> u128 {
        let amount = self
            .ledgers
            .get_mut(token_id)
            .map_or(0, |l| std::mem::take(&mut l.escrowed));
        self.escrow_total -= amount;
        amount
    }

    fn unfreeze(&mut self, token_id: &TokenId) {
        if let Some(s) = self.shares.get_mut(token_id) {
            s.dispute = None;
        }
        if let Some(l) = self.ledgers.get_mut(token_id) {
            l.epoch += 1;
        }
    }

    fn require_claimant_minted(&self, claimant: &TokenId) {
        require!(
            self.share(claimant).status == ShareStatus::Active,
            "claimant share is still minting"
        );
    }

    fn set_void(&mut self, token_id: &TokenId, dispute_id: DisputeId) {
        if let Some(s) = self.shares.get_mut(token_id) {
            s.status = ShareStatus::Void;
        }
        emit(
            "share_voided",
            json!({ "token_id": token_id, "dispute_id": dispute_id }),
        );
    }

    /// Voids a work and every active share in its groups.
    fn void_work(&mut self, work_id: WorkId, dispute_id: DisputeId) {
        let work = self
            .works
            .get(&work_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("unknown work"));
        for key in &work.groups {
            let group = self.groups.get(key).cloned().unwrap_or_default();
            require!(
                group.open_dispute.is_none(),
                "resolve the open share dispute in this work first"
            );
            require!(
                group.reserved_bps == 0,
                "shares of this work are still minting"
            );
            for t in &group.shares {
                if self.share(t).status == ShareStatus::Active {
                    self.set_void(t, dispute_id);
                }
            }
            if let Some(g) = self.groups.get_mut(key) {
                g.active_bps = 0;
            }
        }
        if let Some(w) = self.works.get_mut(&work_id) {
            w.status = WorkStatus::Void { dispute_id };
        }
        emit(
            "work_voided",
            json!({ "work_id": work_id, "dispute_id": dispute_id }),
        );
    }
}
