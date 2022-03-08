#![allow(clippy::enum_variant_names)]
#![allow(missing_docs)]

use async_trait::async_trait;
use nomad_core::*;
use std::sync::Arc;

use crate::bindings::xappconnectionmanager::XAppConnectionManager as EthereumConnectionManagerInternal;

use crate::report_tx;

/// A reference to a XAppConnectionManager contract on some Ethereum chain
#[derive(Debug)]
pub struct EthereumConnectionManager<M>
where
    M: ethers::providers::Middleware,
{
    contract: EthereumConnectionManagerInternal<M>,
    provider: Arc<M>,
    domain: u32,
    name: String,
}

impl<M> EthereumConnectionManager<M>
where
    M: ethers::providers::Middleware,
{
    /// Create a reference to a XAppConnectionManager at a specific Ethereum
    /// address on some chain
    #[allow(dead_code)]
    pub fn new(
        provider: Arc<M>,
        ContractLocator {
            name,
            domain,
            address,
        }: &ContractLocator,
    ) -> Self {
        Self {
            contract: EthereumConnectionManagerInternal::new(address, provider.clone()),
            provider,
            domain: *domain,
            name: name.to_owned(),
        }
    }
}

#[async_trait]
impl<M> ConnectionManager for EthereumConnectionManager<M>
where
    M: ethers::providers::Middleware + 'static,
{
    fn local_domain(&self) -> u32 {
        self.domain
    }

    #[tracing::instrument(err)]
    async fn is_replica(&self, address: NomadIdentifier) -> Result<bool, ChainCommunicationError> {
        Ok(self
            .contract
            .is_replica(address.as_ethereum_address())
            .call()
            .await?)
    }

    #[tracing::instrument(err)]
    async fn watcher_permission(
        &self,
        address: NomadIdentifier,
        domain: u32,
    ) -> Result<bool, ChainCommunicationError> {
        Ok(self
            .contract
            .watcher_permission(address.as_ethereum_address(), domain)
            .call()
            .await?)
    }

    #[tracing::instrument(err)]
    async fn owner_enroll_replica(
        &self,
        replica: NomadIdentifier,
        domain: u32,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self
            .contract
            .owner_enroll_replica(replica.as_ethereum_address(), domain);

        let tx: TxOutcome = report_tx!(tx, &self.provider).into();
        tx.check()?;

        Ok(tx)
    }

    #[tracing::instrument(err)]
    async fn owner_unenroll_replica(
        &self,
        replica: NomadIdentifier,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self
            .contract
            .owner_unenroll_replica(replica.as_ethereum_address());

        let tx: TxOutcome = report_tx!(tx, &self.provider).into();
        tx.check()?;

        Ok(tx)
    }

    #[tracing::instrument(err)]
    async fn set_home(&self, home: NomadIdentifier) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self.contract.set_home(home.as_ethereum_address());

        let tx: TxOutcome = report_tx!(tx, &self.provider).into();
        tx.check()?;

        Ok(tx)
    }

    #[tracing::instrument(err)]
    async fn set_watcher_permission(
        &self,
        watcher: NomadIdentifier,
        domain: u32,
        access: bool,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let tx =
            self.contract
                .set_watcher_permission(watcher.as_ethereum_address(), domain, access);

        let tx: TxOutcome = report_tx!(tx, &self.provider).into();
        tx.check()?;

        Ok(tx)
    }

    #[tracing::instrument(err)]
    async fn unenroll_replica(
        &self,
        signed_failure: &SignedFailureNotification,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self.contract.unenroll_replica(
            signed_failure.notification.home_domain,
            signed_failure.notification.updater.into(),
            signed_failure.signature.to_vec().into(),
        );

        let tx: TxOutcome = report_tx!(tx, &self.provider).into();
        tx.check()?;

        Ok(tx)
    }
}
