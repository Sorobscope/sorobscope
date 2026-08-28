//! Phase 0 throwaway: proves we can actually reach Soroban RPC and decode a response
//! before any command logic exists. Delete this once Phase 1's `network.rs` lands and
//! the real commands exercise the same path.
//!
//! Run with: cargo run --example ping_testnet

use soroban_client::{Options, Server};

const TESTNET_RPC: &str = "https://soroban-testnet.stellar.org";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::new(TESTNET_RPC, Options::default())?;

    let ledger = server.get_latest_ledger().await?;

    println!("reached {TESTNET_RPC}");
    println!("  sequence         : {}", ledger.sequence);
    println!("  protocol version : {}", ledger.protocol_version);
    println!("  ledger hash      : {}", ledger.id);

    Ok(())
}
