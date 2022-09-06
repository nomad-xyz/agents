#![allow(clippy::enum_variant_names)]
#![allow(missing_docs)]

use async_trait::async_trait;
use color_eyre::Result;
use ethers::core::types::{Signature, H256, U256};
use futures_util::future::join_all;
use nomad_core::{
    accumulator::NomadProof, Common, CommonIndexer, ContractLocator, DoubleUpdate, Encode,
    MessageStatus, NomadMessage, Replica, SignedUpdate, SignedUpdateWithMeta, State, TxOutcome,
    Update, UpdateMeta,
};
use nomad_xyz_configuration::ReplicaGasLimits;
use std::{convert::TryFrom, sync::Arc};
use tracing::instrument;

use crate::{
    bindings::replica::Replica as EthereumReplicaInternal, utils, EthereumError, TxSubmitter,
};

#[derive(Debug)]
/// Struct that retrieves indexes event data for Ethereum replica
pub struct EthereumReplicaIndexer<R>
where
    R: ethers::providers::Middleware + 'static,
{
    contract: Arc<EthereumReplicaInternal<R>>,
    provider: Arc<R>,
}

impl<R> EthereumReplicaIndexer<R>
where
    R: ethers::providers::Middleware + 'static,
{
    /// Create new EthereumReplicaIndexer
    pub fn new(
        provider: Arc<R>,
        ContractLocator {
            name,
            domain,
            address,
        }: &ContractLocator,
    ) -> Self {
        tracing::info!(
            address = ?address.as_ethereum_address(),
            name = name,
            domain = domain,
            "Connecting Replica Indexer"
        );
        Self {
            contract: Arc::new(EthereumReplicaInternal::new(
                address.as_ethereum_address().expect("!eth address"),
                provider.clone(),
            )),
            provider,
        }
    }
}

#[async_trait]
impl<R> CommonIndexer for EthereumReplicaIndexer<R>
where
    R: ethers::providers::Middleware + 'static,
{
    type Error = EthereumError;

    #[instrument(err, skip(self))]
    async fn get_block_number(&self) -> Result<u32, Self::Error> {
        Ok(self
            .provider
            .get_block_number()
            .await
            .map_err(|e| EthereumError::MiddlewareError(e.into()))?
            .as_u32())
    }

    #[instrument(err, skip(self))]
    async fn fetch_sorted_updates(
        &self,
        from: u32,
        to: u32,
    ) -> Result<Vec<SignedUpdateWithMeta>, Self::Error> {
        let mut events = self
            .contract
            .update_filter()
            .from_block(from)
            .to_block(to)
            .query_with_meta()
            .await?;

        events.sort_by(|a, b| {
            let mut ordering = a.1.block_number.cmp(&b.1.block_number);
            if ordering == std::cmp::Ordering::Equal {
                ordering = a.1.transaction_index.cmp(&b.1.transaction_index);
            }

            ordering
        });

        let update_futs: Vec<_> = events
            .iter()
            .map(|event| async {
                let signature = Signature::try_from(event.0.signature.as_ref())
                    .expect("chain accepted invalid signature");

                let update = Update {
                    home_domain: event.0.home_domain,
                    previous_root: event.0.old_root.into(),
                    new_root: event.0.new_root.into(),
                };

                let block_number = event.1.block_number.as_u64();
                let timestamp = self
                    .provider
                    .get_block(block_number)
                    .await
                    .ok()
                    .flatten()
                    .map(|b| b.timestamp.as_u64());

                SignedUpdateWithMeta {
                    signed_update: SignedUpdate { update, signature },
                    metadata: UpdateMeta {
                        block_number,
                        timestamp,
                    },
                }
            })
            .collect();

        Ok(join_all(update_futs).await)
    }
}

/// A struct that provides access to an Ethereum replica contract
#[derive(Debug)]
pub struct EthereumReplica<W, R>
where
    W: ethers::providers::Middleware + 'static,
    R: ethers::providers::Middleware + 'static,
{
    submitter: TxSubmitter<W>,
    contract: Arc<EthereumReplicaInternal<R>>,
    domain: u32,
    name: String,
    gas: Option<ReplicaGasLimits>,
}

impl<W, R> EthereumReplica<W, R>
where
    W: ethers::providers::Middleware + 'static,
    R: ethers::providers::Middleware + 'static,
{
    /// Create a reference to a Replica at a specific Ethereum address on some
    /// chain
    pub fn new(
        submitter: TxSubmitter<W>,
        read_provider: Arc<R>,
        ContractLocator {
            name,
            domain,
            address,
        }: &ContractLocator,
        gas: Option<ReplicaGasLimits>,
    ) -> Self {
        tracing::info!(
            address = ?address.as_ethereum_address(),
            name = name,
            domain = domain,
            "Connecting Replica"
        );
        Self {
            submitter,
            contract: Arc::new(EthereumReplicaInternal::new(
                address.as_ethereum_address().expect("!eth address"),
                read_provider,
            )),
            domain: *domain,
            name: name.to_owned(),
            gas,
        }
    }
}

impl<W, R> std::fmt::Display for EthereumReplica<W, R>
where
    W: ethers::providers::Middleware + 'static,
    R: ethers::providers::Middleware + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EthereumHome {{ address: {}, domain: {}, name: {} }}",
            self.contract.address(),
            self.domain,
            self.name
        )
    }
}

#[async_trait]
impl<W, R> Common for EthereumReplica<W, R>
where
    W: ethers::providers::Middleware + 'static,
    R: ethers::providers::Middleware + 'static,
{
    type Error = EthereumError;

    fn name(&self) -> &str {
        &self.name
    }

    #[tracing::instrument(err)]
    async fn status(&self, txid: H256) -> Result<Option<TxOutcome>, Self::Error> {
        self.contract
            .client()
            .get_transaction_receipt(txid)
            .await
            .map_err(|e| EthereumError::MiddlewareError(e.into()))?
            .map(utils::try_transaction_receipt_to_tx_outcome)
            .transpose()
    }

    #[tracing::instrument(err)]
    async fn updater(&self) -> Result<H256, Self::Error> {
        Ok(self.contract.updater().call().await?.into())
    }

    #[tracing::instrument(err)]
    async fn state(&self) -> Result<State, Self::Error> {
        let state = self.contract.state().call().await?;
        match state {
            0 => Ok(State::Uninitialized),
            1 => Ok(State::Active),
            2 => Ok(State::Failed),
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(err)]
    async fn committed_root(&self) -> Result<H256, Self::Error> {
        Ok(self.contract.committed_root().call().await?.into())
    }

    #[tracing::instrument(err)]
    async fn update(&self, update: &SignedUpdate) -> Result<TxOutcome, Self::Error> {
        let mut tx = self.contract.update(
            update.update.previous_root.to_fixed_bytes(),
            update.update.new_root.to_fixed_bytes(),
            update.signature.to_vec().into(),
        );

        if let Some(limits) = &self.gas {
            tx.tx.set_gas(U256::from(limits.update));
        }

        self.submitter
            .submit(self.domain, self.contract.address(), tx.tx)
            .await
    }

    #[tracing::instrument(err)]
    async fn double_update(&self, _: &DoubleUpdate) -> Result<TxOutcome, Self::Error> {
        tracing::warn!("double-update submission has been deprecated");
        Ok(TxOutcome {
            txid: Default::default(),
        })
    }
}

#[async_trait]
impl<W, R> Replica for EthereumReplica<W, R>
where
    W: ethers::providers::Middleware + 'static,
    R: ethers::providers::Middleware + 'static,
{
    fn local_domain(&self) -> u32 {
        self.domain
    }

    async fn remote_domain(&self) -> Result<u32, <Self as Common>::Error> {
        Ok(self.contract.remote_domain().call().await?)
    }

    #[tracing::instrument(err)]
    async fn prove(&self, proof: &NomadProof) -> Result<TxOutcome, <Self as Common>::Error> {
        let mut sol_proof: [[u8; 32]; 32] = Default::default();
        sol_proof
            .iter_mut()
            .enumerate()
            .for_each(|(i, elem)| *elem = proof.path[i].to_fixed_bytes());

        let mut tx = self
            .contract
            .prove(proof.leaf.into(), sol_proof, proof.index.into());

        if let Some(limits) = &self.gas {
            tx.tx.set_gas(U256::from(limits.prove));
        }

        self.submitter
            .submit(self.domain, self.contract.address(), tx.tx)
            .await
    }

    #[tracing::instrument(err)]
    async fn process(&self, message: &NomadMessage) -> Result<TxOutcome, <Self as Common>::Error> {
        let mut tx = self.contract.process(message.to_vec().into());

        if let Some(limits) = &self.gas {
            tx.tx.set_gas(U256::from(limits.process));
        }

        self.submitter
            .submit(self.domain, self.contract.address(), tx.tx)
            .await
    }

    #[tracing::instrument(err)]
    async fn prove_and_process(
        &self,
        message: &NomadMessage,
        proof: &NomadProof,
    ) -> Result<TxOutcome, <Self as Common>::Error> {
        let mut sol_proof: [[u8; 32]; 32] = Default::default();
        sol_proof
            .iter_mut()
            .enumerate()
            .for_each(|(i, elem)| *elem = proof.path[i].to_fixed_bytes());

        let mut tx = self
            .contract
            .prove_and_process(message.to_vec().into(), sol_proof, proof.index.into())
            .gas(1_900_000);

        if let Some(limits) = &self.gas {
            tx.tx.set_gas(U256::from(limits.prove_and_process));
        }

        self.submitter
            .submit(self.domain, self.contract.address(), tx.tx)
            .await
    }

    #[tracing::instrument(err)]
    async fn message_status(&self, leaf: H256) -> Result<MessageStatus, <Self as Common>::Error> {
        Ok(self.contract.messages(leaf.into()).call().await?.into())
    }

    async fn acceptable_root(&self, root: H256) -> Result<bool, <Self as Common>::Error> {
        Ok(self.contract.acceptable_root(root.into()).call().await?)
    }
}
