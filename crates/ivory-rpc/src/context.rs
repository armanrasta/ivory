//! Shared RPC backend handles.

use std::sync::Arc;

use ivory_chain::BlockStore;
use ivory_core::Transaction;
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;

/// Live chain, pool, and state for JSON-RPC methods.
#[derive(Clone)]
pub struct RpcContext {
    /// Canonical chain.
    pub store: Arc<BlockStore>,
    /// Mempool.
    pub pool: Arc<TransactionPool>,
    /// Live account state (same handle as the executor).
    pub state: StateDB,
    /// `eth_chainId` value.
    pub chain_id: u64,
    /// Invoked after a raw tx is admitted (node wires this to gossip).
    pub on_tx: Option<Arc<dyn Fn(Transaction) + Send + Sync>>,
}

impl RpcContext {
    /// Build a context from shared handles.
    #[must_use]
    pub fn new(
        store: Arc<BlockStore>,
        pool: Arc<TransactionPool>,
        state: StateDB,
        chain_id: u64,
    ) -> Self {
        Self {
            store,
            pool,
            state,
            chain_id,
            on_tx: None,
        }
    }

    /// Call `f` after [`crate::RpcHandler`] admits a raw transaction.
    #[must_use]
    pub fn with_gossip(mut self, f: impl Fn(Transaction) + Send + Sync + 'static) -> Self {
        self.on_tx = Some(Arc::new(f));
        self
    }
}
