mod cli;
mod commands;
mod decode;
mod error;
mod network;
mod outcome;
mod style;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::network::Connection;
use crate::outcome::Outcome;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    // Decided once, before anything prints.
    style::init(args.color);

    match run(args).await {
        Ok(Outcome::Found) => ExitCode::SUCCESS,
        // Distinct from both success and failure: the query worked, the thing isn't there.
        Ok(Outcome::NotFound) => ExitCode::from(2),
        Err(e) => {
            // Errors go to stderr so `--json` output on stdout stays pipeable even when
            // a run fails partway.
            eprintln!("{} {e}", style::error_prefix());
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Cli) -> anyhow::Result<Outcome> {
    // One shared constructor for every command — see network::Connection.
    let connection = Connection::open(args.network, args.rpc_url.as_deref())?;

    match args.command {
        Commands::Events {
            contract_id,
            since_ledger,
            follow,
        } => {
            commands::events::run(&connection, &contract_id, since_ledger, follow, args.json)
                .await
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
            .await
        }

        Commands::Tx { hash } => commands::tx::run(&connection, &hash, args.json).await,
    }
}
