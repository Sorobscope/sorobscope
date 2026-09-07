//! User-facing failure types.
//!
//! The reason this module exists at all: "I couldn't reach the endpoint", "the endpoint
//! answered with an error", and "the endpoint answered fine but I couldn't decode what it
//! sent" are three genuinely different problems with three different fixes. Collapsing
//! them into one "something went wrong" would push the diagnosis back onto the user.

use std::fmt;

use soroban_client::error::Error as ClientError;

use crate::network::Network;

/// Anything that can go wrong between "user typed a command" and "we hold a decoded
/// response". Each variant maps to a distinct thing the user would do about it.
#[derive(Debug)]
pub enum RpcFailure {
    /// The chosen network has no default public endpoint, so `--rpc-url` is required.
    NoDefaultEndpoint { network: Network },

    /// The URL itself is malformed or otherwise unusable — we never got as far as sending.
    BadUrl { url: String, reason: String },

    /// We couldn't get a response at all: DNS, TLS, connection refused, timeout.
    Unreachable { url: String, detail: String },

    /// We reached the RPC and it answered with a JSON-RPC error. The server is fine; it's
    /// telling us the request was wrong or couldn't be served.
    Rpc {
        url: String,
        code: i32,
        message: String,
    },

    /// We got a response but couldn't turn it into something meaningful. Either the XDR
    /// didn't decode or the JSON wasn't the shape we expected — usually a version skew
    /// between this tool and the endpoint.
    Decode { url: String, detail: String },

    /// Anything the client crate reports that doesn't fit above.
    Other { url: String, detail: String },
}

impl RpcFailure {
    /// Translate a `soroban-client` error into the category the user needs to see.
    pub fn classify(err: ClientError, url: &str) -> Self {
        let url = url.to_string();
        match err {
            ClientError::InvalidRpc(e) => RpcFailure::BadUrl {
                url,
                reason: e.to_string(),
            },

            // Note: the payload here is a `reqwest::Error`, but `reqwest` is only a
            // transitive dependency. Calling its methods needs no import; naming its type
            // in a signature would, so the classification stays inline rather than pulling
            // in a direct dependency pinned to whatever soroban-client happens to use.
            ClientError::NetworkError(e) => {
                let what = if e.is_timeout() {
                    "the request timed out"
                } else if e.is_connect() {
                    "could not open a connection (DNS, TLS, or connection refused)"
                } else if e.is_request() {
                    "the request could not be sent"
                } else if e.is_body() || e.is_decode() {
                    "the response body could not be read"
                } else {
                    "the request failed"
                };

                let detail = match deepest_cause(&e) {
                    Some(cause) => format!("{what}: {cause}"),
                    None => what.to_string(),
                };
                RpcFailure::Unreachable { url, detail }
            }

            ClientError::RPCError { code, message } => RpcFailure::Rpc { url, code, message },

            ClientError::XdrError => RpcFailure::Decode {
                url,
                detail: "the response contained XDR this tool could not decode".to_string(),
            },
            // The payload here is the raw response body, which for a wrong-endpoint
            // mistake is a whole HTML page. Clamp it: enough to recognise what answered,
            // not enough to bury the message under someone's landing page.
            ClientError::JsonError(what) => RpcFailure::Decode {
                url,
                detail: format!("could not parse the response body: {}", summarize(&what)),
            },

            other => RpcFailure::Other {
                url,
                detail: other.to_string(),
            },
        }
    }
}

/// How much of an unparseable response body to echo back.
const BODY_EXCERPT: usize = 160;

/// Collapse a response body to a single readable line. Whitespace runs become single
/// spaces so an HTML page doesn't sprawl over the terminal, and anything past
/// [`BODY_EXCERPT`] is cut — the first line is enough to tell "this is an HTML error page"
/// from "this is JSON with unexpected fields", which is the distinction that matters.
fn summarize(body: &str) -> String {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");

    if flat.is_empty() {
        return "(empty response)".to_string();
    }

    match flat.char_indices().nth(BODY_EXCERPT) {
        Some((cut, _)) => format!("{}…", &flat[..cut]),
        None => flat,
    }
}

/// Walk to the innermost source of an error chain. That bottom link is usually the
/// specific cause ("dns error", "tcp connect error"), which is exactly what tells a DNS
/// failure apart from a refused port — a distinction the outer layers smooth over.
fn deepest_cause(e: &dyn std::error::Error) -> Option<String> {
    let mut source = e.source();
    let mut deepest = None;
    while let Some(s) = source {
        deepest = Some(s.to_string());
        source = s.source();
    }
    deepest
}

impl fmt::Display for RpcFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcFailure::NoDefaultEndpoint { network } => {
                write!(
                    f,
                    "no default RPC endpoint for {}\n\n\
                     There is no free public {} RPC to fall back on, so you need to name \
                     one yourself:\n  \
                     --rpc-url https://your-rpc-provider.example\n\n\
                     Use a commercial RPC provider, or run your own instance.",
                    network.name(),
                    network.name(),
                )
            }

            RpcFailure::BadUrl { url, reason } => {
                write!(
                    f,
                    "unusable RPC URL: {url}\n\n\
                     {reason}\n\n\
                     Expected something like https://soroban-testnet.stellar.org"
                )
            }

            RpcFailure::Unreachable { url, detail } => {
                write!(
                    f,
                    "could not reach the RPC endpoint at {url}\n\n\
                     {detail}\n\n\
                     The endpoint never answered, so this is a connectivity or address \
                     problem rather than anything to do with the contract you asked about. \
                     Check the URL and your network."
                )
            }

            RpcFailure::Rpc { url, code, message } => {
                write!(
                    f,
                    "the RPC endpoint at {url} rejected the request\n\n\
                     JSON-RPC error {code}: {message}\n\n\
                     The endpoint is reachable and healthy — it declined this particular \
                     request. Check the arguments you passed."
                )
            }

            RpcFailure::Decode { url, detail } => {
                write!(
                    f,
                    "could not decode the response from {url}\n\n\
                     {detail}\n\n\
                     Something answered, so this is not a connectivity problem. Either the \
                     URL isn't a Soroban RPC endpoint, or it speaks a different protocol \
                     version than this build of sorobscope expects."
                )
            }

            RpcFailure::Other { url, detail } => {
                write!(f, "unexpected failure talking to {url}\n\n{detail}")
            }
        }
    }
}

impl std::error::Error for RpcFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_collapses_whitespace() {
        assert_eq!(
            summarize("  <html>\n  <body>\ttext  "),
            "<html> <body> text"
        );
    }

    #[test]
    fn summarize_reports_empty_bodies() {
        assert_eq!(summarize(""), "(empty response)");
        assert_eq!(summarize("   \n\t "), "(empty response)");
    }

    #[test]
    fn summarize_keeps_short_bodies_intact() {
        let short = "{\"jsonrpc\":\"2.0\"}";
        assert_eq!(summarize(short), short);
    }

    #[test]
    fn summarize_truncates_long_bodies() {
        let out = summarize(&"a".repeat(BODY_EXCERPT * 2));
        assert!(out.ends_with('…'));
        assert_eq!(out.chars().count(), BODY_EXCERPT + 1);
    }

    /// Truncation is by character, not byte. Slicing a multi-byte body at a byte offset
    /// would panic mid-codepoint, and an RPC returning non-ASCII is not exotic.
    #[test]
    fn summarize_does_not_split_multibyte_characters() {
        let out = summarize(&"é".repeat(BODY_EXCERPT * 2));
        assert!(out.ends_with('…'));
        assert_eq!(out.chars().count(), BODY_EXCERPT + 1);
    }
}
