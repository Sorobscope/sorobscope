# `instance-counter` — instance-storage fixture

Deliberately the **stock** `soroban-examples` increment contract: it keeps its counter in
**instance** storage. It exists to cover the case its sibling [`increment`](../increment/README.md)
cannot.

| | |
|---|---|
| Contract ID | `CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY` |
| Network | testnet |

## Why both fixtures exist

`increment` was reshaped to use *persistent* storage so `entry --key-symbol COUNTER` had a
standalone ledger entry to read. That covers the direct path — but it isn't what most real
contracts look like.

This one is the common shape. A value written with `env.storage().instance()` lives *inside*
the contract instance entry, so a bare Symbol ledger key finds nothing at all:

```console
$ stellar contract read --id CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY --key COUNTER --durability persistent --network testnet
❌ error: no matching contract data entries were found for the specified contract id
```

`sorobscope` checks the instance's storage map as well, so the same lookup works and says
where the value actually lives:

```console
$ sorobscope entry CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY --key-symbol COUNTER
entry for CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY on testnet
  key         COUNTER
  durability  instance
  value       2
  live until  (tied to the contract instance's own TTL)
  as of       ledger 4439447

Found in the contract's instance storage, not as a standalone entry. Values written with env.storage().instance() live inside the contract instance and share its TTL.
```

## Deploying your own

```sh
cd fixtures/instance-counter
stellar contract build
stellar contract deploy --wasm target/wasm32v1-none/release/instance_counter.wasm \
  --source my-dev --network testnet
stellar contract invoke --id <id> --source my-dev --network testnet -- increment
```
