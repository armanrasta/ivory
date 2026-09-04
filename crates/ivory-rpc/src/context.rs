//! Shared RPC backend handles.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use ivory_chain::BlockStore;
use ivory_core::{Block, Transaction};
use ivory_primitives::{Address, H256};
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;
use tokio::sync::broadcast;

use crate::metrics::IvoryMetrics;

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

/// Fan-out for WebSocket `eth_subscribe`.
#[derive(Clone, Debug)]
pub enum RpcEvent {
    /// Canonical head moved.
    NewHead {
        /// Block number.
        number: u64,
        /// Block hash.
        hash: H256,
        /// Parent hash.
        parent_hash: H256,
        /// Sealer address.
        miner: Address,
        /// Header timestamp.
        timestamp: u64,
        /// Post-state root.
        state_root: H256,
        /// Block gas limit.
        gas_limit: u64,
        /// Gas used in the block.
        gas_used: u64,
    },
    /// Transaction admitted to the mempool.
    NewPendingTx {
        /// Transaction hash.
        hash: H256,
    },
    /// Logs from a newly imported head (WS `logs`).
    NewLogs {
        /// Receipt logs in that block.
        logs: Vec<ivory_core::Log>,
        /// Block number.
        block_number: u64,
        /// Block hash.
        block_hash: H256,
    },
}

impl RpcEvent {
    /// Build a `logs` payload from a canonical block’s receipts.
    #[must_use]
    pub fn new_logs(block: &Block) -> Self {
        let mut logs = Vec::new();
        for receipt in &block.receipts {
            logs.extend(receipt.logs.iter().cloned());
        }
        Self::NewLogs {
            logs,
            block_number: block.header.number,
            block_hash: block.hash(),
        }
    }

    /// Build a `newHeads` payload from a canonical block.
    #[must_use]
    pub fn new_head(block: &Block) -> Self {
        Self::NewHead {
            number: block.header.number,
            hash: block.hash(),
            parent_hash: block.header.parent_hash,
            miner: block.header.miner,
            timestamp: block.header.timestamp,
            state_root: block.header.state_root,
            gas_limit: block.header.gas_limit,
            gas_used: block.header.gas_used,
        }
    }
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
    /// `eth_subscribe` fan-out (`newHeads` / `newPendingTransactions` / `logs`).
    pub events: broadcast::Sender<RpcEvent>,
    /// Process metrics (`GET /metrics`).
    pub metrics: Arc<IvoryMetrics>,
    /// If set, only these methods are dispatched (read-only RPC bind).
    pub allow_methods: Option<Vec<String>>,
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
            events: broadcast::channel(256).0,
            metrics: Arc::new(IvoryMetrics::new()),
            allow_methods: None,
        }
    }

    /// Restrict this handler to an allowlist of method names.
    #[must_use]
    pub fn with_allow_methods(mut self, methods: Vec<String>) -> Self {
        self.allow_methods = Some(methods);
        self
    }

    /// Subscribe to head and pending-tx notifications.
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<RpcEvent> {
        self.events.subscribe()
    }

    /// Best-effort emit (lagging subscribers are dropped by broadcast).
    pub fn emit(&self, event: RpcEvent) {
        let _ = self.events.send(event);
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
