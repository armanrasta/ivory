//! Shared RPC backend handles.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use ivory_chain::BlockStore;
use ivory_core::Transaction;
use ivory_primitives::{Address, H256};
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;

/// Producer (genesis validator key) or follower (bootstrap peer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeRole {
    /// Seals blocks when the loaded key matches genesis.
    Producer,
    /// Imports gossiped blocks; does not produce.
    #[default]
    Follower,
}

impl NodeRole {
    /// Wire string for `ivory_nodeInfo`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Producer => "producer",
            Self::Follower => "follower",
        }
    }
}

/// Lookup `keccak256(code)` → file catalog row.
pub type ContractLookup = Arc<dyn Fn(&H256) -> Option<ContractMeta> + Send + Sync>;

/// Catalog row for a file-backed contract (`contracts/*.yaml`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContractMeta {
    /// Manifest `name`.
    pub name: String,
    /// Manifest `schema`.
    pub schema: String,
    /// Source path shown in the explorer.
    pub source: String,
    /// Optional description.
    pub description: String,
}

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
    /// Producer vs follower.
    pub role: NodeRole,
    /// Local account derived from the node key.
    pub address: Address,
    /// libp2p peer id (empty in unit tests).
    pub peer_id: String,
    /// Live connection count.
    pub peers: Arc<AtomicUsize>,
    /// Configured bootstrap multiaddrs.
    pub bootstrap: Vec<String>,
    /// Resolve `keccak256(code)` to a YAML/WAT package (reloaded by the node).
    pub contract_lookup: Option<ContractLookup>,
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
            role: NodeRole::Follower,
            address: Address::ZERO,
            peer_id: String::new(),
            peers: Arc::new(AtomicUsize::new(0)),
            bootstrap: Vec::new(),
            contract_lookup: None,
        }
    }

    /// Call `f` after [`crate::RpcHandler`] admits a raw transaction.
    #[must_use]
    pub fn with_gossip(mut self, f: impl Fn(Transaction) + Send + Sync + 'static) -> Self {
        self.on_tx = Some(Arc::new(f));
        self
    }

    /// Attach identity and P2P stats for `ivory_nodeInfo`.
    #[must_use]
    pub fn with_node_info(
        mut self,
        role: NodeRole,
        address: Address,
        peer_id: String,
        peers: Arc<AtomicUsize>,
        bootstrap: Vec<String>,
    ) -> Self {
        self.role = role;
        self.address = address;
        self.peer_id = peer_id;
        self.peers = peers;
        self.bootstrap = bootstrap;
        self
    }

    /// Attach file-based contract metadata for `ivory_listContracts`.
    #[must_use]
    pub fn with_contract_lookup(
        mut self,
        f: impl Fn(&H256) -> Option<ContractMeta> + Send + Sync + 'static,
    ) -> Self {
        self.contract_lookup = Some(Arc::new(f));
        self
    }
}
