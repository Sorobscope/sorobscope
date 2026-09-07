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

    /// Environment variable that supplies this network's RPC URL when `--rpc-url` isn't
    /// given.
    ///
    /// Per-network rather than one global override, so a mainnet endpoint can be set
    /// permanently without also hijacking testnet, which has a perfectly good default.
    pub fn env_var(self) -> &'static str {
        match self {
            Network::Testnet => "SOROBSCOPE_TESTNET_RPC_URL",
            Network::Futurenet => "SOROBSCOPE_FUTURENET_RPC_URL",
            Network::Mainnet => "SOROBSCOPE_MAINNET_RPC_URL",
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

/// Pick the endpoint for `network`, in precedence order.
///
/// An explicit `--rpc-url` wins over the environment, which wins over the built-in default
/// — the usual convention, and it keeps a one-off override possible on a machine that has
/// the variable set permanently. An empty variable counts as unset rather than as an empty
/// URL, since that is almost always an unset shell variable rather than intent.
///
/// Split out from [`Connection::open`] so the precedence can be tested without mutating
/// the process environment, which is global and would race across parallel tests.
fn resolve_url(network: Network, flag: Option<&str>, from_env: Option<&str>) -> Option<String> {
    if let Some(explicit) = flag {
        return Some(explicit.to_string());
    }
    if let Some(value) = from_env
        && !value.trim().is_empty()
    {
        return Some(value.to_string());
    }
    network.default_rpc_url().map(str::to_string)
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
        let from_env = std::env::var(network.env_var()).ok();
        let url = resolve_url(network, rpc_url, from_env.as_deref())
            .ok_or(RpcFailure::NoDefaultEndpoint { network })?;

        // Plain HTTP is refused by default. We relax it only for a URL the user supplied
        // themselves — by flag or by environment — which is the local-instance case
        // (http://localhost:8000). The built-in defaults are all HTTPS and stay that way.
        let user_supplied = rpc_url.is_some() || from_env.is_some();
        let allow_http = user_supplied && url.starts_with("http://");

        let options = Options {
            allow_http,
            timeout: TIMEOUT_SECS,
            headers: HashMap::new(),
            friendbot_url: None,
        };

        let server = Server::new(&url, options).map_err(|e| RpcFailure::classify(e, &url))?;

        Ok(Connection {
            server,
            url,
            network,
        })
    }

    /// Attach this connection's endpoint to an error from the client crate.
    pub fn fail(&self, err: soroban_client::error::Error) -> RpcFailure {
        RpcFailure::classify(err, &self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_flag_beats_everything() {
        let url = resolve_url(
            Network::Testnet,
            Some("https://flag.example"),
            Some("https://env.example"),
        );
        assert_eq!(url.as_deref(), Some("https://flag.example"));
    }

    #[test]
    fn the_environment_beats_the_built_in_default() {
        let url = resolve_url(Network::Testnet, None, Some("https://env.example"));
        assert_eq!(url.as_deref(), Some("https://env.example"));
    }

    #[test]
    fn the_default_applies_when_nothing_is_supplied() {
        let url = resolve_url(Network::Testnet, None, None);
        assert_eq!(url.as_deref(), Some("https://soroban-testnet.stellar.org"));
    }

    /// An empty variable is an unset shell variable far more often than it is intent, and
    /// treating it as a URL would produce a baffling connection error.
    #[test]
    fn an_empty_environment_variable_counts_as_unset() {
        assert_eq!(
            resolve_url(Network::Testnet, None, Some("")).as_deref(),
            Some("https://soroban-testnet.stellar.org")
        );
        assert_eq!(
            resolve_url(Network::Testnet, None, Some("   ")).as_deref(),
            Some("https://soroban-testnet.stellar.org")
        );
    }

    /// Mainnet has no built-in default on purpose, so with nothing supplied it must stay
    /// unresolved rather than inventing an endpoint.
    #[test]
    fn mainnet_has_no_default_but_accepts_an_override() {
        assert_eq!(resolve_url(Network::Mainnet, None, None), None);
        assert_eq!(
            resolve_url(Network::Mainnet, None, Some("https://mainnet.example")).as_deref(),
            Some("https://mainnet.example")
        );
    }

    #[test]
    fn each_network_reads_its_own_variable() {
        assert_eq!(Network::Mainnet.env_var(), "SOROBSCOPE_MAINNET_RPC_URL");
        assert_eq!(Network::Testnet.env_var(), "SOROBSCOPE_TESTNET_RPC_URL");
        assert_eq!(Network::Futurenet.env_var(), "SOROBSCOPE_FUTURENET_RPC_URL");
    }
}
