# sorobscope

**Human-readable Soroban RPC inspection.** A read-only Rust CLI that decodes contract
events, storage entries, and transaction summaries — instead of hand-parsing base64 XDR or
piping `stellar-cli` JSON through `jq` every time.

Readable output by default, `--json` on every command for piping into other tooling.

> ### Status: all three commands work
>
> `events`, `entry`, and `tx` all work against real testnet data, and every transcript
> below is real captured output. What remains is polish, wider testing, and packaging —
> Phases 5–7 of [`docs/ROADMAP.md`](docs/ROADMAP.md).

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
searching from ledger 4422004 to 4439284 (last ~17280 ledgers; pass --since-ledger to widen)

ledger 4429704  2026-08-31T10:28:27Z
  topics      [counter, inc]
  data        4
  tx          3e27afda4981783376f7c9160e6aa54a8b6a1cff7c1f9acd207ddf3b4da410a0

ledger 4429706  2026-08-31T10:28:37Z
  topics      [counter, tag, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]
  data        [live, 4, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]
  tx          57cc3d10f015741ced4ee558c3f36cf3cb7e6253f2c32d35982c8e493c849ab3
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

### `entry` — working

Reads **one** entry whose key you supply. There is no storage-dump mode, because the RPC
has no call that enumerates a contract's storage.

```console
$ sorobscope entry CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX --key-symbol COUNTER
entry for CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX on testnet
  key         COUNTER
  durability  persistent
  value       4
  updated     ledger 4429704
  live until  ledger 4483568  (44283 ledgers away, ~2d 13h)
  as of       ledger 4439285
```

```console
$ sorobscope entry CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX --key-symbol COUNTER --json
{"contractId":"CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX","durability":"persistent","key":"COUNTER","lastModifiedLedger":4429704,"latestLedger":4439285,"ledgersRemaining":44283,"liveUntilLedger":4483568,"network":"testnet","value":4}
```

The TTL line is the part raw tooling makes you work out for yourself: an entry about to
expire looks identical to a healthy one until someone subtracts the current ledger from
`liveUntilLedgerSeq`.

Key shapes:

```sh
sorobscope entry <CONTRACT_ID> --key-symbol COUNTER   # a bare Symbol
sorobscope entry <CONTRACT_ID> --key-address G...     # a bare Address
sorobscope entry <CONTRACT_ID> --key-xdr <base64>     # any other shape, as raw ScVal XDR
```

Durability isn't a flag: persistent and temporary storage are queried together in one
request, and the contract's instance storage is checked too, so you don't have to know
which one a contract used before you can read from it.

### `tx` — working

```console
$ sorobscope tx 57cc3d10f015741ced4ee558c3f36cf3cb7e6253f2c32d35982c8e493c849ab3
transaction 57cc3d10f015741ced4ee558c3f36cf3cb7e6253f2c32d35982c8e493c849ab3 on testnet
  status      SUCCESS
  ledger      4429706
  at          2026-08-31T10:28:37Z

  contract    CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX
  function    tag
  args        GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB
              live
  returned    4

  events      1
              [counter, tag, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]  [live, 4, GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB]
```

The contract, function name, and arguments aren't fields the RPC hands back — they live
inside the transaction envelope XDR, which is most of what this command does for you.

A transaction the endpoint doesn't have reports the retained window, because Soroban RPC
cannot tell "no such transaction" apart from "older than I keep":

```console
$ sorobscope tx 0000000000000000000000000000000000000000000000000000000000000000
Transaction 0000000000000000000000000000000000000000000000000000000000000000 was not found.

This endpoint currently retains ledgers 4318327 to 4439286.

Soroban RPC reports both "no such transaction" and "older than this endpoint retains" the same way, so this could be either. If the transaction closed before ledger 4318327, it is outside the window and no longer queryable here — point --rpc-url at an endpoint with deeper history, or use an indexer.
```

## Exit codes

Chosen so a script can tell a failed lookup from a broken tool:

| Code | Meaning |
|---|---|
| `0` | The query succeeded |
| `2` | The query succeeded, but the entry or transaction does not exist |
| `1` | Something went wrong — unreachable endpoint, rejected request, undecodable response |

`events` returning no matches is exit `0`: the range was scanned and the contract was
simply quiet, which is a real answer rather than a missing identifier.

## Global flags

`--network <testnet|futurenet|mainnet>` (default `testnet`), `--rpc-url <URL>` to point at
a custom or local RPC, `--json`, and `--color <auto|always|never>`.

Colour is used only when writing to a terminal, so piped and redirected output stays clean.
`NO_COLOR` disables it; an explicit `--color always` overrides both, which is what you want
for `| less -R`.

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
