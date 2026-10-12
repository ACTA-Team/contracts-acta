use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Symbol};

use crate::errors::FactoryError;

const ONE_DAY_LEDGERS: u32 = 17_280; // ~5s per ledger

const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30;
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS;

const LEDGER_THRESHOLD_CONTRACTS: u32 = ONE_DAY_LEDGERS * 100;
const LEDGER_BUMP_CONTRACTS: u32 = LEDGER_THRESHOLD_CONTRACTS + ONE_DAY_LEDGERS * 20;

/// Maximum accepted fee amount in token base units (10^18).
pub const MAX_FEE_AMOUNT: i128 = 1_000_000_000_000_000_000;

/// Per-issuer custom fee with optional expiry (unix timestamp seconds).
/// `expires_at == None` => permanent. Expired customs fall back to standard.
#[derive(Clone)]
#[contracttype]
pub struct CustomFee {
    pub amount: i128,
    pub expires_at: Option<u64>,
}

/// Fee quote returned to a vault at issuance time.
#[derive(Clone)]
#[contracttype]
pub struct FeeQuote {
    pub enabled: bool,
    pub amount: i128,
    pub token: Option<Address>,
    pub dest: Option<Address>,
}

#[derive(Clone)]
#[contracttype]
pub enum VaultFactoryDataKey {
    Contracts(Address),
    FeeCustom(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct VaultInitMeta {
    pub vault_hash: BytesN<32>,
    pub contract_admin: Address,
}

pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

pub fn set_vault_init_meta(e: &Env, meta: &VaultInitMeta) {
    e.storage()
        .instance()
        .set::<Symbol, VaultInitMeta>(&Symbol::new(e, "VaultMeta"), meta);
}

pub fn get_vault_init_meta(e: &Env) -> VaultInitMeta {
    e.storage()
        .instance()
        .get::<Symbol, VaultInitMeta>(&Symbol::new(e, "VaultMeta"))
        .unwrap_or_else(|| panic_with_error!(e, FactoryError::NotInitialized))
}

pub fn set_deployed(e: &Env, vault_address: &Address) {
    let key = VaultFactoryDataKey::Contracts(vault_address.clone());
    e.storage()
        .persistent()
        .set::<VaultFactoryDataKey, bool>(&key, &true);
    e.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD_CONTRACTS, LEDGER_BUMP_CONTRACTS);
}

pub fn is_deployed(e: &Env, vault_address: &Address) -> bool {
    let key = VaultFactoryDataKey::Contracts(vault_address.clone());
    if let Some(result) = e
        .storage()
        .persistent()
        .get::<VaultFactoryDataKey, bool>(&key)
    {
        e.storage().persistent().extend_ttl(
            &key,
            LEDGER_THRESHOLD_CONTRACTS,
            LEDGER_BUMP_CONTRACTS,
        );
        result
    } else {
        false
    }
}

const ADMIN_KEY: &str = "Admin";
const PENDING_ADMIN_KEY: &str = "PendAdmF";
const FEE_ENABLED_KEY: &str = "FeeEnab";
const FEE_TOKEN_KEY: &str = "FeeToken";
const FEE_DEST_KEY: &str = "FeeDest";
const FEE_STANDARD_KEY: &str = "FeeStd";
const MIN_FEE_KEY: &str = "MinFee";

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage()
        .instance()
        .set(&Symbol::new(e, ADMIN_KEY), admin);
}
pub fn get_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&Symbol::new(e, ADMIN_KEY))
        .unwrap()
}
pub fn set_pending_admin(e: &Env, admin: &Address) {
    e.storage()
        .instance()
        .set(&Symbol::new(e, PENDING_ADMIN_KEY), admin);
}
pub fn get_pending_admin(e: &Env) -> Option<Address> {
    e.storage()
        .instance()
        .get(&Symbol::new(e, PENDING_ADMIN_KEY))
}
pub fn remove_pending_admin(e: &Env) {
    e.storage()
        .instance()
        .remove(&Symbol::new(e, PENDING_ADMIN_KEY));
}

pub fn read_fee_enabled(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&Symbol::new(e, FEE_ENABLED_KEY))
        .unwrap_or(false)
}
pub fn write_fee_enabled(e: &Env, v: bool) {
    e.storage()
        .instance()
        .set(&Symbol::new(e, FEE_ENABLED_KEY), &v);
}
pub fn try_read_fee_token(e: &Env) -> Option<Address> {
    e.storage().instance().get(&Symbol::new(e, FEE_TOKEN_KEY))
}
pub fn write_fee_token(e: &Env, a: &Address) {
    e.storage()
        .instance()
        .set(&Symbol::new(e, FEE_TOKEN_KEY), a);
}
pub fn try_read_fee_dest(e: &Env) -> Option<Address> {
    e.storage().instance().get(&Symbol::new(e, FEE_DEST_KEY))
}
pub fn write_fee_dest(e: &Env, a: &Address) {
    e.storage().instance().set(&Symbol::new(e, FEE_DEST_KEY), a);
}
pub fn try_read_fee_standard(e: &Env) -> Option<i128> {
    e.storage()
        .instance()
        .get(&Symbol::new(e, FEE_STANDARD_KEY))
}
pub fn write_fee_standard(e: &Env, v: i128) {
    e.storage()
        .instance()
        .set(&Symbol::new(e, FEE_STANDARD_KEY), &v);
}
pub fn read_min_fee(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&Symbol::new(e, MIN_FEE_KEY))
        .unwrap_or(0)
}
pub fn write_min_fee(e: &Env, v: i128) {
    e.storage().instance().set(&Symbol::new(e, MIN_FEE_KEY), &v);
}

pub fn read_fee_custom(e: &Env, issuer: &Address) -> Option<CustomFee> {
    let key = VaultFactoryDataKey::FeeCustom(issuer.clone());
    let v = e.storage().persistent().get::<_, CustomFee>(&key);
    if v.is_some() {
        e.storage().persistent().extend_ttl(
            &key,
            LEDGER_THRESHOLD_CONTRACTS,
            LEDGER_BUMP_CONTRACTS,
        );
    }
    v
}
pub fn write_fee_custom(e: &Env, issuer: &Address, fee: &CustomFee) {
    let key = VaultFactoryDataKey::FeeCustom(issuer.clone());
    e.storage().persistent().set(&key, fee);
    e.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD_CONTRACTS, LEDGER_BUMP_CONTRACTS);
}
pub fn remove_fee_custom(e: &Env, issuer: &Address) {
    e.storage()
        .persistent()
        .remove(&VaultFactoryDataKey::FeeCustom(issuer.clone()));
}
