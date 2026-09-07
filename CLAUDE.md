# sorobscope — Project Context for Claude Code

> Working name: **sorobscope** (Soroban + scope). Rename freely — if you do, update this header and `Cargo.toml`'s `name` field together.

## What this is

A Rust command-line tool that makes **Soroban RPC's raw output human-readable and scriptable** — decoded contract events, decoded storage entry reads, and decoded transaction summaries, instead of hand-parsing base64 XDR or piping `stellar-cli` JSON through `jq` every time. It's being built as a repo submission to the **Stellar Wave Program** (Drips × Stellar Development Foundation): https://www.drips.network/wave/stellar

## Why this project, why this category, why a CLI

This started as a mobile reputation-tracking app idea, but moved to a **Dev Tooling / Infrastructure** project instead, for two reasons that were explicit design decisions, not defaults:

1. **Category gap.** Of the approved Stellar Wave repos surveyed, escrow and payments had 5+ entries each; identity/reputation had none but was still a product category. Dev tooling had almost none either (`tansu` was the only clear example) — and a good tool compounds in value across every future contributor, not just users of one app.
2. **Practicability over platform.** A native mobile app needs a simulator/device, App Store-shaped review concerns, and a UI for every flow. A CLI needs `cargo build` and a terminal. It's genuinely faster to get to something real, easier to test in CI, and trivially distributable (`cargo install`) — which matters more for a solo build under a Wave cycle's time pressure than platform novelty does.

## Current status

**Phase 7 substantially complete — Phase 8 (Wave submission) is next.** All three commands
work, styled consistently, with unit and gated integration tests.

The repo is public at https://github.com/adewuyito/sorobscope, licensed Apache-2.0, with
seven seeded issues covering deferred work. `Cargo.toml` carries full crates.io metadata
and `cargo publish --dry-run` passes, but **nothing has been published** — that call is
deliberately left to the maintainer, since crates.io allows yanking but never deletion.

**Attribution note:** commits carry no `Co-Authored-By` trailer, at the maintainer's
request. Do not add one unless explicitly asked.

Read `docs/ROADMAP.md` next. Update this section as phases complete.

Phase 1 built `src/network.rs` (`Connection::open` — the one place a `Server` is
constructed) and `src/error.rs` (`RpcFailure` — translates client errors into distinct
messages for unreachable / rejected / undecodable). `--network` and `--rpc-url` are live.
Commands still print `not implemented`, but each one now opens a real connection first, so
the error paths are exercised rather than theoretical.

Phase 2 built `src/decode.rs` (the `ScVal` renderers — readable and JSON twins) and
`src/commands/events.rs`. `events` is live: it decodes topics and payloads, supports
`--since-ledger`, `--follow` (5s poll, clean Ctrl-C) and `--json`. Verified end to end by
invoking the fixture and watching the event appear decoded within one poll interval.

Phase 3 built `src/commands/entry.rs`. `entry` reads one known key and decodes it, and
surfaces `liveUntilLedgerSeq` as both a ledger number and an approximate time — the small
thing raw tooling makes you compute yourself. Verified against the fixture's `COUNTER`
(value 4, live until 4483568), cross-checked against `stellar contract read`.

Design note: **durability is not a flag.** The same symbol in persistent and temporary
storage are two distinct ledger entries, so rather than make the user guess, `entry` asks
for persistent, temporary, and the contract instance in a single batched
`getLedgerEntries` call and reports whichever exists. The instance lookup exists because a
contract using `env.storage().instance()` keeps values inside the instance entry, where a
bare Symbol key finds nothing — the exact trap the fixture was reshaped to avoid in Phase 0.
The instance path was unit-tested only until Phase 6, which deployed a second fixture to
cover it against a live contract — see below.

`guard` moved from `commands/events.rs` to `decode.rs`, since `LedgerEntryResult::to_key()`
and `to_data()` panic the same way `EventResponse`'s accessors do.

Phase 4 built `src/commands/tx.rs` and `src/outcome.rs`. `tx` decodes the invoked
contract, function name, arguments, return value and emitted events. The invocation is not
a field the RPC returns — it is dug out of the transaction envelope XDR, under the
operation's `InvokeHostFunction`. Non-invocation transactions (deploys, wasm uploads) are
named by their actual host function rather than described vaguely.

Two findings from Phase 4:

- **The RPC cannot distinguish "no such transaction" from "outside the retention window".**
  Both come back as `NOT_FOUND`. The roadmap asked for these to read differently; the
  honest maximum is to report `NOT_FOUND` while stating the endpoint's retained range
  (`oldestLedger`–`latestLedger`), which the response does carry, so the user can judge
  which case they are in. Claiming to tell them apart would be a lie.
- **`TransactionDetails`' accessors do not panic** — unlike `EventResponse` and
  `LedgerEntryResult`, they decode with `.ok()` and return `Option`. `guard` is not needed
  in `tx.rs`.

Exit codes are now meaningful: `0` found, `2` looked up successfully but absent, `1` error.
`events` finding nothing stays `0`, since a quiet range is a real answer rather than a
missing identifier. `src/outcome.rs` carries that distinction from commands to `main`.

Phase 5 added `src/style.rs` and `--color <auto|always|never>`. All three commands now
print through `style::field`, so they align on one label column instead of three ad-hoc
layouts. Two rules matter here:

- **Colour is off unless stdout is a terminal.** A tool whose `--json` is meant to pipe
  into `jq` cannot leak escape codes into a pipeline. `NO_COLOR` is honoured; an explicit
  `--color always` overrides it, matching how `git` and `ripgrep` treat an explicit flag.
- **Padding is applied before styling.** ANSI codes count toward a format width, so
  padding a coloured string silently misaligns every column. `style::field` pads the raw
  label and colours afterwards.

Styling uses bare ANSI constants rather than a colour crate — it is a handful of codes, and
`std::io::IsTerminal` covers the detection.

Phase 6 added `tests/integration_testnet.rs` (10 tests, gated behind
`SOROBSCOPE_TEST_NETWORK`) and extended the unit suite to 33. Gated so a flaky network or
an expired fixture can't take the unit suite down with it; assertions check the *shape* of
output, not exact text, so formatting changes don't break them.

**The instance-storage gap flagged in Phase 3 is now closed.** A second fixture,
`fixtures/instance-counter` (`CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY`), is the stock
`soroban-examples` shape: `COUNTER` in instance storage. It is the case that makes the
fallback worth having — `stellar contract read` fails outright on it ("no matching contract
data entries were found") because there is no standalone entry, while `entry` finds the
value in the instance's storage map and says where it lives. That comparison is the
clearest demonstration the project has of doing something the existing tooling does not.

Also verified against a contract this project didn't write (the `reputation` contract at
`CASDNMYRVVFRTCS23GK2M77VP3W2YCB6NXKT33KL5ZIX6VW4W4TYEPOM`), per `docs/CLI_SPEC.md`'s
requirement to check graceful degradation on an unfamiliar shape. It decoded a `Map`
payload (`{timestamp: ...}`) — a branch neither fixture exercises — and a composite
`Vec` key through `--key-xdr`. Nothing panicked.

Two things Phase 2 turned up that later phases inherit:

- **`EventResponse::topic()` and `value()` panic on bad XDR.** They decode with `.expect()`
  and the raw base64 fields behind them are private, so there is no non-panicking path
  through the crate's public API. `commands/events.rs::guard` catches the unwind and
  renders `<undecodable>` instead, so one malformed event can't kill a `--follow` session.
  Phases 3-4 should reuse that guard for any other accessor that decodes XDR eagerly.
- **`getEvents` does not scan the whole range in one call.** It covers a bounded slice of
  ledgers per request and hands back a cursor; a single call can return zero events while
  real ones sit further along the range. You must page until the cursor's ledger reaches
  `latestLedger`, or you silently under-report. The cursor is `"<toid>-<index>"` and the
  TOID's high 32 bits are the ledger, which is how `events.rs::cursor_ledger` decides
  whether the scan has caught up.
- **`Pagination` and `EventFilter` are not `Clone`**, so a polling loop has to rebuild them
  each pass rather than cloning one outside the loop.

One finding worth carrying forward: **mainnet has no default RPC URL and that is
deliberate.** The SDF runs public RPC for the test networks only; there is no free public
mainnet endpoint to fall back on, so `--network mainnet` errors and tells the user to
supply `--rpc-url`. Inventing a plausible-looking default would produce a confusing
connection failure instead of a clear one.

Resolved dependency versions (Phase 0): `clap` 4.6.6, `tokio` 1.53.1, `soroban-client` 0.5.9,
`serde_json` 1.0.151, `anyhow` 1.0.104. `soroban-client` 0.5.9's real API is
`Server::new(url: &str, opts: Options) -> Result<Server, Error>` and
`get_latest_ledger() -> Result<GetLatestLedgerResponse, Error>` with fields
`id` / `protocol_version` / `sequence` — close enough to `docs/CLI_SPEC.md`'s sketches
that no correction was needed, but keep verifying before relying on any other signature.

### Testnet test fixture

| | |
|---|---|
| Contract ID | `CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX` |
| Network | testnet |
| Deployer identity | `sorobscope-dev` (stellar-cli alias) |
| Functions | `increment() -> u32`, `get() -> u32`, `tag(who: Address, note: Symbol) -> u32` |

A variant of `soroban-examples`' `increment`, with two deliberate changes that make it a
usable target for Phases 2-3:

- **Persistent storage, not instance storage.** The stock example keeps its counter in
  instance storage, where it lives *inside* the contract instance entry and is therefore
  not addressable by a bare Symbol ledger key. Using `persistent()` makes `COUNTER` a real
  standalone `ContractData` entry, which is what `entry --key-symbol COUNTER` needs to
  exercise its primary path. `extend_ttl` also gives it a real `liveUntilLedgerSeq` for
  Phase 3 to surface.
- **It emits events.** The stock example publishes none, so Phase 2's "trigger a real
  event and watch it appear" would have been unsatisfiable. `increment` emits a 2-topic
  event with a `u32` payload; `tag` emits a 3-topic event including an `Address` topic and
  a `(Symbol, u32, Address)` tuple payload, so the decoder has something non-trivial to
  render.

Regenerate activity at any time with
`stellar contract invoke --id <id> --source sorobscope-dev --network testnet -- increment`.

> The fixture's **source** currently lives only in this session's scratchpad, which is
> temporary. The deployed contract is unaffected (it's on testnet and invocable by ID),
> but redeploying would need the source rewritten. Move it into the repo if reproducibility
> for other contributors matters — note it would need to stay outside any Cargo workspace
> `sorobscope` participates in, since this repo is a native binary, not a contract project.

## Tech stack

| Layer | Choice | Notes |
|---|---|---|
| Language | Rust | Native binary, no runtime dependency for end users. |
| CLI parsing | `clap` (derive feature) | Standard, well-documented, not worth second-guessing. |
| Stellar/Soroban RPC client | [`soroban-client`](https://crates.io/crates/soroban-client) crate | Community-maintained Rust client for Soroban RPC (get_events, get_ledger_entries, get_transaction, simulate, etc.). Verify current API surface against docs.rs before relying on any exact method signature in these docs — it's younger and less API-stable than the official JS/Python SDKs. |
| Async runtime | `tokio` | `soroban-client`'s own examples assume it. |
| Output | Human-readable by default, `--json` flag for machine-readable | Standard good-CLI practice — makes the tool pipeable into `jq` etc. without sacrificing readability for interactive use. |

## Ground rules

1. **Read-only, always.** This tool never signs or submits a transaction and never needs a wallet or secret key, on any network. That's a deliberate scope boundary — `stellar-cli` already does writes well, and adding key-handling here would be scope creep with real risk for no real benefit. If a future version genuinely needs to write something, that's a big enough decision to revisit this rule explicitly, not to quietly cross.
2. **Don't trust hardcoded crate versions in these docs.** Resolve current versions from crates.io at build time (`cargo add <crate>` does this automatically).
3. **Be honest about what Soroban RPC can't do.** This tool cannot enumerate a contract's full storage or see history older than the RPC's retention window — these are real limits of the RPC surface itself, not gaps this tool will casually close later. `docs/ARCHITECTURE.md` explains why; don't let a command's `--help` text or the README imply otherwise.
4. **Small, real, and working beats large and mocked.** Same as any Wave submission — a 3-command tool that actually decodes real testnet events beats a 6-command tool with stubbed output.
5. **Seed issues as you go**, not all at the end — see Phase 7 in the roadmap.
6. **This doc set is a starting frame, not gospel.** Where reality contradicts what's written here — a crate API that doesn't match, a design that turns out awkward — follow reality and update the doc.

## Where things live

```
.
├── CLAUDE.md                 ← you are here
├── docs/
│   ├── ROADMAP.md             ← phased build plan — start here
│   ├── ARCHITECTURE.md        ← design principles, data flow, known RPC limitations, non-goals
│   └── CLI_SPEC.md            ← command reference, module structure, code sketches
├── src/                        ← not yet created — Phase 0/1
└── tests/                      ← not yet created — Phase 6
```

## Useful external references

- Stellar Wave Program, approved-repos board: https://www.drips.network/wave/stellar/repos
- `soroban-client` crate docs: https://docs.rs/soroban-client
- Soroban RPC method reference (`getEvents`, `getLedgerEntries`, `getTransaction`): https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods
- Official agent-oriented Soroban skill: https://github.com/stellar/stellar-dev-skill/blob/main/skills/smart-contracts/SKILL.md
- `stellar/soroban-examples` — good, well-documented contracts to test this tool against if you don't have your own deployed: https://github.com/stellar/soroban-examples
