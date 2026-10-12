//! VC Vault Contract
//!
//! Soroban contract for Verifiable Credential storage and issuance status registry.
//! Provides per-owner vaults, issuer authorization, and VC lifecycle (issue, verify, revoke).

#![no_std]
#![allow(dead_code)]

pub mod constants;
pub mod contract;
mod error;
mod events;
mod interface;
mod storage;
pub mod types;
mod validator;
mod vault;

#[cfg(test)]
mod test;
