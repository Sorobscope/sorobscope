# CLI Spec

This is a spec and reference skeleton, not final code. `soroban-client` is a younger, less API-stable crate than Stellar's official SDKs — verify exact method/type names against https://docs.rs/soroban-client for whatever version `cargo add` actually resolves, rather than trusting the signatures below character-for-character.

## Module structure

```
src/
├── main.rs              // entry point: parse Cli, dispatch to a command module
├── cli.rs                // Cli / Commands enum (clap derive)
├── network.rs            // testnet/futurenet/mainnet RPC URLs + passphrases, --rpc-url override, builds a configured Server
├── decode.rs             // ScVal -> readable String, and ScVal -> serde_json::Value (twin functions, one file)
└── commands/
    ├── mod.rs
    ├── events.rs
    ├── entry.rs
    └── tx.rs
tests/
└── integration_testnet.rs   // gated tests against real testnet — see ROADMAP Phase 6
```

## CLI shape

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "sorobscope", version, about = "Human-readable Soroban RPC inspection")]
struct Cli {
    #[arg(long, global = true, default_value = "testnet")]
    network: Network,

    /// Override the default RPC URL for --network
    #[arg(long, global = true)]
    rpc_url: Option<String>,

    /// Emit machine-readable JSON instead of formatted text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone)]
enum Network {
    Testnet,
    Futurenet,
    Mainnet,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch and decode contract events
    Events {
        contract_id: String,
        #[arg(long)] since_ledger: Option<u32>,
        #[arg(long)] follow: bool,
    },
    /// Read and decode one known storage entry
    Entry {
        contract_id: String,
        #[arg(long)] key_symbol: Option<String>,
        #[arg(long)] key_address: Option<String>,
        #[arg(long)] key_xdr: Option<String>,
    },
    /// Decode a transaction and the events it emitted
    Tx {
        hash: String,
    },
}
```

## `events` — sketch

```rust
use soroban_client::{Server, Pagination, EventFilter};
use soroban_client::soroban_rpc::EventType;

pub async fn run(
    server: &Server,
    contract_id: &str,
    since_ledger: Option<u32>,
    follow: bool,
    json: bool,
) -> anyhow::Result<()> {
    let start = match since_ledger {
        Some(l) => l,
        // default lookback window — tune this; RPC retention bounds how far back this can usefully go anyway
        None => server.get_latest_ledger().await?.sequence.saturating_sub(1000),
    };

    let filter = EventFilter::new(EventType::All).contract(contract_id);

    loop {
        let response = server
            .get_events(Pagination::From(start), vec![filter.clone()], 50)
            .await?;

        for event in response.events {
            if json {
                println!("{}", crate::decode::event_to_json(&event)?);
            } else {
                println!("{}", crate::decode::event_to_readable(&event)?);
            }
        }

        if !follow {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    Ok(())
}
```

## `entry` — sketch

Only the two simplest key shapes are sketched here (bare Symbol, bare Address) — genuinely composite/custom-typed keys (like the `DataKey::Worker(Address)`-style enum keys a real contract like the earlier reputation-app spec used) need either the raw `--key-xdr` escape hatch, or the schema-aware key builder flagged as a v2 issue in `ARCHITECTURE.md`.

```rust
use soroban_client::Server;
// exact key-construction API (LedgerKey::ContractData { .. } or equivalent) needs
// confirming against soroban-client's current docs.rs — sketch below is illustrative

pub async fn run(
    server: &Server,
    contract_id: &str,
    key_symbol: Option<String>,
    key_address: Option<String>,
    key_xdr: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let key = build_ledger_key(contract_id, key_symbol, key_address, key_xdr)?;
    let response = server.get_ledger_entries(vec![key]).await?;

    match response.entries.and_then(|mut e| e.pop()) {
        Some(entry) => {
            if json {
                println!("{}", crate::decode::entry_to_json(&entry)?);
            } else {
                println!("{}", crate::decode::entry_to_readable(&entry)?);
                // surface live_until_ledger_seq plainly here — see ROADMAP Phase 3
            }
        }
        None => println!("No entry found for that key."),
    }
    Ok(())
}
```

## `tx` — sketch

```rust
pub async fn run(server: &Server, hash: &str, json: bool) -> anyhow::Result<()> {
    match server.get_transaction(hash).await {
        Ok(tx) => {
            // decode: invoked contract id, function name, args, result, emitted events
            if json {
                println!("{}", crate::decode::tx_to_json(&tx)?);
            } else {
                println!("{}", crate::decode::tx_to_readable(&tx)?);
            }
        }
        Err(e) => {
            // distinguish "not found" from "outside retention window" here if the
            // error/response shape allows it — see ROADMAP Phase 4
            eprintln!("Couldn't fetch transaction {hash}: {e}");
        }
    }
    Ok(())
}
```

## `decode.rs` — the actual product

The one piece of logic every command depends on: turning an `ScVal` into something readable. Sketch of the shape, not a complete match arm list:

```rust
use soroban_client::xdr::ScVal;

pub fn scval_to_readable(v: &ScVal) -> String {
    match v {
        ScVal::Symbol(s) => format!("{s}"),
        ScVal::Address(a) => format!("{a}"),
        ScVal::U32(n) => n.to_string(),
        ScVal::U64(n) => n.to_string(),
        ScVal::I128(n) => format!("{n:?}"), // i128 parts need proper reconstruction — check current ScVal shape
        ScVal::Bytes(b) => format!("0x{}", hex_encode(b)),
        ScVal::Vec(Some(vec)) => format!("[{}]", vec.iter().map(scval_to_readable).collect::<Vec<_>>().join(", ")),
        ScVal::Map(Some(map)) => format!(
            "{{{}}}",
            map.iter()
                .map(|entry| format!("{}: {}", scval_to_readable(&entry.key), scval_to_readable(&entry.val)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => format!("{other:?}"), // fallback for anything not explicitly handled yet
    }
}

pub fn scval_to_json(v: &ScVal) -> serde_json::Value {
    // twin of the above, producing structured JSON instead of a display string
    todo!()
}
```

Exact `ScVal` variant names/shapes should be checked against whatever `stellar-xdr`/`soroban-client` version actually resolves — this enum has grown over time and the sketch above isn't guaranteed to be exhaustive or exactly current.

## Testing plan

- `decode.rs` functions: pure unit tests, one per `ScVal` variant handled, no network needed. These are the tests to write first since they don't depend on anything external.
- `commands/*`: integration-style tests behind a gate (env var like `SOROBSCOPE_TEST_NETWORK=testnet`, or a separate `cargo test` target) that hit real testnet against the contract chosen in Phase 0. Assert on decoded output shape, not exact byte-for-byte formatting, so minor formatting tweaks don't break the suite.
- Before Phase 6 is "done": run the tool against a contract you didn't write yourself and confirm nothing panics — even if the decoded output is less pretty for an unfamiliar contract shape, it should degrade gracefully (the `other => format!("{other:?}")` fallback above exists for exactly this).
