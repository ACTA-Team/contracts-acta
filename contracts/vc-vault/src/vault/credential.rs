//! Credential lifecycle: store, issue with fee, revoke.

use crate::error::ContractError;
use crate::storage;
use crate::types::{FeeQuote, VCStatus, VerifiableCredential};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, IntoVal, String};

pub fn store_vc(e: &Env, id: String, data: String, issuance_contract: Address, issuer_did: String) {
    let new_vc = VerifiableCredential {
        id: id.clone(),
        data,
        issuance_contract,
        issuer_did,
    };
    storage::write_vault_vc(e, &id, &new_vc);
    storage::append_vc_to_index(e, &id);
}

/// Reads the central fee from the factory and, if enabled and > 0, transfers
/// `amount` from `issuer` to the configured destination. Returns the amount charged.
pub fn charge_fee(e: &Env, factory: &Address, issuer: &Address) -> i128 {
    let q: FeeQuote = e.invoke_contract(
        factory,
        &symbol_short!("quote_fee"),
        (issuer.clone(),).into_val(e),
    );
    if q.enabled && q.amount > 0 {
        let token = q.token.unwrap();
        let dest = q.dest.unwrap();
        e.invoke_contract::<()>(
            &token,
            &symbol_short!("transfer"),
            (issuer.clone(), dest, q.amount).into_val(e),
        );
    }
    q.amount
}

/// Returns (enabled, amount, token, dest) without transferring. token/dest are
/// only meaningful when enabled. Used by batch_issue to charge a single total.
pub fn charge_fee_quote_only(
    e: &Env,
    factory: &Address,
    issuer: &Address,
) -> (bool, i128, Address, Address) {
    let q: FeeQuote = e.invoke_contract(
        factory,
        &symbol_short!("quote_fee"),
        (issuer.clone(),).into_val(e),
    );
    if q.enabled {
        (true, q.amount, q.token.unwrap(), q.dest.unwrap())
    } else {
        // dummy addresses are never read because enabled is false
        (false, 0, issuer.clone(), issuer.clone())
    }
}

pub fn transfer_fee(e: &Env, token: &Address, dest: &Address, from: &Address, amount: i128) {
    e.invoke_contract::<()>(
        token,
        &symbol_short!("transfer"),
        (from.clone(), dest.clone(), amount).into_val(e),
    );
}

pub fn revoke_vc(e: &Env, vc_id: String, date: String) {
    let vc_status = storage::read_vc_status(e, &vc_id);
    if vc_status != VCStatus::Valid {
        panic_with_error!(e, ContractError::VCAlreadyRevoked)
    }
    storage::write_vc_status(e, &vc_id, &VCStatus::Revoked(date))
}
