//! Contract implementation: public entrypoints.

use crate::error::ContractError;
use crate::events;
use crate::interface::VcVaultTrait;
use crate::storage;
use crate::types::VCStatus;
use crate::validator::*;
use crate::vault;
use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, symbol_short, Address, Env, IntoVal,
    String, Vec,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

contractmeta!(
    key = "Description",
    val = "VC Vault: Verifiable Credential storage + issuance status registry",
);

#[allow(dead_code)]
#[contract]
pub struct VcVaultContract;

#[contractimpl]
impl VcVaultContract {
    pub fn __constructor(
        e: Env,
        vault_owner: Address,
        contract_admin: Address,
        did_uri: String,
        factory_address: Address,
    ) {
        require_did_uri_len(&e, &did_uri);
        storage::write_vault_owner(&e, &vault_owner);
        storage::write_factory_address(&e, &factory_address);
        storage::write_contract_admin(&e, &contract_admin);
        storage::write_vault_did(&e, &did_uri);
        storage::write_vault_admin(&e, &vault_owner);
        storage::extend_vault_ttl(&e);
        storage::extend_instance_ttl(&e);
        events::contract_initialized(&e, &contract_admin);
        events::vault_created(&e, &vault_owner, &did_uri);
    }
}

#[contractimpl]
impl VcVaultTrait for VcVaultContract {
    // --- Global config ---

    fn nominate_admin(e: Env, new_admin: Address) {
        let current = require_contract_admin(&e);
        storage::write_pending_admin(&e, &new_admin);
        storage::extend_instance_ttl(&e);
        events::admin_nominated(&e, &current, &new_admin);
    }

    fn accept_contract_admin(e: Env) {
        let pending = match storage::read_pending_admin(&e) {
            Some(a) => a,
            None => panic_with_error!(e, ContractError::NoPendingAdmin),
        };
        pending.require_auth();
        let old_admin = storage::read_contract_admin(&e);
        storage::write_contract_admin(&e, &pending);
        storage::remove_pending_admin(&e);
        storage::extend_instance_ttl(&e);
        events::admin_transferred(&e, &old_admin, &pending);
    }

    fn version(e: Env) -> String {
        String::from_str(&e, VERSION)
    }

    // --- Vault management ---

    fn set_vault_admin(e: Env, new_admin: Address) {
        require_vault_admin(&e);
        require_vault_active(&e);
        let old_admin = storage::read_vault_admin(&e);
        storage::write_vault_admin(&e, &new_admin);
        storage::extend_vault_ttl(&e);
        events::vault_admin_changed(&e, &old_admin, &new_admin);
    }

    fn set_vault_did(e: Env, did_uri: String) {
        require_did_uri_len(&e, &did_uri);
        require_vault_active(&e);
        let owner = storage::read_vault_owner(&e);
        owner.require_auth();
        storage::write_vault_did(&e, &did_uri);
        storage::extend_vault_ttl(&e);
        events::vault_did_changed(&e, &did_uri);
    }

    fn vault_did(e: Env) -> Option<String> {
        storage::extend_vault_ttl(&e);
        storage::read_vault_did(&e)
    }

    fn vault_owner(e: Env) -> Address {
        storage::extend_vault_ttl(&e);
        storage::read_vault_owner(&e)
    }

    fn deny_issuer(e: Env, issuer_addr: Address) {
        require_vault_admin(&e);
        require_vault_active(&e);
        storage::append_denied_issuer_to_index(&e, &issuer_addr);
        storage::extend_vault_ttl(&e);
        events::issuer_denied(&e, &issuer_addr);
    }

    fn allow_issuer(e: Env, issuer_addr: Address) {
        require_vault_admin(&e);
        require_vault_active(&e);
        storage::remove_denied_issuer_from_index(&e, &issuer_addr);
        storage::extend_vault_ttl(&e);
        events::issuer_allowed(&e, &issuer_addr);
    }

    fn revoke_vault(e: Env) {
        require_vault_admin(&e);
        require_vault_active(&e);
        storage::write_vault_revoked(&e, &true);
        storage::extend_vault_ttl(&e);
        events::vault_revoked(&e);
    }

    // --- Credential queries ---

    fn list_vc_ids(e: Env, offset: u32, limit: u32) -> Vec<String> {
        if limit > storage::MAX_LIST_LIMIT {
            panic_with_error!(e, ContractError::LimitTooLarge);
        }
        storage::extend_vault_ttl(&e);
        let mut ids = Vec::new(&e);
        if limit == 0 {
            return ids;
        }
        let count = storage::read_vc_count(&e);
        if offset >= count {
            return ids;
        }
        let end = offset.saturating_add(limit).min(count);
        for i in offset..end {
            if let Some(vc_id) = storage::read_vc_id_at_extend(&e, i) {
                ids.push_back(vc_id);
            }
        }
        ids
    }

    fn vc_count(e: Env) -> u32 {
        storage::extend_vault_ttl(&e);
        storage::read_vc_count(&e)
    }

    fn get_vc(e: Env, vc_id: String) -> Option<crate::types::VerifiableCredential> {
        require_vc_id_len(&e, &vc_id);
        storage::extend_vault_ttl(&e);
        let vc = storage::read_vault_vc(&e, &vc_id);
        if vc.is_some() {
            storage::extend_vc_ttl(&e, &vc_id);
        }
        vc
    }

    fn verify_vc(e: Env, vc_id: String) -> VCStatus {
        require_vc_id_len(&e, &vc_id);
        storage::extend_vault_ttl(&e);
        let vc_opt = storage::read_vault_vc(&e, &vc_id);
        if vc_opt.is_none() {
            return VCStatus::Invalid;
        }
        let vc = vc_opt.unwrap();
        storage::extend_vc_ttl(&e, &vc_id);
        let issuance_contract = vc.issuance_contract;
        if issuance_contract == e.current_contract_address() {
            return storage::read_vc_status(&e, &vc_id);
        }
        e.invoke_contract::<VCStatus>(
            &issuance_contract,
            &symbol_short!("verify"),
            (vc_id,).into_val(&e),
        )
    }

    // --- Issuance ---

    fn issue(
        e: Env,
        vc_id: String,
        vc_data: String,
        vault_contract: Address,
        issuer_addr: Address,
        issuer_did: String,
    ) -> String {
        require_vc_id_len(&e, &vc_id);
        require_vc_data_len(&e, &vc_data);
        require_issuer_did_len(&e, &issuer_did);
        issuer_addr.require_auth();
        let this = e.current_contract_address();
        if vault_contract != this {
            panic_with_error!(e, ContractError::InvalidVaultContract);
        }
        require_vault_active(&e);
        if storage::denied_issuer_index_contains(&e, &issuer_addr) {
            panic_with_error!(e, ContractError::IssuerDenied);
        }

        if storage::read_vault_vc(&e, &vc_id).is_some()
            || storage::read_vc_status(&e, &vc_id) != VCStatus::Invalid
        {
            panic_with_error!(e, ContractError::VCAlreadyExists);
        }

        let factory = storage::read_factory_address(&e);
        vault::charge_fee(&e, &factory, &issuer_addr);

        vault::store_vc(&e, vc_id.clone(), vc_data, this.clone(), issuer_did);
        storage::write_vc_status(&e, &vc_id, &VCStatus::Valid);
        storage::extend_vault_ttl(&e);
        storage::extend_vc_ttl(&e, &vc_id);
        events::vc_issued(&e, &vc_id, &issuer_addr);
        vc_id
    }

    fn batch_issue(
        e: Env,
        issuer_addr: Address,
        vault_contract: Address,
        issuer_did: String,
        vcs: Vec<(String, String)>,
    ) -> Vec<String> {
        require_issuer_did_len(&e, &issuer_did);
        for entry in vcs.iter() {
            let (vc_id, vc_data) = entry;
            require_vc_id_len(&e, &vc_id);
            require_vc_data_len(&e, &vc_data);
        }
        issuer_addr.require_auth();
        let n = vcs.len();
        if n == 0 {
            panic_with_error!(e, ContractError::BatchEmpty);
        }
        if n > storage::MAX_BATCH_SIZE {
            panic_with_error!(e, ContractError::BatchTooLarge);
        }
        let this = e.current_contract_address();
        if vault_contract != this {
            panic_with_error!(e, ContractError::InvalidVaultContract);
        }
        require_vault_active(&e);
        if storage::denied_issuer_index_contains(&e, &issuer_addr) {
            panic_with_error!(e, ContractError::IssuerDenied);
        }

        let factory = storage::read_factory_address(&e);
        let unit = vault::charge_fee_quote_only(&e, &factory, &issuer_addr);
        if unit.0 && unit.1 > 0 {
            let total = unit
                .1
                .checked_mul(n as i128)
                .unwrap_or_else(|| panic_with_error!(e, ContractError::FeeOutOfBounds));
            vault::transfer_fee(&e, &unit.2, &unit.3, &issuer_addr, total);
        }

        let mut result = Vec::new(&e);
        for entry in vcs.iter() {
            let (vc_id, vc_data) = entry;
            if storage::read_vault_vc(&e, &vc_id).is_some()
                || storage::read_vc_status(&e, &vc_id) != VCStatus::Invalid
            {
                panic_with_error!(e, ContractError::VCAlreadyExists);
            }
            vault::store_vc(&e, vc_id.clone(), vc_data, this.clone(), issuer_did.clone());
            storage::write_vc_status(&e, &vc_id, &VCStatus::Valid);
            storage::extend_vc_ttl(&e, &vc_id);
            events::vc_issued(&e, &vc_id, &issuer_addr);
            result.push_back(vc_id);
        }
        storage::extend_vault_ttl(&e);
        result
    }

    fn revoke(e: Env, vc_id: String, date: String) {
        require_vc_id_len(&e, &vc_id);
        require_date_len(&e, &date);
        let owner = storage::read_vault_owner(&e);
        owner.require_auth();
        if storage::read_vault_vc(&e, &vc_id).is_none()
            || storage::read_vc_status(&e, &vc_id) != VCStatus::Valid
        {
            panic_with_error!(e, ContractError::VCNotFound);
        }
        vault::revoke_vc(&e, vc_id.clone(), date.clone());
        storage::remove_vc_from_index(&e, &vc_id);
        storage::extend_vault_ttl(&e);
        storage::extend_vc_status_ttl(&e, &vc_id);
        events::vc_revoked(&e, &vc_id, &date);
    }

    fn push(e: Env, vc_id: String, dest_vault: Address) {
        require_vc_id_len(&e, &vc_id);
        require_vault_admin(&e);
        require_vault_active(&e);
        let vc = match storage::read_vault_vc(&e, &vc_id) {
            Some(v) => v,
            None => panic_with_error!(e, ContractError::VCNotFound),
        };
        if storage::read_vc_status(&e, &vc_id) != VCStatus::Valid {
            panic_with_error!(e, ContractError::VCNotFound);
        }
        // Pass our own owner to the destination so it can enforce the
        // same-owner migration rule without re-entering this contract (the
        // source vault is still on the call stack here, so a callback into
        // `vault_owner` would trip Soroban's re-entry guard). The value is
        // trustworthy because `receive_push` requires this vault's auth.
        let src_owner = storage::read_vault_owner(&e);
        e.invoke_contract::<()>(
            &dest_vault,
            &soroban_sdk::Symbol::new(&e, "receive_push"),
            (
                e.current_contract_address(),
                src_owner,
                vc_id.clone(),
                vc.data,
                vc.issuer_did,
            )
                .into_val(&e),
        );
        storage::remove_vc_from_index(&e, &vc_id);
        storage::remove_vault_vc(&e, &vc_id);
        storage::remove_vc_status(&e, &vc_id);
        storage::extend_vault_ttl(&e);
        events::vc_pushed(&e, &vc_id, &dest_vault);
    }

    fn receive_push(
        e: Env,
        source_vault: Address,
        source_owner: Address,
        vc_id: String,
        vc_data: String,
        issuer_did: String,
    ) {
        require_vc_id_len(&e, &vc_id);
        require_vc_data_len(&e, &vc_data);
        require_issuer_did_len(&e, &issuer_did);
        require_vault_active(&e);
        source_vault.require_auth();
        // Verify source_vault was deployed by the same factory.
        let factory = storage::read_factory_address(&e);
        let is_legit: bool = e.invoke_contract(
            &factory,
            &soroban_sdk::Symbol::new(&e, "is_vault"),
            (source_vault.clone(),).into_val(&e),
        );
        if !is_legit {
            panic_with_error!(e, ContractError::SourceNotAVault);
        }
        // push is owner-scoped migration: the source vault must belong to the
        // same owner as this destination vault. `source_owner` is supplied by
        // the authenticated source vault (it reads its own stored owner before
        // invoking us), so we avoid re-entering the source vault - which is
        // still on the call stack - to query `vault_owner`.
        if source_owner != storage::read_vault_owner(&e) {
            panic_with_error!(e, ContractError::PushOwnerMismatch);
        }
        // Mirror the duplicate guard used by issue()/batch_issue(): the index
        // entry is removed on revoke() but the Revoked status persists, so an
        // index-only check would let a pushed VC silently overwrite a revoked
        // credential back to Valid. Checking the status closes that bypass.
        if storage::read_vault_vc(&e, &vc_id).is_some()
            || storage::read_vc_status(&e, &vc_id) != VCStatus::Invalid
        {
            panic_with_error!(e, ContractError::VCAlreadyExists);
        }
        let dest = e.current_contract_address();
        vault::store_vc(&e, vc_id.clone(), vc_data, dest.clone(), issuer_did);
        storage::write_vc_status(&e, &vc_id, &VCStatus::Valid);
        storage::extend_vault_ttl(&e);
        storage::extend_vc_ttl(&e, &vc_id);
        events::vc_issued(&e, &vc_id, &dest);
    }

    // --- Issuer queries ---

    fn list_denied_issuers(e: Env, offset: u32, limit: u32) -> Vec<Address> {
        if limit > storage::MAX_LIST_LIMIT {
            panic_with_error!(e, ContractError::LimitTooLarge);
        }
        storage::extend_vault_ttl(&e);
        let mut result = Vec::new(&e);
        if limit == 0 {
            return result;
        }
        let count = storage::read_denied_issuer_count(&e);
        if offset >= count {
            return result;
        }
        let end = offset.saturating_add(limit).min(count);
        for i in offset..end {
            if let Some(addr) = storage::read_denied_issuer_at_extend(&e, i) {
                result.push_back(addr);
            }
        }
        result
    }

    fn denied_issuer_count(e: Env) -> u32 {
        storage::extend_vault_ttl(&e);
        storage::read_denied_issuer_count(&e)
    }
}
