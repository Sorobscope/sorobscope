# Roadmap

Nine phases. Work them in order; don't treat "done" loosely — later phases assume earlier ones genuinely work.

---

## Phase 0 — Project Initialization & Tooling

**Goal:** a working, verified Rust dev environment, an empty-but-correct binary crate, and confirmed network reachability. No real commands implemented yet.

- [x] Confirm toolchain: `rustc --version`, `cargo --version`. This project doesn't need the `wasm32v1-none` target or any Soroban contract build tooling — it's a plain native Rust binary, not a contract.
- [x] `cargo init --name sorobscope` (or your chosen name) to scaffold the binary crate.
- [x] Add core dependencies and confirm each resolves: `cargo add clap --features derive`, `cargo add tokio --features full`, `cargo add soroban-client`, `cargo add serde_json`, `cargo add anyhow` (or your preferred error-handling crate).
- [x] `cargo build` succeeds with nothing but a placeholder `fn main() { println!("sorobscope"); }` — proves the toolchain and dependency resolution work before any real logic exists.
- [x] Sketch the `clap` `Cli`/`Commands` skeleton (see `docs/CLI_SPEC.md`) with three empty subcommands (`events`, `entry`, `tx`) that just print "not implemented" — confirm `cargo run -- --help` shows all three and `cargo run -- events --help` shows that subcommand's flags.
- [x] Get a real target to test against: either point at a contract you already control (e.g., a testnet deployment from a prior project) or deploy one of `stellar/soroban-examples`' simple contracts (e.g. `increment`) to testnet if you don't have one handy — you'll want something to point real commands at starting Phase 2.
- [x] Prove network reachability before building any command logic: a throwaway call to `soroban-client`'s `get_latest_ledger()` against testnet should return a real, current ledger sequence number.
- [x] Repo hygiene: `.gitignore` should cover `target/` — but **commit `Cargo.lock`**, not ignore it; that's the convention for binary crates (as opposed to libraries, where it's usually ignored). Short top-level `README.md` pointing at `CLAUDE.md`.

**Definition of done:** `cargo run -- --help` shows a real (even if unimplemented) three-subcommand list, and a throwaway `get_latest_ledger()` call against testnet succeeds and prints a real number — proving both the CLI scaffold and network connectivity work before any command has real logic.

---

## Phase 1 — RPC Client Foundation

- Wrap `soroban_client::Server` construction in a small internal module (`src/network.rs`) parameterized by network — testnet/futurenet/mainnet RPC URLs and passphrases — so no command hardcodes a URL directly.
- Implement `--network` (default `testnet`) and a `--rpc-url` escape hatch for pointing at a custom or local RPC instance.
- Basic, distinct error handling: "couldn't reach the RPC endpoint" vs. "RPC responded with a JSON-RPC error" vs. "got a response but couldn't decode the XDR" are three different problems and should produce three different user-facing messages later, not one generic "something went wrong."

**Done when:** every subsequent command can get a configured `Server` from one shared function, and a deliberately-broken `--rpc-url` produces a clear, specific error instead of a panic.

## Phase 2 — `events` command

- Implement `sorobscope events <contract-id>`: build an `EventFilter` scoped to the contract, call `get_events`, decode the returned event topics/data into readable output.
- Add `--since-ledger`, `--follow` (poll loop, sane interval, clean Ctrl-C handling), and respect the global `--json` flag.
- Test against the real contract chosen in Phase 0 — trigger an actual event (invoke the test contract from another terminal/tool) and confirm it shows up, ideally within one poll interval under `--follow`.

**Done when:** a real event you just triggered shows up in the output, decoded, not as a base64 blob.

## Phase 3 — `entry` command (known-key storage read)

- Implement `sorobscope entry <contract-id>` for the simple, common key shapes first: a bare Symbol key (`--key-symbol foo`), a bare Address key (`--key-address G...`). Add `--key-xdr <base64>` as an escape hatch for any key shape the simple flags can't express.
- Decode the returned entry into readable output, and surface `liveUntilLedgerSeq` (TTL/expiry) plainly — this is a small but genuinely useful thing raw `stellar-cli` output doesn't make obvious at a glance.
- Make the command's own `--help` text explicit that this reads **one entry you already know the key for** — not a storage dump. See `docs/ARCHITECTURE.md`'s Non-goals for why a generic dump isn't a real RPC capability to build toward.

**Done when:** you can read back a real value from the test contract's storage (e.g. the `increment` contract's counter) and see it decoded correctly, matching what you know it should be.

## Phase 4 — `tx` command

- Implement `sorobscope tx <hash>`: call `getTransaction`, decode the invoked contract ID, function name, arguments, and result, and list any events that transaction emitted.
- Handle "not found" distinctly from "found but outside the RPC's retention window" — these should read differently to the user, since the second one has a different (and honest) explanation.

**Done when:** pointing the command at a transaction hash from Phase 2's triggered event produces a correct, readable summary of what actually happened.

## Phase 5 — Output & UX Polish

- Consistent formatting across all three commands — aligned columns or a light table, not three different ad-hoc styles — plus a genuinely working `--json` mode on all three, not just the one it was first built for.
- Terminal color for readability (success/failure state, key names) — small effort, real payoff for a tool people will stare at.
- Solid `--help` text on every command and flag. For a CLI dev tool, the help text is most people's actual documentation.

## Phase 6 — Testing

- Unit tests for the ScVal-decoding/pretty-printing logic specifically — these are pure functions with no network dependency and should run on every commit.
- A small set of integration tests that run the real binary against real testnet, gated (feature flag, env var, or separate `cargo test` target) so they can be skipped when testnet is flaky without losing the unit suite.
- Point the tool at a contract you didn't build yourself (something else off the Wave repo board, for instance) before calling this phase done — testing only against your own contract tends to bake in assumptions you won't notice until someone else's contract breaks them.

## Phase 7 — Docs, Packaging, Demo

- Write the real `README.md`: what it does, install instructions (`cargo install --path .` for now; `cargo install sorobscope` if/once published to crates.io), and real terminal-output examples for each command — actual transcripts, not mockups.
- Consider publishing to crates.io. This is genuinely low-effort for a Rust binary crate and directly supports the "least install friction" pitch from `CLAUDE.md`.
- Open v2 issues for anything intentionally deferred: a schema-aware key builder for `entry` (parsing a contract's XDR interface to support composite/custom-typed keys automatically), integration with a historical indexer (Mercury or similar) for data beyond the RPC's retention window, shell completions, a `watch` mode that combines events + entry lookups live. This is what makes the repo useful to other Wave contributors, not just to you.

## Phase 8 — Wave Program Submission

- Re-check the current repo-application flow before submitting (it has changed between Waves before — check https://www.drips.network/blog for anything newer).
- Apply at https://www.drips.network/wave/maintainer-onboarding/install-app
- Confirm before submitting: real README, architecture doc, a working demo against real testnet activity, 5+ well-scoped open issues.

**Done when:** the repo is submitted and you have a maintainer-dashboard confirmation, not just a form submission. Check https://www.drips.network/wave/stellar for the actual current Wave number before submitting — don't assume it's still whichever Wave was current when this doc set was written.
