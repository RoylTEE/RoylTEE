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
    /// it. A payout lock is cleared WITHOUT rolling back the debit: if the
    /// withdraw happened the debit is right; if not, `sync` finds the money
    /// again (and `total_paid_out` overstates by that amount, flagged by an
    /// `anomaly` event). A transfer lock is cleared and the current owner
    /// keeps the token.
    pub fn release_stale_lock(&mut self, token_id: TokenId) {
        let now = env::block_height();
        let stale = |k: &Option<Lock>| {
            k.as_ref()
                .is_some_and(|k| now >= k.since_block.0.saturating_add(STALE_LOCK_BLOCKS))
        };
        let l = self.ledger_mut(&token_id);
        let mut released = Vec::new();
        if stale(&l.payout_in_flight) {
            released.push(("payout_in_flight", l.payout_in_flight.take()));
        }
        if stale(&l.transfer_pending) {
            released.push(("transfer_pending", l.transfer_pending.take()));
        }
        require!(!released.is_empty(), "no stale lock");
        l.epoch += 1;
        for (kind, lock) in released {
            emit(
                "lock_released_stale",
                json!({ "token_id": token_id, "lock": kind, "detail": lock }),
            );
            if kind == "payout_in_flight" {
                emit(
                    "anomaly",
                    json!({ "token_id": token_id, "kind": "payout_lock_released_stale", "detail": "sync to reconcile the vault balance" }),
                );
            }
        }
    }

    /// Reads the vault's real token balance and records it, so plain
    /// `ft_transfer` payments straight into a vault count as earned.
    /// Anyone may call it. Refused while any lock or forward is open (T21).
    pub fn sync(&mut self, token_id: TokenId) -> Promise {
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
        let ledger = self.ledger_mut(&token_id);
        require!(ledger.payout_in_flight.is_none(), "payout in flight");
        require!(ledger.transfer_pending.is_none(), "transfer pending");
        require!(ledger.inbound_pending == 0, "payment forward in flight");
        let epoch = ledger.epoch;
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

    /// Discards the reading if the ledger changed since the read began.
    /// Never panics. Returns the earned total after the sync.
    #[private]
    pub fn on_sync(&mut self, token_id: TokenId, epoch: U64) -> Option<U128> {
        let observed = callback_result(64).and_then(|v| serde_json::from_slice::<U128>(&v).ok());
        let ledger = self.ledgers.get_mut(&token_id)?;
        let Some(observed) = observed else {
            emit(
                "sync_discarded",
                json!({ "token_id": token_id, "reason": "read failed" }),
            );
            return None;
        };
        if ledger.epoch != epoch.0
            || ledger.payout_in_flight.is_some()
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
    /// `undelivered` (T26). Never panics.
    #[private]
    pub fn on_forward(&mut self, token_id: TokenId, amount: U128) -> bool {
        let ok = callback_result(64).is_some();
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
            + Gas::from_tgas(10).as_gas();
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
        // escrow (T11).
        let paid: Vec<(TokenId, u64, Bps, bool)> = group
            .shares
            .iter()
            .map(|t| (t, self.share(t)))
            .filter(|(_, s)| s.status == ShareStatus::Active && !self.is_claimant(s))
            .map(|(t, s)| (t.clone(), s.number, s.percent_bps, s.dispute.is_some()))
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
                escrowed.push(json!({ "token_id": token_id, "amount": U128(credit), "dispute_id": self.share(token_id).dispute }));
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
        if amount == 0 {
            return Err("zero amount");
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
        let frozen = share.dispute;
        if frozen.is_some() {
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
                "credits": if frozen.is_none() { json!([{ "token_id": p.token_id, "amount": U128(amount) }]) } else { json!([]) },
                "escrowed": if let Some(id) = frozen { json!([{ "token_id": p.token_id, "amount": U128(amount), "dispute_id": id }]) } else { json!([]) },
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
                    .on_forward(token_id.clone(), U128(amount)),
            )
            .detach();
    }
}
