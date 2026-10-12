//! TTL extension helpers. Call from every read/write path to prevent ledger
//! entry archival.

use super::credential::read_vc_position;
use super::VcVaultDataKey;
use crate::constants::{
    INSTANCE_TTL_EXTEND_TO, INSTANCE_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO,
    PERSISTENT_TTL_THRESHOLD,
};
use soroban_sdk::{Env, String};

/// Extend instance TTL (admin, fees). Call from handlers that touch global state.
pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

/// Extend TTL of vault-level keys. Call when reading/writing vault metadata.
pub fn extend_vault_ttl(e: &Env) {
    let keys = [
        VcVaultDataKey::VaultOwner,
        VcVaultDataKey::VaultFactory,
        VcVaultDataKey::VaultAdmin,
        VcVaultDataKey::VaultDid,
        VcVaultDataKey::VaultRevoked,
        VcVaultDataKey::VaultDeniedIssuerCount,
        VcVaultDataKey::VaultVCCount,
    ];
    for key in keys {
        if e.storage().persistent().has(&key) {
            e.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
        }
    }
}

/// Extend TTL of VC payload, status, and the per-VC index entries.
pub fn extend_vc_ttl(e: &Env, vc_id: &String) {
    let vc_key = VcVaultDataKey::VaultVC(vc_id.clone());
    let status_key = VcVaultDataKey::VCStatus(vc_id.clone());
    let position_key = VcVaultDataKey::VaultVCPosition(vc_id.clone());
    for key in [&vc_key, &status_key, &position_key] {
        if e.storage().persistent().has(key) {
            e.storage().persistent().extend_ttl(
                key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
        }
    }
    if let Some(pos) = read_vc_position(e, vc_id) {
        let index_key = VcVaultDataKey::VaultVCIndex(pos);
        if e.storage().persistent().has(&index_key) {
            e.storage().persistent().extend_ttl(
                &index_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
        }
    }
}

/// Extend TTL of VC status only. Call from revoke flow and push tombstone.
pub fn extend_vc_status_ttl(e: &Env, vc_id: &String) {
    let key = VcVaultDataKey::VCStatus(vc_id.clone());
    if e.storage().persistent().has(&key) {
        e.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_EXTEND_TO,
        );
    }
}
