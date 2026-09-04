//! JSON-RPC method unit tests.

use std::sync::Arc;

use ivory_chain::BlockStore;
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{
    Account, Block, BlockHeader, QuantEnvelope, QuantMetric, empty_list_roots, list_root,
};
use ivory_crypto::{keypair_from_byte, signed_transfer, signed_tx};
use ivory_primitives::{Address, Bytes, H256, U256};
use ivory_rpc::{NodeRole, RpcContext, RpcError, RpcHandler};
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;
use serde_json::json;

fn miner_sk() -> ivory_primitives::SecretKey {
    keypair_from_byte(9).0
}

fn miner() -> Address {
    keypair_from_byte(9).2
}

fn poa() -> PoAConsensus {
    PoAConsensus::from_secret(&miner_sk()).unwrap()
}

fn genesis() -> Block {
    let (tx_root, rx_root) = empty_list_roots();
    let mut header = BlockHeader {
        number: 0,
        parent_hash: H256::ZERO,
        timestamp: 1,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 0,
        state_root: H256::ZERO,
        transactions_root: tx_root,
        receipts_root: rx_root,
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    Block {
        header,
        transactions: Vec::new(),
        receipts: Vec::new(),
    }
}

fn handler_with_genesis() -> (RpcHandler, Arc<BlockStore>, StateDB, Arc<TransactionPool>) {
    let store = Arc::new(BlockStore::new(poa()));
    store.insert_genesis(genesis()).unwrap();
    let state = StateDB::new();
    let pool = Arc::new(TransactionPool::new());
    let handler = RpcHandler::new(RpcContext::new(
        Arc::clone(&store),
        Arc::clone(&pool),
        state.clone(),
        1,
    ));
    (handler, store, state, pool)
}

fn empty_handler() -> RpcHandler {
    RpcHandler::new(RpcContext::new(
        Arc::new(BlockStore::new(poa())),
        Arc::new(TransactionPool::new()),
        StateDB::new(),
        99,
    ))
}

#[test]
fn chain_id() {
    let h = empty_handler();
    assert_eq!(h.handle("eth_chainId", json!([])).unwrap(), json!("0x63"));
}

#[test]
fn transaction_count_missing_account_is_zero() {
    let (h, _, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(3).2;
    assert_eq!(
        h.handle("eth_getTransactionCount", json!([addr.to_hex(), "latest"]))
            .unwrap(),
        json!("0x0")
    );
}

#[test]
fn chain_id_ignores_params() {
    let h = empty_handler();
    assert!(h.handle("eth_chainId", json!(["x"])).is_ok());
}

#[test]
fn block_number_missing_genesis() {
    let h = empty_handler();
    assert_eq!(
        h.handle("eth_blockNumber", json!([])).unwrap_err(),
        RpcError::BlockNotFound
    );
}

#[test]
fn block_number_genesis_is_zero() {
    let (h, _, _, _) = handler_with_genesis();
    assert_eq!(
        h.handle("eth_blockNumber", json!([])).unwrap(),
        json!("0x0")
    );
}

#[test]
fn get_balance_missing_account_is_zero() {
    let (h, _, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2.to_hex();
    let v = h.handle("eth_getBalance", json!([addr, "latest"])).unwrap();
    assert_eq!(v, json!(U256::ZERO.to_hex()));
}

#[test]
fn get_balance_funded() {
    let (h, _, state, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2;
    let mut acc = Account::new();
    acc.balance = U256::from(42u64);
    state.set_account(addr, acc);
    let v = h
        .handle("eth_getBalance", json!([addr.to_hex(), "latest"]))
        .unwrap();
    assert_eq!(v, json!(U256::from(42u64).to_hex()));
}

#[test]
fn get_balance_bad_address() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBalance", json!(["0xzz", "latest"])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_balance_missing_params() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBalance", json!([])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_balance_not_array() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBalance", json!({"addr": "x"})),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_code_empty() {
    let (h, _, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2.to_hex();
    assert_eq!(
        h.handle("eth_getCode", json!([addr, "latest"])).unwrap(),
        json!("0x")
    );
}

#[test]
fn get_code_set() {
    let (h, _, state, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2;
    state.set_code(addr, Bytes::from_slice(&[0xaa, 0xbb]));
    assert_eq!(
        h.handle("eth_getCode", json!([addr.to_hex(), "latest"]))
            .unwrap(),
        json!("0xaabb")
    );
}

#[test]
fn get_storage_at_zero() {
    let (h, _, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2.to_hex();
    let slot = H256::ZERO.to_hex();
    let v = h
        .handle("eth_getStorageAt", json!([addr, slot, "latest"]))
        .unwrap();
    assert_eq!(v, json!(U256::ZERO.to_hex()));
}

#[test]
fn get_storage_at_value() {
    let (h, _, state, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2;
    let slot = H256::from_bytes([3u8; 32]);
    state.set_storage(addr, slot, U256::from(7u64));
    let v = h
        .handle(
            "eth_getStorageAt",
            json!([addr.to_hex(), slot.to_hex(), "latest"]),
        )
        .unwrap();
    assert_eq!(v, json!(U256::from(7u64).to_hex()));
}

#[test]
fn get_storage_missing_slot_param() {
    let (h, _, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(1).2.to_hex();
    assert!(matches!(
        h.handle("eth_getStorageAt", json!([addr])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_block_by_number_latest() {
    let (h, store, _, _) = handler_with_genesis();
    let v = h
        .handle("eth_getBlockByNumber", json!(["latest", false]))
        .unwrap();
    assert_eq!(v["number"], json!("0x0"));
    assert_eq!(
        v["hash"],
        json!(store.head_block().unwrap().hash().to_hex())
    );
}

#[test]
fn get_block_by_number_earliest() {
    let (h, _, _, _) = handler_with_genesis();
    let v = h
        .handle("eth_getBlockByNumber", json!(["earliest", false]))
        .unwrap();
    assert_eq!(v["number"], json!("0x0"));
}

#[test]
fn get_block_by_number_hex() {
    let (h, _, _, _) = handler_with_genesis();
    let v = h
        .handle("eth_getBlockByNumber", json!(["0x0", false]))
        .unwrap();
    assert_eq!(v["number"], json!("0x0"));
}

#[test]
fn get_block_by_number_int() {
    let (h, _, _, _) = handler_with_genesis();
    let v = h.handle("eth_getBlockByNumber", json!([0, false])).unwrap();
    assert_eq!(v["number"], json!("0x0"));
}

#[test]
fn get_block_by_number_pending_alias() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(
        h.handle("eth_getBlockByNumber", json!(["pending", false]))
            .is_ok()
    );
}

#[test]
fn get_block_by_number_missing() {
    let (h, _, _, _) = handler_with_genesis();
    assert_eq!(
        h.handle("eth_getBlockByNumber", json!(["0x99", false]))
            .unwrap_err(),
        RpcError::BlockNotFound
    );
}

#[test]
fn get_block_by_number_bad_id() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBlockByNumber", json!(["nope", false])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_block_by_number_missing_params() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBlockByNumber", json!([])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_block_by_hash() {
    let (h, store, _, _) = handler_with_genesis();
    let hash = store.head().unwrap().to_hex();
    let v = h
        .handle("eth_getBlockByHash", json!([hash, false]))
        .unwrap();
    assert_eq!(v["number"], json!("0x0"));
}

#[test]
fn get_block_by_hash_missing() {
    let (h, _, _, _) = handler_with_genesis();
    let hash = H256::from_bytes([1u8; 32]).to_hex();
    assert_eq!(
        h.handle("eth_getBlockByHash", json!([hash])).unwrap_err(),
        RpcError::BlockNotFound
    );
}

#[test]
fn get_block_by_hash_bad() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_getBlockByHash", json!(["0x1"])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn send_raw_and_get_pending_tx() {
    let (h, _, _, pool) = handler_with_genesis();
    let tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let hash = tx.hash();
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    let v = h.handle("eth_sendRawTransaction", json!([raw])).unwrap();
    assert_eq!(v, json!(hash.to_hex()));
    assert!(pool.contains(&hash));
    let got = h
        .handle("eth_getTransactionByHash", json!([hash.to_hex()]))
        .unwrap();
    assert_eq!(got["hash"], json!(hash.to_hex()));
    assert!(got["blockHash"].is_null());
}

#[test]
fn send_raw_invokes_gossip_hook() {
    use std::sync::Mutex;

    let store = Arc::new(BlockStore::new(poa()));
    store.insert_genesis(genesis()).unwrap();
    let seen = Arc::new(Mutex::new(None));
    let seen_cb = Arc::clone(&seen);
    let handler = RpcHandler::new(
        RpcContext::new(store, Arc::new(TransactionPool::new()), StateDB::new(), 1).with_gossip(
            move |tx| {
                *seen_cb.lock().unwrap() = Some(tx.hash());
            },
        ),
    );
    let tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let hash = tx.hash();
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    handler
        .handle("eth_sendRawTransaction", json!([raw]))
        .unwrap();
    assert_eq!(*seen.lock().unwrap(), Some(hash));
}

#[test]
fn send_raw_bad_hex() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_sendRawTransaction", json!(["0xzz"])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn send_raw_bad_payload() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_sendRawTransaction", json!(["0xabcd"])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn send_raw_invalid_signature() {
    let (h, _, _, _) = handler_with_genesis();
    let mut tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let mut bytes = tx.signature.to_bytes();
    bytes[0] ^= 0xff;
    tx.signature = ivory_primitives::Signature::from_bytes(bytes);
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    assert!(matches!(
        h.handle("eth_sendRawTransaction", json!([raw])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn send_raw_missing_params() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle("eth_sendRawTransaction", json!([])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn get_transaction_missing() {
    let (h, _, _, _) = handler_with_genesis();
    assert_eq!(
        h.handle("eth_getTransactionByHash", json!([H256::ZERO.to_hex()]))
            .unwrap_err(),
        RpcError::TransactionNotFound
    );
}

#[test]
fn get_receipt_missing() {
    let (h, _, _, _) = handler_with_genesis();
    assert_eq!(
        h.handle("eth_getTransactionReceipt", json!([H256::ZERO.to_hex()]))
            .unwrap_err(),
        RpcError::TransactionNotFound
    );
}

#[test]
fn get_tx_and_receipt_from_block() {
    let (h, store, _, _) = handler_with_genesis();
    let tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let hash = tx.hash();
    let parent = store.head_block().unwrap();
    let receipts = vec![ivory_core::Receipt {
        tx_hash: hash,
        block_number: 1,
        gas_used: 21_000,
        status: true,
        logs: Vec::new(),
    }];
    let mut header = BlockHeader {
        number: 1,
        parent_hash: parent.hash(),
        timestamp: 2,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 21_000,
        state_root: H256::ZERO,
        transactions_root: list_root(std::slice::from_ref(&tx)),
        receipts_root: list_root(&receipts),
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    let block = Block {
        header,
        transactions: vec![tx],
        receipts,
    };
    store.insert_block(block).unwrap();
    let got = h
        .handle("eth_getTransactionByHash", json!([hash.to_hex()]))
        .unwrap();
    assert_eq!(got["blockNumber"], json!("0x1"));
    let rec = h
        .handle("eth_getTransactionReceipt", json!([hash.to_hex()]))
        .unwrap();
    assert_eq!(rec["status"], json!("0x1"));
    assert_eq!(rec["gasUsed"], json!("0x5208"));
    assert_eq!(rec["logs"], json!([]));
    assert!(rec["contractAddress"].is_null());
}

#[test]
fn send_raw_quant_envelope_roundtrip() {
    let (h, _, _, _) = handler_with_genesis();
    let env = QuantEnvelope {
        version: ivory_core::QUANT_SCHEMA_VERSION,
        decision_id: "d-1".into(),
        schema: "app.v1".into(),
        metrics: vec![QuantMetric {
            name: "score".into(),
            value: "1".into(),
        }],
        content_hash: None,
        cid: None,
    };
    let data = env.encode();
    let gas = 21_000u64.saturating_add(16 * data.as_slice().len() as u64);
    let tx = signed_tx(
        &keypair_from_byte(1).0,
        Some(keypair_from_byte(2).2),
        0,
        U256::ZERO,
        gas,
        U256::ONE,
        data,
    );
    let hash = tx.hash();
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    h.handle("eth_sendRawTransaction", json!([raw])).unwrap();
    let got = h
        .handle("eth_getTransactionByHash", json!([hash.to_hex()]))
        .unwrap();
    let input = got["input"].as_str().unwrap();
    let bytes = hex::decode(input.trim_start_matches("0x")).unwrap();
    let decoded = QuantEnvelope::decode(&bytes).unwrap();
    assert_eq!(decoded.decision_id, "d-1");
    assert_eq!(decoded.schema, "app.v1");
}

#[test]
fn node_info_defaults_to_follower() {
    let (h, store, _, _) = handler_with_genesis();
    let genesis = store.head_block().unwrap();
    let info = h.handle("ivory_nodeInfo", json!([])).unwrap();
    assert_eq!(info["role"], json!("follower"));
    assert_eq!(info["chainId"], json!("0x1"));
    assert_eq!(info["pending"], json!(0));
    assert_eq!(info["peers"], json!(0));
    assert_eq!(info["headNumber"], json!("0x0"));
    assert_eq!(info["headHash"], json!(genesis.hash().to_hex()));
    assert_eq!(info["bootstrap"], json!([]));
}

#[test]
fn node_info_pending_and_producer_role() {
    use std::sync::atomic::AtomicUsize;

    let store = Arc::new(BlockStore::new(poa()));
    store.insert_genesis(genesis()).unwrap();
    let pool = Arc::new(TransactionPool::new());
    let addr = miner();
    let h = RpcHandler::new(
        RpcContext::new(Arc::clone(&store), Arc::clone(&pool), StateDB::new(), 7).with_node_info(
            NodeRole::Producer,
            addr,
            "peer-test".into(),
            Arc::new(AtomicUsize::new(2)),
            vec!["/ip4/127.0.0.1/tcp/9000".into()],
        ),
    );
    let tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    h.handle("eth_sendRawTransaction", json!([raw])).unwrap();
    let info = h.handle("ivory_nodeInfo", json!([])).unwrap();
    assert_eq!(info["role"], json!("producer"));
    assert_eq!(info["address"], json!(addr.to_hex()));
    assert_eq!(info["chainId"], json!("0x7"));
    assert_eq!(info["peerId"], json!("peer-test"));
    assert_eq!(info["peers"], json!(2));
    assert_eq!(info["pending"], json!(1));
    assert_eq!(info["bootstrap"], json!(["/ip4/127.0.0.1/tcp/9000"]));
}

#[test]
fn list_contracts_empty_without_creates() {
    let (h, _, _, _) = handler_with_genesis();
    assert_eq!(
        h.handle("ivory_listContracts", json!([])).unwrap(),
        json!([])
    );
}

#[test]
fn list_contracts_after_create() {
    let (h, store, state, _) = handler_with_genesis();
    let code = Bytes::from_vec(vec![0xaa, 0xbb, 0xcc]);
    let tx = signed_tx(
        &keypair_from_byte(1).0,
        None,
        0,
        U256::ZERO,
        100_000,
        U256::ONE,
        code.clone(),
    );
    let hash = tx.hash();
    let addr = Address::create(&tx.from, tx.nonce);
    state.set_code(addr, code);
    let parent = store.head_block().unwrap();
    let receipts = vec![ivory_core::Receipt {
        tx_hash: hash,
        block_number: 1,
        gas_used: 50_000,
        status: true,
        logs: Vec::new(),
    }];
    let mut header = BlockHeader {
        number: 1,
        parent_hash: parent.hash(),
        timestamp: 2,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 50_000,
        state_root: H256::ZERO,
        transactions_root: list_root(std::slice::from_ref(&tx)),
        receipts_root: list_root(&receipts),
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    store
        .insert_block(Block {
            header,
            transactions: vec![tx],
            receipts,
        })
        .unwrap();
    let list = h.handle("ivory_listContracts", json!([])).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["address"], json!(addr.to_hex()));
    assert_eq!(list[0]["codeSize"], json!(3));
    assert_eq!(list[0]["blockNumber"], json!("0x1"));
    assert_eq!(list[0]["transactionHash"], json!(hash.to_hex()));
    assert_eq!(list[0]["registered"], json!(false));
}

#[test]
fn list_contracts_matches_file_catalog() {
    use ivory_primitives::keccak256;
    use ivory_rpc::ContractMeta;

    let (_old, store, state, _) = handler_with_genesis();
    let code = Bytes::from_vec(vec![0x11, 0x22]);
    let hash_code = keccak256(code.as_slice());
    let tx = signed_tx(
        &keypair_from_byte(1).0,
        None,
        0,
        U256::ZERO,
        100_000,
        U256::ONE,
        code.clone(),
    );
    let tx_hash = tx.hash();
    let addr = Address::create(&tx.from, tx.nonce);
    state.set_code(addr, code);
    let parent = store.head_block().unwrap();
    let receipts = vec![ivory_core::Receipt {
        tx_hash,
        block_number: 1,
        gas_used: 50_000,
        status: true,
        logs: Vec::new(),
    }];
    let mut header = BlockHeader {
        number: 1,
        parent_hash: parent.hash(),
        timestamp: 2,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 50_000,
        state_root: H256::ZERO,
        transactions_root: list_root(std::slice::from_ref(&tx)),
        receipts_root: list_root(&receipts),
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    store
        .insert_block(Block {
            header,
            transactions: vec![tx],
            receipts,
        })
        .unwrap();
    let h = RpcHandler::new(
        RpcContext::new(store, Arc::new(TransactionPool::new()), state, 1).with_contract_lookup(
            move |h| {
                if *h == hash_code {
                    Some(ContractMeta {
                        name: "tracker".into(),
                        schema: "app.v1".into(),
                        source: "contracts/tracker.yaml".into(),
                        description: "marker".into(),
                    })
                } else {
                    None
                }
            },
        ),
    );
    let list = h.handle("ivory_listContracts", json!([])).unwrap();
    assert_eq!(list[0]["name"], json!("tracker"));
    assert_eq!(list[0]["schema"], json!("app.v1"));
    assert_eq!(list[0]["registered"], json!(true));
}

#[test]
fn method_not_found() {
    let h = empty_handler();
    assert!(matches!(
        h.handle("eth_foo", json!([])),
        Err(RpcError::MethodNotFound(_))
    ));
}

#[test]
fn eth_subscribe_http_is_websocket_only() {
    let h = empty_handler();
    assert!(matches!(
        h.handle("eth_subscribe", json!(["newHeads"])),
        Err(RpcError::Server(_))
    ));
}

#[test]
fn eth_call_eoa_does_not_mutate_live() {
    let (h, _, state, _) = handler_with_genesis();
    let from = keypair_from_byte(1).2;
    let to = keypair_from_byte(2).2;
    let mut acc = Account::new();
    acc.balance = U256::from(1_000_000u64);
    state.set_account(from, acc);
    let out = h
        .handle(
            "eth_call",
            json!([{
                "from": from.to_hex(),
                "to": to.to_hex(),
                "value": "0x64",
                "gas": "0x5208"
            }]),
        )
        .unwrap();
    assert_eq!(out, json!("0x"));
    assert_eq!(
        state.get_account(&from).unwrap().balance,
        U256::from(1_000_000u64)
    );
    assert!(state.get_account(&to).is_none());
}

#[test]
fn eth_call_wasm_returns_padded_i32() {
    let (h, _, state, _) = handler_with_genesis();
    let wasm = wat::parse_str(
        r#"(module
          (func (export "call") (result i32)
            i32.const 42
          )
        )"#,
    )
    .unwrap();
    let to = keypair_from_byte(3).2;
    state.set_code(to, Bytes::from_vec(wasm));
    let out = h
        .handle("eth_call", json!([{ "to": to.to_hex() }, "latest"]))
        .unwrap();
    let hex = out.as_str().unwrap();
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 66);
    assert!(hex.ends_with("2a"));
}

#[test]
fn eth_estimate_gas_create_includes_data_cost() {
    let (h, _, _, _) = handler_with_genesis();
    let from = keypair_from_byte(1).2;
    let out = h
        .handle(
            "eth_estimateGas",
            json!([{
                "from": from.to_hex(),
                "to": null,
                "data": "0x01020304",
                "gas": "0x186a0"
            }]),
        )
        .unwrap();
    let qty = out.as_str().unwrap();
    let n = u64::from_str_radix(qty.trim_start_matches("0x"), 16).unwrap();
    assert!(n >= 21_000 + 4 * 16);
}

#[test]
fn eth_call_historical_uses_state_at_snapshot() {
    let (h, store, live, _) = handler_with_genesis();
    let wasm = wat::parse_str(
        r#"(module
          (func (export "call") (result i32)
            i32.const 42
          )
        )"#,
    )
    .unwrap();
    let to = keypair_from_byte(3).2;
    let parent = store.head_block().unwrap();
    let (tx_root, rx_root) = empty_list_roots();
    let mut header = BlockHeader {
        number: 1,
        parent_hash: parent.hash(),
        timestamp: 2,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 0,
        state_root: H256::ZERO,
        transactions_root: tx_root,
        receipts_root: rx_root,
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    let block = Block {
        header,
        transactions: Vec::new(),
        receipts: Vec::new(),
    };
    let hash = store.insert_block(block).unwrap().hash;
    let snap = StateDB::new();
    snap.set_code(to, Bytes::from_vec(wasm));
    store.record_state(hash, snap);
    assert!(live.get_code(&to).is_empty());
    let historic = h
        .handle("eth_call", json!([{ "to": to.to_hex() }, "0x1"]))
        .unwrap();
    let hex = historic.as_str().unwrap();
    assert_eq!(hex.len(), 66);
    assert!(hex.ends_with("2a"));
    let latest = h
        .handle("eth_call", json!([{ "to": to.to_hex() }, "latest"]))
        .unwrap();
    assert_eq!(latest, json!("0x"));
}

#[test]
fn eth_estimate_gas_transfer_at_least_intrinsic() {
    let (h, _, state, _) = handler_with_genesis();
    let from = keypair_from_byte(1).2;
    let to = keypair_from_byte(2).2;
    let mut acc = Account::new();
    acc.balance = U256::from(1_000_000u64);
    state.set_account(from, acc);
    let out = h
        .handle(
            "eth_estimateGas",
            json!([{
                "from": from.to_hex(),
                "to": to.to_hex(),
                "value": "0x1",
                "gas": "0x186a0"
            }]),
        )
        .unwrap();
    let qty = out.as_str().unwrap();
    let n = u64::from_str_radix(qty.trim_start_matches("0x"), 16).unwrap();
    assert!(n >= 21_000);
}

#[test]
fn eth_call_unknown_block_tag_number() {
    let (h, _, _, _) = handler_with_genesis();
    assert!(matches!(
        h.handle(
            "eth_call",
            json!([{ "to": keypair_from_byte(2).2.to_hex() }, "0x1"])
        ),
        Err(RpcError::BlockNotFound)
    ));
}

#[test]
fn eth_call_missing_tx_object() {
    let h = empty_handler();
    assert!(matches!(
        h.handle("eth_call", json!([])),
        Err(RpcError::InvalidParams(_))
    ));
}

#[test]
fn eth_send_transaction_not_found() {
    let h = empty_handler();
    assert!(matches!(
        h.handle("eth_sendTransaction", json!([])),
        Err(RpcError::MethodNotFound(_))
    ));
}

#[test]
fn eth_fee_history_not_found() {
    let h = empty_handler();
    assert!(matches!(
        h.handle("eth_feeHistory", json!([])),
        Err(RpcError::MethodNotFound(_))
    ));
}

#[test]
fn eth_call_historical_uses_state_at() {
    let (h, store, state, _) = handler_with_genesis();
    let to = keypair_from_byte(3).2;
    let hist =
        wat::parse_str(r#"(module (func (export "call") (result i32) i32.const 1))"#).unwrap();
    let live =
        wat::parse_str(r#"(module (func (export "call") (result i32) i32.const 2))"#).unwrap();
    let genesis = store.get_block_by_number(0).unwrap();
    let snap = StateDB::new();
    snap.set_code(to, Bytes::from_vec(hist));
    store.record_state(genesis.hash(), snap);
    state.set_code(to, Bytes::from_vec(live));
    let hist_out = h
        .handle("eth_call", json!([{ "to": to.to_hex() }, "0x0"]))
        .unwrap();
    let live_out = h
        .handle("eth_call", json!([{ "to": to.to_hex() }, "latest"]))
        .unwrap();
    assert!(hist_out.as_str().unwrap().ends_with("01"));
    assert!(live_out.as_str().unwrap().ends_with("02"));
}

#[test]
fn eth_estimate_gas_create_covers_intrinsic_and_data() {
    let (h, _, state, _) = handler_with_genesis();
    let from = keypair_from_byte(1).2;
    let mut acc = Account::new();
    acc.balance = U256::from(1_000_000u64);
    state.set_account(from, acc);
    let data = format!("0x{}", "aa".repeat(10));
    let out = h
        .handle(
            "eth_estimateGas",
            json!([{
                "from": from.to_hex(),
                "data": data,
                "gas": "0x186a0"
            }]),
        )
        .unwrap();
    let n = u64::from_str_radix(out.as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
    assert!(n >= 21_000 + 16 * 10);
}

#[test]
fn eth_call_calldata_first_byte() {
    let (h, _, state, _) = handler_with_genesis();
    let wasm = wat::parse_str(
        r#"(module
          (import "env" "calldata_at" (func $at (param i32) (result i32)))
          (func (export "call") (result i32)
            i32.const 0
            call $at
          )
        )"#,
    )
    .unwrap();
    let to = keypair_from_byte(4).2;
    state.set_code(to, Bytes::from_vec(wasm));
    let out = h
        .handle(
            "eth_call",
            json!([{ "to": to.to_hex(), "data": "0x2a" }, "latest"]),
        )
        .unwrap();
    assert!(out.as_str().unwrap().ends_with("2a"));
}

#[test]
fn eth_get_logs_filters_address_and_caps_range() {
    let (h, store, _, _) = handler_with_genesis();
    let addr = keypair_from_byte(5).2;
    let tx = signed_transfer(
        &keypair_from_byte(1).0,
        keypair_from_byte(2).2,
        0,
        U256::from(1u64),
        21_000,
    );
    let log = ivory_core::Log {
        address: addr,
        topics: Vec::new(),
        data: Bytes::new(),
    };
    let receipt = ivory_core::Receipt {
        tx_hash: tx.hash(),
        block_number: 1,
        gas_used: 21_000,
        status: true,
        logs: vec![log],
    };
    let parent = store.head_block().unwrap();
    let mut header = BlockHeader {
        number: 1,
        parent_hash: parent.hash(),
        timestamp: 2,
        miner: miner(),
        gas_limit: 30_000_000,
        gas_used: 21_000,
        state_root: H256::ZERO,
        transactions_root: list_root(std::slice::from_ref(&tx)),
        receipts_root: list_root(std::slice::from_ref(&receipt)),
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa()
        .seal_header(&mut header, &miner(), &miner_sk())
        .unwrap();
    store
        .insert_block(Block {
            header,
            transactions: vec![tx],
            receipts: vec![receipt],
        })
        .unwrap();
    let logs = h
        .handle(
            "eth_getLogs",
            json!([{ "fromBlock": "0x0", "toBlock": "latest", "address": addr.to_hex() }]),
        )
        .unwrap();
    assert_eq!(logs.as_array().unwrap().len(), 1);
    assert!(
        h.handle(
            "eth_getLogs",
            json!([{ "fromBlock": "0x0", "toBlock": "0x3e9" }]),
        )
        .is_err()
    );
}

#[test]
fn ivory_get_header_by_number() {
    let (h, _, _, _) = handler_with_genesis();
    let hdr = h.handle("ivory_getHeaderByNumber", json!(["0x0"])).unwrap();
    assert!(hdr.get("transactionsRoot").is_some());
    assert!(hdr.get("transactions").is_none());
}

#[test]
fn unknown_method_maps_jsonrpc_code() {
    let err: ivory_rpc::JsonRpcError = RpcError::MethodNotFound("x".into()).into();
    assert_eq!(err.code, -32601);
}

#[test]
fn invalid_params_maps_jsonrpc_code() {
    let err: ivory_rpc::JsonRpcError = RpcError::InvalidParams("x".into()).into();
    assert_eq!(err.code, -32602);
}

#[test]
fn block_not_found_maps_custom_code() {
    let err: ivory_rpc::JsonRpcError = RpcError::BlockNotFound.into();
    assert_eq!(err.code, -32001);
}

#[test]
fn tx_not_found_maps_custom_code() {
    let err: ivory_rpc::JsonRpcError = RpcError::TransactionNotFound.into();
    assert_eq!(err.code, -32000);
}

#[test]
fn parse_error_code() {
    assert_eq!(ivory_rpc::JsonRpcError::parse_error().code, -32700);
}

#[test]
fn invalid_request_code() {
    assert_eq!(ivory_rpc::JsonRpcError::invalid_request().code, -32600);
}
