#![no_std]
#![allow(dead_code)]

mod contract;
mod errors;
mod events;
mod storage;

#[cfg(test)]
mod test;

pub use contract::{
    VaultFactoryClient, VaultFactoryContract, VaultFactoryContractClient, VaultInitMeta,
};
pub use storage::{CustomFee, FeeQuote};
