//! Vault metadata storage: owner, admin, DID, revoked flag.

use super::VcVaultDataKey;
use crate::constants::{PERSISTENT_TTL_EXTEND_TO, PERSISTENT_TTL_THRESHOLD};
use soroban_sdk::{Address, Env, String};

// --- Vault owner ---

pub fn write_vault_owner(e: &Env, owner: &Address) {
    let key = VcVaultDataKey::VaultOwner;
    e.storage().persistent().set(&key, owner);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

pub fn read_vault_owner(e: &Env) -> Address {
    e.storage()
        .persistent()
        .get(&VcVaultDataKey::VaultOwner)
        .unwrap()
}

// --- Factory address ---

pub fn write_factory_address(e: &Env, factory: &Address) {
    let key = VcVaultDataKey::VaultFactory;
    e.storage().persistent().set(&key, factory);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

pub fn read_factory_address(e: &Env) -> Address {
    e.storage()
        .persistent()
        .get(&VcVaultDataKey::VaultFactory)
        .unwrap()
}

// --- Vault admin ---

pub fn has_vault_admin(e: &Env) -> bool {
    e.storage().persistent().has(&VcVaultDataKey::VaultAdmin)
}

pub fn read_vault_admin(e: &Env) -> Address {
    e.storage()
        .persistent()
        .get(&VcVaultDataKey::VaultAdmin)
        .unwrap()
}

pub fn write_vault_admin(e: &Env, admin: &Address) {
    e.storage()
        .persistent()
        .set(&VcVaultDataKey::VaultAdmin, admin);
}

// --- DID URI ---

pub fn write_vault_did(e: &Env, did: &String) {
    e.storage().persistent().set(&VcVaultDataKey::VaultDid, did);
}

pub fn read_vault_did(e: &Env) -> Option<String> {
    e.storage().persistent().get(&VcVaultDataKey::VaultDid)
}

// --- Revoked flag ---

pub fn read_vault_revoked(e: &Env) -> bool {
    e.storage()
        .persistent()
        .get(&VcVaultDataKey::VaultRevoked)
        .unwrap_or(false)
}

pub fn write_vault_revoked(e: &Env, revoked: &bool) {
    e.storage()
        .persistent()
        .set(&VcVaultDataKey::VaultRevoked, revoked);
}
