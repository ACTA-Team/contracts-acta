use soroban_sdk::{
    contract, contractclient, contractimpl, xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal,
    String,
};

pub use crate::storage::VaultInitMeta;
use crate::{events, storage};

#[contract]
pub struct VaultFactoryContract;

#[contractclient(name = "VaultFactoryClient")]
pub trait VaultFactory {
    /// Deploy a new single-tenant vault for `owner` and register it in the factory.
    ///
    /// `user_salt` is mixed with `owner` via keccak256 so each (owner, salt) pair
    /// produces a unique deterministic address and frontrunning is prevented.
    fn deploy(e: Env, owner: Address, did_uri: String, user_salt: BytesN<32>) -> Address;

    /// Deploy a vault on behalf of `owner`. `deployer` signs and pays; the vault
    /// belongs to `owner` from creation. Anyone can be a deployer - no whitelist.
    fn deploy_sponsored(
        e: Env,
        deployer: Address,
        owner: Address,
        did_uri: String,
        user_salt: BytesN<32>,
    ) -> Address;

    /// Return true if `vault_address` was deployed by this factory.
    fn is_vault(e: Env, vault_address: Address) -> bool;
}

#[contractimpl]
impl VaultFactoryContract {
    pub fn __constructor(e: Env, vault_init_meta: VaultInitMeta) {
        storage::set_vault_init_meta(&e, &vault_init_meta);
        storage::set_admin(&e, &vault_init_meta.contract_admin);
    }

    pub fn nominate_admin(e: Env, new_admin: Address) {
        let admin = storage::get_admin(&e);
        admin.require_auth();
        storage::set_pending_admin(&e, &new_admin);
        storage::extend_instance(&e);
        events::admin_nominated(&e, &admin, &new_admin);
    }

    pub fn accept_admin(e: Env) {
        let pending = match storage::get_pending_admin(&e) {
            Some(a) => a,
            None => soroban_sdk::panic_with_error!(e, crate::errors::FactoryError::NoPendingAdmin),
        };
        pending.require_auth();
        let old = storage::get_admin(&e);
        storage::set_admin(&e, &pending);
        storage::remove_pending_admin(&e);
        storage::extend_instance(&e);
        events::admin_transferred(&e, &old, &pending);
    }

    pub fn get_admin(e: Env) -> Address {
        storage::get_admin(&e)
    }

    fn require_admin(e: &Env) {
        storage::get_admin(e).require_auth();
        storage::extend_instance(e);
    }

    fn validate_amount(e: &Env, amount: i128) {
        use crate::errors::FactoryError;
        if amount < 0 {
            soroban_sdk::panic_with_error!(e, FactoryError::InvalidFeeAmount);
        }
        if amount > storage::MAX_FEE_AMOUNT {
            soroban_sdk::panic_with_error!(e, FactoryError::FeeOutOfBounds);
        }
        if amount < storage::read_min_fee(e) {
            soroban_sdk::panic_with_error!(e, FactoryError::FeeBelowMin);
        }
    }

    pub fn set_fee_config(e: Env, token: Address, dest: Address, standard: i128) {
        Self::require_admin(&e);
        Self::validate_amount(&e, standard);
        storage::write_fee_token(&e, &token);
        storage::write_fee_dest(&e, &dest);
        storage::write_fee_standard(&e, standard);
        events::fee_config_set(&e, &token, &dest, standard);
    }

    pub fn set_fee_enabled(e: Env, enabled: bool) {
        Self::require_admin(&e);
        if enabled {
            let configured = storage::try_read_fee_token(&e).is_some()
                && storage::try_read_fee_dest(&e).is_some()
                && storage::try_read_fee_standard(&e).is_some();
            if !configured {
                soroban_sdk::panic_with_error!(e, crate::errors::FactoryError::FeeNotConfigured);
            }
        }
        storage::write_fee_enabled(&e, enabled);
        events::fee_enabled_changed(&e, enabled);
    }

    pub fn set_fee_standard(e: Env, amount: i128) {
        Self::require_admin(&e);
        Self::validate_amount(&e, amount);
        storage::write_fee_standard(&e, amount);
        events::fee_standard_set(&e, amount);
    }

    pub fn set_fee_custom(e: Env, issuer: Address, amount: i128, expires_at: Option<u64>) {
        Self::require_admin(&e);
        Self::validate_amount(&e, amount);
        if let Some(exp) = expires_at {
            if exp <= e.ledger().timestamp() {
                soroban_sdk::panic_with_error!(e, crate::errors::FactoryError::ExpiryInPast);
            }
        }
        storage::write_fee_custom(&e, &issuer, &storage::CustomFee { amount, expires_at });
        events::fee_custom_set(&e, &issuer, amount, expires_at);
    }

    pub fn remove_fee_custom(e: Env, issuer: Address) {
        Self::require_admin(&e);
        storage::remove_fee_custom(&e, &issuer);
        events::fee_custom_removed(&e, &issuer);
    }

    pub fn quote_fee(e: Env, issuer: Address) -> storage::FeeQuote {
        storage::extend_instance(&e);
        if !storage::read_fee_enabled(&e) {
            return storage::FeeQuote {
                enabled: false,
                amount: 0,
                token: None,
                dest: None,
            };
        }
        let standard = storage::try_read_fee_standard(&e).unwrap_or(0);
        let amount = match storage::read_fee_custom(&e, &issuer) {
            Some(c) => {
                let valid = match c.expires_at {
                    Some(exp) => e.ledger().timestamp() <= exp,
                    None => true,
                };
                if valid {
                    c.amount
                } else {
                    standard
                }
            }
            None => standard,
        };
        storage::FeeQuote {
            enabled: true,
            amount,
            token: storage::try_read_fee_token(&e),
            dest: storage::try_read_fee_dest(&e),
        }
    }

    pub fn set_min_fee(e: Env, amount: i128) {
        Self::require_admin(&e);
        use crate::errors::FactoryError;
        if amount < 0 {
            soroban_sdk::panic_with_error!(e, FactoryError::InvalidFeeAmount);
        }
        if amount > storage::MAX_FEE_AMOUNT {
            soroban_sdk::panic_with_error!(e, FactoryError::FeeOutOfBounds);
        }
        storage::write_min_fee(&e, amount);
        events::min_fee_set(&e, amount);
    }
}

fn derive_salt(e: &Env, user_salt: BytesN<32>, owner: &Address) -> BytesN<32> {
    // deploy_salt = keccak256( user_salt(32 bytes) || XDR(owner) )
    //
    // Mixing the owner into the salt binds the deterministic vault address to a
    // specific owner (so a sponsored deploy lands on the same address the owner
    // would compute, and two owners can't collide on one address). The owner is
    // serialized via its canonical XDR form rather than to_string(): XDR is the
    // stable wire encoding an off-chain client reproduces directly, and it
    // matches the preimage documented in the README. to_string() is a display
    // (StrKey) encoding and ties determinism to a fixed 56-byte assumption.
    let mut preimage: Bytes = user_salt.into_val(e);
    preimage.append(&owner.clone().to_xdr(e));
    e.crypto().keccak256(&preimage).into()
}

fn deploy_vault(e: &Env, owner: &Address, did_uri: String, user_salt: BytesN<32>) -> Address {
    let meta = storage::get_vault_init_meta(e);
    let new_salt = derive_salt(e, user_salt, owner);
    let factory_address = e.current_contract_address();
    let vault_address = e.deployer().with_current_contract(new_salt).deploy_v2(
        meta.vault_hash,
        (owner.clone(), meta.contract_admin, did_uri, factory_address),
    );
    storage::set_deployed(e, &vault_address);
    vault_address
}

#[contractimpl]
impl VaultFactory for VaultFactoryContract {
    fn deploy(e: Env, owner: Address, did_uri: String, user_salt: BytesN<32>) -> Address {
        owner.require_auth();
        storage::extend_instance(&e);
        let vault_address = deploy_vault(&e, &owner, did_uri, user_salt);
        events::vault_deployed(&e, &owner, &vault_address);
        vault_address
    }

    fn deploy_sponsored(
        e: Env,
        deployer: Address,
        owner: Address,
        did_uri: String,
        user_salt: BytesN<32>,
    ) -> Address {
        deployer.require_auth();
        storage::extend_instance(&e);
        let vault_address = deploy_vault(&e, &owner, did_uri, user_salt);
        events::sponsored_vault_deployed(&e, &deployer, &owner, &vault_address);
        vault_address
    }

    fn is_vault(e: Env, vault_address: Address) -> bool {
        storage::extend_instance(&e);
        storage::is_deployed(&e, &vault_address)
    }
}
