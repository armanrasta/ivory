//! Block, transaction, receipt, and log types.

use ivory_primitives::{Address, Bytes, H256, PublicKey, Signature, U256, keccak256};
use serde::{Deserialize, Serialize};

use crate::error::BlockError;

fn keccak_bincode<T: Serialize>(value: &T) -> H256 {
    let encoded = bincode::serialize(value).expect("serialization is infallible");
    keccak256(&encoded)
}

/// Block header fields committed by consensus.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlockHeader {
    /// Block height.
    pub number: u64,
    /// Hash of the parent block.
    pub parent_hash: H256,
    /// Block timestamp (seconds since Unix epoch).
    pub timestamp: u64,
    /// Miner / validator address.
    pub miner: Address,
    /// Gas limit for this block.
    pub gas_limit: u64,
    /// Total gas used by all transactions.
    pub gas_used: u64,
    /// Merkle root of the state trie.
    pub state_root: H256,
    /// Merkle root of the transaction trie.
    pub transactions_root: H256,
    /// Merkle root of the receipts trie.
    pub receipts_root: H256,
    /// Difficulty (unused for PoA; retained for header compatibility).
    pub difficulty: U256,
    /// Extra data (nonce + seal for PoA).
    pub extra_data: Bytes,
}

impl BlockHeader {
    /// Hash this header: `keccak256(bincode(header))`.
    #[must_use]
    pub fn hash(&self) -> H256 {
        keccak_bincode(self)
    }
}

/// A signed ledger transaction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    /// Sender address (recovered from the signature).
    pub from: Address,
    /// Recipient (`None` for contract creation).
    pub to: Option<Address>,
    /// Value to transfer (wei).
    pub value: U256,
    /// Calldata or init code.
    pub data: Bytes,
    /// Gas price (wei per unit gas).
    pub gas_price: U256,
    /// Gas limit for execution.
    pub gas: u64,
    /// Sender nonce for replay protection.
    pub nonce: u64,
    /// Ed25519 transaction signature.
    pub signature: Signature,
    /// Ed25519 public key used to verify [`Self::signature`].
    ///
    /// Not part of [`Self::signing_hash`]; admission still checks that
    /// `address_from_public_key(public_key) == from`.
    pub public_key: PublicKey,
}

/// Unsigned fields hashed for Ed25519 (excludes `signature` and `public_key`).
#[derive(Serialize)]
struct UnsignedTransaction<'a> {
    from: Address,
    to: Option<Address>,
    value: U256,
    data: &'a Bytes,
    gas_price: U256,
    gas: u64,
    nonce: u64,
}

impl Transaction {
    /// Hash this transaction (includes signature and public key): `keccak256(bincode(tx))`.
    #[must_use]
    pub fn hash(&self) -> H256 {
        keccak_bincode(self)
    }

    /// Domain-separated signing payload for Ed25519: `keccak256(bincode(unsigned fields))`.
    ///
    /// Independent of `signature` / `public_key`.
    #[must_use]
    pub fn signing_hash(&self) -> H256 {
        let unsigned = UnsignedTransaction {
            from: self.from,
            to: self.to,
            value: self.value,
            data: &self.data,
            gas_price: self.gas_price,
            gas: self.gas,
            nonce: self.nonce,
        };
        keccak_bincode(&unsigned)
    }

    /// `true` when this transaction creates a contract (`to` is `None`).
    #[must_use]
    pub fn is_create(&self) -> bool {
        self.to.is_none()
    }

    /// Length of calldata / init code in bytes.
    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.as_slice().len()
    }
}

/// Execution receipt for a transaction included in a block.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Receipt {
    /// Hash of the included transaction.
    pub tx_hash: H256,
    /// Block number that included the transaction.
    pub block_number: u64,
    /// Gas consumed by execution.
    pub gas_used: u64,
    /// `true` if the transaction succeeded.
    pub status: bool,
    /// Logs emitted during execution.
    pub logs: Vec<Log>,
}

/// A contract log / event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Log {
    /// Contract that emitted the log.
    pub address: Address,
    /// Indexed topics.
    pub topics: Vec<H256>,
    /// Unindexed log data.
    pub data: Bytes,
}

/// A block: header plus transactions and receipts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    /// Block header.
    pub header: BlockHeader,
    /// Transactions included in this block.
    pub transactions: Vec<Transaction>,
    /// Receipts corresponding to `transactions`.
    pub receipts: Vec<Receipt>,
}

impl Block {
    /// Hash of this block (currently the header hash).
    #[must_use]
    pub fn hash(&self) -> H256 {
        self.header.hash()
    }

    /// Validate header fields that do not require chain context.
    ///
    /// # Errors
    ///
    /// Returns [`BlockError::GasExceeded`] when `gas_used` is greater than `gas_limit`.
    pub fn validate(&self) -> Result<(), BlockError> {
        if self.header.gas_used > self.header.gas_limit {
            return Err(BlockError::GasExceeded);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_header(gas_limit: u64, gas_used: u64) -> BlockHeader {
        BlockHeader {
            number: 1,
            parent_hash: H256::ZERO,
            timestamp: 1_700_000_000,
            miner: Address::zero(),
            gas_limit,
            gas_used,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        }
    }

    fn test_tx(to: Option<Address>) -> Transaction {
        Transaction {
            from: Address::from_bytes([1u8; 20]),
            to,
            value: U256::from(10u64),
            data: Bytes::from_slice(&[0xaa, 0xbb]),
            gas_price: U256::from(1u64),
            gas: 21_000,
            nonce: 0,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        }
    }

    fn empty_block() -> Block {
        Block {
            header: test_header(30_000_000, 0),
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    #[test]
    fn empty_block_validates() {
        assert!(empty_block().validate().is_ok());
    }

    #[test]
    fn validate_passes_when_gas_used_equals_limit() {
        let block = Block {
            header: test_header(21_000, 21_000),
            transactions: Vec::new(),
            receipts: Vec::new(),
        };
        assert!(block.validate().is_ok());
    }

    #[test]
    fn validate_fails_when_gas_exceeds_limit() {
        let block = Block {
            header: test_header(21_000, 21_001),
            transactions: Vec::new(),
            receipts: Vec::new(),
        };
        assert_eq!(block.validate(), Err(BlockError::GasExceeded));
    }

    #[test]
    fn header_hash_is_deterministic() {
        let header = test_header(30_000_000, 0);
        assert_eq!(header.hash(), header.hash());
        assert_ne!(header.hash(), H256::ZERO);
    }

    #[test]
    fn block_hash_matches_header_hash() {
        let block = empty_block();
        assert_eq!(block.hash(), block.header.hash());
    }

    #[test]
    fn different_headers_have_different_hashes() {
        let a = test_header(30_000_000, 0);
        let mut b = a.clone();
        b.number = 2;
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn header_serde_json_roundtrip() {
        let original = test_header(8_000_000, 21_000);
        let json = serde_json::to_string(&original).unwrap();
        let decoded: BlockHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn transaction_serde_json_roundtrip() {
        let original = test_tx(Some(Address::from_bytes([9u8; 20])));
        let json = serde_json::to_string(&original).unwrap();
        let decoded: Transaction = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn contract_creation_tx_has_no_recipient() {
        let tx = test_tx(None);
        assert!(tx.to.is_none());
        assert!(tx.is_create());
    }

    #[test]
    fn call_tx_is_not_create() {
        let tx = test_tx(Some(Address::from_bytes([9u8; 20])));
        assert!(!tx.is_create());
    }

    #[test]
    fn data_len_matches_payload() {
        let tx = test_tx(None);
        assert_eq!(tx.data_len(), 2);
    }

    #[test]
    fn tx_hash_is_deterministic() {
        let tx = test_tx(Some(Address::zero()));
        assert_eq!(tx.hash(), tx.hash());
        assert_ne!(tx.hash(), H256::ZERO);
    }

    #[test]
    fn tx_hash_changes_with_nonce() {
        let a = test_tx(Some(Address::zero()));
        let mut b = a.clone();
        b.nonce = 1;
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn signing_hash_ignores_signature() {
        let a = test_tx(Some(Address::zero()));
        let mut b = a.clone();
        b.signature = Signature::from_bytes([1u8; 64]);
        assert_eq!(a.signing_hash(), b.signing_hash());
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn block_serde_json_roundtrip() {
        let original = Block {
            header: test_header(30_000_000, 21_000),
            transactions: vec![test_tx(None)],
            receipts: vec![Receipt {
                tx_hash: H256::from_bytes([5u8; 32]),
                block_number: 1,
                gas_used: 21_000,
                status: true,
                logs: vec![Log {
                    address: Address::from_bytes([2u8; 20]),
                    topics: vec![H256::from_bytes([7u8; 32])],
                    data: Bytes::from_slice(&[1, 2, 3]),
                }],
            }],
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn log_equality() {
        let log = Log {
            address: Address::zero(),
            topics: vec![H256::ZERO],
            data: Bytes::new(),
        };
        assert_eq!(log, log.clone());
    }

    #[test]
    fn receipt_equality() {
        let receipt = Receipt {
            tx_hash: H256::ZERO,
            block_number: 0,
            gas_used: 0,
            status: false,
            logs: Vec::new(),
        };
        assert_eq!(receipt, receipt.clone());
    }

    #[test]
    fn bincode_header_roundtrip() {
        let original = test_header(1_000, 500);
        let bytes = bincode::serialize(&original).unwrap();
        let decoded: BlockHeader = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn bincode_transaction_roundtrip() {
        let original = test_tx(Some(Address::zero()));
        let bytes = bincode::serialize(&original).unwrap();
        let decoded: Transaction = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn gas_exceeded_error_display() {
        assert_eq!(
            BlockError::GasExceeded.to_string(),
            "gas used exceeds limit"
        );
    }
}
