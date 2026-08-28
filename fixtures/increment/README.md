# `increment` — testnet fixture contract

A small Soroban contract that exists purely to give `sorobscope` something real to read.
**It is not part of the `sorobscope` binary** and is not built by `cargo build` at the repo
root — it's a separate crate, compiled to wasm only when you want to deploy your own copy.

## Currently deployed

| | |
|---|---|
| Contract ID | `CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX` |
| Network | testnet |

You don't need to deploy anything to use that one — it's public and read-only from
`sorobscope`'s point of view. Deploy your own only if you want a target you control, or if
the one above has aged out of the RPC's retention window.

## How it differs from `soroban-examples`' `increment`, and why

Two deliberate changes. Both exist because the stock example can't exercise the paths
`sorobscope` is built to test:

1. **Persistent storage, not instance storage.** The stock example keeps its counter in
   instance storage, where it lives *inside* the contract instance entry and so isn't
   addressable by a bare Symbol ledger key. `sorobscope entry --key-symbol COUNTER` would
   find nothing. Using `persistent()` makes `COUNTER` a standalone `ContractData` entry,
   and `extend_ttl` gives it a real `liveUntilLedgerSeq` for the `entry` command to surface.
2. **It emits events.** The stock example publishes none, which would leave the `events`
   command with nothing to decode. `increment` emits a 2-topic event with a `u32` payload;
   `tag` emits a 3-topic event including an `Address` topic and a `(Symbol, u32, Address)`
   tuple payload, so the decoder gets something more interesting than bare integers.

## Functions

| Function | Emits | Notes |
|---|---|---|
| `increment() -> u32` | 2-topic event, `u32` payload | Bumps and returns the counter |
| `get() -> u32` | — | Reads the counter without mutating |
| `tag(who: Address, note: Symbol) -> u32` | 3-topic event, tuple payload | Richer shapes for the decoder |

## Deploying your own

Needs the `wasm32v1-none` target (`rustup target add wasm32v1-none`). The root
`sorobscope` binary does **not** need it — only this contract does.

```sh
cd fixtures/increment
stellar contract build

stellar keys generate my-dev --network testnet --fund

stellar contract deploy \
  --wasm target/wasm32v1-none/release/increment.wasm \
  --source my-dev --network testnet
```

Then generate some activity to read:

```sh
C=<contract-id-from-deploy>
stellar contract invoke --id $C --source my-dev --network testnet -- increment
stellar contract invoke --id $C --source my-dev --network testnet -- \
  tag --who $(stellar keys address my-dev) --note hello
```

> Built against `soroban-sdk` 27. Note that SDK 27 deprecates `events().publish()` in
> favour of the `#[contractevent]` macro; the deprecated call still emits ordinary events,
> so it works fine here, but that's the modern API if this contract is ever revised.
