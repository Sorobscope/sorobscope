# sorobscope

**Human-readable Soroban RPC inspection.** A read-only Rust CLI that decodes contract
events, storage entries, and transaction summaries — instead of hand-parsing base64 XDR or
piping `stellar-cli` JSON through `jq` every time.

Readable output by default, `--json` on every command for piping into other tooling.

> ### Status: `events` works; `entry` and `tx` do not yet
>
> `events` works and decodes real testnet events. `entry` and `tx` still print
> `not implemented` — they land in Phases 3–4 of [`docs/ROADMAP.md`](docs/ROADMAP.md).
> The `events` transcript below is real output. The `entry` and `tx` lines show the
> intended interface only.

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

### `events` — working

```console
$ sorobscope events CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX
events for CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX on testnet
searching from ledger 4412479 to 4429759 (last ~17280 ledgers; pass --since-ledger to widen)

ledger 4429704  2026-08-31T10:28:27Z
  topics  [counter, inc]
  data    4
  tx      3e27afda4981783376f7c9160e6aa54a8b6a1cff7c1f9acd207ddf3b4da410a0

ledger 4429706  2026-08-31T10:28:37Z
  topics  [counter, tag, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]
  data    [live, 4, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]
  tx      57cc3d10f015741ced4ee558c3f36cf3cb7e6253f2c32d35982c8e493c849ab3
```

`--follow` polls every 5s and streams new events until Ctrl-C. `--since-ledger N` widens
the search past the default ~1 day window.

With `--json`, events are emitted one JSON object per line, so `--follow` pipes straight
into `jq` without buffering an array that never ends:

```console
$ sorobscope events CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX --json | jq -c '{ledger, topics, data}'
{"ledger":4429704,"topics":["counter","inc"],"data":4}
{"ledger":4429706,"topics":["counter","tag","GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB"],"data":["live",4,"GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB"]}
```

### `entry` and `tx` — not implemented yet

```sh
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
