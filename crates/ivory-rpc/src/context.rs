//! Shared RPC backend handles.

use std::sync::Arc;

use ivory_chain::BlockStore;
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
        }
    }
}
