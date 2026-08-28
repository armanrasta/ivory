//! In-memory state database.

use std::collections::HashMap;
use std::sync::Arc;

use ivory_core::Account;
use ivory_primitives::{Address, H256, U256};
use parking_lot::RwLock;

/// In-memory account and storage maps.
///
/// Cloning shares the same underlying maps (cheap `Arc` handle).
#[derive(Clone)]
pub struct StateDB {
    accounts: Arc<RwLock<HashMap<Address, Account>>>,
    storage: Arc<RwLock<HashMap<(Address, H256), U256>>>,
}

impl Default for StateDB {
    fn default() -> Self {
        Self::new()
    }
}

impl StateDB {
    /// Create an empty in-memory state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Look up an account.
    #[must_use]
    pub fn get_account(&self, addr: &Address) -> Option<Account> {
        self.accounts.read().get(addr).cloned()
    }

    /// Insert or replace an account.
    pub fn set_account(&self, addr: Address, account: Account) {
        self.accounts.write().insert(addr, account);
    }

    /// Read a storage slot. Missing slots are [`U256::ZERO`].
    #[must_use]
    pub fn get_storage(&self, addr: &Address, slot: &H256) -> U256 {
        self.storage
            .read()
            .get(&(*addr, *slot))
            .copied()
            .unwrap_or(U256::ZERO)
    }

    /// Write a storage slot.
    pub fn set_storage(&self, addr: Address, slot: H256, value: U256) {
        self.storage.write().insert((addr, slot), value);
    }

    /// Placeholder state root until a merkle-patricia trie is implemented (#22).
    #[must_use]
    pub fn root_hash(&self) -> H256 {
        // TODO(#22): compute merkle root of accounts + storage
        H256::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::from_bytes([byte; 20])
    }

    fn slot(byte: u8) -> H256 {
        H256::from_bytes([byte; 32])
    }

    #[test]
    fn missing_account_is_none() {
        let db = StateDB::new();
        assert!(db.get_account(&addr(1)).is_none());
    }

    #[test]
    fn set_and_get_account_roundtrip() {
        let db = StateDB::new();
        let mut account = Account::new();
        account.nonce = 3;
        account.balance = U256::from(42u64);
        db.set_account(addr(1), account.clone());
        assert_eq!(db.get_account(&addr(1)), Some(account));
    }

    #[test]
    fn accounts_are_isolated_by_address() {
        let db = StateDB::new();
        let mut a = Account::new();
        a.nonce = 1;
        let mut b = Account::new();
        b.nonce = 2;
        db.set_account(addr(1), a.clone());
        db.set_account(addr(2), b.clone());
        assert_eq!(db.get_account(&addr(1)), Some(a));
        assert_eq!(db.get_account(&addr(2)), Some(b));
    }

    #[test]
    fn missing_storage_is_zero() {
        let db = StateDB::new();
        assert_eq!(db.get_storage(&addr(1), &slot(1)), U256::ZERO);
    }

    #[test]
    fn set_and_get_storage_roundtrip() {
        let db = StateDB::new();
        db.set_storage(addr(1), slot(1), U256::from(99u64));
        assert_eq!(db.get_storage(&addr(1), &slot(1)), U256::from(99u64));
    }

    #[test]
    fn storage_overwrite() {
        let db = StateDB::new();
        db.set_storage(addr(1), slot(1), U256::from(1u64));
        db.set_storage(addr(1), slot(1), U256::from(2u64));
        assert_eq!(db.get_storage(&addr(1), &slot(1)), U256::from(2u64));
    }

    #[test]
    fn storage_slots_are_isolated() {
        let db = StateDB::new();
        db.set_storage(addr(1), slot(1), U256::from(10u64));
        db.set_storage(addr(1), slot(2), U256::from(20u64));
        db.set_storage(addr(2), slot(1), U256::from(30u64));
        assert_eq!(db.get_storage(&addr(1), &slot(1)), U256::from(10u64));
        assert_eq!(db.get_storage(&addr(1), &slot(2)), U256::from(20u64));
        assert_eq!(db.get_storage(&addr(2), &slot(1)), U256::from(30u64));
    }

    #[test]
    fn root_hash_is_placeholder_zero() {
        let db = StateDB::new();
        db.set_account(addr(1), Account::new());
        assert_eq!(db.root_hash(), H256::ZERO);
    }

    #[test]
    fn clone_shares_underlying_maps() {
        let db = StateDB::new();
        let clone = db.clone();
        let mut account = Account::new();
        account.nonce = 9;
        clone.set_account(addr(1), account.clone());
        assert_eq!(db.get_account(&addr(1)), Some(account));
    }

    #[test]
    fn concurrent_reads_and_writes() {
        let db = StateDB::new();
        let writer = db.clone();
        let handle = std::thread::spawn(move || {
            let mut account = Account::new();
            account.balance = U256::from(5u64);
            writer.set_account(addr(1), account);
            writer.set_storage(addr(1), slot(1), U256::from(7u64));
        });
        db.set_storage(addr(2), slot(2), U256::from(8u64));
        handle.join().unwrap();
        assert_eq!(
            db.get_account(&addr(1)).map(|a| a.balance),
            Some(U256::from(5u64))
        );
        assert_eq!(db.get_storage(&addr(1), &slot(1)), U256::from(7u64));
        assert_eq!(db.get_storage(&addr(2), &slot(2)), U256::from(8u64));
    }
}
