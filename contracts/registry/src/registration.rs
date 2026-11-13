//! Works and shares: register_work, register_share and the two-phase mint.
//!
//! register_share reserves bps, creates the vault `s<N>.<registry>` in one
//! batch (CreateAccount, Transfer, DeployContract, no AddKey), registers the
//! vault with the token contract, and only then mints the NFT. If any step
//! fails the share is Failed, no NFT exists and the reservation is released
//! (T1, T15).

use crate::*;
use near_contract_standards::non_fungible_token::events::NftMint;
use near_contract_standards::non_fungible_token::metadata::TokenMetadata;
use near_sdk::PromiseOrValue;

#[near]
impl Registry {
    /// Registers a song by ISRC (recording) or ISWC (composition). Personal
    /// data stays off chain: only hashes and account ids. A code that is
    /// already registered opens a duplicate-code dispute and payments to the
    /// duplicate are refused until resolve (decision 5).
    #[payable]
    pub fn register_work(
        &mut self,
        code_type: CodeType,
        code: String,
        title_hash: Hash32,
        provenance: Provenance,
        metadata_uri: String,
    ) -> WorkId {
        let caller = self.assert_active_caller();
        let code = logic::normalize_code(code_type, &code)
            .unwrap_or_else(|| env::panic_str("code does not have a valid ISRC or ISWC shape"));
        require!(
            logic::is_hash32(&title_hash),
            "title_hash must be 64 lowercase hex characters"
        );
        require!(metadata_uri.len() <= MAX_URI_LEN, "metadata_uri too long");
        let initial = env::storage_usage();

        let id = self.next_work;
        self.next_work += 1;
        let index_key = format!("{code_type:?}:{code}");
        let status = match self.code_index.get(&index_key).copied() {
            Some(original) => {
                let dispute_id = self.next_dispute;
                self.next_dispute += 1;
                self.disputes.insert(
                    dispute_id,
                    Dispute {
                        id: dispute_id,
                        kind: DisputeKind::DuplicateCode {
                            original,
                            duplicate: id,
                        },
                        opened_by: caller.clone(),
                        opened_at: U64(env::block_timestamp()),
                        status: DisputeStatus::Open,
                    },
                );
                emit(
                    "dispute_opened",
                    json!({
                        "dispute_id": dispute_id,
                        "kind": "duplicate_code",
                        "work_id": id,
                        "original": original,
                        "opened_by": caller,
                    }),
                );
                WorkStatus::DuplicateOpen {
                    dispute_id,
                    original,
                }
            }
            None => {
                self.code_index.insert(index_key, id);
                WorkStatus::Active
            }
        };
        let work = Work {
            id,
            code_type,
            code: code.clone(),
            title_hash: title_hash.clone(),
            provenance,
            metadata_uri: metadata_uri.clone(),
            registered_by: caller.clone(),
            status,
            groups: Vec::new(),
            created_at: U64(env::block_timestamp()),
        };
        self.works.insert(id, work);
        self.flush_all();
        charge_storage(initial, 0, NearToken::from_yoctonear(0));
        emit(
            "work_registered",
            json!({
                "work_id": id,
                "code_type": code_type,
                "code": code,
                "title_hash": title_hash,
                "provenance": provenance,
                "metadata_uri": metadata_uri,
                "registered_by": caller,
            }),
        );
        id
    }

    /// Registers one rights share and starts the two-phase mint. Caller and
    /// holder must be active members. Attach `VAULT_NEAR + FT_STORAGE_DEPOSIT`
    /// plus registry storage; the rest is refunded. The promise resolves to
    /// true if the NFT was minted.
    #[payable]
    #[allow(clippy::too_many_arguments)]
    pub fn register_share(
        &mut self,
        work_id: WorkId,
        right_type: RightType,
        territory: String,
        percent_bps: Bps,
        basis: Basis,
        holder: AccountId,
        evidence_hash: Hash32,
        contests: Option<Vec<TokenId>>,
    ) -> Promise {
        let caller = self.assert_active_caller();
        require!(self.is_active(&holder), "holder has no active credential");
        let work = self
            .works
            .get(&work_id)
            .unwrap_or_else(|| env::panic_str("unknown work"));
        require!(
            !matches!(work.status, WorkStatus::Void { .. }),
            "work is void"
        );
        require!(
            (1..=10_000).contains(&percent_bps),
            "percent_bps must be 1 to 10000"
        );
        require!(
            logic::is_territory(&territory),
            "territory must be WW or a 2-letter code"
        );
        require!(
            logic::is_hash32(&evidence_hash),
            "evidence_hash must be 64 lowercase hex characters"
        );
        let initial = env::storage_usage();

        let group_key = GroupKey {
            work_id,
            right_type,
            territory,
        };
        let mut group = self.groups.get(&group_key).cloned().unwrap_or_default();
        require!(
            group.open_dispute.is_none(),
            "this work, right and territory has an open dispute"
        );
        let live = group
            .shares
            .iter()
            .filter(|t| {
                !matches!(
                    self.share(t).status,
                    ShareStatus::Failed | ShareStatus::Void
                )
            })
            .count();
        require!(live < MAX_SHARES_PER_GROUP, "group is full");
        require!(
            group.shares.len() < MAX_GROUP_ENTRIES,
            "group has too many entries, failed and void shares included"
        );
        let total =
            group.active_bps + group.reserved_bps + group.claimant_bps + u32::from(percent_bps);
        // Above 100 percent the new share becomes a claimant and an
        // over-allocation dispute opens against `contests` (T11).
        let contested = if total > FULL_BPS {
            require!(
                group.reserved_bps == 0,
                "shares in this group are still minting; retry after they finish"
            );
            let list = contests.unwrap_or_else(|| {
                group
                    .shares
                    .iter()
                    .filter(|t| self.share(t).status == ShareStatus::Active)
                    .cloned()
                    .collect()
            });
            require!(!list.is_empty(), "nothing to contest");
            let mut sorted = list.clone();
            sorted.sort();
            sorted.dedup();
            require!(sorted.len() == list.len(), "contests lists a share twice");
            let mut contested_bps = 0u32;
            for t in &list {
                let s = self
                    .shares
                    .get(t)
                    .unwrap_or_else(|| env::panic_str("contests names an unknown share"));
                require!(
                    s.group == group_key,
                    "contests names a share outside this group"
                );
                require!(
                    s.status == ShareStatus::Active && s.dispute.is_none(),
                    "contests names a share that is not active"
                );
                contested_bps += u32::from(s.percent_bps);
            }
            require!(
                contested_bps >= total - FULL_BPS,
                "contested shares do not cover the excess over 100 percent"
            );
            Some(list)
        } else {
            require!(
                contests.is_none(),
                "contests only apply when the group would exceed 100 percent"
            );
            None
        };
        if !work.groups.contains(&group_key) {
            require!(
                work.groups.len() < MAX_GROUPS_PER_WORK,
                "work has too many groups"
            );
            if let Some(w) = self.works.get_mut(&work_id) {
                w.groups.push(group_key.clone());
            }
        }

        self.next_share += 1;
        let number = self.next_share;
        let token_id = format!("s{number}");
        let vault: AccountId = format!("{token_id}.{}", env::current_account_id())
            .parse()
            .unwrap_or_else(|_| env::panic_str("registry account id too long for vault names"));
        group.shares.push(token_id.clone());
        group.reserved_bps += u32::from(percent_bps);
        self.groups.insert(group_key.clone(), group);
        let now = U64(env::block_timestamp());
        self.shares.insert(
            token_id.clone(),
            Share {
                token_id: token_id.clone(),
                number,
                group: group_key.clone(),
                percent_bps,
                basis,
                evidence_hash: evidence_hash.clone(),
                status: ShareStatus::Pending,
                vault: vault.clone(),
                holder_at_mint: holder.clone(),
                registered_by: caller.clone(),
                note: None,
                payer_attestation: None,
                dispute: None,
                confirmations: Vec::new(),
                last_effective_from: now,
                created_at: now,
            },
        );
        self.ledgers.insert(token_id.clone(), Ledger::default());
        if let Some(list) = contested {
            self.open_overallocation(&caller, &group_key, &token_id, list);
        }
        self.flush_all();
        charge_storage(
            initial,
            MINT_RESERVE_BYTES,
            VAULT_NEAR.saturating_add(FT_STORAGE_DEPOSIT),
        );
        emit(
            "share_registered",
            json!({
                "token_id": token_id,
                "work_id": work_id,
                "right_type": right_type,
                "territory": group_key.territory,
                "percent_bps": percent_bps,
                "basis": basis,
                "holder": holder,
                "vault": vault,
                "evidence_hash": evidence_hash,
                "status": ShareStatus::Pending,
            }),
        );

        // No AddKey action: the vault never has an access key (T1).
        Promise::new(vault)
            .create_account()
            .transfer(VAULT_NEAR)
            .deploy_contract(VAULT_WASM.to_vec())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CREATE_VAULT_CB)
                    .on_vault_created(token_id, caller),
            )
    }

    #[private]
    pub fn on_vault_created(
        &mut self,
        token_id: TokenId,
        payer: AccountId,
    ) -> PromiseOrValue<bool> {
        let vault = self.share(&token_id).vault.clone();
        if env::promise_results_count() != 1 || env::promise_result_checked(0, 64).is_err() {
            // The NEAR sent with the failed batch came back to the registry.
            self.fail_share(&token_id, "vault account creation failed");
            Promise::new(payer)
                .transfer(VAULT_NEAR.saturating_add(FT_STORAGE_DEPOSIT))
                .detach();
            return PromiseOrValue::Value(false);
        }
        Promise::new(self.usdc.clone())
            .function_call(
                "storage_deposit".to_string(),
                json_bytes(json!({ "account_id": vault, "registration_only": true })),
                FT_STORAGE_DEPOSIT,
                GAS_STORAGE_DEPOSIT,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_VAULT_READY_CB)
                    .on_vault_ready(token_id, payer),
            )
            .into()
    }

    #[private]
    pub fn on_vault_ready(&mut self, token_id: TokenId, payer: AccountId) -> bool {
        if callback_result(512).is_none() {
            self.fail_share(&token_id, "vault token registration failed");
            Promise::new(payer).transfer(FT_STORAGE_DEPOSIT).detach();
            return false;
        }
        let share = self.share(&token_id).clone();
        let holder = share.holder_at_mint.clone();
        let metadata = TokenMetadata {
            title: Some(format!(
                "Rights share {} of work {}",
                share.token_id, share.group.work_id
            )),
            description: None,
            media: None,
            media_hash: None,
            copies: Some(1),
            issued_at: Some(env::block_timestamp().to_string()),
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        };
        // Storage for the mint was prepaid at registration (MINT_RESERVE_BYTES).
        self.tokens.internal_mint_with_refund(
            token_id.clone(),
            holder.clone(),
            Some(metadata),
            None,
        );
        NftMint {
            owner_id: &holder,
            token_ids: &[&token_id],
            memo: None,
        }
        .emit();
        let bps = u32::from(share.percent_bps);
        let claimant = self.is_claimant(&share);
        let group = self
            .groups
            .get_mut(&share.group)
            .unwrap_or_else(|| env::panic_str("unknown group"));
        group.reserved_bps -= bps;
        if claimant {
            group.claimant_bps += bps;
        } else {
            group.active_bps += bps;
        }
        if let Some(s) = self.shares.get_mut(&token_id) {
            s.status = ShareStatus::Active;
        }
        emit(
            "vault_ready",
            json!({ "token_id": token_id, "vault": share.vault, "holder": holder }),
        );
        true
    }
}

impl Registry {
    fn fail_share(&mut self, token_id: &TokenId, reason: &str) {
        let share = self.share(token_id).clone();
        if let Some(g) = self.groups.get_mut(&share.group) {
            g.reserved_bps -= u32::from(share.percent_bps);
        }
        let claimant_of = share.dispute.filter(|_| self.is_claimant(&share));
        if let Some(s) = self.shares.get_mut(token_id) {
            s.status = ShareStatus::Failed;
        }
        if let Some(dispute_id) = claimant_of {
            self.cancel_dispute(dispute_id);
        }
        emit(
            "vault_failed",
            json!({ "token_id": token_id, "vault": share.vault, "reason": reason }),
        );
    }

    /// Writes pending collection changes so storage_usage is accurate.
    pub(crate) fn flush_all(&mut self) {
        self.works.flush();
        self.code_index.flush();
        self.shares.flush();
        self.ledgers.flush();
        self.groups.flush();
        self.disputes.flush();
    }
}
