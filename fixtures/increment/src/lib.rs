#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct IncrementContract;

#[contractimpl]
impl IncrementContract {
    /// Increment the counter, emit an event, return the new value.
    pub fn increment(env: Env) -> u32 {
        let mut count: u32 = env.storage().persistent().get(&COUNTER).unwrap_or(0);
        count += 1;
        env.storage().persistent().set(&COUNTER, &count);
        env.storage().persistent().extend_ttl(&COUNTER, 100, 10_000);

        env.events()
            .publish((symbol_short!("counter"), symbol_short!("inc")), count);

        count
    }

    /// Read the counter without mutating it.
    pub fn get(env: Env) -> u32 {
        env.storage().persistent().get(&COUNTER).unwrap_or(0)
    }

    /// Emits an event carrying a richer payload: an Address topic plus a
    /// (Symbol, u32, Address) tuple as data. Exists so the decoder has
    /// something more interesting than a bare u32 to render.
    pub fn tag(env: Env, who: Address, note: Symbol) -> u32 {
        let count: u32 = env.storage().persistent().get(&COUNTER).unwrap_or(0);

        env.events()
            .publish((symbol_short!("counter"), symbol_short!("tag"), who.clone()), (note, count, who));

        count
    }
}
