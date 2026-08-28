//! Ledger account state.

use ivory_primitives::{H256, U256};
use serde::{Deserialize, Serialize};

/// Account state stored in the ledger.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    /// Account nonce (for replay protection).
    pub nonce: u64,
    /// Account balance (wei).
    pub balance: U256,
    /// Code hash (empty for EOAs, contract hash for contracts).
    pub code_hash: H256,
    /// Storage root for contract state.
    pub storage_root: H256,
}

impl Account {
    /// Create an empty account.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the account has no nonce, balance, code, or storage.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nonce == 0
            && self.balance.is_zero()
            && self.code_hash == H256::ZERO
            && self.storage_root == H256::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_account_is_empty() {
        let account = Account::new();
        assert_eq!(account.nonce, 0);
        assert!(account.balance.is_zero());
        assert_eq!(account.code_hash, H256::ZERO);
        assert_eq!(account.storage_root, H256::ZERO);
        assert!(account.is_empty());
    }

    #[test]
    fn default_matches_new() {
        assert_eq!(Account::new(), Account::default());
    }

    #[test]
    fn nonce_makes_account_non_empty() {
        let mut account = Account::new();
        account.nonce = 1;
        assert!(!account.is_empty());
    }

    #[test]
    fn balance_makes_account_non_empty() {
        let mut account = Account::new();
        account.balance = U256::from(1u64);
        assert!(!account.is_empty());
    }

    #[test]
    fn code_hash_makes_account_non_empty() {
        let mut account = Account::new();
        account.code_hash = H256::from_bytes([1u8; 32]);
        assert!(!account.is_empty());
    }

    #[test]
    fn storage_root_makes_account_non_empty() {
        let mut account = Account::new();
        account.storage_root = H256::from_bytes([2u8; 32]);
        assert!(!account.is_empty());
    }

    #[test]
    fn serde_json_roundtrip() {
        let original = Account {
            nonce: 7,
            balance: U256::from(1_000u64),
            code_hash: H256::from_bytes([3u8; 32]),
            storage_root: H256::from_bytes([4u8; 32]),
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }
}
