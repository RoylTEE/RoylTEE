//! Agents and allowances (T6 to T9, T14, T27, T28).
//!
//! An agent is a direct sub-account of a member organisation, for example
//! `pay-agent.<org>`. Its only key should be a function-call key scoped to
//! this registry with methods `pay_from_allowance` and `report`. Such a key
//! cannot attach a deposit, and every owner method here needs at least 1
//! yoctoNEAR, so the key cannot reach them even if a role check had a bug.
//!
//! The organisation funds the agent's allowance with one `ft_transfer_call`
//! (`msg: {"fund_allowance": "<agent>"}`). The agent spends it with
//! `pay_from_allowance`, naming only a target, never a payee: the registry
//! resolves the vaults (T8). Limits, expiry, revocation and one payment per
//! statement line are enforced here (T7, T9, T14).

use crate::*;
use near_sdk::assert_one_yocto;

pub(crate) fn line_key(org: &AccountId, line_id: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(org.len() + 1 + line_id.len());
    v.extend_from_slice(org.as_bytes());
    v.push(0);
    v.extend_from_slice(line_id.as_bytes());
    env::sha256(&v)
}

fn line_cost() -> NearToken {
    env::storage_byte_cost().saturating_mul(u128::from(LINE_STORAGE_BYTES))
}

#[near]
impl Registry {
    /// Grants (or re-grants after revoke) an agent role. Caller: an active
    /// member, and the agent must be its direct sub-account. Attach the role
    /// storage plus prepaid statement-line storage (0.001 NEAR per line);
    /// whole lines are credited, the rest refunded. Needs a deposit, so a
    /// function-call key cannot call it.
    #[payable]
    pub fn grant_role(
        &mut self,
        agent: AccountId,
        methods: Vec<AgentMethod>,
        per_tx_limit: U128,
        daily_limit: U128,
        max_tx_per_day: u32,
        expires_at: U64,
    ) {
        require!(
            !env::attached_deposit().is_zero(),
            "attach a deposit for role storage"
        );
        let org = self.assert_active_caller();
        let label = agent
            .as_str()
            .strip_suffix(&format!(".{org}"))
            .unwrap_or_else(|| env::panic_str("agent must be a direct sub-account of the caller"));
        require!(
            !label.is_empty() && !label.contains('.'),
            "agent must be a direct sub-account of the caller"
        );
        require!(
            self.credentials.get(&agent).is_none(),
            "accounts with a credential cannot be agents"
        );
        require!(
            !self.payers.contains(&agent),
            "payer accounts cannot be agents"
        );
        require!(!methods.is_empty(), "methods must not be empty");
        require!(
            per_tx_limit.0 > 0 && per_tx_limit.0 <= daily_limit.0,
            "need 0 < per_tx_limit <= daily_limit"
        );
        require!(
            (1..=MAX_TX_PER_DAY).contains(&max_tx_per_day),
            "max_tx_per_day out of range"
        );
        require!(
            expires_at.0 > env::block_timestamp(),
            "expires_at must be in the future"
        );
        let initial = env::storage_usage();
        let previous = self.roles.get(&agent).cloned();
        if let Some(p) = &previous {
            require!(p.org == org, "agent belongs to another organisation");
            require!(p.revoked, "role already active; revoke it first");
        }
        let mut methods = methods;
        methods.sort_by_key(|m| *m as u8);
        methods.dedup();
        let p = previous.as_ref();
        let role = AgentRole {
            org: org.clone(),
            methods: methods.clone(),
            per_tx_limit: per_tx_limit.0,
            daily_limit: daily_limit.0,
            max_tx_per_day,
            expires_at: expires_at.0,
            revoked: false,
            allowance: p.map_or(0, |r| r.allowance),
            line_credits: p.map_or(0, |r| r.line_credits),
            spent: p.map_or([0; ROLLING_HOURS], |r| r.spent),
            counts: p.map_or([0; ROLLING_HOURS], |r| r.counts),
            bucket_hour: p.map_or(env::block_timestamp() / NS_PER_HOUR, |r| r.bucket_hour),
            granted_at: env::block_timestamp(),
        };
        self.roles.insert(agent.clone(), role);
        self.roles.flush();
        // Reserve the window tail slot now, so the organisation pays its
        // storage (review L2).
        let tail = Self::read_tail(&agent).unwrap_or((0, 0, 0));
        Self::write_tail(&agent, tail);
        let storage = env::storage_byte_cost()
            .saturating_mul(u128::from(env::storage_usage().saturating_sub(initial)));
        let attached = env::attached_deposit();
        require!(
            attached >= storage,
            format!(
                "attach at least {} yoctoNEAR",
                storage.saturating_add(line_cost()).as_yoctonear()
            )
        );
        self.credit_lines(&agent, attached.saturating_sub(storage));
        emit(
            "role_granted",
            json!({
                "agent": agent,
                "org": org,
                "methods": methods,
                "per_tx_limit": per_tx_limit,
                "daily_limit": daily_limit,
                "max_tx_per_day": max_tx_per_day,
                "expires_at": expires_at,
            }),
        );
    }

    /// Prepays more statement-line storage for an agent. Organisation only.
    #[payable]
    pub fn add_line_credits(&mut self, agent: AccountId) {
        let caller = env::predecessor_account_id();
        let role = self
            .roles
            .get(&agent)
            .unwrap_or_else(|| env::panic_str("no such agent"));
        require!(role.org == caller, "only the agent's organisation");
        require!(
            env::attached_deposit() >= line_cost(),
            "attach at least one line credit"
        );
        self.credit_lines(&agent, env::attached_deposit());
    }

    /// Revokes an agent at once (T14). Organisation or registrar. The
    /// allowance stays withdrawable by the organisation.
    #[payable]
    pub fn revoke_role(&mut self, agent: AccountId) {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        let registrar = self.registrar.clone();
        let role = self
            .roles
            .get_mut(&agent)
            .unwrap_or_else(|| env::panic_str("no such agent"));
        require!(
            caller == role.org || caller == registrar,
            "only the organisation or the registrar"
        );
        require!(!role.revoked, "already revoked");
        role.revoked = true;
        emit("role_revoked", json!({ "agent": agent, "by": caller }));
    }

    /// Sends allowance back to the organisation. Organisation only, 1
    /// yoctoNEAR, debit before the transfer, restored if it fails.
    #[payable]
    pub fn withdraw_allowance(&mut self, agent: AccountId, amount: U128) -> Promise {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        require!(
            !self.is_suspended(&caller),
            "organisation credential is suspended"
        );
        let role = self
            .roles
            .get_mut(&agent)
            .unwrap_or_else(|| env::panic_str("no such agent"));
        require!(role.org == caller, "only the agent's organisation");
        require!(
            amount.0 > 0 && amount.0 <= role.allowance,
            "amount exceeds allowance"
        );
        role.allowance -= amount.0;
        self.allowance_total -= amount.0;
        Promise::new(self.usdc.clone())
            .function_call(
                "ft_transfer".to_string(),
                json_bytes(json!({ "receiver_id": caller, "amount": amount })),
                NearToken::from_yoctonear(1),
                GAS_FT_TRANSFER,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FORWARD_CB)
                    .on_allowance_withdrawn(agent, caller, amount),
            )
    }

    #[private]
    pub fn on_allowance_withdrawn(
        &mut self,
        agent: AccountId,
        org: AccountId,
        amount: U128,
    ) -> bool {
        let ok = callback_result(64).is_some();
        if !ok {
            if let Some(r) = self.roles.get_mut(&agent) {
                r.allowance += amount.0;
            }
            self.allowance_total += amount.0;
        }
        emit(
            "allowance_withdrawn",
            json!({ "agent": agent, "org": org, "amount": amount, "ok": ok }),
        );
        ok
    }

    /// The agent's payment method. No deposit, so a function-call key can
    /// call it. There is no payee argument: the registry pays the vaults of
    /// the target's registered shares (T8). Fails before any state change if
    /// any check fails.
    pub fn pay_from_allowance(
        &mut self,
        target: PayTarget,
        amount: U128,
        statement_line_id: String,
        period: String,
        statement_hash: Hash32,
    ) {
        let agent = env::predecessor_account_id();
        let org = self.assert_agent(&agent, AgentMethod::PayFromAllowance);
        let amount = amount.0;
        require!(
            !statement_line_id.is_empty() && statement_line_id.len() <= MAX_LINE_ID_LEN,
            "statement_line_id must be 1 to 64 bytes"
        );
        require!(amount > 0, "amount must be positive");
        let key = line_key(&org, &statement_line_id);
        require!(
            !self.statement_lines.contains(&key),
            "statement line already paid"
        );
        // All checks run on a copy first; state changes only after routing
        // succeeds (a panic reverts on chain anyway, this keeps the order
        // obvious).
        let now_hour = env::block_timestamp() / NS_PER_HOUR;
        let mut role = self
            .roles
            .get(&agent)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no such agent"));
        require!(amount <= role.per_tx_limit, "over per-transaction limit");
        let (tail_spent, tail_count) = Self::tail_now(&agent, &role, now_hour);
        let hour = logic::roll_window(&mut role.spent, role.bucket_hour, now_hour);
        logic::roll_window(&mut role.counts, role.bucket_hour, now_hour);
        role.bucket_hour = hour;
        let spent: u128 = role.spent.iter().sum::<u128>() + tail_spent;
        let count: u32 = role.counts.iter().sum::<u32>() + tail_count;
        require!(
            spent.saturating_add(amount) <= role.daily_limit,
            "over rolling 24 hour limit"
        );
        require!(count < role.max_tx_per_day, "over max transactions per day");
        require!(amount <= role.allowance, "allowance too low");
        require!(
            role.line_credits > 0,
            "no prepaid statement-line storage left"
        );
        let result = match target.clone() {
            PayTarget::Work {
                work_id,
                right_type,
                territory,
            } => self.pay_work(
                &org,
                amount,
                PayWork {
                    work_id,
                    right_type,
                    territory,
                    period,
                    statement_hash,
                    statement_line_id: Some(statement_line_id.clone()),
                },
                Some(&agent),
            ),
            PayTarget::Share { token_id } => self.pay_share(
                &org,
                amount,
                PayShare {
                    token_id,
                    period,
                    statement_hash,
                },
                Some(&agent),
                Some(&statement_line_id),
            ),
        };
        if let Err(reason) = result {
            env::panic_str(reason);
        }
        let idx = (hour % ROLLING_HOURS as u64) as usize;
        role.spent[idx] += amount;
        role.counts[idx] += 1;
        role.allowance -= amount;
        role.line_credits -= 1;
        self.roles.insert(agent.clone(), role);
        Self::write_tail(
            &agent,
            (
                hour.saturating_sub(ROLLING_HOURS as u64),
                tail_spent,
                tail_count,
            ),
        );
        self.statement_lines.insert(key);
        self.allowance_total -= amount;
        emit(
            "allowance_spent",
            json!({
                "agent": agent,
                "org": org,
                "amount": U128(amount),
                "statement_line_id": statement_line_id,
                "target": target,
            }),
        );
    }

    /// Agent report: an event only, no state, no money.
    pub fn report(&mut self, statement_line_id: String, report_hash: Hash32) {
        let agent = env::predecessor_account_id();
        let org = self.assert_agent(&agent, AgentMethod::Report);
        require!(
            !statement_line_id.is_empty() && statement_line_id.len() <= MAX_LINE_ID_LEN,
            "statement_line_id must be 1 to 64 bytes"
        );
        require!(
            logic::is_hash32(&report_hash),
            "report_hash must be 64 lowercase hex characters"
        );
        emit(
            "agent_report",
            json!({ "agent": agent, "org": org, "statement_line_id": statement_line_id, "report_hash": report_hash }),
        );
    }

    pub fn get_role(&self, agent: AccountId) -> Option<RoleView> {
        let r = self.roles.get(&agent)?;
        let now_hour = env::block_timestamp() / NS_PER_HOUR;
        let (tail_spent, tail_count) = Self::tail_now(&agent, r, now_hour);
        let mut spent = r.spent;
        let mut counts = r.counts;
        logic::roll_window(&mut spent, r.bucket_hour, now_hour);
        logic::roll_window(&mut counts, r.bucket_hour, now_hour);
        Some(RoleView {
            agent,
            org: r.org.clone(),
            methods: r.methods.clone(),
            per_tx_limit: U128(r.per_tx_limit),
            daily_limit: U128(r.daily_limit),
            max_tx_per_day: r.max_tx_per_day,
            expires_at: U64(r.expires_at),
            revoked: r.revoked,
            expired: env::block_timestamp() >= r.expires_at,
            allowance: U128(r.allowance),
            line_credits: r.line_credits,
            spent_last_24h: U128(spent.iter().sum::<u128>() + tail_spent),
            tx_last_24h: counts.iter().sum::<u32>() + tail_count,
        })
    }

    pub fn is_statement_line_used(&self, org: AccountId, statement_line_id: String) -> bool {
        self.statement_lines
            .contains(&line_key(&org, &statement_line_id))
    }
}

impl Registry {
    /// Checks in order: role exists, not revoked, not expired, method
    /// allowed, organisation credential active. Returns the organisation.
    fn assert_agent(&self, agent: &AccountId, method: AgentMethod) -> AccountId {
        let role = self
            .roles
            .get(agent)
            .unwrap_or_else(|| env::panic_str("caller has no agent role"));
        require!(!role.revoked, "agent role revoked");
        require!(
            env::block_timestamp() < role.expires_at,
            "agent role expired"
        );
        require!(
            role.methods.contains(&method),
            "method not allowed for this agent"
        );
        require!(
            self.is_active(&role.org),
            "organisation credential is not active"
        );
        role.org.clone()
    }

    /// The window tail (spent, count) for `now_hour` (review L2). Kept in
    /// its own storage slot `l2t:<agent>`, so AgentRole is unchanged.
    fn tail_now(agent: &AccountId, role: &AgentRole, now_hour: u64) -> (u128, u32) {
        let stored = Self::read_tail(agent);
        let spent = logic::window_tail(
            &role.spent,
            role.bucket_hour,
            now_hour,
            stored.map(|(h, s, _)| (h, s)),
        );
        let count = logic::window_tail(
            &role.counts,
            role.bucket_hour,
            now_hour,
            stored.map(|(h, _, c)| (h, c)),
        );
        (spent, count)
    }

    fn tail_key(agent: &AccountId) -> Vec<u8> {
        [b"l2t:".as_slice(), agent.as_bytes()].concat()
    }

    fn read_tail(agent: &AccountId) -> Option<(u64, u128, u32)> {
        env::storage_read(&Self::tail_key(agent)).and_then(|v| near_sdk::borsh::from_slice(&v).ok())
    }

    fn write_tail(agent: &AccountId, tail: (u64, u128, u32)) {
        let v = near_sdk::borsh::to_vec(&tail).unwrap_or_else(|_| env::panic_str("borsh"));
        env::storage_write(&Self::tail_key(agent), &v);
    }

    fn credit_lines(&mut self, agent: &AccountId, deposit: NearToken) {
        let cost = line_cost().as_yoctonear();
        let lines = deposit.as_yoctonear() / cost;
        let refund = deposit.as_yoctonear() - lines * cost;
        let lines =
            u32::try_from(lines).unwrap_or_else(|_| env::panic_str("too many line credits"));
        if let Some(r) = self.roles.get_mut(agent) {
            r.line_credits = r.line_credits.saturating_add(lines);
        }
        if refund > 0 {
            Promise::new(env::predecessor_account_id())
                .transfer(NearToken::from_yoctonear(refund))
                .detach();
        }
        emit(
            "line_credits_added",
            json!({ "agent": agent, "lines": lines }),
        );
    }

    /// `ft_on_transfer` with `{"fund_allowance": "<agent>"}`. The sender
    /// must be the agent's organisation, active, and the role live (T28).
    pub(crate) fn fund_allowance(
        &mut self,
        sender: &AccountId,
        amount: u128,
        agent: AccountId,
    ) -> Result<(), &'static str> {
        if amount == 0 {
            return Err("zero amount");
        }
        let active = self.is_active(sender);
        let role = self.roles.get_mut(&agent).ok_or("no such agent")?;
        if &role.org != sender {
            return Err("only the agent's organisation can fund it");
        }
        if !active {
            return Err("organisation credential is not active");
        }
        if role.revoked || env::block_timestamp() >= role.expires_at {
            return Err("agent role is revoked or expired");
        }
        role.allowance += amount;
        let allowance = role.allowance;
        self.allowance_total += amount;
        emit(
            "allowance_funded",
            json!({ "agent": agent, "org": sender, "amount": U128(amount), "allowance": U128(allowance) }),
        );
        Ok(())
    }
}
