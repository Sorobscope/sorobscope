//! Network selection and RPC client construction.
//!
//! Every command gets its `Server` from [`Connection::open`] rather than building one
//! itself, so RPC URLs live in exactly one place and `--rpc-url` works everywhere for free.

use std::collections::HashMap;

use clap::ValueEnum;
// `stellar-baselib` is a transitive dependency, not a direct one; soroban-client
// re-exports it wholesale, so reach the passphrase constants through that.
use soroban_client::network::{NetworkPassphrase, Networks};
use soroban_client::{Options, Server};

use crate::error::RpcFailure;

/// How long to wait on an RPC call. The client crate defaults to 10s, which is tight for
/// `getEvents` over a wide ledger range on a busy public endpoint.
const TIMEOUT_SECS: u64 = 30;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Testnet,
    Futurenet,
    Mainnet,
}

impl Network {
    pub fn name(self) -> &'static str {
        match self {
            Network::Testnet => "testnet",
            Network::Futurenet => "futurenet",
            Network::Mainnet => "mainnet",
        }
    }

    /// The endpoint to use when `--rpc-url` wasn't given.
    ///
    /// `None` for mainnet, and that is deliberate rather than an omission: the SDF runs
    /// public RPC for the test networks as a developer convenience, but there is no
    /// equivalent free public mainnet endpoint to default to. Guessing one would hand the
    /// user a confusing connection error; saying so plainly points them at the actual fix.
    pub fn default_rpc_url(self) -> Option<&'static str> {
        match self {
            Network::Testnet => Some("https://soroban-testnet.stellar.org"),
            Network::Futurenet => Some("https://rpc-futurenet.stellar.org"),
            Network::Mainnet => None,
        }
    }

    /// The network passphrase.
    ///
    /// Unused today, and worth explaining why it's here anyway: a passphrase identifies
    /// which network a *signature* is valid for, and sorobscope never signs anything. It
    /// is kept because it names the network unambiguously — useful for a future
    /// `--json` field or a "connected to..." line — and because the alternative is
    /// rediscovering these constants later.
    #[allow(dead_code)]
    pub fn passphrase(self) -> &'static str {
        match self {
            Network::Testnet => Networks::testnet(),
            Network::Futurenet => Networks::futurenet(),
            Network::Mainnet => Networks::public(),
        }
    }
}

/// A configured RPC client plus the context needed to report failures usefully.
///
/// The URL is carried alongside the `Server` because the client crate's errors don't
/// mention which endpoint they came from, and "couldn't reach the RPC" is much less
/// helpful without naming the address that didn't answer.
pub struct Connection {
    pub server: Server,
    pub url: String,
    pub network: Network,
}

impl Connection {
    /// Build the client for `network`, honouring an explicit `--rpc-url` override.
    ///
    /// Note this does not make a request — it only constructs. Nothing here proves the
    /// endpoint is alive; the first real call is what surfaces an unreachable host.
    pub fn open(network: Network, rpc_url: Option<&str>) -> Result<Self, RpcFailure> {
        let url = match rpc_url {
            Some(explicit) => explicit.to_string(),
            None => network
                .default_rpc_url()
                .ok_or(RpcFailure::NoDefaultEndpoint { network })?
                .to_string(),
        };

        // Plain HTTP is refused by default. We relax that only for an explicitly supplied
        // --rpc-url, which is the local-instance case (http://localhost:8000); the built-in
        // defaults are all HTTPS and stay that way.
        let allow_http = rpc_url.is_some() && url.starts_with("http://");

        let options = Options {
            allow_http,
            timeout: TIMEOUT_SECS,
            headers: HashMap::new(),
            friendbot_url: None,
        };

        let server = Server::new(&url, options).map_err(|e| RpcFailure::classify(e, &url))?;

        Ok(Connection { server, url, network })
    }

    /// Attach this connection's endpoint to an error from the client crate.
    pub fn fail(&self, err: soroban_client::error::Error) -> RpcFailure {
        RpcFailure::classify(err, &self.url)
    }
}
