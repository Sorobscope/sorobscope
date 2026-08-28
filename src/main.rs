mod cli;

use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() {
    let args = Cli::parse();

    // Phase 0: every subcommand is a stub. Real implementations land in Phases 2-4;
    // the shared RPC client they all depend on lands in Phase 1.
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
}
