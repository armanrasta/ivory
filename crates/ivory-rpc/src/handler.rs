//! JSON-RPC method dispatch.

use std::sync::atomic::Ordering;

use ivory_core::{Block, Transaction};
use ivory_executor::{Executor, SimulateRequest};
use ivory_primitives::{Address, Bytes, H256, U256, keccak256};
use ivory_state::StateDB;
use ivory_txpool::TxOrigin;
use serde_json::{Value, json};

use crate::context::{RpcContext, RpcEvent};
use crate::error::RpcError;
use crate::jsonrpc::JsonRpcError;
use crate::types::{BlockNumberOrTag, BlockTag, TransactionRequest};

/// Dispatches `eth_*` methods against [`RpcContext`].
#[derive(Clone)]
pub struct RpcHandler {
    ctx: RpcContext,
}

impl RpcHandler {
    /// Create a handler.
    #[must_use]
    pub fn new(ctx: RpcContext) -> Self {
        Self { ctx }
    }

    /// Shared backend.
    #[must_use]
    pub fn context(&self) -> &RpcContext {
        &self.ctx
    }

    /// Dispatch `method` with JSON-RPC `params`.
    ///
    /// # Errors
    ///
    /// [`RpcError`] for unknown methods, bad params, or missing objects.
    pub fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        if let Some(allow) = &self.ctx.allow_methods
            && !allow.iter().any(|m| m == method)
        {
            let err = RpcError::MethodNotFound(method.to_string());
            self.ctx.metrics.rpc_request(method);
            self.ctx.metrics.rpc_error(method, -32601);
            return Err(err);
        }
        let result = match method {
            "eth_chainId" => self.chain_id(),
            "eth_blockNumber" => self.block_number(),
            "eth_getBalance" => self.get_balance(params),
            "eth_getCode" => self.get_code(params),
            "eth_getStorageAt" => self.get_storage_at(params),
            "eth_getBlockByNumber" => self.get_block_by_number(params),
            "eth_getBlockByHash" => self.get_block_by_hash(params),
            "eth_getTransactionByHash" => self.get_transaction_by_hash(params),
            "eth_getTransactionReceipt" => self.get_transaction_receipt(params),
            "eth_getTransactionCount" => self.get_transaction_count(params),
            "eth_sendRawTransaction" => self.send_raw_transaction(params),
            "eth_call" => self.eth_call(params),
            "eth_estimateGas" => self.eth_estimate_gas(params),
            "eth_getLogs" => self.eth_get_logs(params),
            "eth_getProof" => Err(RpcError::Server(
                "eth_getProof needs persisted Patricia nodes; see docs/rpc.md".into(),
            )),
            "eth_subscribe" | "eth_unsubscribe" => Err(RpcError::Server("WebSocket only".into())),
            "ivory_nodeInfo" => self.node_info(),
            "ivory_listContracts" => self.list_contracts(),
            "ivory_getHeaderByNumber" => self.get_header_by_number(params),
            other => Err(RpcError::MethodNotFound(other.to_string())),
        };
        self.ctx.metrics.rpc_request(method);
        if let Err(err) = &result {
            let code = JsonRpcError::from(err.clone()).code;
            self.ctx.metrics.rpc_error(method, code);
        }
        result
    }

    fn chain_id(&self) -> Result<Value, RpcError> {
        Ok(Value::String(encode_qty(self.ctx.chain_id)))
    }

    fn block_number(&self) -> Result<Value, RpcError> {
        let n = self
            .ctx
            .store
            .head_block()
            .map(|b| b.header.number)
            .ok_or(RpcError::BlockNotFound)?;
        Ok(Value::String(encode_qty(n)))
    }

    fn get_balance(&self, params: Value) -> Result<Value, RpcError> {
        let addr = parse_address_at(&params, 0)?;
        let _tag = parse_block_tag_at(&params, 1).unwrap_or(BlockTag::Latest);
        let balance = self
            .ctx
            .state
            .get_account(&addr)
            .map(|a| a.balance)
            .unwrap_or(U256::ZERO);
        Ok(Value::String(balance.to_hex()))
    }

    fn get_code(&self, params: Value) -> Result<Value, RpcError> {
        let addr = parse_address_at(&params, 0)?;
        let code = self.ctx.state.get_code(&addr);
        Ok(Value::String(format!("0x{}", hex::encode(code))))
    }

    fn get_storage_at(&self, params: Value) -> Result<Value, RpcError> {
        let addr = parse_address_at(&params, 0)?;
        let slot = parse_h256_at(&params, 1)?;
        let val = self.ctx.state.get_storage(&addr, &slot);
        Ok(Value::String(val.to_hex()))
    }

    fn get_transaction_count(&self, params: Value) -> Result<Value, RpcError> {
        let addr = parse_address_at(&params, 0)?;
        let _tag = parse_block_tag_at(&params, 1).unwrap_or(BlockTag::Latest);
        let nonce = self
            .ctx
            .state
            .get_account(&addr)
            .map(|a| a.nonce)
            .unwrap_or(0);
        Ok(Value::String(encode_qty(nonce)))
    }

    fn get_block_by_number(&self, params: Value) -> Result<Value, RpcError> {
        let tag = parse_block_id_at(&params, 0)?;
        let block = self.block_by_tag(tag).ok_or(RpcError::BlockNotFound)?;
        Ok(block_to_json(&block))
    }

    fn get_block_by_hash(&self, params: Value) -> Result<Value, RpcError> {
        let hash = parse_h256_at(&params, 0)?;
        let block = self
            .ctx
            .store
            .get_block(&hash)
            .ok_or(RpcError::BlockNotFound)?;
        Ok(block_to_json(&block))
    }

    fn get_transaction_by_hash(&self, params: Value) -> Result<Value, RpcError> {
        let hash = parse_h256_at(&params, 0)?;
        if let Some(tx) = self.ctx.pool.get(&hash) {
            return Ok(tx_to_json(&tx, None, None, None));
        }
        let (tx, loc) = self
            .ctx
            .store
            .get_transaction(&hash)
            .ok_or(RpcError::TransactionNotFound)?;
        Ok(tx_to_json(
            &tx,
            Some(loc.block_hash),
            Some(loc.block_number),
            Some(loc.index as u64),
        ))
    }

    fn get_transaction_receipt(&self, params: Value) -> Result<Value, RpcError> {
        let hash = parse_h256_at(&params, 0)?;
        let (tx, loc) = self
            .ctx
            .store
            .get_transaction(&hash)
            .ok_or(RpcError::TransactionNotFound)?;
        let block = self
            .ctx
            .store
            .get_block(&loc.block_hash)
            .ok_or(RpcError::BlockNotFound)?;
        let receipt = block
            .receipts
            .get(loc.index)
            .ok_or(RpcError::TransactionNotFound)?;
        let contract_address = if tx.is_create() {
            Value::String(Address::create(&tx.from, tx.nonce).to_hex())
        } else {
            Value::Null
        };
        Ok(json!({
            "transactionHash": hash.to_hex(),
            "transactionIndex": encode_qty(loc.index as u64),
            "blockHash": loc.block_hash.to_hex(),
            "blockNumber": encode_qty(loc.block_number),
            "from": tx.from.to_hex(),
            "to": tx.to.map(|a| a.to_hex()),
            "gasUsed": encode_qty(receipt.gas_used),
            "cumulativeGasUsed": encode_qty(receipt.gas_used),
            "status": if receipt.status { "0x1" } else { "0x0" },
            "logs": receipt.logs.iter().map(log_to_json).collect::<Vec<_>>(),
            "contractAddress": contract_address,
        }))
    }

    fn send_raw_transaction(&self, params: Value) -> Result<Value, RpcError> {
        let hex_str = parse_string_at(&params, 0)?;
        let raw = decode_hex(&hex_str).map_err(RpcError::InvalidParams)?;
        let tx: Transaction =
            bincode::deserialize(&raw).map_err(|e| RpcError::InvalidParams(e.to_string()))?;
        let admitted = tx.clone();
        let hash = self
            .ctx
            .pool
            .add_transaction(tx, TxOrigin::Local)
            .map_err(|e| RpcError::InvalidParams(e.to_string()))?;
        if let Some(cb) = &self.ctx.on_tx {
            cb(admitted);
        }
        self.ctx.emit(RpcEvent::NewPendingTx { hash });
        Ok(Value::String(hash.to_hex()))
    }

    fn eth_call(&self, params: Value) -> Result<Value, RpcError> {
        let out = self.simulate(params)?;
        Ok(Value::String(format!(
            "0x{}",
            hex::encode(out.call.output.as_slice())
        )))
    }

    fn eth_estimate_gas(&self, params: Value) -> Result<Value, RpcError> {
        let out = self.simulate(params)?;
        Ok(Value::String(encode_qty(out.gas_used)))
    }

    fn simulate(&self, params: Value) -> Result<ivory_executor::SimulateOutcome, RpcError> {
        let (req, tag) = parse_call_args(&params)?;
        let state = self.state_for_tag(tag)?;
        Executor::new(state)
            .simulate(req)
            .map_err(|e| RpcError::Server(e.to_string()))
    }

    fn state_for_tag(&self, tag: BlockNumberOrTag) -> Result<StateDB, RpcError> {
        match tag {
            BlockNumberOrTag::Tag(BlockTag::Latest)
            | BlockNumberOrTag::Tag(BlockTag::Pending)
            | BlockNumberOrTag::Tag(BlockTag::Safe)
            | BlockNumberOrTag::Tag(BlockTag::Finalized) => Ok(self.ctx.state.fork()),
            other => {
                let block = self.block_by_tag(other).ok_or(RpcError::BlockNotFound)?;
                self.ctx
                    .store
                    .state_at(&block.hash())
                    .ok_or(RpcError::BlockNotFound)
            }
        }
    }

    fn node_info(&self) -> Result<Value, RpcError> {
        let (head_number, head_hash) = match self.ctx.store.head_block() {
            Some(block) => (
                encode_qty(block.header.number),
                Value::String(block.hash().to_hex()),
            ),
            None => ("0x0".into(), Value::Null),
        };
        Ok(json!({
            "role": self.ctx.role.as_str(),
            "address": self.ctx.address.to_hex(),
            "chainId": encode_qty(self.ctx.chain_id),
            "peerId": self.ctx.peer_id,
            "peers": self.ctx.peers.load(Ordering::Relaxed),
            "pending": self.ctx.pool.pending_count(),
            "headNumber": head_number,
            "headHash": head_hash,
            "bootstrap": self.ctx.bootstrap,
        }))
    }

    fn list_contracts(&self) -> Result<Value, RpcError> {
        let Some(head) = self.ctx.store.head_block() else {
            return Ok(json!([]));
        };
        let mut out = Vec::new();
        for n in 1..=head.header.number {
            let Some(block) = self.ctx.store.get_block_by_number(n) else {
                continue;
            };
            for (tx, receipt) in block.transactions.iter().zip(block.receipts.iter()) {
                if !tx.is_create() || !receipt.status {
                    continue;
                }
                let addr = Address::create(&tx.from, tx.nonce);
                let code = self.ctx.state.get_code(&addr);
                let meta = self
                    .ctx
                    .contract_lookup
                    .as_ref()
                    .and_then(|f| f(&keccak256(&code)));
                out.push(json!({
                    "address": addr.to_hex(),
                    "codeSize": code.len(),
                    "blockNumber": encode_qty(n),
                    "transactionHash": tx.hash().to_hex(),
                    "name": meta.as_ref().map(|m| m.name.clone()),
                    "schema": meta.as_ref().map(|m| m.schema.clone()),
                    "source": meta.as_ref().map(|m| m.source.clone()),
                    "description": meta.as_ref().map(|m| m.description.clone()),
                    "registered": meta.is_some(),
                }));
            }
        }
        Ok(Value::Array(out))
    }

    fn get_header_by_number(&self, params: Value) -> Result<Value, RpcError> {
        let tag = parse_block_id_at(&params, 0)?;
        let block = self.block_by_tag(tag).ok_or(RpcError::BlockNotFound)?;
        Ok(header_to_json(&block))
    }

    fn eth_get_logs(&self, params: Value) -> Result<Value, RpcError> {
        let arr = params_array(&params)?;
        let filter = arr
            .first()
            .and_then(Value::as_object)
            .ok_or_else(|| RpcError::InvalidParams("expected filter object".into()))?;
        let head = self
            .ctx
            .store
            .head_block()
            .ok_or(RpcError::BlockNotFound)?
            .header
            .number;
        let from = match filter.get("fromBlock").map(parse_block_id).transpose()? {
            Some(BlockNumberOrTag::Number(n)) => n,
            Some(tag) => self.block_by_tag(tag).map(|b| b.header.number).unwrap_or(0),
            None => 0,
        };
        let to = match filter.get("toBlock").map(parse_block_id).transpose()? {
            Some(BlockNumberOrTag::Number(n)) => n,
            Some(tag) => self
                .block_by_tag(tag)
                .map(|b| b.header.number)
                .unwrap_or(head),
            None => head,
        };
        if to < from {
            return Err(RpcError::InvalidParams("toBlock < fromBlock".into()));
        }
        if to.saturating_sub(from) > 1000 {
            return Err(RpcError::InvalidParams(
                "log scan exceeds 1000 blocks".into(),
            ));
        }
        let addrs = parse_log_addresses(filter.get("address"))?;
        let mut out = Vec::new();
        for n in from..=to {
            let Some(block) = self.ctx.store.get_block_by_number(n) else {
                continue;
            };
            let hash = block.hash();
            for (tx_idx, receipt) in block.receipts.iter().enumerate() {
                for (log_idx, log) in receipt.logs.iter().enumerate() {
                    if !addrs.is_empty() && !addrs.contains(&log.address) {
                        continue;
                    }
                    out.push(json!({
                        "address": log.address.to_hex(),
                        "topics": log.topics.iter().map(|t| t.to_hex()).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(log.data.as_slice())),
                        "blockNumber": encode_qty(n),
                        "blockHash": hash.to_hex(),
                        "transactionHash": receipt.tx_hash.to_hex(),
                        "transactionIndex": encode_qty(tx_idx as u64),
                        "logIndex": encode_qty(log_idx as u64),
                    }));
                }
            }
        }
        Ok(Value::Array(out))
    }

    fn block_by_tag(&self, tag: BlockNumberOrTag) -> Option<Block> {
        match tag {
            BlockNumberOrTag::Number(n) => self.ctx.store.get_block_by_number(n),
            BlockNumberOrTag::Tag(BlockTag::Earliest) => self.ctx.store.get_block_by_number(0),
            BlockNumberOrTag::Tag(BlockTag::Latest)
            | BlockNumberOrTag::Tag(BlockTag::Pending)
            | BlockNumberOrTag::Tag(BlockTag::Safe)
            | BlockNumberOrTag::Tag(BlockTag::Finalized) => self.ctx.store.head_block(),
        }
    }
}

fn encode_qty(n: u64) -> String {
    if n == 0 {
        "0x0".into()
    } else {
        format!("0x{n:x}")
    }
}

fn header_to_json(block: &Block) -> Value {
    json!({
        "number": encode_qty(block.header.number),
        "hash": block.hash().to_hex(),
        "parentHash": block.header.parent_hash.to_hex(),
        "miner": block.header.miner.to_hex(),
        "timestamp": encode_qty(block.header.timestamp),
        "gasLimit": encode_qty(block.header.gas_limit),
        "gasUsed": encode_qty(block.header.gas_used),
        "stateRoot": block.header.state_root.to_hex(),
        "transactionsRoot": block.header.transactions_root.to_hex(),
        "receiptsRoot": block.header.receipts_root.to_hex(),
        "difficulty": block.header.difficulty.to_hex(),
        "extraData": format!("0x{}", hex::encode(block.header.extra_data.as_slice())),
    })
}

fn parse_log_addresses(v: Option<&Value>) -> Result<Vec<Address>, RpcError> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    if v.is_null() {
        return Ok(Vec::new());
    }
    if let Some(s) = v.as_str() {
        return Ok(vec![
            Address::from_hex(s).map_err(|e| RpcError::InvalidParams(e.to_string()))?,
        ]);
    }
    let arr = v
        .as_array()
        .ok_or_else(|| RpcError::InvalidParams("address must be hex or array".into()))?;
    arr.iter()
        .map(|x| {
            let s = x
                .as_str()
                .ok_or_else(|| RpcError::InvalidParams("address entry must be hex".into()))?;
            Address::from_hex(s).map_err(|e| RpcError::InvalidParams(e.to_string()))
        })
        .collect()
}

fn block_to_json(block: &Block) -> Value {
    let hash = block.hash();
    json!({
        "number": encode_qty(block.header.number),
        "hash": hash.to_hex(),
        "parentHash": block.header.parent_hash.to_hex(),
        "miner": block.header.miner.to_hex(),
        "timestamp": encode_qty(block.header.timestamp),
        "gasLimit": encode_qty(block.header.gas_limit),
        "gasUsed": encode_qty(block.header.gas_used),
        "stateRoot": block.header.state_root.to_hex(),
        "transactionsRoot": block.header.transactions_root.to_hex(),
        "receiptsRoot": block.header.receipts_root.to_hex(),
        "extraData": format!("0x{}", hex::encode(block.header.extra_data.as_slice())),
        "transactions": block.transactions.iter().map(|t| t.hash().to_hex()).collect::<Vec<_>>(),
    })
}

fn tx_to_json(
    tx: &Transaction,
    block_hash: Option<H256>,
    block_number: Option<u64>,
    index: Option<u64>,
) -> Value {
    json!({
        "hash": tx.hash().to_hex(),
        "nonce": encode_qty(tx.nonce),
        "blockHash": block_hash.map(|h| h.to_hex()),
        "blockNumber": block_number.map(encode_qty),
        "transactionIndex": index.map(encode_qty),
        "from": tx.from.to_hex(),
        "to": tx.to.map(|a| a.to_hex()),
        "value": tx.value.to_hex(),
        "gas": encode_qty(tx.gas),
        "gasPrice": tx.gas_price.to_hex(),
        "input": format!("0x{}", hex::encode(tx.data.as_slice())),
    })
}

fn log_to_json(log: &ivory_core::Log) -> Value {
    json!({
        "address": log.address.to_hex(),
        "topics": log.topics.iter().map(|t| t.to_hex()).collect::<Vec<_>>(),
        "data": format!("0x{}", hex::encode(log.data.as_slice())),
    })
}

fn params_array(params: &Value) -> Result<&Vec<Value>, RpcError> {
    params
        .as_array()
        .ok_or_else(|| RpcError::InvalidParams("expected array".into()))
}

fn parse_string_at(params: &Value, idx: usize) -> Result<String, RpcError> {
    let arr = params_array(params)?;
    arr.get(idx)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| RpcError::InvalidParams(format!("missing string at {idx}")))
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    hex::decode(s).map_err(|e| e.to_string())
}

fn parse_address_at(params: &Value, idx: usize) -> Result<Address, RpcError> {
    let s = parse_string_at(params, idx)?;
    Address::from_hex(&s).map_err(|e| RpcError::InvalidParams(e.to_string()))
}

fn parse_h256_at(params: &Value, idx: usize) -> Result<H256, RpcError> {
    let s = parse_string_at(params, idx)?;
    H256::from_hex(&s).map_err(|e| RpcError::InvalidParams(e.to_string()))
}

fn parse_block_tag_at(params: &Value, idx: usize) -> Result<BlockTag, RpcError> {
    match parse_block_id_at(params, idx)? {
        BlockNumberOrTag::Tag(t) => Ok(t),
        BlockNumberOrTag::Number(_) => Err(RpcError::InvalidParams("expected block tag".into())),
    }
}

fn parse_block_id_at(params: &Value, idx: usize) -> Result<BlockNumberOrTag, RpcError> {
    let arr = params_array(params)?;
    let v = arr
        .get(idx)
        .ok_or_else(|| RpcError::InvalidParams(format!("missing block id at {idx}")))?;
    parse_block_id(v)
}

fn parse_call_args(params: &Value) -> Result<(SimulateRequest, BlockNumberOrTag), RpcError> {
    let arr = params_array(params)?;
    let obj = arr
        .first()
        .ok_or_else(|| RpcError::InvalidParams("missing transaction object".into()))?;
    let req: TransactionRequest =
        serde_json::from_value(obj.clone()).map_err(|e| RpcError::InvalidParams(e.to_string()))?;
    let tag = match arr.get(1) {
        Some(v) => parse_block_id(v)?,
        None => BlockNumberOrTag::Tag(BlockTag::Latest),
    };
    let data = match req.data.as_deref() {
        None | Some("") => Bytes::new(),
        Some(s) => Bytes::from_vec(decode_hex(s).map_err(RpcError::InvalidParams)?),
    };
    Ok((
        SimulateRequest {
            from: req.from.unwrap_or(Address::ZERO),
            to: req.to,
            value: req.value.unwrap_or(U256::ZERO),
            data,
            gas: gas_limit(req.gas),
        },
        tag,
    ))
}

fn gas_limit(gas: Option<U256>) -> u64 {
    match gas {
        None => 10_000_000,
        Some(v) if v.is_zero() => 10_000_000,
        Some(v) => {
            if v > U256::from(u64::MAX) {
                u64::MAX
            } else {
                v.low_u64()
            }
        }
    }
}

fn parse_block_id(v: &Value) -> Result<BlockNumberOrTag, RpcError> {
    if let Some(n) = v.as_u64() {
        return Ok(BlockNumberOrTag::Number(n));
    }
    let s = v
        .as_str()
        .ok_or_else(|| RpcError::InvalidParams("block id must be string or number".into()))?;
    match s {
        "latest" => Ok(BlockNumberOrTag::Tag(BlockTag::Latest)),
        "earliest" => Ok(BlockNumberOrTag::Tag(BlockTag::Earliest)),
        "pending" => Ok(BlockNumberOrTag::Tag(BlockTag::Pending)),
        "safe" => Ok(BlockNumberOrTag::Tag(BlockTag::Safe)),
        "finalized" => Ok(BlockNumberOrTag::Tag(BlockTag::Finalized)),
        other => {
            let stripped = other
                .strip_prefix("0x")
                .or_else(|| other.strip_prefix("0X"))
                .unwrap_or(other);
            let n = u64::from_str_radix(stripped, 16)
                .map_err(|_| RpcError::InvalidParams("invalid block number".into()))?;
            Ok(BlockNumberOrTag::Number(n))
        }
    }
}
