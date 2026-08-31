//! `sorobscope tx <hash>` — decode a transaction and the events it emitted.

use serde_json::json;
use soroban_client::soroban_rpc::{GetTransactionResponse, TransactionStatus};
use soroban_client::xdr::{
    ContractEvent, ContractEventBody, HostFunction, OperationBody, ScVal, TransactionEnvelope,
};

use crate::decode::{scval_to_json, scval_to_readable};
use crate::outcome::Outcome;
use crate::style;
use crate::network::Connection;

pub async fn run(connection: &Connection, hash: &str, json: bool) -> anyhow::Result<Outcome> {
    let response = connection
        .server
        .get_transaction(hash)
        .await
        .map_err(|e| connection.fail(e))?;

    if response.status == TransactionStatus::NotFound {
        report_missing(hash, &response, json);
        return Ok(Outcome::NotFound);
    }

    let invocation = find_invocation(&response);
    let (_, return_value) = response.to_result_meta().unzip();
    let return_value = return_value.flatten();
    let events = contract_events(&response);

    if json {
        report_json(connection, hash, &response, invocation, return_value, &events);
    } else {
        report_readable(connection, hash, &response, invocation, return_value, &events);
    }

    Ok(Outcome::Found)
}

/// What the transaction actually asked the network to do.
enum Call {
    /// A contract function call — the case this tool exists for.
    Contract(Invocation),
    /// A Soroban host function that isn't a contract call: a deploy, a wasm upload.
    /// Named rather than described vaguely, since the RPC tells us exactly which.
    OtherHostFunction(&'static str),
    /// No Soroban operation at all — a payment, a trustline change, and so on.
    NonSoroban,
}

struct Invocation {
    contract: String,
    function: String,
    args: Vec<ScVal>,
}

/// Dig the contract call out of the transaction envelope.
///
/// `getTransaction` doesn't hand back a "which function was called" field — that lives
/// inside the envelope XDR, under the operation's host function. A transaction can carry
/// up to 100 operations, but a Soroban contract call is one `InvokeHostFunction`, so the
/// first one is the interesting one.
fn find_invocation(response: &GetTransactionResponse) -> Call {
    let Some(envelope) = response.to_envelope() else {
        return Call::NonSoroban;
    };

    let operations = match &envelope {
        TransactionEnvelope::Tx(v1) => v1.tx.operations.to_vec(),
        TransactionEnvelope::TxV0(v0) => v0.tx.operations.to_vec(),
        // A fee-bump wraps an inner transaction; the real operations are one level down.
        TransactionEnvelope::TxFeeBump(bump) => match &bump.tx.inner_tx {
            soroban_client::xdr::FeeBumpTransactionInnerTx::Tx(inner) => {
                inner.tx.operations.to_vec()
            }
        },
    };

    let host_function = operations.iter().find_map(|op| match &op.body {
        OperationBody::InvokeHostFunction(invoke) => Some(&invoke.host_function),
        _ => None,
    });

    match host_function {
        Some(HostFunction::InvokeContract(args)) => Call::Contract(Invocation {
            contract: args.contract_address.to_string(),
            function: args.function_name.0.to_utf8_string_lossy(),
            args: args.args.to_vec(),
        }),
        Some(HostFunction::CreateContract(_)) => Call::OtherHostFunction("CreateContract"),
        Some(HostFunction::CreateContractV2(_)) => Call::OtherHostFunction("CreateContractV2"),
        Some(HostFunction::UploadContractWasm(_)) => {
            Call::OtherHostFunction("UploadContractWasm")
        }
        None => Call::NonSoroban,
    }
}

/// Contract events emitted by this transaction, flattened across operations.
fn contract_events(response: &GetTransactionResponse) -> Vec<ContractEvent> {
    match response.to_events() {
        Some((_, per_operation)) => per_operation.into_iter().flatten().collect(),
        None => Vec::new(),
    }
}

fn event_parts(event: &ContractEvent) -> (Vec<ScVal>, ScVal) {
    match &event.body {
        ContractEventBody::V0(body) => (body.topics.to_vec(), body.data.clone()),
    }
}

fn report_readable(
    connection: &Connection,
    hash: &str,
    response: &GetTransactionResponse,
    invocation: Call,
    return_value: Option<ScVal>,
    events: &[ContractEvent],
) {
    println!(
        "{}",
        style::heading(&format!(
            "transaction {hash} on {}",
            connection.network.name()
        ))
    );

    let word = status_word(&response.status);
    let status = match response.status {
        TransactionStatus::Success => style::good(word),
        TransactionStatus::Failed => style::bad(word),
        TransactionStatus::NotFound => style::warn(word),
    };
    style::field("status", &status);

    if let Some(ledger) = response.ledger {
        style::field("ledger", &ledger.to_string());
    }
    if let Some(created) = &response.created_at {
        match created.parse::<i64>() {
            Ok(secs) => style::field("at", &iso_utc(secs)),
            Err(_) => style::field("at", created),
        }
    }

    println!();
    match invocation {
        Call::Contract(call) => {
            style::field("contract", &call.contract);
            style::field("function", &call.function);
            if call.args.is_empty() {
                style::field("args", &style::muted("(none)"));
            } else {
                for (i, arg) in call.args.iter().enumerate() {
                    let rendered = scval_to_readable(arg);
                    if i == 0 {
                        style::field("args", &rendered);
                    } else {
                        style::field_continued(&rendered);
                    }
                }
            }
        }
        Call::OtherHostFunction(name) => {
            style::field("operation", name);
            style::field_continued(&style::muted(
                "(a Soroban host function, but not a contract call)",
            ));
        }
        Call::NonSoroban => {
            style::field("operation", "not a Soroban operation");
            style::field_continued(&style::muted(
                "(a payment or other classic Stellar operation)",
            ));
        }
    }

    if let Some(value) = &return_value {
        style::field("returned", &scval_to_readable(value));
    }

    println!();
    if events.is_empty() {
        style::field("events", &style::muted("(none)"));
    } else {
        style::field("events", &format!("{}", events.len()));
        for event in events {
            let (topics, data) = event_parts(event);
            let rendered: Vec<String> = topics.iter().map(scval_to_readable).collect();
            style::field_continued(&format!(
                "[{}]  {}",
                rendered.join(", "),
                scval_to_readable(&data)
            ));
        }
    }

    if response.status == TransactionStatus::Failed {
        println!();
        println!(
            "{}",
            style::muted(
                "This transaction was included in a ledger but failed. Its effects were rolled \
                 back, so any events above describe work that did not stick."
            )
        );
    }
}

fn report_json(
    connection: &Connection,
    hash: &str,
    response: &GetTransactionResponse,
    invocation: Call,
    return_value: Option<ScVal>,
    events: &[ContractEvent],
) {
    let events_json: Vec<_> = events
        .iter()
        .map(|event| {
            let (topics, data) = event_parts(event);
            json!({
                "topics": topics.iter().map(scval_to_json).collect::<Vec<_>>(),
                "data": scval_to_json(&data),
            })
        })
        .collect();

    let invocation_json = match invocation {
        Call::Contract(call) => json!({
            "type": "invokeContract",
            "contractId": call.contract,
            "function": call.function,
            "args": call.args.iter().map(scval_to_json).collect::<Vec<_>>(),
        }),
        Call::OtherHostFunction(name) => json!({"type": name}),
        Call::NonSoroban => json!({"type": "nonSoroban"}),
    };

    println!(
        "{}",
        json!({
            "hash": hash,
            "network": connection.network.name(),
            "status": status_word(&response.status),
            "ledger": response.ledger,
            "createdAt": response.created_at,
            "invocation": invocation_json,
            "returnValue": return_value.as_ref().map(scval_to_json),
            "events": events_json,
            "latestLedger": response.latest_ledger,
            "oldestLedger": response.oldest_ledger,
        })
    );
}

fn report_missing(hash: &str, response: &GetTransactionResponse, json: bool) {
    if json {
        println!(
            "{}",
            json!({
                "hash": hash,
                "status": "NOT_FOUND",
                "found": false,
                "oldestLedger": response.oldest_ledger,
                "latestLedger": response.latest_ledger,
            })
        );
        return;
    }

    println!(
        "{}",
        style::warn(&format!("Transaction {hash} was not found."))
    );
    println!();
    println!(
        "This endpoint currently retains ledgers {} to {}.",
        response.oldest_ledger, response.latest_ledger
    );
    println!();
    // The RPC reports a transaction it never had and one it has since dropped
    // identically, as NOT_FOUND. It cannot tell us which — but the retained range above
    // usually can, so state it rather than pretending to a certainty we don't have.
    println!(
        "{}",
        style::muted(&format!(
            "Soroban RPC reports both \"no such transaction\" and \"older than this endpoint \
             retains\" the same way, so this could be either. If the transaction closed before \
             ledger {}, it is outside the window and no longer queryable here — point --rpc-url \
             at an endpoint with deeper history, or use an indexer.",
            response.oldest_ledger
        ))
    );
}

fn status_word(status: &TransactionStatus) -> &'static str {
    match status {
        TransactionStatus::Success => "SUCCESS",
        TransactionStatus::Failed => "FAILED",
        TransactionStatus::NotFound => "NOT_FOUND",
    }
}

/// Format a Unix timestamp as an ISO-8601 UTC string.
///
/// Hand-rolled rather than pulling in a date crate for one line of output. Uses Howard
/// Hinnant's civil-from-days algorithm, which is exact for the whole range we care about
/// and avoids a dependency whose only job would be this.
fn iso_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);

    let (hour, minute, second) = (
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60,
    );

    // Shift the epoch to 0000-03-01 so leap days land at the end of the cycle.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;

    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_utc_matches_known_timestamps() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        // A leap day, which is where a naive conversion usually slips.
        assert_eq!(iso_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn iso_utc_handles_pre_epoch_times() {
        assert_eq!(iso_utc(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn status_words_are_stable() {
        assert_eq!(status_word(&TransactionStatus::Success), "SUCCESS");
        assert_eq!(status_word(&TransactionStatus::Failed), "FAILED");
        assert_eq!(status_word(&TransactionStatus::NotFound), "NOT_FOUND");
    }
}
