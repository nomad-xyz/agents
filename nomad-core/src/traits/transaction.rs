use crate::{ChainCommunicationError, PersistedTransaction, TxOutcome};
use async_trait::async_trait;
use color_eyre::Result;
use tokio::task::JoinHandle;

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

/// Interface for creating transaction submission tasks in contracts
pub trait TxSubmitTask: Send + Sync + std::fmt::Debug {
    /// Create and return transaction submission task
    fn submit_task(&mut self) -> Option<JoinHandle<Result<()>>> {
        None
    }
}
