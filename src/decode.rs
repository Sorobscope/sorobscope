//! Turning `ScVal` into something a person or a script can read.
//!
//! This is the actual product. Everything else in this tool is plumbing to get an `ScVal`
//! in front of these two functions, which are deliberate twins: [`scval_to_readable`] for
//! a terminal and [`scval_to_json`] for a pipe. When you add a variant to one, add it to
//! the other.
//!
//! Two conventions worth knowing about the JSON side, because they're chosen rather than
//! obvious:
//!
//! - **Integers wider than 32 bits become JSON strings.** Many JSON consumers parse
//!   numbers as IEEE doubles, which silently mangles anything past 2^53 — and i128 balances
//!   are routine in Soroban contracts. A string round-trips exactly.
//! - **Maps become objects only when every key is a Symbol or String.** Soroban map keys
//!   can be any `ScVal`, which JSON object keys cannot represent, so mixed-key maps become
//!   an array of `{"key":…,"value":…}` pairs instead of being silently stringified.

use serde_json::{json, Map as JsonMap, Value};
use soroban_client::xdr::{Int128Parts, ScVal, UInt128Parts};

/// Render an `ScVal` as a single readable line.
pub fn scval_to_readable(v: &ScVal) -> String {
    match v {
        ScVal::Bool(b) => b.to_string(),
        ScVal::Void => "void".to_string(),

        ScVal::U32(n) => n.to_string(),
        ScVal::I32(n) => n.to_string(),
        ScVal::U64(n) => n.to_string(),
        ScVal::I64(n) => n.to_string(),

        ScVal::Timepoint(t) => format!("timepoint({})", t.0),
        ScVal::Duration(d) => format!("duration({})", d.0),

        ScVal::U128(parts) => u128_from(parts).to_string(),
        ScVal::I128(parts) => i128_from(parts).to_string(),

        // Rust has no 256-bit primitive and stellar-xdr keeps its decimal helpers
        // crate-private, so these render as hex rather than being reassembled by hand.
        // Lossless and unambiguous, just not decimal.
        ScVal::U256(p) => format!(
            "0x{:016x}{:016x}{:016x}{:016x}",
            p.hi_hi, p.hi_lo, p.lo_hi, p.lo_lo
        ),
        ScVal::I256(p) => format!(
            "0x{:016x}{:016x}{:016x}{:016x}",
            p.hi_hi, p.hi_lo, p.lo_hi, p.lo_lo
        ),

        ScVal::Bytes(b) => format!("0x{}", hex(b.0.as_slice())),

        // Strings are quoted, symbols are not: a Symbol is an identifier (a function or
        // event name), and quoting it just adds noise to the common case.
        ScVal::String(s) => format!("{:?}", s.0.to_utf8_string_lossy()),
        ScVal::Symbol(s) => s.0.to_utf8_string_lossy(),

        // ScAddress's Display yields strkey — the G.../C... form people recognise.
        ScVal::Address(a) => a.to_string(),

        ScVal::Vec(Some(items)) => format!(
            "[{}]",
            items
                .0
                .iter()
                .map(scval_to_readable)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ScVal::Vec(None) => "[]".to_string(),

        ScVal::Map(Some(entries)) => format!(
            "{{{}}}",
            entries
                .0
                .iter()
                .map(|e| format!(
                    "{}: {}",
                    scval_to_readable(&e.key),
                    scval_to_readable(&e.val)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ScVal::Map(None) => "{}".to_string(),

        ScVal::Error(e) => format!("error({e:?})"),

        ScVal::ContractInstance(_) => "<contract instance>".to_string(),
        ScVal::LedgerKeyContractInstance => "<contract instance key>".to_string(),
        ScVal::LedgerKeyNonce(n) => format!("<nonce {}>", n.nonce),
    }
}

/// Render an `ScVal` as JSON. Twin of [`scval_to_readable`]; see the module docs for the
/// two conventions that aren't self-evident.
pub fn scval_to_json(v: &ScVal) -> Value {
    match v {
        ScVal::Bool(b) => json!(b),
        ScVal::Void => Value::Null,

        // 32-bit values fit a double exactly, so they stay real JSON numbers.
        ScVal::U32(n) => json!(n),
        ScVal::I32(n) => json!(n),

        // Everything wider becomes a string — see the module docs.
        ScVal::U64(n) => json!(n.to_string()),
        ScVal::I64(n) => json!(n.to_string()),
        ScVal::Timepoint(t) => json!(t.0.to_string()),
        ScVal::Duration(d) => json!(d.0.to_string()),
        ScVal::U128(parts) => json!(u128_from(parts).to_string()),
        ScVal::I128(parts) => json!(i128_from(parts).to_string()),
        ScVal::U256(p) => json!(format!(
            "0x{:016x}{:016x}{:016x}{:016x}",
            p.hi_hi, p.hi_lo, p.lo_hi, p.lo_lo
        )),
        ScVal::I256(p) => json!(format!(
            "0x{:016x}{:016x}{:016x}{:016x}",
            p.hi_hi, p.hi_lo, p.lo_hi, p.lo_lo
        )),

        ScVal::Bytes(b) => json!(format!("0x{}", hex(b.0.as_slice()))),
        ScVal::String(s) => json!(s.0.to_utf8_string_lossy()),
        ScVal::Symbol(s) => json!(s.0.to_utf8_string_lossy()),
        ScVal::Address(a) => json!(a.to_string()),

        ScVal::Vec(Some(items)) => {
            Value::Array(items.0.iter().map(scval_to_json).collect())
        }
        ScVal::Vec(None) => Value::Array(Vec::new()),

        ScVal::Map(Some(entries)) => {
            let stringy = entries
                .0
                .iter()
                .all(|e| matches!(e.key, ScVal::Symbol(_) | ScVal::String(_)));

            if stringy {
                let mut object = JsonMap::new();
                for e in entries.0.iter() {
                    let key = match &e.key {
                        ScVal::Symbol(s) => s.0.to_utf8_string_lossy(),
                        ScVal::String(s) => s.0.to_utf8_string_lossy(),
                        other => scval_to_readable(other),
                    };
                    object.insert(key, scval_to_json(&e.val));
                }
                Value::Object(object)
            } else {
                Value::Array(
                    entries
                        .0
                        .iter()
                        .map(|e| json!({"key": scval_to_json(&e.key), "value": scval_to_json(&e.val)}))
                        .collect(),
                )
            }
        }
        ScVal::Map(None) => Value::Object(JsonMap::new()),

        ScVal::Error(e) => json!({"error": format!("{e:?}")}),

        ScVal::ContractInstance(_) => json!("<contract instance>"),
        ScVal::LedgerKeyContractInstance => json!("<contract instance key>"),
        ScVal::LedgerKeyNonce(n) => json!({"nonce": n.nonce.to_string()}),
    }
}

/// Run a decoding accessor without letting it take the process down.
///
/// Several soroban-client accessors — `EventResponse::topic()`/`value()`,
/// `LedgerEntryResult::to_key()`/`to_data()` — decode base64 XDR with `.expect()`, and the
/// raw fields behind them are private — so there is no non-panicking path to that data through
/// the crate's public API. One malformed event would otherwise abort a `--follow` session
/// that is working perfectly well; catching the unwind downgrades it to one line marked
/// `<undecodable>`. The panic hook is silenced for the duration so the crate's message
/// doesn't land in the middle of the output.
pub fn guard<T>(f: impl FnOnce() -> T) -> Option<T> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous);
    result.ok()
}

/// Reassemble a `u128` from its XDR hi/lo halves.
fn u128_from(p: &UInt128Parts) -> u128 {
    ((p.hi as u128) << 64) | (p.lo as u128)
}

/// Reassemble an `i128` from its XDR hi/lo halves.
///
/// `hi` is signed and `lo` is not, so the halves are joined as raw bits and reinterpreted
/// at the end. Sign-extending `lo` instead would corrupt any value with the high bit of
/// the low word set.
fn i128_from(p: &Int128Parts) -> i128 {
    ((((p.hi as u64) as u128) << 64) | (p.lo as u128)) as i128
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_client::xdr::{
        Int128Parts, ScBytes, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, ScVec, UInt128Parts,
    };

    fn sym(s: &str) -> ScVal {
        ScVal::Symbol(ScSymbol(s.try_into().unwrap()))
    }
    fn text(s: &str) -> ScVal {
        ScVal::String(ScString(s.try_into().unwrap()))
    }
    fn vec_of(items: Vec<ScVal>) -> ScVal {
        ScVal::Vec(Some(ScVec(items.try_into().unwrap())))
    }
    fn map_of(pairs: Vec<(ScVal, ScVal)>) -> ScVal {
        let entries: Vec<ScMapEntry> = pairs
            .into_iter()
            .map(|(key, val)| ScMapEntry { key, val })
            .collect();
        ScVal::Map(Some(ScMap(entries.try_into().unwrap())))
    }

    #[test]
    fn scalars_render_plainly() {
        assert_eq!(scval_to_readable(&ScVal::U32(7)), "7");
        assert_eq!(scval_to_readable(&ScVal::I32(-7)), "-7");
        assert_eq!(scval_to_readable(&ScVal::Bool(true)), "true");
        assert_eq!(scval_to_readable(&ScVal::Void), "void");
    }

    /// Symbols are identifiers (event and function names), so they read better unquoted;
    /// strings are data and keep their quotes so an empty or padded one stays visible.
    #[test]
    fn symbols_are_bare_and_strings_are_quoted() {
        assert_eq!(scval_to_readable(&sym("counter")), "counter");
        assert_eq!(scval_to_readable(&text("counter")), "\"counter\"");
    }

    #[test]
    fn bytes_render_as_hex() {
        let b = ScVal::Bytes(ScBytes(vec![0xde, 0xad, 0x00, 0x0f].try_into().unwrap()));
        assert_eq!(scval_to_readable(&b), "0xdead000f");
    }

    #[test]
    fn collections_nest() {
        let v = vec_of(vec![sym("a"), ScVal::U32(1)]);
        assert_eq!(scval_to_readable(&v), "[a, 1]");

        let m = map_of(vec![(sym("k"), ScVal::U32(2))]);
        assert_eq!(scval_to_readable(&m), "{k: 2}");

        assert_eq!(scval_to_readable(&ScVal::Vec(None)), "[]");
        assert_eq!(scval_to_readable(&ScVal::Map(None)), "{}");
    }

    #[test]
    fn u128_reassembles_from_halves() {
        let one = UInt128Parts { hi: 0, lo: 1 };
        assert_eq!(scval_to_readable(&ScVal::U128(one)), "1");

        let two_64 = UInt128Parts { hi: 1, lo: 0 };
        assert_eq!(
            scval_to_readable(&ScVal::U128(two_64)),
            "18446744073709551616"
        );
    }

    /// The low half is unsigned even though the high half is signed. Sign-extending `lo`
    /// would turn any value with its top bit set into a negative number, so these three
    /// cases pin the bit-joining down.
    #[test]
    fn i128_respects_the_unsigned_low_half() {
        // hi = 0, lo = u64::MAX is the case a naive `lo as i64` cast reports as -1.
        let big = Int128Parts { hi: 0, lo: u64::MAX };
        assert_eq!(
            scval_to_readable(&ScVal::I128(big)),
            "18446744073709551615"
        );

        // All bits set is genuinely -1.
        let minus_one = Int128Parts { hi: -1, lo: u64::MAX };
        assert_eq!(scval_to_readable(&ScVal::I128(minus_one)), "-1");

        let minus_two_64 = Int128Parts { hi: -1, lo: 0 };
        assert_eq!(
            scval_to_readable(&ScVal::I128(minus_two_64)),
            "-18446744073709551616"
        );
    }

    #[test]
    fn json_keeps_small_ints_numeric_and_widens_the_rest_to_strings() {
        assert_eq!(scval_to_json(&ScVal::U32(7)), json!(7));
        assert_eq!(scval_to_json(&ScVal::I32(-7)), json!(-7));

        // Wider than 2^53, so a JSON number would lose precision in most consumers.
        assert_eq!(scval_to_json(&ScVal::U64(u64::MAX)), json!("18446744073709551615"));
        assert!(scval_to_json(&ScVal::U64(1)).is_string());
    }

    #[test]
    fn json_maps_with_text_keys_become_objects() {
        let m = map_of(vec![(sym("count"), ScVal::U32(3))]);
        assert_eq!(scval_to_json(&m), json!({"count": 3}));
    }

    /// Soroban map keys can be any ScVal, but JSON object keys cannot. Rather than
    /// stringify a non-text key and quietly lose its type, those maps become pair arrays.
    #[test]
    fn json_maps_with_non_text_keys_become_pair_arrays() {
        let m = map_of(vec![(ScVal::U32(1), sym("one"))]);
        assert_eq!(
            scval_to_json(&m),
            json!([{"key": 1, "value": "one"}])
        );
    }

    #[test]
    fn json_void_is_null() {
        assert_eq!(scval_to_json(&ScVal::Void), Value::Null);
    }
}
