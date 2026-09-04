//! Header-chain checks for a light Ivory client.

use ivory_core::BlockHeader;
use ivory_primitives::{Address, Bytes, H256, U256};
use serde_json::Value;
use thiserror::Error;

/// Header walk failed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LightError {
    /// `parent_hash` does not equal the previous header hash.
    #[error("header parent mismatch")]
    ParentMismatch,
    /// Height is not previous + 1.
    #[error("header number is not parent + 1")]
    NumberMismatch,
    /// PoA seal (`extra_data`) is missing.
    #[error("header extra_data seal is empty")]
    MissingSeal,
    /// RPC JSON is missing a field or has a bad hex value.
    #[error("invalid header rpc object: {0}")]
    InvalidRpc(&'static str),
    /// Recomputed header hash does not match the RPC `hash` field.
    #[error("rpc header hash mismatch")]
    HashMismatch,
}

/// `next` must sit directly on `prev` (hash link, height, non-empty seal).
///
/// # Errors
///
/// [`LightError`] when the link, height, or seal check fails.
pub fn verify_header_link(prev: &BlockHeader, next: &BlockHeader) -> Result<(), LightError> {
    if next.parent_hash != prev.hash() {
        return Err(LightError::ParentMismatch);
    }
    if next.number != prev.number.saturating_add(1) {
        return Err(LightError::NumberMismatch);
    }
    if next.extra_data.is_empty() {
        return Err(LightError::MissingSeal);
    }
    Ok(())
}

/// Decode `ivory_getHeaderByNumber` JSON into a header.
///
/// # Errors
///
/// [`LightError::InvalidRpc`] on missing or malformed fields.
pub fn header_from_rpc(v: &Value) -> Result<BlockHeader, LightError> {
    Ok(BlockHeader {
        number: parse_qty(v.get("number").ok_or(LightError::InvalidRpc("number"))?)?,
        parent_hash: parse_h256(
            v.get("parentHash")
                .ok_or(LightError::InvalidRpc("parentHash"))?,
        )?,
        timestamp: parse_qty(
            v.get("timestamp")
                .ok_or(LightError::InvalidRpc("timestamp"))?,
        )?,
        miner: parse_address(v.get("miner").ok_or(LightError::InvalidRpc("miner"))?)?,
        gas_limit: parse_qty(
            v.get("gasLimit")
                .ok_or(LightError::InvalidRpc("gasLimit"))?,
        )?,
        gas_used: parse_qty(v.get("gasUsed").ok_or(LightError::InvalidRpc("gasUsed"))?)?,
        state_root: parse_h256(
            v.get("stateRoot")
                .ok_or(LightError::InvalidRpc("stateRoot"))?,
        )?,
        transactions_root: parse_h256(
            v.get("transactionsRoot")
                .ok_or(LightError::InvalidRpc("transactionsRoot"))?,
        )?,
        receipts_root: parse_h256(
            v.get("receiptsRoot")
                .ok_or(LightError::InvalidRpc("receiptsRoot"))?,
        )?,
        difficulty: match v.get("difficulty") {
            Some(d) => parse_u256(d)?,
            None => U256::ZERO,
        },
        extra_data: parse_bytes(
            v.get("extraData")
                .ok_or(LightError::InvalidRpc("extraData"))?,
        )?,
    })
}

/// Check that `header.hash()` matches the RPC `hash` field.
///
/// # Errors
///
/// [`LightError::HashMismatch`] or [`LightError::InvalidRpc`].
pub fn verify_rpc_hash(header: &BlockHeader, v: &Value) -> Result<(), LightError> {
    let reported = parse_h256(v.get("hash").ok_or(LightError::InvalidRpc("hash"))?)?;
    if reported != header.hash() {
        return Err(LightError::HashMismatch);
    }
    Ok(())
}

fn parse_qty(v: &Value) -> Result<u64, LightError> {
    let s = v.as_str().ok_or(LightError::InvalidRpc("qty"))?;
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(stripped, 16).map_err(|_| LightError::InvalidRpc("qty"))
}

fn parse_h256(v: &Value) -> Result<H256, LightError> {
    let s = v.as_str().ok_or(LightError::InvalidRpc("hash"))?;
    H256::from_hex(s).map_err(|_| LightError::InvalidRpc("hash"))
}

fn parse_address(v: &Value) -> Result<Address, LightError> {
    let s = v.as_str().ok_or(LightError::InvalidRpc("address"))?;
    Address::from_hex(s).map_err(|_| LightError::InvalidRpc("address"))
}

fn parse_u256(v: &Value) -> Result<U256, LightError> {
    let s = v.as_str().ok_or(LightError::InvalidRpc("u256"))?;
    U256::from_hex(s).map_err(|_| LightError::InvalidRpc("u256"))
}

fn parse_bytes(v: &Value) -> Result<Bytes, LightError> {
    let s = v.as_str().ok_or(LightError::InvalidRpc("bytes"))?;
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let raw = hex::decode(s).map_err(|_| LightError::InvalidRpc("bytes"))?;
    Ok(Bytes::from_vec(raw))
}

#[cfg(test)]
mod tests {
    use ivory_core::empty_list_roots;
    use ivory_primitives::Address;
    use serde_json::json;

    use super::*;

    fn header(number: u64, parent: H256, extra: &[u8]) -> BlockHeader {
        let (tx_root, rx_root) = empty_list_roots();
        BlockHeader {
            number,
            parent_hash: parent,
            timestamp: number + 1,
            miner: Address::zero(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: tx_root,
            receipts_root: rx_root,
            difficulty: U256::ZERO,
            extra_data: Bytes::from_slice(extra),
        }
    }

    #[test]
    fn verify_header_link_accepts_child() {
        let parent = header(0, H256::ZERO, &[1]);
        let child = header(1, parent.hash(), &[2]);
        verify_header_link(&parent, &child).unwrap();
    }

    #[test]
    fn verify_header_link_rejects_bad_parent_number_and_seal() {
        let parent = header(0, H256::ZERO, &[1]);
        let mut bad = header(1, H256::from_bytes([9u8; 32]), &[2]);
        assert_eq!(
            verify_header_link(&parent, &bad),
            Err(LightError::ParentMismatch)
        );
        bad = header(3, parent.hash(), &[2]);
        assert_eq!(
            verify_header_link(&parent, &bad),
            Err(LightError::NumberMismatch)
        );
        bad = header(1, parent.hash(), &[]);
        assert_eq!(
            verify_header_link(&parent, &bad),
            Err(LightError::MissingSeal)
        );
    }

    #[test]
    fn header_from_rpc_roundtrip() {
        let (tx_root, rx_root) = empty_list_roots();
        let header = header(2, H256::from_bytes([3u8; 32]), &[0xaa, 0xbb]);
        let v = json!({
            "number": "0x2",
            "hash": header.hash().to_hex(),
            "parentHash": header.parent_hash.to_hex(),
            "miner": header.miner.to_hex(),
            "timestamp": "0x3",
            "gasLimit": format!("0x{:x}", header.gas_limit),
            "gasUsed": "0x0",
            "stateRoot": header.state_root.to_hex(),
            "transactionsRoot": tx_root.to_hex(),
            "receiptsRoot": rx_root.to_hex(),
            "difficulty": U256::ZERO.to_hex(),
            "extraData": "0xaabb",
        });
        let decoded = header_from_rpc(&v).unwrap();
        assert_eq!(decoded, header);
        verify_rpc_hash(&decoded, &v).unwrap();
    }
}
