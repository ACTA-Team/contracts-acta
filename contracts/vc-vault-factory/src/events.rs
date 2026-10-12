use soroban_sdk::{contractevent, Address, Env};

#[contractevent]
pub struct VaultDeployed {
    pub owner: Address,
    pub vault_address: Address,
}

#[contractevent]
pub struct SponsoredVaultDeployed {
    pub deployer: Address,
    pub owner: Address,
    pub vault_address: Address,
}

#[contractevent]
pub struct AdminNominated {
    pub current: Address,
    pub nominee: Address,
}

#[contractevent]
pub struct AdminTransferred {
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contractevent]
pub struct FeeConfigSet {
    pub token: Address,
    pub dest: Address,
    pub standard: i128,
}

#[contractevent]
pub struct FeeEnabledChanged {
    pub enabled: bool,
}

#[contractevent]
pub struct FeeStandardSet {
    pub amount: i128,
}

#[contractevent]
pub struct FeeCustomSet {
    pub issuer: Address,
    pub amount: i128,
    pub expires_at: Option<u64>,
}

#[contractevent]
pub struct FeeCustomRemoved {
    pub issuer: Address,
}

#[contractevent]
pub struct MinFeeSet {
    pub amount: i128,
}

pub fn fee_config_set(e: &Env, token: &Address, dest: &Address, standard: i128) {
    FeeConfigSet {
        token: token.clone(),
        dest: dest.clone(),
        standard,
    }
    .publish(e);
}
pub fn fee_enabled_changed(e: &Env, enabled: bool) {
    FeeEnabledChanged { enabled }.publish(e);
}
pub fn fee_standard_set(e: &Env, amount: i128) {
    FeeStandardSet { amount }.publish(e);
}
pub fn fee_custom_set(e: &Env, issuer: &Address, amount: i128, expires_at: Option<u64>) {
    FeeCustomSet {
        issuer: issuer.clone(),
        amount,
        expires_at,
    }
    .publish(e);
}
pub fn fee_custom_removed(e: &Env, issuer: &Address) {
    FeeCustomRemoved {
        issuer: issuer.clone(),
    }
    .publish(e);
}
pub fn min_fee_set(e: &Env, amount: i128) {
    MinFeeSet { amount }.publish(e);
}

pub fn admin_nominated(e: &Env, current: &Address, nominee: &Address) {
    AdminNominated {
        current: current.clone(),
        nominee: nominee.clone(),
    }
    .publish(e);
}

pub fn admin_transferred(e: &Env, old_admin: &Address, new_admin: &Address) {
    AdminTransferred {
        old_admin: old_admin.clone(),
        new_admin: new_admin.clone(),
    }
    .publish(e);
}

pub fn vault_deployed(e: &Env, owner: &Address, vault_address: &Address) {
    VaultDeployed {
        owner: owner.clone(),
        vault_address: vault_address.clone(),
    }
    .publish(e);
}

pub fn sponsored_vault_deployed(
    e: &Env,
    deployer: &Address,
    owner: &Address,
    vault_address: &Address,
) {
    SponsoredVaultDeployed {
        deployer: deployer.clone(),
        owner: owner.clone(),
        vault_address: vault_address.clone(),
    }
    .publish(e);
}
