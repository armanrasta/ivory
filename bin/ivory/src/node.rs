//! Node runtime: store, pool, producer, network, RPC.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use ivory_chain::{BlockProducer, BlockStore, ProduceParams, import_and_apply};
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Account, Block, BlockHeader, empty_list_roots};
use ivory_crypto::address_from_secret;
use ivory_executor::{ExecutionContext, Executor};
use ivory_network::{
    Multiaddr, NetworkConfig, NetworkEvent, NetworkHandle, PeerId, start as start_network,
};
use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};
use ivory_rpc::{
    IvoryMetrics, NodeRole, RpcContext, RpcEvent, RpcHandler, RpcHttpConfig, router_with_config,
};
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::config::{DataPaths, GenesisFile, NodeFileConfig};
use crate::persist::ChainPersist;

const READ_ONLY_METHODS: &[&str] = &[
    "eth_chainId",
    "eth_blockNumber",
    "eth_getBalance",
    "eth_getCode",
    "eth_getStorageAt",
    "eth_getBlockByNumber",
    "eth_getBlockByHash",
    "eth_getTransactionByHash",
    "eth_getTransactionReceipt",
    "eth_getTransactionCount",
    "eth_call",
    "eth_estimateGas",
    "eth_getLogs",
    "eth_getProof",
    "ivory_nodeInfo",
    "ivory_listContracts",
    "ivory_getHeaderByNumber",
];

/// Running node handles.
pub struct Node {
    /// JSON-RPC handler (for tests).
    pub handler: RpcHandler,
    /// Mempool.
    pub pool: Arc<TransactionPool>,
    /// Chain store.
    pub store: Arc<BlockStore>,
    /// P2P handle.
    pub network: NetworkHandle,
    /// Bound JSON-RPC address (port may be ephemeral).
    pub rpc_addr: SocketAddr,
    /// Bound libp2p listen multiaddr.
    pub p2p_addr: Multiaddr,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

/// Start networking, RPC, import loop, and optional block production.
///
/// Canonical blocks are persisted under [`DataPaths::chain`] and replayed on
/// restart. A reorg resets live executor state from the new-head snapshot.
///
/// # Errors
///
/// Bind, genesis, persistence, or network failures.
pub async fn run_node(
    cfg: NodeFileConfig,
    genesis: GenesisFile,
    validator_key: SecretKey,
    mut shutdown: watch::Receiver<bool>,
    paths: &DataPaths,
) -> Result<Node> {
    let poa = PoAConsensus::new(genesis.poa_config()?)?;
    let local_addr = address_from_secret(&validator_key);
    let validator_addr =
        Address::from_hex(&genesis.validator.address).context("genesis validator address")?;
    let is_producer =
        cfg.role.may_produce() && local_addr == validator_addr && poa.is_validator(&local_addr);

    let state = StateDB::new();
    for (addr, bal) in genesis.parsed_alloc()? {
        let mut acc = Account::new();
        acc.balance = bal;
        state.set_account(addr, acc);
    }

    let persist = Arc::new(ChainPersist::open_with(
        &paths.chain,
        cfg.archive,
        cfg.archive_keep,
    )?);
    let store = Arc::new(BlockStore::new(poa.clone()));
    let genesis_block = genesis_block(&genesis)?;
    let genesis_hash = genesis_block.hash();
    let loaded_height = persist.load_into(&store, &genesis_block)?;
    if loaded_height.is_none() {
        store.insert_genesis(genesis_block.clone())?;
        persist.persist_canonical(&store, &genesis_block)?;
    }
    store.record_state(genesis_hash, state.fork());

    let pool = Arc::new(TransactionPool::new());
    let executor = Arc::new(Executor::new(state.clone()));
    if let Some(head_n) = loaded_height {
        for n in 1..=head_n {
            let block = store
                .get_block_by_number(n)
                .with_context(|| format!("replay: missing block {n}"))?;
            let mut ctx = ExecutionContext::new(block.header.number, block.header.timestamp);
            for tx in &block.transactions {
                executor
                    .execute_transaction(tx, &mut ctx)
                    .with_context(|| format!("replay tx in block {n}"))?;
            }
            store.record_state(block.hash(), executor.state().fork());
        }
    }

    let producer = BlockProducer::with_gas_limit(genesis.gas_limit);

    let allowlist = cfg
        .p2p_allowlist
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<PeerId>().context("p2p allowlist peer id"))
        .collect::<Result<Vec<_>>>()?;
    let net_cfg = NetworkConfig {
        listen: cfg.p2p_listen.parse().context("p2p listen multiaddr")?,
        bootstrap: cfg
            .bootstrap
            .iter()
            .map(|s| s.parse().context("bootstrap multiaddr"))
            .collect::<Result<Vec<_>>>()?,
        allowlist,
    };
    let (network, mut events) = start_network(net_cfg).await?;

    let gossip_net = network.clone();
    let role = if is_producer {
        NodeRole::Producer
    } else {
        NodeRole::Follower
    };
    let handler = RpcHandler::new(
        RpcContext::new(Arc::clone(&store), Arc::clone(&pool), state, cfg.chain_id)
            .with_gossip(move |tx| {
                let _ = gossip_net.broadcast_transaction(tx);
            })
            .with_node_info(
                role,
                local_addr,
                network.peer_id().to_string(),
                network.peer_count_handle(),
                cfg.bootstrap.clone(),
            )
            .with_contract_lookup({
                let data_contracts = paths.contracts.clone();
                let extra = if cfg.contracts_dir.is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(&cfg.contracts_dir))
                };
                move |hash| {
                    let dirs = crate::contract::catalog_dirs(&data_contracts, extra.as_deref());
                    crate::contract::load_catalog(&dirs).get(hash).cloned()
                }
            }),
    );

    let rpc_bind: SocketAddr = cfg.rpc_addr.parse().context("rpc addr")?;
    let listener = TcpListener::bind(rpc_bind).await.context("bind rpc")?;
    let rpc_addr = listener.local_addr().context("rpc local addr")?;
    let rpc_handler = handler.clone();
    let mut shutdown_rpc = shutdown.clone();
    let rpc_task = tokio::spawn(async move {
        tokio::select! {
            _ = axum::serve(listener, router_with_config(rpc_handler, RpcHttpConfig::from_env())) => {}
            _ = shutdown_rpc.changed() => {}
        }
    });
    let mut extra_rpc = Vec::new();
    if !cfg.rpc_read_addr.is_empty() {
        let read_bind: SocketAddr = cfg.rpc_read_addr.parse().context("rpc read addr")?;
        let read_listener = TcpListener::bind(read_bind)
            .await
            .context("bind rpc read")?;
        let read_handler = RpcHandler::new(
            handler
                .context()
                .clone()
                .with_allow_methods(READ_ONLY_METHODS.iter().map(|s| (*s).to_string()).collect()),
        );
        let mut shutdown_read = shutdown.clone();
        extra_rpc.push(tokio::spawn(async move {
            tokio::select! {
                _ = axum::serve(
                    read_listener,
                    router_with_config(read_handler, RpcHttpConfig::from_env()),
                ) => {}
                _ = shutdown_read.changed() => {}
            }
        }));
    }

    let (listen_tx, listen_rx) = oneshot::channel();
    let orphans: Arc<Mutex<HashMap<H256, Block>>> = Arc::new(Mutex::new(HashMap::new()));
    let import_store = Arc::clone(&store);
    let import_pool = Arc::clone(&pool);
    let import_exec = Arc::clone(&executor);
    let import_net = network.clone();
    let import_orphans = Arc::clone(&orphans);
    let import_persist = Arc::clone(&persist);
    let import_events = handler.context().events.clone();
    let import_metrics = Arc::clone(&handler.context().metrics);
    let mut shutdown_import = shutdown.clone();
    let import_task = tokio::spawn(async move {
        let mut listen_tx = Some(listen_tx);
        loop {
            tokio::select! {
                ev = events.recv() => {
                    let Some(ev) = ev else {
                        break;
                    };
                    match ev {
                        NetworkEvent::ListenAddr(addr) => {
                            if let Some(tx) = listen_tx.take() {
                                let _ = tx.send(addr);
                            }
                        }
                        NetworkEvent::TxReceived(tx) => {
                            import_metrics.p2p_message("tx");
                            if let Ok(hash) = import_pool.add_transaction(tx, TxOrigin::Remote) {
                                import_metrics.set_txpool_pending(import_pool.pending_count() as u64);
                                let _ = import_events.send(RpcEvent::NewPendingTx { hash });
                            }
                        }
                        NetworkEvent::BlockReceived(block) => {
                            import_metrics.p2p_message("block");
                            import_block(
                                &import_store,
                                &import_exec,
                                &import_pool,
                                &import_net,
                                &import_persist,
                                &import_orphans,
                                &import_events,
                                Some(Arc::clone(&import_metrics)),
                                block,
                            );
                        }
                        NetworkEvent::BlockRequest(hash) => {
                            import_metrics.p2p_message("get_block");
                            if let Some(block) = import_store.get_block(&hash) {
                                let _ = import_net.broadcast_block(block);
                            }
                        }
                        NetworkEvent::PeerConnected(_) | NetworkEvent::PeerDisconnected(_) => {
                            import_metrics.set_p2p_peers(import_net.peer_count() as u64);
                        }
                    }
                }
                _ = shutdown_import.changed() => break,
            }
        }
    });

    let p2p_addr = tokio::time::timeout(Duration::from_secs(10), listen_rx)
        .await
        .context("waiting for p2p listen")?
        .context("p2p listen oneshot")?;

    let mut tasks = vec![rpc_task, import_task];
    tasks.extend(extra_rpc);

    if is_producer {
        let prod_store = Arc::clone(&store);
        let prod_pool = Arc::clone(&pool);
        let prod_exec = Arc::clone(&executor);
        let prod_net = network.clone();
        let prod_persist = Arc::clone(&persist);
        let prod_events = handler.context().events.clone();
        let prod_metrics = Arc::clone(&handler.context().metrics);
        let interval = Duration::from_millis(cfg.block_interval_ms.max(50));
        let key = validator_key;
        let consensus = poa;
        let prod_task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        if prod_pool.pending_count() == 0 {
                            continue;
                        }
                        let Some(parent) = prod_store.head_block() else {
                            continue;
                        };
                        let Some(parent_state) = prod_store.state_at(&parent.hash()) else {
                            continue;
                        };
                        let trial_exec = Executor::new(parent_state);
                        let ts = parent.header.timestamp.saturating_add(1);
                        let started = std::time::Instant::now();
                        match producer.produce_block(ProduceParams {
                            parent: &parent,
                            pool: &prod_pool,
                            executor: &trial_exec,
                            consensus: &consensus,
                            miner: local_addr,
                            miner_key: &key,
                            timestamp: ts,
                            max_txs: 128,
                        }) {
                            Ok(block) => {
                                prod_metrics
                                    .observe_produce_seconds(started.elapsed().as_secs_f64());
                                prod_metrics.inc_blocks_produced();
                                match import_and_apply(
                                    &prod_store,
                                    prod_exec.state(),
                                    &prod_pool,
                                    block.clone(),
                                ) {
                                    Ok(outcome) => {
                                        if outcome.head_changed {
                                            let _ = prod_events.send(RpcEvent::new_head(&block));
                                            let _ = prod_events.send(RpcEvent::new_logs(&block));
                                        }
                                        prod_metrics.set_head(
                                            block.header.number,
                                            block.header.timestamp,
                                        );
                                        prod_metrics.set_txpool_pending(
                                            prod_pool.pending_count() as u64,
                                        );
                                        if let Err(e) =
                                            prod_persist.persist_canonical(&prod_store, &block)
                                        {
                                            tracing::warn!(error = %e, "persist produced block");
                                        }
                                        let _ = prod_persist
                                            .persist_trie_nodes(&prod_exec.state().trie_nodes());
                                        prune_store(&prod_store, &prod_persist);
                                        let _ = prod_net.broadcast_block(block);
                                    }
                                    Err(e) => tracing::debug!(error = %e, "import produced block"),
                                }
                            }
                            Err(e) => tracing::debug!(error = %e, "produce failed"),
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
        tasks.push(prod_task);
    }

    Ok(Node {
        handler,
        pool,
        store,
        network,
        rpc_addr,
        p2p_addr,
        tasks,
    })
}

fn genesis_block(genesis: &GenesisFile) -> Result<Block> {
    let miner = Address::from_hex(&genesis.validator.address).context("validator address")?;
    let extra = decode_extra(&genesis.extra_data)?;
    let (tx_root, rx_root) = empty_list_roots();
    Ok(Block {
        header: BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: genesis.timestamp,
            miner,
            gas_limit: genesis.gas_limit,
            gas_used: 0,
            state_root: genesis.alloc_state_root()?,
            transactions_root: tx_root,
            receipts_root: rx_root,
            difficulty: U256::ZERO,
            extra_data: extra,
        },
        transactions: Vec::new(),
        receipts: Vec::new(),
    })
}

fn prune_store(store: &BlockStore, persist: &ChainPersist) {
    if persist.is_archive() {
        store.prune_snapshots();
    } else {
        store.prune_snapshots_keep(persist.archive_keep());
    }
}

fn decode_extra(s: &str) -> Result<Bytes> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let raw = hex::decode(s).context("genesis extra_data")?;
    Ok(Bytes::from_vec(raw))
}

#[allow(clippy::too_many_arguments)]
fn emit_logs(events: &tokio::sync::broadcast::Sender<RpcEvent>, block: &Block) {
    let logs: Vec<ivory_core::Log> = block.receipts.iter().flat_map(|r| r.logs.clone()).collect();
    if logs.is_empty() {
        return;
    }
    let _ = events.send(RpcEvent::NewLogs {
        logs,
        block_number: block.header.number,
        block_hash: block.hash(),
    });
}

#[allow(clippy::too_many_arguments)]
fn import_block(
    store: &BlockStore,
    executor: &Executor,
    pool: &TransactionPool,
    network: &NetworkHandle,
    persist: &ChainPersist,
    orphans: &Mutex<HashMap<H256, Block>>,
    events: &tokio::sync::broadcast::Sender<RpcEvent>,
    metrics: Option<Arc<IvoryMetrics>>,
    block: Block,
) {
    match import_and_apply(store, executor.state(), pool, block.clone()) {
        Ok(outcome) => {
            if let Some(m) = metrics.as_deref() {
                m.inc_blocks_imported();
                m.set_head(block.header.number, block.header.timestamp);
                m.set_txpool_pending(pool.pending_count() as u64);
            }
            if outcome.head_changed {
                let _ = events.send(RpcEvent::new_head(&block));
                emit_logs(events, &block);
            }
            if let Err(e) = persist.persist_canonical(store, &block) {
                tracing::warn!(error = %e, "persist imported block");
            }
            let _ = persist.persist_trie_nodes(&executor.state().trie_nodes());
            prune_store(store, persist);
            let hash = block.hash();
            let mut pending = orphans.lock().unwrap();
            let children: Vec<Block> = pending
                .values()
                .filter(|b| b.header.parent_hash == hash)
                .cloned()
                .collect();
            for child in children {
                pending.remove(&child.hash());
                drop(pending);
                import_block(
                    store,
                    executor,
                    pool,
                    network,
                    persist,
                    orphans,
                    events,
                    metrics.clone(),
                    child,
                );
                pending = orphans.lock().unwrap();
            }
        }
        Err(ivory_chain::ChainError::UnknownParent)
        | Err(ivory_chain::ChainError::UnknownParentState) => {
            network.request_block(block.header.parent_hash).ok();
            orphans.lock().unwrap().insert(block.hash(), block);
        }
        Err(_) => {}
    }
}
