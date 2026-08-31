mod cli;
mod commands;
mod decode;
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

    match args.command {
        Commands::Events {
            contract_id,
            since_ledger,
            follow,
        } => {
            commands::events::run(&connection, &contract_id, since_ledger, follow, args.json)
                .await?;
        }

        Commands::Entry {
            contract_id,
            key_symbol,
            key_address,
            key_xdr,
        } => {
            commands::entry::run(
                &connection,
                &contract_id,
                key_symbol,
                key_address,
                key_xdr,
                args.json,
            )
            .await?;
        }

        // Phase 4. Still opens a real connection above, so its error paths are exercised
        // even though the command body isn't written yet.
        Commands::Tx { hash } => {
            println!("not implemented: tx {hash}");
        }
    }

    Ok(())
}
