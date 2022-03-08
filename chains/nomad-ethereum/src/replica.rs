#![allow(clippy::enum_variant_names)]
#![allow(missing_docs)]

use async_trait::async_trait;
use color_eyre::Result;
use ethers::core::types::{Signature, H256};
use futures_util::future::join_all;
use nomad_core::{
    accumulator::merkle::Proof, ChainCommunicationError, Common, CommonIndexer, ContractLocator,
    DoubleUpdate, Encode, MessageStatus, NomadMessage, Replica, SignedUpdate, SignedUpdateWithMeta,
    State, TxOutcome, Update, UpdateMeta,
};
use std::{convert::TryFrom, error::Error as StdError, sync::Arc};
use tracing::instrument;

use crate::{bindings::replica::Replica as EthereumReplicaInternal, report_tx};

#[derive(Debug)]
/// Struct that retrieves indexes event data for Ethereum replica
pub struct EthereumReplicaIndexer<M>
where
    M: ethers::providers::Middleware,
{
    contract: Arc<EthereumReplicaInternal<M>>,
    provider: Arc<M>,
    from_height: u32,
    chunk_size: u32,
}

impl<M> EthereumReplicaIndexer<M>
where
    M: ethers::providers::Middleware + 'static,
{
    /// Create new EthereumHomeIndexer
    pub fn new(
        provider: Arc<M>,
        ContractLocator {
            name: _,
            domain: _,
            address,
        }: &ContractLocator,
        from_height: u32,
        chunk_size: u32,
    ) -> Self {
        Self {
            contract: Arc::new(EthereumReplicaInternal::new(address, provider.clone())),
            provider,
            from_height,
            chunk_size,
        }
    }
}

#[async_trait]
impl<M> CommonIndexer for EthereumReplicaIndexer<M>
where
    M: ethers::providers::Middleware + 'static,
{
    #[instrument(err, skip(self))]
    async fn get_block_number(&self) -> Result<u32> {
        Ok(self.provider.get_block_number().await?.as_u32())
    }

    #[instrument(err, skip(self))]
    async fn fetch_sorted_updates(&self, from: u32, to: u32) -> Result<Vec<SignedUpdateWithMeta>> {
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
pub struct EthereumReplica<M>
where
    M: ethers::providers::Middleware,
{
    contract: Arc<EthereumReplicaInternal<M>>,
    domain: u32,
    name: String,
    provider: Arc<M>,
}

impl<M> EthereumReplica<M>
where
    M: ethers::providers::Middleware,
{
    /// Create a reference to a Replica at a specific Ethereum address on some
    /// chain
    pub fn new(
        provider: Arc<M>,
        ContractLocator {
            name,
            domain,
            address,
        }: &ContractLocator,
    ) -> Self {
        Self {
            contract: Arc::new(EthereumReplicaInternal::new(address, provider.clone())),
            domain: *domain,
            name: name.to_owned(),
            provider,
        }
    }
}

#[async_trait]
impl<M> Common for EthereumReplica<M>
where
    M: ethers::providers::Middleware + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    #[tracing::instrument(err)]
    async fn status(&self, txid: H256) -> Result<Option<TxOutcome>, ChainCommunicationError> {
        self.contract
            .client()
            .get_transaction_receipt(txid)
            .await
            .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?
            .map(|receipt| receipt.try_into())
            .transpose()
    }

    #[tracing::instrument(err)]
    async fn updater(&self) -> Result<H256, ChainCommunicationError> {
        Ok(self.contract.updater().call().await?.into())
    }

    #[tracing::instrument(err)]
    async fn state(&self) -> Result<State, ChainCommunicationError> {
        let state = self.contract.state().call().await?;
        match state {
            0 => Ok(State::Uninitialized),
            1 => Ok(State::Active),
            2 => Ok(State::Failed),
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(err)]
    async fn committed_root(&self) -> Result<H256, ChainCommunicationError> {
        Ok(self.contract.committed_root().call().await?.into())
    }

    #[tracing::instrument(err)]
    async fn update(&self, update: &SignedUpdate) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self.contract.update(
            update.update.previous_root.to_fixed_bytes(),
            update.update.new_root.to_fixed_bytes(),
            update.signature.to_vec().into(),
        );

        report_tx!(tx, &self.provider).try_into()
    }

    #[tracing::instrument(err)]
    async fn double_update(
        &self,
        double: &DoubleUpdate,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self.contract.double_update(
            double.0.update.previous_root.to_fixed_bytes(),
            [
                double.0.update.new_root.to_fixed_bytes(),
                double.1.update.new_root.to_fixed_bytes(),
            ],
            double.0.signature.to_vec().into(),
            double.1.signature.to_vec().into(),
        );

        report_tx!(tx, &self.provider).try_into()
    }
}

#[async_trait]
impl<M> Replica for EthereumReplica<M>
where
    M: ethers::providers::Middleware + 'static,
{
    fn local_domain(&self) -> u32 {
        self.domain
    }

    async fn remote_domain(&self) -> Result<u32, ChainCommunicationError> {
        Ok(self.contract.remote_domain().call().await?)
    }

    #[tracing::instrument(err)]
    async fn prove(&self, proof: &Proof) -> Result<TxOutcome, ChainCommunicationError> {
        let mut sol_proof: [[u8; 32]; 32] = Default::default();
        sol_proof
            .iter_mut()
            .enumerate()
            .for_each(|(i, elem)| *elem = proof.path[i].to_fixed_bytes());

        let tx = self
            .contract
            .prove(proof.leaf.into(), sol_proof, proof.index.into());

        report_tx!(tx, &self.provider).try_into()
    }

    #[tracing::instrument(err)]
    async fn process(&self, message: &NomadMessage) -> Result<TxOutcome, ChainCommunicationError> {
        let tx = self
            .contract
            .process(message.to_vec().into())
            .gas(1_500_000);

        report_tx!(tx, &self.provider).try_into()
    }

    #[tracing::instrument(err)]
    async fn prove_and_process(
        &self,
        message: &NomadMessage,
        proof: &Proof,
    ) -> Result<TxOutcome, ChainCommunicationError> {
        let mut sol_proof: [[u8; 32]; 32] = Default::default();
        sol_proof
            .iter_mut()
            .enumerate()
            .for_each(|(i, elem)| *elem = proof.path[i].to_fixed_bytes());

        let tx = self
            .contract
            .prove_and_process(message.to_vec().into(), sol_proof, proof.index.into())
            .gas(1_800_000);

        report_tx!(tx, &self.provider).try_into()
    }

    #[tracing::instrument(err)]
    async fn message_status(&self, leaf: H256) -> Result<MessageStatus, ChainCommunicationError> {
        let status = self.contract.messages(leaf.into()).call().await?;
        match status {
            0 => Ok(MessageStatus::None),
            1 => Ok(MessageStatus::Proven),
            2 => Ok(MessageStatus::Processed),
            _ => panic!("Bad status from solidity"),
        }
    }

    async fn acceptable_root(&self, root: H256) -> Result<bool, ChainCommunicationError> {
        Ok(self.contract.acceptable_root(root.into()).call().await?)
    }
}
