//! Chain-specific configuration types

pub mod ethereum;

pub mod substrate;

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Rpc style of chain
#[derive(Default, Copy, Clone, Debug, PartialEq, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RpcStyle {
    #[default]
    /// Ethereum
    Ethereum,
    /// Substrate
    Substrate,
}

impl FromStr for RpcStyle {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "ethereum" => Ok(Self::Ethereum),
            "substrate" => Ok(Self::Substrate),
            _ => panic!("Unknown RpcStyle"),
        }
    }
}

impl std::fmt::Display for RpcStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let style = match self {
            RpcStyle::Ethereum => "ethereum",
            RpcStyle::Substrate => "substrate",
        };

        write!(f, "{}", style)
    }
}

/// Chain connection configuration
#[derive(Debug, Clone, PartialEq)]
pub enum Connection {
    /// HTTP connection details
    Http(
        /// Fully qualified URI to connect to
        String,
    ),
    /// Websocket connection details
    Ws(
        /// Fully qualified URI to connect to
        String,
    ),
}

impl Connection {
    fn from_string(s: String) -> eyre::Result<Self> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(Self::Http(s))
        } else if s.starts_with("wss://") || s.starts_with("ws://") {
            Ok(Self::Ws(s))
        } else {
            eyre::bail!("Expected http or websocket URI")
        }
    }
}

impl FromStr for Connection {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s.to_owned())
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self::Http(Default::default())
    }
}

impl<'de> serde::Deserialize<'de> for Connection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_string(s).map_err(serde::de::Error::custom)
    }
}

/// A connection to _some_ blockchain.
///
/// Specify the chain name (enum variant) in toml under the `chain` key
/// Specify the connection details as a toml object under the `connection` key.
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(tag = "rpcStyle", content = "connection", rename_all = "camelCase")]
pub enum ChainConf {
    /// Ethereum configuration
    Ethereum(Connection),
    /// Substrate configuration
    Substrate(Connection),
}

impl Default for ChainConf {
    fn default() -> Self {
        Self::Ethereum(Default::default())
    }
}

impl ChainConf {
    /// Build ChainConf from env vars. Will use default RPCSTYLE if
    /// network-specific not provided.
    pub fn from_env(network: &str) -> Option<Self> {
        let mut rpc_style = std::env::var(&format!("{}_RPCSTYLE", network)).ok();

        if rpc_style.is_none() {
            rpc_style = std::env::var("DEFAULT_RPCSTYLE").ok();
        }

        let rpc_url = std::env::var(&format!("{}_CONNECTION_URL", network)).ok()?;

        let json = json!({
            "rpcStyle": rpc_style,
            "connection": rpc_url,
        });

        Some(
            serde_json::from_value(json)
                .unwrap_or_else(|_| panic!("malformed json for {} rpc", network)),
        )
    }
}

/// Transaction submssion configuration for some chain.
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(tag = "rpcStyle", rename_all = "camelCase")]
pub enum TxSubmitterConf {
    /// Ethereum configuration
    Ethereum(ethereum::TxSubmitterConf),
    /// Substrate configuration
    Substrate(substrate::TxSubmitterConf),
}

impl TxSubmitterConf {
    /// Build TxSubmitterConf from env. Looks for default RPC style if
    /// network-specific not defined.
    pub fn from_env(network: &str) -> Option<Self> {
        let rpc_style = crate::utils::network_or_default_from_env(network, "RPCSTYLE")?;

        match RpcStyle::from_str(&rpc_style).unwrap() {
            RpcStyle::Ethereum => Some(Self::Ethereum(ethereum::TxSubmitterConf::from_env(
                network,
            )?)),
            RpcStyle::Substrate => Some(Self::Substrate(substrate::TxSubmitterConf::from_env(
                network,
            )?)),
        }
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::Connection;

    #[test]
    fn it_desers_rpc_configs() {
        let value = json! {
            "https://google.com"
        };
        let connection: Connection = serde_json::from_value(value).unwrap();
        assert_eq!(
            connection,
            Connection::Http("https://google.com".to_owned())
        );
        let value = json! {
            "http://google.com"
        };
        let connection: Connection = serde_json::from_value(value).unwrap();
        assert_eq!(connection, Connection::Http("http://google.com".to_owned()));
        let value = json! {
            "wss://google.com"
        };
        let connection: Connection = serde_json::from_value(value).unwrap();
        assert_eq!(connection, Connection::Ws("wss://google.com".to_owned()));
        let value = json! {
            "ws://google.com"
        };
        let connection: Connection = serde_json::from_value(value).unwrap();
        assert_eq!(connection, Connection::Ws("ws://google.com".to_owned()));
    }
}
