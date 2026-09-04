//! Bincode wire format for gossip payloads.

use ivory_core::{Block, Transaction};
use ivory_primitives::H256;
use serde::{Deserialize, Serialize};

use crate::error::NetworkError;

/// Gossip / sync payload.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkMessage {
    /// Full block announcement.
    Block(Block),
    /// Transaction gossip.
    Transaction(Transaction),
    /// Request a missing block by hash (parent walk).
    GetBlock(H256),
}

impl NetworkMessage {
    /// Encode with bincode.
    ///
    /// # Errors
    ///
    /// [`NetworkError::InvalidMessage`] if serialization fails (should not for these types).
    pub fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        bincode::serialize(self).map_err(|_| NetworkError::InvalidMessage)
    }

    /// Decode a gossip payload.
    ///
    /// # Errors
    ///
    /// [`NetworkError::InvalidMessage`] on malformed bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        bincode::deserialize(bytes).map_err(|_| NetworkError::InvalidMessage)
    }
}

#[cfg(test)]
mod tests {
    use ivory_core::{Block, BlockHeader};
    use ivory_crypto::{keypair_from_byte, signed_transfer};
    use ivory_primitives::{Address, Bytes, H256, U256};

    use super::*;

    fn empty_block() -> Block {
        Block {
            header: BlockHeader {
                number: 0,
                parent_hash: H256::ZERO,
                timestamp: 1,
                miner: Address::zero(),
                gas_limit: 1,
                gas_used: 0,
                state_root: H256::ZERO,
                transactions_root: H256::ZERO,
                receipts_root: H256::ZERO,
                difficulty: U256::ZERO,
                extra_data: Bytes::new(),
            },
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    #[test]
    fn roundtrip_block() {
        let msg = NetworkMessage::Block(empty_block());
        let bytes = msg.encode().unwrap();
        assert_eq!(NetworkMessage::decode(&bytes).unwrap(), msg);
    }

    #[test]
    fn roundtrip_transaction() {
        let (sk, _, _) = keypair_from_byte(1);
        let to = keypair_from_byte(2).2;
        let tx = signed_transfer(&sk, to, 0, U256::from(1u64), 21_000);
        let msg = NetworkMessage::Transaction(tx);
        let bytes = msg.encode().unwrap();
        assert_eq!(NetworkMessage::decode(&bytes).unwrap(), msg);
    }

    #[test]
    fn roundtrip_get_block() {
        let msg = NetworkMessage::GetBlock(H256::from_bytes([7u8; 32]));
        let bytes = msg.encode().unwrap();
        assert_eq!(NetworkMessage::decode(&bytes).unwrap(), msg);
    }

    #[test]
    fn roundtrip_get_block_and_block_with_txs() {
        let get = NetworkMessage::GetBlock(H256::from_bytes([9u8; 32]));
        assert_eq!(NetworkMessage::decode(&get.encode().unwrap()).unwrap(), get);
        let (sk, _, _) = keypair_from_byte(1);
        let tx = signed_transfer(&sk, keypair_from_byte(2).2, 0, U256::from(1u64), 21_000);
        let mut block = empty_block();
        block.transactions.push(tx);
        let msg = NetworkMessage::Block(block);
        assert_eq!(NetworkMessage::decode(&msg.encode().unwrap()).unwrap(), msg);
    }

    #[test]
    fn decode_garbage_is_invalid() {
        assert!(matches!(
            NetworkMessage::decode(&[0xff, 0x00, 0x01]),
            Err(NetworkError::InvalidMessage)
        ));
    }

    #[test]
    fn decode_empty_is_invalid() {
        assert!(matches!(
            NetworkMessage::decode(&[]),
            Err(NetworkError::InvalidMessage)
        ));
    }

    #[test]
    fn roundtrip_get_block_and_block_with_tx() {
        let get = NetworkMessage::GetBlock(H256::from_bytes([9u8; 32]));
        let bytes = get.encode().unwrap();
        assert_eq!(NetworkMessage::decode(&bytes).unwrap(), get);

        let (sk, _, _) = keypair_from_byte(1);
        let tx = signed_transfer(&sk, keypair_from_byte(2).2, 0, U256::from(1u64), 21_000);
        let receipt = ivory_core::Receipt {
            tx_hash: tx.hash(),
            block_number: 1,
            gas_used: 21_000,
            status: true,
            logs: Vec::new(),
        };
        let block = Block {
            header: BlockHeader {
                number: 1,
                parent_hash: H256::ZERO,
                timestamp: 2,
                miner: Address::zero(),
                gas_limit: 30_000_000,
                gas_used: 21_000,
                state_root: H256::ZERO,
                transactions_root: ivory_core::list_root(std::slice::from_ref(&tx)),
                receipts_root: ivory_core::list_root(std::slice::from_ref(&receipt)),
                difficulty: U256::ZERO,
                extra_data: Bytes::new(),
            },
            transactions: vec![tx],
            receipts: vec![receipt],
        };
        let msg = NetworkMessage::Block(block);
        let encoded = msg.encode().unwrap();
        assert_eq!(NetworkMessage::decode(&encoded).unwrap(), msg);
    }
}
