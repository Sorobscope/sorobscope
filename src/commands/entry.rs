//! `sorobscope entry <contract-id> --key-…` — read one known storage entry and decode it.
//!
//! This reads a single entry whose key you already supply. Soroban RPC has no call that
//! enumerates a contract's storage, so there is no "dump everything" mode to grow into
//! here — see `docs/ARCHITECTURE.md`.

use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde_json::json;
use soroban_client::xdr::{
    ContractDataDurability, LedgerEntryData, LedgerKey, LedgerKeyContractData, Limits, ReadXdr,
    ScAddress, ScSymbol, ScVal,
};

use crate::decode::{guard, scval_to_json, scval_to_readable};
use crate::network::Connection;
use crate::outcome::Outcome;
use crate::style;

/// Ledgers close about every 5 seconds. Used only to put a human-readable figure next to
/// a TTL; it is an approximation, and labelled as one in the output.
const SECONDS_PER_LEDGER: u64 = 5;

pub async fn run(
    connection: &Connection,
    contract_id: &str,
    key_symbol: Option<String>,
    key_address: Option<String>,
    key_xdr: Option<String>,
    json: bool,
) -> anyhow::Result<Outcome> {
    let contract = parse_contract(contract_id)?;
    let key = build_key(key_symbol, key_address, key_xdr)?;

    // Durability is part of an entry's address, not a property you can query around: the
    // same symbol in persistent and temporary storage are two different ledger entries.
    // Rather than make the user guess, ask for both at once — plus the contract instance,
    // so a contract that keeps its state in instance storage gets a useful answer instead
    // of a bare "not found". getLedgerEntries takes a batch, so this is still one trip.
    let keys = vec![
        contract_data_key(&contract, &key, ContractDataDurability::Persistent),
        contract_data_key(&contract, &key, ContractDataDurability::Temporary),
        contract_data_key(
            &contract,
            &ScVal::LedgerKeyContractInstance,
            ContractDataDurability::Persistent,
        ),
    ];

    let response = connection
        .server
        .get_ledger_entries(keys)
        .await
        .map_err(|e| connection.fail(e))?;

    let latest = response.latestLedger;
    let entries = response.entries.unwrap_or_default();

    // Look for a standalone ContractData entry whose key is the one asked for. The
    // instance entry comes back under this same call, so match on the key rather than
    // assuming the RPC preserved our request order.
    let mut found: Option<Found> = None;
    let mut instance_storage: Option<Vec<(ScVal, ScVal)>> = None;
    // Captured alongside the storage map: a value in instance storage shares the instance
    // entry's TTL, so this is the real expiry to report for it.
    let mut instance_ttl: Option<u32> = None;
    let mut instance_modified: Option<u32> = None;

    for result in &entries {
        let Some(LedgerEntryData::ContractData(data)) = guard(|| result.to_data()) else {
            continue;
        };

        if data.key == ScVal::LedgerKeyContractInstance {
            if let ScVal::ContractInstance(instance) = &data.val {
                instance_storage = instance
                    .storage
                    .as_ref()
                    .map(|m| m.0.iter().map(|e| (e.key.clone(), e.val.clone())).collect());
                instance_ttl = result.live_until_ledger_seq;
                instance_modified = result.last_modified_ledger_seq;
            }
            continue;
        }

        if data.key == key {
            found = Some(Found {
                value: data.val.clone(),
                durability: match data.durability {
                    ContractDataDurability::Persistent => "persistent",
                    ContractDataDurability::Temporary => "temporary",
                },
                last_modified: result.last_modified_ledger_seq,
                live_until: result.live_until_ledger_seq,
            });
        }
    }

    // Fall back to the contract instance's own storage map. A contract using
    // `env.storage().instance()` keeps its values inside the instance entry rather than as
    // addressable entries of their own, so the direct lookup above legitimately misses it.
    if found.is_none()
        && let Some(value) = instance_storage
            .as_deref()
            .and_then(|storage| find_in_instance_storage(storage, &key))
    {
        found = Some(Found {
            value,
            durability: "instance",
            last_modified: instance_modified,
            live_until: instance_ttl,
        });
    }

    match found {
        Some(entry) => {
            report(connection, contract_id, &key, &entry, latest, json);
            Ok(Outcome::Found)
        }
        None => {
            report_missing(contract_id, &key, latest, json);
            Ok(Outcome::NotFound)
        }
    }
}

struct Found {
    value: ScVal,
    durability: &'static str,
    last_modified: Option<u32>,
    live_until: Option<u32>,
}

fn report(
    connection: &Connection,
    contract_id: &str,
    key: &ScVal,
    entry: &Found,
    latest: u32,
    json: bool,
) {
    // A liveUntilLedgerSeq of 0 is not a ledger number: it is how the RPC reports an entry
    // with no live TTL, having been archived or reclaimed. Subtracting the current ledger
    // from it would claim the entry expired several million ledgers ago, which is nonsense.
    let ttl = entry.live_until.filter(|&until| until > 0);
    let remaining = ttl.map(|until| until as i64 - latest as i64);
    let archived = entry.live_until == Some(0);

    if json {
        println!(
            "{}",
            json!({
                "contractId": contract_id,
                "network": connection.network.name(),
                "key": scval_to_json(key),
                "durability": entry.durability,
                "value": scval_to_json(&entry.value),
                "lastModifiedLedger": entry.last_modified,
                "liveUntilLedger": ttl,
                "ledgersRemaining": remaining,
                "archived": archived,
                "latestLedger": latest,
            })
        );
        return;
    }

    println!(
        "{}",
        style::heading(&format!(
            "entry for {contract_id} on {}",
            connection.network.name()
        ))
    );
    style::field("key", &scval_to_readable(key));
    style::field("durability", entry.durability);
    style::field("value", &scval_to_readable(&entry.value));

    if let Some(seq) = entry.last_modified {
        style::field("updated", &format!("ledger {seq}"));
    }

    // Surfacing the TTL plainly is the point of this command over raw tooling: an entry
    // that is about to expire looks identical to a healthy one unless someone does this
    // subtraction for you. Colour carries the same signal at a glance.
    match (ttl, remaining) {
        (Some(until), Some(left)) if left > 0 => style::field(
            "live until",
            &format!(
                "ledger {until}  {}",
                style::good(&format!(
                    "({left} ledgers away, ~{})",
                    approximate_duration(left as u64)
                ))
            ),
        ),
        (Some(until), Some(left)) => style::field(
            "live until",
            &format!(
                "ledger {until}  {}",
                style::bad(&format!(
                    "(EXPIRED — {} ledgers ago; the entry may be archived)",
                    left.abs()
                ))
            ),
        ),
        _ if archived => style::field(
            "live until",
            &style::bad("expired — the entry has no live TTL and may be archived"),
        ),
        _ => {
            if entry.durability == "instance" {
                style::field(
                    "live until",
                    &style::muted("(the contract instance reported no TTL)"),
                );
            }
        }
    }

    style::field("as of", &format!("ledger {latest}"));

    if entry.durability == "instance" {
        println!();
        println!(
            "{}",
            style::muted(
                "Found in the contract's instance storage, not as a standalone entry. Values \
                 written with env.storage().instance() live inside the contract instance, so \
                 the TTL above is the instance's own — they expire together."
            )
        );
    }
}

fn report_missing(contract_id: &str, key: &ScVal, latest: u32, json: bool) {
    if json {
        println!(
            "{}",
            json!({
                "contractId": contract_id,
                "key": scval_to_json(key),
                "found": false,
                "latestLedger": latest,
            })
        );
        return;
    }

    println!(
        "{}",
        style::warn(&format!(
            "No entry found for key {} on {contract_id}.",
            scval_to_readable(key)
        ))
    );
    println!();
    println!(
        "{}",
        style::muted(
            "Checked persistent storage, temporary storage, and the contract's instance \
             storage. An entry can also be missing because it expired and was archived — \
             Soroban reclaims entries whose TTL has run out."
        )
    );
}

/// Look a key up inside a contract instance's own storage map.
///
/// Split out from `run` so it can be tested without a live contract: instance storage is
/// the one lookup path here that no contract currently to hand actually exercises.
fn find_in_instance_storage(storage: &[(ScVal, ScVal)], key: &ScVal) -> Option<ScVal> {
    storage
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// Round a ledger count into something a person can judge at a glance.
fn approximate_duration(ledgers: u64) -> String {
    let seconds = ledgers * SECONDS_PER_LEDGER;
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        format!("{}m", (seconds % 3_600) / 60)
    }
}

fn contract_data_key(
    contract: &ScAddress,
    key: &ScVal,
    durability: ContractDataDurability,
) -> LedgerKey {
    LedgerKey::ContractData(LedgerKeyContractData {
        contract: contract.clone(),
        key: key.clone(),
        durability,
    })
}

fn parse_contract(contract_id: &str) -> anyhow::Result<ScAddress> {
    let address = ScAddress::from_str(contract_id)
        .with_context(|| format!("{contract_id} is not a valid Stellar address"))?;

    match address {
        ScAddress::Contract(_) => Ok(address),
        _ => Err(anyhow!(
            "{contract_id} is an account address, not a contract.\n\n\
             Contract storage lives under a contract ID, which starts with C."
        )),
    }
}

fn build_key(
    key_symbol: Option<String>,
    key_address: Option<String>,
    key_xdr: Option<String>,
) -> anyhow::Result<ScVal> {
    match (key_symbol, key_address, key_xdr) {
        (Some(symbol), _, _) => {
            let inner = symbol.as_str().try_into().map_err(|_| {
                anyhow!(
                    "{symbol:?} is not a valid Symbol.\n\n\
                     Symbols are at most 32 characters of [a-zA-Z0-9_]."
                )
            })?;
            Ok(ScVal::Symbol(ScSymbol(inner)))
        }

        (_, Some(address), _) => {
            let parsed = ScAddress::from_str(&address)
                .with_context(|| format!("{address} is not a valid Stellar address"))?;
            Ok(ScVal::Address(parsed))
        }

        (_, _, Some(xdr)) => ScVal::from_xdr_base64(&xdr, Limits::none())
            .map_err(|_| anyhow!("--key-xdr is not valid base64-encoded ScVal XDR")),

        (None, None, None) => Err(anyhow!(
            "no key given.\n\n\
             This command reads one entry you already know the key for, so you must say \
             which:\n  \
             --key-symbol COUNTER      a bare Symbol key\n  \
             --key-address G...        a bare Address key\n  \
             --key-xdr <base64>        any other key shape, as raw ScVal XDR\n\n\
             There is no RPC call that lists a contract's storage, so there is no way to \
             show you the available keys."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_client::xdr::ScSymbol;

    fn sym(s: &str) -> ScVal {
        ScVal::Symbol(ScSymbol(s.try_into().unwrap()))
    }

    #[test]
    fn instance_storage_lookup_matches_on_key() {
        let storage = vec![
            (sym("OTHER"), ScVal::U32(1)),
            (sym("COUNTER"), ScVal::U32(9)),
        ];
        assert_eq!(
            find_in_instance_storage(&storage, &sym("COUNTER")),
            Some(ScVal::U32(9))
        );
    }

    #[test]
    fn instance_storage_lookup_misses_cleanly() {
        let storage = vec![(sym("OTHER"), ScVal::U32(1))];
        assert_eq!(find_in_instance_storage(&storage, &sym("COUNTER")), None);
        assert_eq!(find_in_instance_storage(&[], &sym("COUNTER")), None);
    }

    /// Keys are compared as whole ScVals, so a Symbol and a String that read the same are
    /// still different keys — which is exactly how the ledger treats them.
    #[test]
    fn instance_storage_lookup_distinguishes_symbol_from_string() {
        use soroban_client::xdr::ScString;
        let storage = vec![(
            ScVal::String(ScString("COUNTER".try_into().unwrap())),
            ScVal::U32(9),
        )];
        assert_eq!(find_in_instance_storage(&storage, &sym("COUNTER")), None);
    }

    #[test]
    fn durations_read_at_a_glance() {
        assert_eq!(approximate_duration(17_280), "1d 0h");
        assert_eq!(approximate_duration(720), "1h");
        assert_eq!(approximate_duration(12), "1m");
    }

    #[test]
    fn a_key_is_required() {
        assert!(build_key(None, None, None).is_err());
    }

    #[test]
    fn oversized_symbols_are_rejected_not_truncated() {
        assert!(build_key(Some("A".repeat(33)), None, None).is_err());
        assert!(build_key(Some("A".repeat(32)), None, None).is_ok());
    }

    #[test]
    fn account_addresses_are_rejected_as_contracts() {
        assert!(
            parse_contract("GCTWQOEUM67COLWIAVTMBE2J2NG7GHCBHFVWIOS6XDVXCJIORSYG3GZB").is_err()
        );
        assert!(parse_contract("CB6L3DIL5IHX7PCVHGJKYDZROSEHS5CGHKSMD6CGKV5HW7RDXY5NOBCX").is_ok());
        assert!(parse_contract("not-an-address").is_err());
    }
}
