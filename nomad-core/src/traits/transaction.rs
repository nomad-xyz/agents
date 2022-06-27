use crate::{ChainCommunicationError, PersistedTransaction, TxOutcome};
use async_trait::async_trait;

/// Interface for chain-agnostic to chain-specifc transaction translators
#[async_trait]
pub trait TxTranslator {
    /// Concrete transaction type
    type Transaction;

    /// Translate to chain-specific type
    async fn convert(
        &self,
        tx: PersistedTransaction,
    ) -> Result<Self::Transaction, ChainCommunicationError>;
}

/// Interface for forwarding between abstract and concrete contract instances
#[async_trait]
pub trait TxForwarder: Send + Sync + std::fmt::Debug {
    /// Translate to chain-specific type
    async fn forward(&self, tx: PersistedTransaction)
        -> Result<TxOutcome, ChainCommunicationError>;
}

/// Interface for submitting PersistentTransaction to a contract
#[async_trait]
pub trait TxSender {
    /// Translate to chain-specific type
    async fn send(&self, tx: PersistedTransaction) -> Result<TxOutcome, ChainCommunicationError>;
}

/// Interface for checking chain-specific / tx-specific tx status status via a contract
#[async_trait]
pub trait TxStatus {
    /// Translate to chain-specific type
    async fn status(&self, tx: &PersistedTransaction) -> Result<TxOutcome, ChainCommunicationError>;
}
