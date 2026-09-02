//! Node runtime: store, pool, producer, network, RPC.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use ivory_chain::{BlockProducer, BlockStore, ProduceParams};
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Account, Block, BlockHeader};
use ivory_crypto::address_from_secret;
use ivory_executor::{ExecutionContext, Executor};
use ivory_network::{NetworkConfig, NetworkEvent, NetworkHandle, start as start_network};
use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};
use ivory_rpc::{RpcContext, RpcHandler, serve};
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};
use tokio::sync::watch;

use crate::config::{GenesisFile, NodeFileConfig};

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
}

/// Start networking, RPC, import loop, and optional block production.
///
/// # Errors
///
/// Bind, genesis, or network failures.
pub async fn run_node(
    cfg: NodeFileConfig,
    genesis: GenesisFile,
    validator_key: SecretKey,
    mut shutdown: watch::Receiver<bool>,
) -> Result<Node> {
    let poa = PoAConsensus::new(genesis.poa_config()?)?;
    let local_addr = address_from_secret(&validator_key);
    let validator_addr =
        Address::from_hex(&genesis.validator.address).context("genesis validator address")?;
    let is_producer = local_addr == validator_addr && poa.is_validator(&local_addr);

    let state = StateDB::new();
    for (addr, bal) in genesis.parsed_alloc()? {
        let mut acc = Account::new();
        acc.balance = bal;
        state.set_account(addr, acc);
    }

    let store = Arc::new(BlockStore::new(poa.clone()));
    let genesis_block = genesis_block(&genesis)?;
    store.insert_genesis(genesis_block)?;
    store.record_state(0, state.clone());

    let pool = Arc::new(TransactionPool::new());
    let executor = Arc::new(Executor::new(state.clone()));
    let producer = BlockProducer::with_gas_limit(genesis.gas_limit);

    let net_cfg = NetworkConfig {
        listen: cfg.p2p_listen.parse().context("p2p listen multiaddr")?,
        bootstrap: cfg
            .bootstrap
            .iter()
            .map(|s| s.parse().context("bootstrap multiaddr"))
            .collect::<Result<Vec<_>>>()?,
    };
    let (network, mut events) = start_network(net_cfg).await?;

    let handler = RpcHandler::new(RpcContext::new(
        Arc::clone(&store),
        Arc::clone(&pool),
        state,
        cfg.chain_id,
    ));

    let rpc_addr: SocketAddr = cfg.rpc_addr.parse().context("rpc addr")?;
    let rpc_handler = handler.clone();
    let mut shutdown_rpc = shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = serve(rpc_handler, rpc_addr) => {}
            _ = shutdown_rpc.changed() => {}
        }
    });

    let orphans: Arc<Mutex<HashMap<H256, Block>>> = Arc::new(Mutex::new(HashMap::new()));
    let import_store = Arc::clone(&store);
    let import_pool = Arc::clone(&pool);
    let import_exec = Arc::clone(&executor);
    let import_net = network.clone();
    let import_orphans = Arc::clone(&orphans);
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            match ev {
                NetworkEvent::TxReceived(tx) => {
                    let _ = import_pool.add_transaction(tx, TxOrigin::Remote);
                }
                NetworkEvent::BlockReceived(block) => {
                    import_block(
                        &import_store,
                        &import_exec,
                        &import_pool,
                        &import_net,
                        &import_orphans,
                        block,
                    );
                }
                NetworkEvent::BlockRequest(hash) => {
                    if let Some(block) = import_store.get_block(&hash) {
                        let _ = import_net.broadcast_block(block);
                    }
                }
                _ => {}
            }
        }
    });

    if is_producer {
        let prod_store = Arc::clone(&store);
        let prod_pool = Arc::clone(&pool);
        let prod_exec = Arc::clone(&executor);
        let prod_net = network.clone();
        let interval = Duration::from_millis(cfg.block_interval_ms.max(50));
        let key = validator_key;
        let consensus = poa;
        tokio::spawn(async move {
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
                        let ts = parent.header.timestamp.saturating_add(1);
                        match producer.produce_block(ProduceParams {
                            parent: &parent,
                            pool: &prod_pool,
                            executor: &prod_exec,
                            consensus: &consensus,
                            miner: local_addr,
                            miner_key: &key,
                            timestamp: ts,
                            max_txs: 128,
                        }) {
                            Ok(block) => {
                                for tx in &block.transactions {
                                    prod_pool.remove(&tx.hash());
                                }
                                if prod_store.insert_block(block.clone()).is_ok() {
                                    prod_store.record_state(
                                        block.header.number,
                                        prod_exec.state().clone(),
                                    );
                                    let _ = prod_net.broadcast_block(block);
                                }
                            }
                            Err(e) => tracing::debug!(error = %e, "produce failed"),
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
    }

    Ok(Node {
        handler,
        pool,
        store,
        network,
    })
}

fn genesis_block(genesis: &GenesisFile) -> Result<Block> {
    let miner = Address::from_hex(&genesis.validator.address).context("validator address")?;
    let extra = decode_extra(&genesis.extra_data)?;
    Ok(Block {
        header: BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: genesis.timestamp,
            miner,
            gas_limit: genesis.gas_limit,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: extra,
        },
        transactions: Vec::new(),
        receipts: Vec::new(),
    })
}

fn decode_extra(s: &str) -> Result<Bytes> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let raw = hex::decode(s).context("genesis extra_data")?;
    Ok(Bytes::from_vec(raw))
}

fn import_block(
    store: &BlockStore,
    executor: &Executor,
    pool: &TransactionPool,
    network: &NetworkHandle,
    orphans: &Mutex<HashMap<H256, Block>>,
    block: Block,
) {
    match store.insert_block(block.clone()) {
        Ok(_) => {
            let mut ctx = ExecutionContext::new(block.header.number, block.header.timestamp);
            for tx in &block.transactions {
                let _ = executor.execute_transaction(tx, &mut ctx);
                pool.remove(&tx.hash());
            }
            store.record_state(block.header.number, executor.state().clone());
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
                import_block(store, executor, pool, network, orphans, child);
                pending = orphans.lock().unwrap();
            }
        }
        Err(ivory_chain::ChainError::UnknownParent) => {
            network.request_block(block.header.parent_hash).ok();
            orphans.lock().unwrap().insert(block.hash(), block);
        }
        Err(_) => {}
    }
}
