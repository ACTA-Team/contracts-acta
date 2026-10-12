//! Global config storage: admin settings. All instance-scoped keys.

use super::VcVaultDataKey;
use soroban_sdk::{Address, Env};

// --- Admin ---

pub fn has_contract_admin(e: &Env) -> bool {
    e.storage().instance().has(&VcVaultDataKey::ContractAdmin)
}

pub fn read_contract_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&VcVaultDataKey::ContractAdmin)
        .unwrap()
}

pub fn write_contract_admin(e: &Env, admin: &Address) {
    e.storage()
        .instance()
        .set(&VcVaultDataKey::ContractAdmin, admin);
}

pub fn has_pending_admin(e: &Env) -> bool {
    e.storage().instance().has(&VcVaultDataKey::PendingAdmin)
}

pub fn read_pending_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&VcVaultDataKey::PendingAdmin)
}

pub fn write_pending_admin(e: &Env, admin: &Address) {
    e.storage()
        .instance()
        .set(&VcVaultDataKey::PendingAdmin, admin);
}

pub fn remove_pending_admin(e: &Env) {
    e.storage().instance().remove(&VcVaultDataKey::PendingAdmin);
}
