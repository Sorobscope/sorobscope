mod cli;
mod error;
mod network;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::network::Connection;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Errors go to stderr so `--json` output on stdout stays pipeable even when
            // a run fails partway.
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Cli) -> anyhow::Result<()> {
    // One shared constructor for every command — see network::Connection.
    let connection = Connection::open(args.network, args.rpc_url.as_deref())?;

    // Phase 1: no command has real logic yet, but each one does make a live call, so the
    // error paths in error.rs are exercised for real rather than sitting untested. Phases
    // 2-4 replace this probe with getEvents / getLedgerEntries / getTransaction.
    let ledger = connection
        .server
        .get_latest_ledger()
        .await
        .map_err(|e| connection.fail(e))?;

    println!(
        "connected to {} at {}",
        connection.network.name(),
        connection.url
    );
    println!("  latest ledger : {}", ledger.sequence);
    println!("  protocol      : {}", ledger.protocol_version);
    println!();

    match args.command {
        Commands::Events { contract_id, .. } => {
            println!("not implemented: events {contract_id}");
        }
        Commands::Entry { contract_id, .. } => {
            println!("not implemented: entry {contract_id}");
        }
        Commands::Tx { hash } => {
            println!("not implemented: tx {hash}");
        }
    }

    Ok(())
}
