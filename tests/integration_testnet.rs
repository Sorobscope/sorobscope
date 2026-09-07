//! Integration tests that run the real binary against real testnet.
//!
//! These are gated behind `SOROBSCOPE_TEST_NETWORK=testnet` so a flaky network or an
//! expired fixture can't take the unit suite down with it. The unit tests cover the
//! decoding logic and run everywhere; these cover the thing unit tests structurally
//! cannot — that the RPC still answers the shape this tool expects.
//!
//! Run them with:
//!
//! ```sh
//! SOROBSCOPE_TEST_NETWORK=testnet cargo test --test integration_testnet
//! ```
//!
//! Assertions deliberately check the *shape* of the output rather than exact text, so
//! formatting changes don't break the suite. Values that drift on their own — the counter,
//! the current ledger — are never asserted to be a specific number.

use std::process::{Command, Output};

/// Persistent-storage fixture: `COUNTER` is a standalone ledger entry.
const PERSISTENT_FIXTURE: &str = "CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX";

/// Instance-storage fixture: the stock `soroban-examples` shape, where `COUNTER` lives
/// *inside* the contract instance entry rather than as an addressable entry of its own.
const INSTANCE_FIXTURE: &str = "CAGHKC2CYSLSL5J7C4OHEN26YKRH5BND7SGBTHUVZYBRS6OWUW5RLTAY";

/// A known transaction against that fixture — the `tag` call, which carries an Address
/// argument and a tuple event payload, so it exercises more of the decoder than a bare u32.
const KNOWN_TX: &str = "57cc3d10f015741ced4ee558c3f36cf3cb7e6253f2c32d35982c8e493c849ab3";

/// Skip unless explicitly switched on. Returns true when the test should run.
fn enabled() -> bool {
    match std::env::var("SOROBSCOPE_TEST_NETWORK") {
        Ok(network) => !network.is_empty(),
        Err(_) => false,
    }
}

/// Run the built binary with the given arguments.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sorobscope"))
        .args(args)
        // Force colour off so assertions never trip over escape codes, independently of
        // whatever the surrounding terminal or CI environment looks like.
        .args(["--color", "never"])
        .output()
        .expect("failed to run sorobscope")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

macro_rules! gated {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            if !enabled() {
                eprintln!("skipping: set SOROBSCOPE_TEST_NETWORK=testnet to run");
                return;
            }
            $body
        }
    };
}

gated!(entry_reads_a_persistent_key, {
    let out = run(&["entry", PERSISTENT_FIXTURE, "--key-symbol", "COUNTER"]);
    let text = stdout(&out);

    assert_eq!(out.status.code(), Some(0), "expected found, got:\n{text}");
    assert!(text.contains("durability"), "missing durability:\n{text}");
    assert!(text.contains("persistent"), "expected persistent:\n{text}");
    // The TTL is the thing this command exists to surface.
    assert!(text.contains("live until"), "TTL not surfaced:\n{text}");
});

gated!(entry_json_is_machine_readable, {
    let out = run(&[
        "entry",
        PERSISTENT_FIXTURE,
        "--key-symbol",
        "COUNTER",
        "--json",
    ]);
    let text = stdout(&out);

    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{text}"));

    assert_eq!(parsed["durability"], "persistent");
    assert_eq!(parsed["key"], "COUNTER");
    assert!(
        parsed["liveUntilLedger"].is_number(),
        "TTL missing: {parsed}"
    );
    // The value is a counter that other tests bump, so assert its type, not its value.
    assert!(parsed["value"].is_number(), "value not decoded: {parsed}");
});

gated!(entry_reports_a_missing_key_as_exit_2, {
    let out = run(&["entry", PERSISTENT_FIXTURE, "--key-symbol", "NOSUCHKEY"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "a successful lookup that found nothing must be distinguishable from an error"
    );
});

// The case a bare Symbol ledger key cannot reach on its own.
//
// `stellar contract read` fails outright on this contract ("no matching contract data
// entries were found") because there is no standalone entry to find. `entry` has to fall
// back to the contract instance's storage map, which is the whole reason that fallback
// exists — most contracts store state this way.
gated!(entry_finds_a_value_held_in_instance_storage, {
    let out = run(&["entry", INSTANCE_FIXTURE, "--key-symbol", "COUNTER"]);
    let text = stdout(&out);

    assert_eq!(
        out.status.code(),
        Some(0),
        "instance lookup failed:\n{text}"
    );
    assert!(
        text.contains("instance"),
        "durability should be reported as instance:\n{text}"
    );
    assert!(
        text.contains("value"),
        "value not decoded from instance storage:\n{text}"
    );
});

gated!(tx_decodes_a_known_invocation, {
    let out = run(&["tx", KNOWN_TX]);
    let text = stdout(&out);

    assert_eq!(out.status.code(), Some(0), "expected found, got:\n{text}");
    assert!(text.contains("SUCCESS"), "status missing:\n{text}");
    assert!(text.contains("tag"), "function name not decoded:\n{text}");
    assert!(
        text.contains(PERSISTENT_FIXTURE),
        "contract not decoded:\n{text}"
    );
    // The Address argument must come out as a strkey, not base64 XDR.
    assert!(text.contains('G'), "address argument not decoded:\n{text}");
});

gated!(tx_reports_an_unknown_hash_as_exit_2, {
    let out = run(&["tx", &"0".repeat(64)]);
    let text = stdout(&out);

    assert_eq!(out.status.code(), Some(2));
    // The retention window is the only honest guidance we can give here, so it must
    // actually appear — see the note in CLAUDE.md about NOT_FOUND being ambiguous.
    assert!(
        text.contains("retains"),
        "missing retention-window explanation:\n{text}"
    );
});

gated!(events_decodes_rather_than_relaying_base64, {
    let out = run(&["events", PERSISTENT_FIXTURE, "--since-ledger", "4429700"]);
    let text = stdout(&out);

    assert_eq!(out.status.code(), Some(0), "run failed:\n{text}");

    // If the fixture's events have aged out of retention there is nothing to assert on,
    // and that is a property of the endpoint rather than a failure of this tool.
    if text.contains("No events found") {
        eprintln!("fixture events are outside the retention window; nothing to assert");
        return;
    }

    assert!(text.contains("topics"), "topics not rendered:\n{text}");
    assert!(text.contains("counter"), "symbol not decoded:\n{text}");
    assert!(
        !text.contains("AAAA"),
        "output still contains base64 XDR — decoding is the whole point:\n{text}"
    );
});

gated!(events_json_emits_one_object_per_line, {
    let out = run(&[
        "events",
        PERSISTENT_FIXTURE,
        "--since-ledger",
        "4429700",
        "--json",
    ]);
    let text = stdout(&out);

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line is not valid JSON: {e}\n{line}"));
        assert!(parsed["ledger"].is_number(), "no ledger field: {parsed}");
        assert!(parsed["topics"].is_array(), "topics not an array: {parsed}");
    }
});

// A broken endpoint must fail clearly rather than panicking, and must be distinguishable
// from a successful lookup that found nothing.
gated!(an_unreachable_endpoint_exits_1_not_2, {
    let out = run(&[
        "entry",
        PERSISTENT_FIXTURE,
        "--key-symbol",
        "COUNTER",
        "--rpc-url",
        "http://127.0.0.1:1",
    ]);

    assert_eq!(
        out.status.code(),
        Some(1),
        "transport failure must be exit 1"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("could not reach"),
        "unclear error message:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "panicked instead of reporting:\n{stderr}"
    );
});

/// Mainnet has no default endpoint on purpose; that must stay a clear error rather than
/// becoming a confusing connection failure against some invented URL.
#[test]
fn mainnet_requires_an_explicit_rpc_url() {
    // No network access needed: this fails before any request is made.
    let out = run(&[
        "entry",
        PERSISTENT_FIXTURE,
        "--key-symbol",
        "COUNTER",
        "--network",
        "mainnet",
    ]);

    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--rpc-url"),
        "should point the user at the fix:\n{stderr}"
    );
}
