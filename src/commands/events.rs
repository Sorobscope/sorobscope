//! `sorobscope events <contract-id>` — fetch a contract's events and decode them.

use std::time::Duration;

use serde_json::json;
use soroban_client::soroban_rpc::EventResponse;
use soroban_client::{EventFilter, Pagination};
use soroban_client::soroban_rpc::EventType;

use crate::decode::{guard, scval_to_json, scval_to_readable};
use crate::outcome::Outcome;
use crate::network::Connection;

/// How far back to look when `--since-ledger` isn't given. Ledgers close about every 5
/// seconds, so this is roughly a day. Deliberately well inside a typical retention window:
/// asking for more than the endpoint retains is an error, not an empty result, and a
/// default that errors on a fresh install would be a bad first impression.
const DEFAULT_LOOKBACK_LEDGERS: u32 = 17_280;

/// Gap between polls under `--follow`, matched to roughly one ledger close.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Events per request.
const PAGE_LIMIT: u32 = 100;

pub async fn run(
    connection: &Connection,
    contract_id: &str,
    since_ledger: Option<u32>,
    follow: bool,
    json: bool,
) -> anyhow::Result<Outcome> {
    let latest = connection
        .server
        .get_latest_ledger()
        .await
        .map_err(|e| connection.fail(e))?
        .sequence;

    let start = since_ledger.unwrap_or_else(|| latest.saturating_sub(DEFAULT_LOOKBACK_LEDGERS));

    if !json {
        println!("events for {contract_id} on {}", connection.network.name());
        match since_ledger {
            Some(l) => println!("searching from ledger {l} to {latest}\n"),
            None => println!(
                "searching from ledger {start} to {latest} (last ~{} ledgers; pass --since-ledger to widen)\n",
                DEFAULT_LOOKBACK_LEDGERS
            ),
        }
    }

    // Paging matters more than it looks. getEvents scans only a bounded slice of ledgers
    // per request, so a single call does NOT cover the whole requested range — it can
    // return zero events and a cursor while real events sit further along. Stopping after
    // one response silently under-reports. So we page until the cursor reaches the head of
    // the chain, and only then consider the range fully searched.
    let mut cursor: Option<String> = None;
    let mut next_from = start;
    let mut seen = 0usize;

    loop {
        // Neither Pagination nor EventFilter is Clone, so both are rebuilt each pass.
        // They're plain value types — this costs nothing next to the round trip.
        let page = match &cursor {
            Some(c) => Pagination::Cursor(c.clone()),
            None => Pagination::From(next_from),
        };
        let filter = EventFilter::new(EventType::Contract).contract(contract_id);

        let response = connection
            .server
            .get_events(page, vec![filter], PAGE_LIMIT)
            .await
            .map_err(|e| connection.fail(e))?;

        for event in &response.events {
            if json {
                println!("{}", event_to_json(event));
            } else {
                print_event(event);
            }
            seen += 1;
        }

        let head = response.latest_ledger as u32;

        match response.cursor.clone() {
            Some(next) => cursor = Some(next),
            // Fallback for an endpoint that returns no cursor: resume from just past the
            // last event we saw, so a --follow poll can't reprint the same page forever.
            None => {
                if let Some(last) = response.events.last() {
                    cursor = None;
                    next_from = last.ledger as u32 + 1;
                }
            }
        }

        // The cursor is a TOID whose high 32 bits are the ledger it reached. While that
        // trails the chain head, there is more of the requested range left to scan.
        let scanned_to = cursor.as_deref().and_then(cursor_ledger);
        let caught_up = scanned_to.is_none_or(|reached| reached >= head);

        if !caught_up {
            // Still working through the range — fetch the next page without waiting.
            continue;
        }

        if !follow {
            break;
        }

        // Wait out the interval, but let Ctrl-C win the race so the tool exits promptly
        // and cleanly instead of stranding the user mid-sleep.
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = tokio::signal::ctrl_c() => {
                if !json {
                    println!("\nstopped.");
                }
                return Ok(Outcome::Found);
            }
        }
    }

    if !json && seen == 0 {
        println!("No events found in that range.");
        println!(
            "The contract may simply not have emitted any — or the range predates what \
             this RPC endpoint still retains."
        );
    }

    // A search that matched nothing is not the same as a missing identifier: the range
    // was scanned successfully and the contract was simply quiet. That is a real answer,
    // so it stays exit 0 — unlike `entry` and `tx`, which look up one specific thing.
    Ok(Outcome::Found)
}

fn print_event(event: &EventResponse) {
    let topics = match decode_topics(event) {
        Some(t) => t.join(", "),
        None => "<undecodable>".to_string(),
    };
    let data = decode_value(event).unwrap_or_else(|| "<undecodable>".to_string());

    println!("ledger {}  {}", event.ledger, event.ledger_closed_at);
    println!("  topics  [{topics}]");
    println!("  data    {data}");
    println!("  tx      {}", event.tx_hash);
    println!();
}

fn event_to_json(event: &EventResponse) -> String {
    let value = json!({
        "ledger": event.ledger,
        "ledgerClosedAt": event.ledger_closed_at,
        "contractId": event.contract_id,
        "id": event.id,
        "txHash": event.tx_hash,
        "topics": decode_topics_json(event),
        "data": decode_value_json(event),
    });
    value.to_string()
}

/// Decode this event's topics, or `None` if the RPC handed back XDR we can't read.
fn decode_topics(event: &EventResponse) -> Option<Vec<String>> {
    guard(|| event.topic().iter().map(scval_to_readable).collect())
}

fn decode_value(event: &EventResponse) -> Option<String> {
    guard(|| scval_to_readable(&event.value()))
}

fn decode_topics_json(event: &EventResponse) -> serde_json::Value {
    match guard(|| {
        event
            .topic()
            .iter()
            .map(scval_to_json)
            .collect::<Vec<_>>()
    }) {
        Some(topics) => serde_json::Value::Array(topics),
        None => json!("<undecodable>"),
    }
}

fn decode_value_json(event: &EventResponse) -> serde_json::Value {
    guard(|| scval_to_json(&event.value())).unwrap_or_else(|| json!("<undecodable>"))
}

/// Pull the ledger sequence out of a paging cursor.
///
/// The cursor is `"<toid>-<event index>"`, and a TOID packs the ledger sequence into its
/// high 32 bits (`ledger << 32 | tx_index << 12 | op_index`). Comparing that against the
/// chain head is how we tell "this page was empty because the range is quiet" from "this
/// page was empty because the scan hasn't got there yet".
fn cursor_ledger(cursor: &str) -> Option<u32> {
    let toid: u64 = cursor.split('-').next()?.parse().ok()?;
    Some((toid >> 32) as u32)
}
