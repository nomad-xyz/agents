use crate::{errors::Error, Environment, Result};
use reqwest;
use serde_yaml;
use std::collections::HashMap;

/// A model for our remote secrets file
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Secrets {
    /// Equivalent to `CONFIG_URL`
    pub config_url: String,
    /// Equivalent to `CONFIG_PATH`. Included for testing only
    pub config_path: Option<String>,
    /// Equivalent to the set of `<NETWORK>_CONNECTION_URL`
    pub connection_urls: HashMap<String, String>,
    /// Equivalent to the set of `<NETWORK>_TXSIGNER_ID`
    pub txsigner_ids: HashMap<String, String>,
    /// Equivalent to the set of `<NETWORK>_ATTESTATION_SIGNER_ID`
    pub attestation_signer_ids: HashMap<String, String>,
}

impl Secrets {
    /// Create a `Secrets` by fetching yaml from a remote URL
    pub(crate) async fn fetch(url: &str) -> Result<Self> {
        let bytes = reqwest::get(url)
            .await
            .map_err(Error::ReqwestError)?
            .bytes()
            .await
            .map_err(Error::ReqwestError)?;
        Ok(serde_yaml::from_slice::<Self>(&bytes[..]).map_err(Error::YamlBadDeser)?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use nomad_test::test_utils;
    use std::fs;

    #[tokio::test]
    #[serial_test::serial]
    async fn it_fetches_and_deserializes_secrets() {
        let secrets = fs::read_to_string("../../fixtures/killswitch_secrets.testing.yaml").unwrap();
        test_utils::run_test_with_http_response(secrets, "application/yaml", |url| async move {
            let secrets = Secrets::fetch(&url).await;
            assert!(secrets.is_ok())
        }).await;
    }
}
