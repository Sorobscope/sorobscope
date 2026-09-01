#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

/// Deliberately the *stock* `soroban-examples` increment contract, storing its counter in
/// **instance** storage.
///
/// The sibling `increment` fixture was reshaped to use persistent storage so that
/// `entry --key-symbol COUNTER` had a standalone ledger entry to read. This one exists to
/// cover the opposite case, which is what most real contracts look like: the value lives
/// *inside* the contract instance entry, so a bare Symbol ledger key finds nothing and
/// `entry` has to fall back to reading the instance's storage map.
#[contract]
pub struct InstanceCounter;

#[contractimpl]
impl InstanceCounter {
    pub fn increment(env: Env) -> u32 {
        let mut count: u32 = env.storage().instance().get(&COUNTER).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&COUNTER, &count);
        env.storage().instance().extend_ttl(100, 10_000);
        count
    }

    pub fn get(env: Env) -> u32 {
        env.storage().instance().get(&COUNTER).unwrap_or(0)
    }
}
