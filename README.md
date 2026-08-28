# sorobscope

**Human-readable Soroban RPC inspection.** A read-only Rust CLI that decodes contract
events, storage entries, and transaction summaries — instead of hand-parsing base64 XDR or
piping `stellar-cli` JSON through `jq` every time.

Readable output by default, `--json` on every command for piping into other tooling.

> ### Status: scaffolding — commands are not implemented yet
>
> Phases 0–1 of [`docs/ROADMAP.md`](docs/ROADMAP.md) are complete: the CLI parses,
> `--network` and `--rpc-url` work, the RPC client reaches real testnet, failures report
> themselves clearly, and there's a deployed contract to test against. But `events`,
> `entry`, and `tx` currently print `not implemented`. The decoding — which is the actual
> product — lands in Phases 2–4.
>
> The examples below are the intended interface, **not** working transcripts. Real
> terminal output replaces them in Phase 7.

## Why this exists

Soroban RPC answers in base64-encoded XDR. Reading a contract's events or storage means
either wiring up an SDK or piping `stellar-cli` output through `jq` and decoding by hand.
`sorobscope` does the decoding step and prints the result.

That decoding is the point. A thin wrapper that pretty-prints raw JSON wouldn't be worth
installing — see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Install

Requires a Rust toolchain. No wasm target or contract tooling needed — this is a plain
native binary.

```sh
git clone <this-repo> && cd Sorobscope
cargo install --path .
```

## Commands

```sh
# Decode the events a contract has emitted
sorobscope events <CONTRACT_ID> [--since-ledger N] [--follow]

# Read and decode ONE storage entry you already know the key for
sorobscope entry <CONTRACT_ID> --key-symbol COUNTER
sorobscope entry <CONTRACT_ID> --key-address G...
sorobscope entry <CONTRACT_ID> --key-xdr <base64>   # escape hatch for composite keys

# Decode a transaction and the events it emitted
sorobscope tx <HASH>
```

Global flags: `--network <testnet|futurenet|mainnet>` (default `testnet`),
`--rpc-url <URL>` to point at a custom or local RPC, and `--json`.

## Scope, and two honest limits

`sorobscope` **never signs or submits a transaction** and never needs a wallet or secret
key, on any network. Writes are `stellar-cli`'s job. This is a deliberate boundary, not a
missing feature.

Two things it cannot do, because Soroban RPC itself cannot:

- **No full storage enumeration.** `getLedgerEntries` requires you to already know the
  ledger key you want, so `entry` reads one known key at a time. There is no "dump every
  entry this contract has written" RPC call to build on — this is a large part of why
  third-party indexers exist in the Stellar ecosystem.
- **Bounded history.** `getEvents` and `getTransaction` serve only a recent retention
  window (commonly around a week, but it varies by endpoint and release — check an
  endpoint's `getHealth` rather than trusting a fixed number). Neither command can reach
  further back than the endpoint currently retains.

Both are properties of the RPC surface, not gaps waiting to be quietly closed. Reaching
past them would mean integrating a historical indexer, which is tracked as future work
rather than implied by the help text.

## Testing against a real contract

A fixture contract is deployed on testnet and ready to point at:

```
CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX
```

It has a `COUNTER` entry in persistent storage and emits events with both simple and
composite payloads. Source and deploy instructions:
[`fixtures/increment/`](fixtures/increment/README.md).

## Docs

| Doc | What's in it |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | Project context, tech stack, ground rules, current status |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Phased build plan |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Design principles, data flow, RPC limitations, non-goals |
| [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) | Command reference and module structure |

Built as a Dev Tooling submission for the
[Stellar Wave Program](https://www.drips.network/wave/stellar).
