//! Money in (ft_on_transfer, plain transfers + sync) and money out (claim).

use crate::*;
use near_sdk::{assert_one_yocto, PromiseOrValue};

#[near]
impl Registry {
    /// Pays the whole known vault balance to the current holder.
    ///
    /// Needs 1 yoctoNEAR, so a function-call key cannot call it (T6). Reads
    /// the holder at call time (T2). Sets `payout_in_flight` and debits the
    /// ledger BEFORE calling the vault (T3); `on_claim` rolls the debit back
    /// only if the transfer failed.
    #[payable]
    pub fn claim(&mut self, token_id: TokenId) -> Promise {
        assert_one_yocto();
        require!(
            env::prepaid_gas().as_gas()
                >= GAS_WITHDRAW.as_gas() + GAS_CLAIM_CB.as_gas() + Gas::from_tgas(10).as_gas(),
            "not enough gas attached for claim"
        );
        let caller = env::predecessor_account_id();
        let owner = self
            .owner_of(&token_id)
            .unwrap_or_else(|| env::panic_str("token not found"));
        require!(caller == owner, "only the current holder can claim");
        require!(!self.is_suspended(&owner), "holder credential is suspended");
        let vault = self.share(&token_id).vault.clone();
        let usdc = self.usdc.clone();
        let ledger = self.ledger_mut(&token_id);
        require!(
            ledger.transfer_pending.is_none(),
            "transfer pending: claim after it resolves"
        );
        require!(
            ledger.payout_in_flight.is_none(),
            "payout already in flight"
        );
        let amount = ledger.known_balance;
        require!(amount > 0, "nothing to claim");
        ledger.known_balance -= amount;
        ledger.total_paid_out += amount;
        ledger.epoch += 1;
        ledger.payout_in_flight = Some(Self::lock_here(owner.clone(), amount));
        emit(
            "claim_started",
            json!({ "token_id": token_id, "owner": owner, "amount": U128(amount) }),
        );
        let args = near_sdk::borsh::to_vec(&(usdc.to_string(), owner.to_string(), amount))
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
                    .on_claim(token_id, owner, U128(amount), U64(env::block_height())),
            )
    }

    /// Settles a claim. If `payout_in_flight` is still this claim's lock
    /// (same `since_block` and amount), clears it and, if the vault's
    /// transfer failed, restores the debit. If the lock was already cleared
    /// by `release_stale_lock`, it changes no balance (T36): a late failure
    /// means the money is still in the vault and `sync` finds it again; a
    /// rollback here could count it twice after such a sync. Never panics.
    /// Returns true if the holder was paid.
    #[private]
    pub fn on_claim(
        &mut self,
        token_id: TokenId,
        receiver: AccountId,
        amount: U128,
        lock_block: U64,
    ) -> bool {
        let ok = callback_result(64).is_some();
        let mut late = false;
        if let Some(l) = self.ledgers.get_mut(&token_id) {
            let mine = l
                .payout_in_flight
                .as_ref()
                .is_some_and(|k| k.since_block == lock_block && k.amount == amount);
            l.epoch += 1;
            if mine {
                l.payout_in_flight = None;
                if !ok {
                    l.known_balance += amount.0;
                    l.total_paid_out = l.total_paid_out.saturating_sub(amount.0);
                }
            } else {
                late = true;
            }
        }
        if late {
            emit(
                "anomaly",
                json!({ "token_id": token_id, "kind": "late_claim_callback", "ok": ok, "amount": amount, "detail": "sync to reconcile the vault balance" }),
            );
        }
        emit(
            "claim_settled",
            json!({ "token_id": token_id, "owner": receiver, "amount": amount, "ok": ok }),
        );
        ok
    }

    /// Re-sends a share's undelivered amount (T26). Anyone may call it. The
    /// amount is debited before the forward; a second failure puts it back.
    pub fn retry_forward(&mut self, token_id: TokenId) {
        require!(
            matches!(
                self.share(&token_id).status,
                ShareStatus::Active | ShareStatus::Void
            ),
            "share has no vault"
        );
        require!(Self::has_gas_for_forwards(1), "not enough gas to forward");
        require!(
            !forwards::watch_older_than(&token_id, STALE_FORWARD_BLOCKS),
            "a stale forward is stuck on this share: release it first"
        );
        let l = self.ledger_mut(&token_id);
        let amount = l.undelivered;
        require!(amount > 0, "nothing undelivered");
        l.undelivered = 0;
        self.undelivered_total -= amount;
        self.forward(&token_id, amount);
        emit(
            "forward_retried",
            json!({ "token_id": token_id, "amount": U128(amount) }),
        );
    }

    /// Clears a lock older than `STALE_LOCK_BLOCKS` (T19). Anyone may call
    /// it.
    ///
    /// A transfer lock is cleared at once and the current owner keeps the
    /// token.
    ///
    /// A payout lock is never cleared blindly (audit F3, A-02, A-03). When
    /// the share has no pending forward and no quarantined amount, the
    /// vault's balance is read and `on_release_payout` clears the lock only
    /// for an exact match: balance equal to the ledger's known balance means
    /// the withdraw happened (the debit stays); balance equal to known balance
    /// plus the locked amount means it did not (the debit is undone). Any
    /// other balance cannot be attributed, because a balance does not show
    /// where its units came from, so the lock stays. The same holds while a
    /// forward is pending or quarantined on the share. A lock that stays is
    /// cleared by the real `on_claim` callback if it still arrives, or by
    /// the registrar with `resolve_stale_payout`.
    ///
    /// If only a payout lock is stale and it cannot be read, the call is
    /// refused. If a stale transfer lock is released in the same call, the
    /// payout lock is simply left as it is.
    pub fn release_stale_lock(&mut self, token_id: TokenId) -> PromiseOrValue<()> {
        let now = env::block_height();
        let stale = |k: &Option<Lock>| {
            k.as_ref()
                .is_some_and(|k| now >= k.since_block.0.saturating_add(STALE_LOCK_BLOCKS))
        };
        let vault = self.share(&token_id).vault.clone();
        let usdc = self.usdc.clone();
        let quarantined = forwards::written_off(&token_id) > 0;
        let l = self.ledger_mut(&token_id);
        let payout_stale = stale(&l.payout_in_flight);
        let transfer_stale = stale(&l.transfer_pending);
        require!(payout_stale || transfer_stale, "no stale lock");
        let read_balance = payout_stale && l.inbound_pending == 0 && !quarantined;
        require!(
            !payout_stale || read_balance || transfer_stale,
            "payout lock cannot be read while a forward is pending or quarantined: the registrar resolves it"
        );
        if read_balance {
            require!(
                env::prepaid_gas().as_gas()
                    >= GAS_FT_BALANCE.as_gas()
                        + GAS_RELEASE_PAYOUT_CB.as_gas()
                        + Gas::from_tgas(5).as_gas(),
                "not enough gas attached"
            );
        }
        let released = transfer_stale.then(|| l.transfer_pending.take());
        l.epoch += 1;
        let epoch = l.epoch;
        let lock = l.payout_in_flight.clone();
        if let Some(lock) = released {
            emit(
                "lock_released_stale",
                json!({ "token_id": token_id, "lock": "transfer_pending", "detail": lock }),
            );
        }
        match lock.filter(|_| read_balance) {
            Some(lock) => PromiseOrValue::Promise(
                Promise::new(usdc)
                    .function_call(
                        "ft_balance_of".to_string(),
                        json_bytes(json!({ "account_id": vault })),
                        NearToken::from_yoctonear(0),
                        GAS_FT_BALANCE,
                    )
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_RELEASE_PAYOUT_CB)
                            .on_release_payout(token_id, U64(epoch), lock.since_block, lock.amount),
                    ),
            ),
            None => PromiseOrValue::Value(()),
        }
    }

    /// Settles the payout part of `release_stale_lock`. With the lock still
    /// the same, the ledger unchanged since the read began and no forward
    /// pending or quarantined:
    /// - vault balance equals the ledger's known balance: the withdraw
    ///   happened, the debit is right, the lock is cleared;
    /// - vault balance is exactly known balance plus the locked amount: the
    ///   withdraw did not happen, so the debit is undone (known balance back,
    ///   paid out back), which keeps `earned` and `total_paid_out` true;
    /// - anything else: the lock stays and `payout_uncertain` is emitted. No
    ///   history is published from an ambiguous balance.
    /// A failed read or a changed ledger leaves the lock too
    /// (`release_discarded`). Never panics. Returns true if the lock was
    /// cleared.
    #[private]
    pub fn on_release_payout(
        &mut self,
        token_id: TokenId,
        epoch: U64,
        lock_block: U64,
        amount: U128,
    ) -> bool {
        let observed = callback_result(64).and_then(|v| serde_json::from_slice::<U128>(&v).ok());
        let quarantined = forwards::written_off(&token_id) > 0;
        let Some(l) = self.ledgers.get_mut(&token_id) else {
            return false;
        };
        let same_lock = l
            .payout_in_flight
            .as_ref()
            .is_some_and(|k| k.since_block == lock_block && k.amount == amount);
        let discard_reason = if observed.is_none() {
            Some("read failed")
        } else if !same_lock
            || l.epoch != epoch.0
            || l.transfer_pending.is_some()
            || l.inbound_pending > 0
            || quarantined
        {
            Some("ledger changed during the read")
        } else {
            None
        };
        if let Some(reason) = discard_reason {
            emit(
                "release_discarded",
                json!({ "token_id": token_id, "reason": reason }),
            );
            return false;
        }
        let observed = observed.map_or(0, |o| o.0);
        let outcome = if observed == l.known_balance {
            "withdraw_happened"
        } else if amount.0 > 0 && observed == l.known_balance.saturating_add(amount.0) {
            "withdraw_failed_debit_undone"
        } else {
            emit(
                "payout_uncertain",
                json!({ "token_id": token_id, "amount": amount, "observed": U128(observed), "known_balance": U128(l.known_balance), "detail": "balance matches neither outcome; the real callback or the registrar settles it" }),
            );
            return false;
        };
        let lock = l.payout_in_flight.take();
        l.epoch += 1;
        if outcome == "withdraw_failed_debit_undone" {
            l.known_balance += amount.0;
            l.total_paid_out = l.total_paid_out.saturating_sub(amount.0);
        }
        emit(
            "lock_released_stale",
            json!({ "token_id": token_id, "lock": "payout_in_flight", "detail": lock, "outcome": outcome }),
        );
        true
    }

    /// Registrar only. Settles a stale payout lock that no callback and no
    /// exact balance could settle. `paid` is whether the vault withdraw
    /// reached the holder, read from the public receipt outcome off chain.
    /// `true`: the debit stays and the lock is cleared. `false`: the debit is
    /// undone (known balance and paid out go back). Only for a lock older than
    /// `STALE_LOCK_BLOCKS`. A wrong answer is a registrar error, see
    /// THREAT_MODEL T12 and T19. A later `sync` then reads the vault balance.
    #[payable]
    pub fn resolve_stale_payout(&mut self, token_id: TokenId, paid: bool, doc_hash: Hash32) {
        assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let now = env::block_height();
        let l = self.ledger_mut(&token_id);
        let lock = l
            .payout_in_flight
            .as_ref()
            .filter(|k| now >= k.since_block.0.saturating_add(STALE_LOCK_BLOCKS))
            .cloned();
        let Some(lock) = lock else {
            env::panic_str("no stale payout lock");
        };
        l.payout_in_flight = None;
        l.epoch += 1;
        if !paid {
            l.known_balance += lock.amount.0;
            l.total_paid_out = l.total_paid_out.saturating_sub(lock.amount.0);
        }
        emit(
            "payout_resolved",
            json!({ "token_id": token_id, "paid": paid, "amount": lock.amount, "doc_hash": doc_hash }),
        );
    }

    /// Reads the vault's real token balance and records it, so plain
    /// `ft_transfer` payments straight into a vault count as earned.
    /// Only the current holder or the registrar may call it, so nobody can
    /// time a read against someone else's vault (review H1). Refused while
    /// any lock or forward is open (T21).
    pub fn sync(&mut self, token_id: TokenId) -> Promise {
        let caller = env::predecessor_account_id();
        require!(
            caller == self.registrar || self.owner_of(&token_id).as_ref() == Some(&caller),
            "only the holder or the registrar can sync"
        );
        let share = self.share(&token_id);
        let vault = share.vault.clone();
        require!(
            matches!(share.status, ShareStatus::Active | ShareStatus::Void),
            "share has no vault"
        );
        // T24: during a dispute plain deposits into the vault stay held and
        // unattributed; the ledger keeps only what it already knew.
        require!(
            share.dispute.is_none(),
            "share is in dispute: sync after resolve"
        );
        // Plain deposits into a share that is not approved stay held
        // (review M2), as during a dispute (T24).
        require!(
            self.is_approved(&token_id),
            "share is not approved by the work registrant"
        );
        let ledger = self.ledger_mut(&token_id);
        require!(ledger.payout_in_flight.is_none(), "payout in flight");
        require!(ledger.transfer_pending.is_none(), "transfer pending");
        require!(ledger.inbound_pending == 0, "payment forward in flight");
        let epoch = ledger.epoch;
        require!(
            forwards::written_off(&token_id) == 0,
            "a quarantined forward is unresolved: wait for its callback or the registrar"
        );
        Promise::new(self.usdc.clone())
            .function_call(
                "ft_balance_of".to_string(),
                json_bytes(json!({ "account_id": vault })),
                NearToken::from_yoctonear(0),
                GAS_FT_BALANCE,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_SYNC_CB)
                    .on_sync(token_id, U64(epoch)),
            )
    }

    /// Discards the reading if the ledger changed since the read began, or
    /// if the share entered a dispute or a transfer_call meanwhile (review
    /// L4). Never panics. Returns the earned total after the sync.
    #[private]
    pub fn on_sync(&mut self, token_id: TokenId, epoch: U64) -> Option<U128> {
        let observed = callback_result(64).and_then(|v| serde_json::from_slice::<U128>(&v).ok());
        let held = self
            .shares
            .get(&token_id)
            .is_none_or(|s| s.dispute.is_some())
            || !self.is_approved(&token_id)
            || forwards::written_off(&token_id) > 0;
        let ledger = self.ledgers.get_mut(&token_id)?;
        let Some(observed) = observed else {
            emit(
                "sync_discarded",
                json!({ "token_id": token_id, "reason": "read failed" }),
            );
            return None;
        };
        if held
            || ledger.epoch != epoch.0
            || ledger.payout_in_flight.is_some()
            || ledger.transfer_pending.is_some()
            || ledger.inbound_pending > 0
        {
            emit(
                "sync_discarded",
                json!({ "token_id": token_id, "reason": "ledger changed" }),
            );
            return None;
        }
        let previous = ledger.known_balance;
        ledger.known_balance = observed.0;
        ledger.epoch += 1;
        let earned = U128(ledger.known_balance + ledger.total_paid_out);
        if observed.0 < previous {
            emit(
                "anomaly",
                json!({ "token_id": token_id, "kind": "vault_below_known", "observed": observed, "previous_known": U128(previous) }),
            );
        }
        emit(
            "sync",
            json!({ "token_id": token_id, "observed": observed, "previous_known": U128(previous), "earned": earned }),
        );
        Some(earned)
    }

    /// Releases unallocated escrow of a work + right + territory to one
    /// active share of that group, for example once the missing writer has
    /// registered (decision 4). Registrar only, 1 yoctoNEAR, with a document
    /// hash. The escrow is debited before the forward; a failed forward
    /// lands in that share's `undelivered`, never lost.
    #[payable]
    pub fn release_unallocated(
        &mut self,
        group: GroupKey,
        token_id: TokenId,
        amount: U128,
        doc_hash: Hash32,
    ) {
        near_sdk::assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        let share = self.share(&token_id);
        require!(share.group == group, "share is not in this group");
        require!(
            share.status == ShareStatus::Active && share.dispute.is_none(),
            "share is not active or is in dispute"
        );
        require!(
            self.is_approved(&token_id),
            "share is not approved by the work registrant"
        );
        require!(amount.0 > 0, "amount must be positive");
        require!(Self::has_gas_for_forwards(1), "not enough gas to forward");
        let g = self
            .groups
            .get_mut(&group)
            .unwrap_or_else(|| env::panic_str("unknown group"));
        require!(
            amount.0 <= g.unallocated.0,
            "amount exceeds unallocated escrow"
        );
        g.unallocated = U128(g.unallocated.0 - amount.0);
        self.unallocated_total -= amount.0;
        self.forward(&token_id, amount.0);
        emit(
            "unallocated_released",
            json!({
                "group": group,
                "token_id": token_id,
                "amount": amount,
                "doc_hash": doc_hash,
                "by": env::predecessor_account_id(),
            }),
        );
    }

    /// Sends unallocated escrow of a voided work back out, for example to
    /// the payer, once a duplicate-code dispute voided the work and no share
    /// of it can ever be paid again (review L9). Registrar only, 1
    /// yoctoNEAR, with a document hash. The escrow is debited before the
    /// transfer and restored if the transfer fails.
    #[payable]
    pub fn refund_unallocated(
        &mut self,
        group: GroupKey,
        receiver: AccountId,
        amount: U128,
        doc_hash: Hash32,
    ) -> Promise {
        near_sdk::assert_one_yocto();
        self.assert_registrar();
        require!(
            logic::is_hash32(&doc_hash),
            "doc_hash must be 64 lowercase hex characters"
        );
        require!(
            matches!(
                self.works.get(&group.work_id).map(|w| &w.status),
                Some(WorkStatus::Void { .. })
            ),
            "work is not void: use release_unallocated"
        );
        require!(amount.0 > 0, "amount must be positive");
        require!(
            receiver != self.registrar,
            "the registrar cannot send escrow to itself"
        );
        require!(Self::has_gas_for_forwards(1), "not enough gas to transfer");
        let g = self
            .groups
            .get_mut(&group)
            .unwrap_or_else(|| env::panic_str("unknown group"));
        require!(
            amount.0 <= g.unallocated.0,
            "amount exceeds unallocated escrow"
        );
        g.unallocated = U128(g.unallocated.0 - amount.0);
        self.unallocated_total -= amount.0;
        self.forwards_in_flight_total += amount.0;
        Promise::new(self.usdc.clone())
            .function_call(
                "ft_transfer".to_string(),
                json_bytes(json!({ "receiver_id": receiver, "amount": amount })),
                NearToken::from_yoctonear(1),
                GAS_FT_TRANSFER,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FORWARD_CB)
                    .on_unallocated_refunded(group, receiver, amount, doc_hash),
            )
    }

    /// Settles a refund_unallocated. On failure the amount returns to the
    /// group's unallocated escrow. Never panics.
    #[private]
    pub fn on_unallocated_refunded(
        &mut self,
        group: GroupKey,
        receiver: AccountId,
        amount: U128,
        doc_hash: Hash32,
    ) -> bool {
        let ok = callback_result(64).is_some();
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(amount.0);
        if !ok {
            if let Some(g) = self.groups.get_mut(&group) {
                g.unallocated = U128(g.unallocated.0 + amount.0);
            }
            self.unallocated_total += amount.0;
        }
        emit(
            "unallocated_refunded",
            json!({ "group": group, "receiver": receiver, "amount": amount, "doc_hash": doc_hash, "ok": ok }),
        );
        ok
    }

    /// NEP-141 receiver. Only the configured token is accepted (T18).
    /// `msg` is `{"pay_work": {...}}` or `{"pay_share": {...}}`. Anything
    /// refused returns the full amount, so the payer is refunded and no
    /// state changes. On success the payment is split by bps and forwarded
    /// to the share vaults (decisions 1, 2, 4).
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        if env::predecessor_account_id() != self.usdc {
            return PromiseOrValue::Value(amount);
        }
        let result = match serde_json::from_str::<PaymentMsg>(&msg) {
            Ok(PaymentMsg::PayWork(p)) => self.pay_work(&sender_id, amount.0, p, None),
            Ok(PaymentMsg::PayShare(p)) => self.pay_share(&sender_id, amount.0, p, None, None),
            Ok(PaymentMsg::FundAllowance(agent)) => {
                self.fund_allowance(&sender_id, amount.0, agent)
            }
            Err(_) => Err("malformed msg"),
        };
        match result {
            Ok(()) => PromiseOrValue::Value(U128(0)),
            Err(reason) => {
                emit(
                    "payment_refused",
                    json!({ "payer": sender_id, "amount": amount, "reason": reason }),
                );
                PromiseOrValue::Value(amount)
            }
        }
    }

    /// Settles one forward. On failure the amount stays in the registry as
    /// `undelivered` (T26). `since` is the block the forward started in;
    /// callbacks that were already in flight before the T19b upgrade carry
    /// none. A forward quarantined by `release_stale_forward` is settled by
    /// `settle_late_forward`: credited if the transfer succeeded, moved to
    /// `undelivered` if not. Panics only on a damaged watch entry (T19b).
    #[private]
    pub fn on_forward(&mut self, token_id: TokenId, amount: U128, since: Option<U64>) -> bool {
        let ok = callback_result(64).is_some();
        if matches!(
            forwards::watch_settle(&token_id, amount.0, since.map(|s| s.0)),
            forwards::Settle::Late
        ) {
            self.settle_late_forward(&token_id, amount.0, ok);
            return ok;
        }
        self.forwards_in_flight_total = self.forwards_in_flight_total.saturating_sub(amount.0);
        if let Some(l) = self.ledgers.get_mut(&token_id) {
            l.inbound_pending = l.inbound_pending.saturating_sub(1);
            l.epoch += 1;
            if ok {
                l.known_balance += amount.0;
                l.total_routed_in += amount.0;
            } else {
                l.undelivered += amount.0;
            }
        }
        if !ok {
            self.undelivered_total += amount.0;
            emit(
                "forward_failed",
                json!({ "token_id": token_id, "amount": amount }),
            );
        }
        ok
    }
}

impl Registry {
    pub(crate) fn check_statement(
        period: &str,
        statement_hash: &str,
        line: Option<&String>,
    ) -> Result<(), &'static str> {
        if period.is_empty() || period.len() > MAX_PERIOD_LEN {
            return Err("period must be 1 to 32 bytes");
        }
        if !logic::is_hash32(statement_hash) {
            return Err("statement_hash must be 64 lowercase hex characters");
        }
        if line.is_some_and(|l| l.is_empty() || l.len() > MAX_LINE_ID_LEN) {
            return Err("statement_line_id must be 1 to 64 bytes");
        }
        Ok(())
    }

    pub(crate) fn has_gas_for_forwards(n: usize) -> bool {
        let need = (GAS_FT_TRANSFER.as_gas() + GAS_FORWARD_CB.as_gas()) * n as u64
            + GAS_FORWARD_BASE.as_gas();
        env::prepaid_gas()
            .as_gas()
            .saturating_sub(env::used_gas().as_gas())
            >= need
    }

    pub(crate) fn pay_work(
        &mut self,
        payer: &AccountId,
        amount: u128,
        p: PayWork,
        agent: Option<&AccountId>,
    ) -> Result<(), &'static str> {
        Self::check_statement(&p.period, &p.statement_hash, p.statement_line_id.as_ref())?;
        if amount < MIN_GROUP_PAYMENT {
            return Err("below minimum group payment");
        }
        let work = self.works.get(&p.work_id).ok_or("unknown work")?;
        if work.status != WorkStatus::Active {
            return Err("work is a disputed duplicate: payments refused until resolve");
        }
        let key = GroupKey {
            work_id: p.work_id,
            right_type: p.right_type,
            territory: p.territory.clone(),
        };
        let group = self
            .groups
            .get(&key)
            .ok_or("no shares registered for this group")?;
        // Pending shares are not paid yet; their bps count as unassigned.
        // A dispute claimant is not paid; contested shares are paid into
        // escrow (T11), and so are shares still waiting for the work
        // registrant's approval (review M2).
        let paid: Vec<(TokenId, u64, Bps, bool)> = group
            .shares
            .iter()
            .map(|t| (t, self.share(t)))
            .filter(|(_, s)| s.status == ShareStatus::Active && !self.is_claimant(s))
            .map(|(t, s)| {
                let held = s.dispute.is_some() || !self.is_approved(t);
                (t.clone(), s.number, s.percent_bps, held)
            })
            .collect();
        if paid.is_empty() {
            return Err("no active shares in this group");
        }
        let parts: Vec<logic::SplitPart> = paid.iter().map(|(_, n, b, _)| (*n, *b)).collect();
        let (credits, unallocated) =
            logic::split_partial(amount, &parts).ok_or("shares total more than 10000 bps")?;
        if !Self::has_gas_for_forwards(paid.iter().filter(|p| !p.3).count()) {
            return Err("not enough gas to forward to every share");
        }
        let mut lines = Vec::with_capacity(paid.len());
        let mut escrowed = Vec::new();
        for ((token_id, _, _, frozen), credit) in paid.iter().zip(credits) {
            if *frozen {
                self.escrow(token_id, credit);
                let dispute_id = self.share(token_id).dispute;
                let reason = if dispute_id.is_some() {
                    "dispute"
                } else {
                    "awaiting_approval"
                };
                escrowed.push(json!({ "token_id": token_id, "amount": U128(credit), "dispute_id": dispute_id, "reason": reason }));
            } else {
                self.forward(token_id, credit);
                lines.push(json!({ "token_id": token_id, "amount": U128(credit) }));
            }
        }
        if unallocated > 0 {
            let g = self
                .groups
                .get_mut(&key)
                .ok_or("no shares registered for this group")?;
            g.unallocated = U128(g.unallocated.0 + unallocated);
            self.unallocated_total += unallocated;
        }
        emit(
            "payment_in",
            json!({
                "source": if agent.is_some() { "allowance" } else { "direct" },
                "payer": payer,
                "agent": agent,
                "target": { "work_id": key.work_id, "right_type": key.right_type, "territory": key.territory },
                "amount": U128(amount),
                "period": p.period,
                "statement_hash": p.statement_hash,
                "statement_line_id": p.statement_line_id,
                "credits": lines,
                "escrowed": escrowed,
                "unallocated": U128(unallocated),
            }),
        );
        Ok(())
    }

    pub(crate) fn pay_share(
        &mut self,
        payer: &AccountId,
        amount: u128,
        p: PayShare,
        agent: Option<&AccountId>,
        line: Option<&String>,
    ) -> Result<(), &'static str> {
        Self::check_statement(&p.period, &p.statement_hash, line)?;
        // Same floor as a group payment, so share payments cannot be used
        // to spam dust events and forwards (review L8).
        if amount < MIN_GROUP_PAYMENT {
            return Err("below minimum share payment");
        }
        let share = self.shares.get(&p.token_id).ok_or("unknown share")?;
        if share.status != ShareStatus::Active {
            return Err("share is not active");
        }
        let work_ok = matches!(
            self.works.get(&share.group.work_id).map(|w| &w.status),
            Some(WorkStatus::Active)
        );
        if !work_ok {
            return Err("work is a disputed duplicate: payments refused until resolve");
        }
        if self.is_claimant(share) {
            return Err("share is a dispute claimant: not paid until resolve");
        }
        let dispute_id = share.dispute;
        let awaiting = !self.is_approved(&p.token_id);
        let frozen = dispute_id.is_some() || awaiting;
        if frozen {
            self.escrow(&p.token_id, amount);
        } else {
            if !Self::has_gas_for_forwards(1) {
                return Err("not enough gas to forward");
            }
            self.forward(&p.token_id, amount);
        }
        emit(
            "payment_in",
            json!({
                "source": if agent.is_some() { "allowance" } else { "direct" },
                "payer": payer,
                "agent": agent,
                "target": { "token_id": p.token_id },
                "amount": U128(amount),
                "period": p.period,
                "statement_hash": p.statement_hash,
                "statement_line_id": line,
                "credits": if frozen { json!([]) } else { json!([{ "token_id": p.token_id, "amount": U128(amount) }]) },
                "escrowed": if frozen { json!([{ "token_id": p.token_id, "amount": U128(amount), "dispute_id": dispute_id, "reason": if awaiting { "awaiting_approval" } else { "dispute" } }]) } else { json!([]) },
            }),
        );
        Ok(())
    }

    /// Sends `amount` from the registry to the share's vault and settles it
    /// in `on_forward`. Zero credits are skipped.
    pub(crate) fn forward(&mut self, token_id: &TokenId, amount: u128) {
        if amount == 0 {
            return;
        }
        // A stuck forward older than STALE_FORWARD_BLOCKS stops all new sends
        // to this share; the money waits in `undelivered` (M1).
        let Some(since) = forwards::watch_begin(token_id, amount) else {
            self.undelivered_total += amount;
            let l = self.ledger_mut(token_id);
            l.undelivered += amount;
            l.epoch += 1;
            emit(
                "forward_held",
                json!({ "token_id": token_id, "amount": U128(amount), "reason": "stale_forward" }),
            );
            return;
        };
        let vault = self.share(token_id).vault.clone();
        self.forwards_in_flight_total += amount;
        let l = self.ledger_mut(token_id);
        l.inbound_pending += 1;
        l.epoch += 1;
        Promise::new(self.usdc.clone())
            .function_call(
                "ft_transfer".to_string(),
                json_bytes(json!({ "receiver_id": vault, "amount": U128(amount) })),
                NearToken::from_yoctonear(1),
                GAS_FT_TRANSFER,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FORWARD_CB)
                    .on_forward(token_id.clone(), U128(amount), Some(U64(since))),
            )
            .detach();
    }
}
