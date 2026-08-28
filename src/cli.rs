use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "sorobscope",
    version,
    about = "Human-readable Soroban RPC inspection",
    long_about = "Read-only inspection of Soroban contracts: decoded events, decoded \
                  storage entries, and decoded transaction summaries.\n\n\
                  sorobscope never signs or submits a transaction and never needs a \
                  wallet or secret key."
)]
pub struct Cli {
    /// Network to query
    #[arg(long, global = true, default_value = "testnet")]
    pub network: Network,

    /// Override the default RPC URL for --network (e.g. a local or custom RPC instance)
    #[arg(long, global = true, value_name = "URL")]
    pub rpc_url: Option<String>,

    /// Emit machine-readable JSON instead of formatted text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Testnet,
    Futurenet,
    Mainnet,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Fetch and decode contract events
    #[command(long_about = "Fetch and decode the events a contract has emitted.\n\n\
                            Only reaches as far back as the RPC endpoint's retention \
                            window — commonly around a week, but it varies by endpoint \
                            and release. Nothing older than that is queryable.")]
    Events {
        /// Contract ID to fetch events for (C...)
        contract_id: String,

        /// Start from this ledger sequence instead of the default lookback window
        #[arg(long, value_name = "SEQ")]
        since_ledger: Option<u32>,

        /// Poll continuously for new events instead of exiting after one fetch
        #[arg(long)]
        follow: bool,
    },

    /// Read and decode ONE storage entry you already know the key for
    #[command(long_about = "Read and decode a single contract storage entry.\n\n\
                            This is not a storage dump. Soroban RPC's getLedgerEntries \
                            requires you to supply the specific key you want, and there \
                            is no RPC call that enumerates every entry a contract has \
                            written — so you must name the key via one of the --key-* \
                            flags. Use --key-xdr for key shapes the simple flags cannot \
                            express.")]
    Entry {
        /// Contract ID to read from (C...)
        contract_id: String,

        /// Key is a bare Symbol, e.g. --key-symbol COUNTER
        #[arg(long, value_name = "SYMBOL", group = "key")]
        key_symbol: Option<String>,

        /// Key is a bare Address, e.g. --key-address G...
        #[arg(long, value_name = "ADDRESS", group = "key")]
        key_address: Option<String>,

        /// Escape hatch: key as base64-encoded ScVal XDR, for composite or custom-typed keys
        #[arg(long, value_name = "BASE64", group = "key")]
        key_xdr: Option<String>,
    },

    /// Decode a transaction and the events it emitted
    #[command(long_about = "Decode a transaction: the contract invoked, the function \
                            called, its arguments and result, and any events emitted.\n\n\
                            Subject to the same RPC retention window as `events` — a \
                            transaction older than the endpoint retains cannot be \
                            fetched, even though it really happened.")]
    Tx {
        /// Transaction hash (64-character hex)
        hash: String,
    },
}
