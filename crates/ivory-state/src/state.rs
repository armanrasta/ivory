//! In-memory state database.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ivory_core::Account;
use ivory_primitives::{Address, Bytes, H256, U256};

use crate::trie::patricia_root;
use parking_lot::RwLock;

/// In-memory account and storage maps.
///
/// [`Clone`] shares the same underlying maps (cheap `Arc` handle) so the
/// executor and RPC can observe one live state. Use [`StateDB::fork`] for an
/// isolated snapshot.
#[derive(Clone)]
pub struct StateDB {
    accounts: Arc<RwLock<HashMap<Address, Account>>>,
    storage: Arc<RwLock<HashMap<(Address, H256), U256>>>,
    code: Arc<RwLock<HashMap<Address, Bytes>>>,
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
            code: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Isolated deep copy (new maps). Mutations do not affect `self`.
    #[must_use]
    pub fn fork(&self) -> Self {
        Self {
            accounts: Arc::new(RwLock::new(self.accounts.read().clone())),
            storage: Arc::new(RwLock::new(self.storage.read().clone())),
            code: Arc::new(RwLock::new(self.code.read().clone())),
        }
    }

    /// Overwrite this handle’s maps with a copy of `snapshot`.
    ///
    /// RPC and the executor that share `self` see the new contents.
    pub fn reset_from(&self, snapshot: &Self) {
        *self.accounts.write() = snapshot.accounts.read().clone();
        *self.storage.write() = snapshot.storage.read().clone();
        *self.code.write() = snapshot.code.read().clone();
    }

    /// Look up an account.
    #[must_use]
    pub fn get_account(&self, addr: &Address) -> Option<Account> {
        self.accounts.read().get(addr).cloned()
    }

    /// Insert or replace an account (storage root is refreshed from slots).
    pub fn set_account(&self, addr: Address, mut account: Account) {
        account.storage_root = self.compute_storage_root(addr);
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

    /// Write a storage slot. Zero deletes the slot and refreshes `storage_root`.
    pub fn set_storage(&self, addr: Address, slot: H256, value: U256) {
        {
            let mut storage = self.storage.write();
            if value.is_zero() {
                storage.remove(&(addr, slot));
            } else {
                storage.insert((addr, slot), value);
            }
        }
        self.refresh_storage_root(addr);
    }

    /// Contract bytecode for `addr` (empty if none).
    #[must_use]
    pub fn get_code(&self, addr: &Address) -> Vec<u8> {
        self.code
            .read()
            .get(addr)
            .map(|b| b.as_slice().to_vec())
            .unwrap_or_default()
    }

    /// Store contract bytecode.
    pub fn set_code(&self, addr: Address, code: Bytes) {
        self.code.write().insert(addr, code);
    }

    /// Account-trie root after storage roots are current.
    ///
    /// Empty state is [`crate::empty_root`], not a hardcoded `0x0` placeholder.
    #[must_use]
    pub fn root_hash(&self) -> H256 {
        self.sync_all_storage_roots();
        let accounts = self.accounts.read();
        let mut pairs = Vec::new();
        for (addr, acc) in accounts.iter() {
            if acc.is_empty() {
                continue;
            }
            let value = bincode::serialize(acc).expect("account bincode");
            pairs.push((addr.as_bytes().to_vec(), value));
        }
        patricia_root(&pairs)
    }
}

impl StateDB {
    fn compute_storage_root(&self, addr: Address) -> H256 {
        let storage = self.storage.read();
        let mut pairs = Vec::new();
        for ((a, slot), val) in storage.iter() {
            if *a == addr && !val.is_zero() {
                pairs.push((slot.to_bytes().to_vec(), val.to_be_bytes().to_vec()));
            }
        }
        if pairs.is_empty() {
            return H256::ZERO;
        }
        patricia_root(&pairs)
    }

    fn refresh_storage_root(&self, addr: Address) {
        let root = self.compute_storage_root(addr);
        let mut accounts = self.accounts.write();
        if let Some(acc) = accounts.get_mut(&addr) {
            acc.storage_root = root;
        } else if root != H256::ZERO {
            let mut acc = Account::new();
            acc.storage_root = root;
            accounts.insert(addr, acc);
        }
    }

    fn sync_all_storage_roots(&self) {
        let mut addrs = HashSet::new();
        addrs.extend(self.accounts.read().keys().copied());
        addrs.extend(self.storage.read().keys().map(|(a, _)| *a));
        for addr in addrs {
            self.refresh_storage_root(addr);
        }
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
    fn empty_root_is_empty_trie_not_placeholder() {
        let db = StateDB::new();
        assert_eq!(db.root_hash(), crate::empty_root());
        assert_ne!(db.root_hash(), H256::ZERO);
    }

    #[test]
    fn root_changes_on_account_and_storage() {
        let db = StateDB::new();
        let empty = db.root_hash();
        let mut acc = Account::new();
        acc.balance = U256::from(1u64);
        db.set_account(addr(1), acc);
        let with_acc = db.root_hash();
        assert_ne!(with_acc, empty);
        db.set_storage(addr(1), slot(1), U256::from(7u64));
        assert_ne!(db.root_hash(), with_acc);
        let acc = db.get_account(&addr(1)).unwrap();
        assert_ne!(acc.storage_root, H256::ZERO);
    }

    #[test]
    fn fork_is_isolated() {
        let db = StateDB::new();
        let mut acc = Account::new();
        acc.nonce = 1;
        db.set_account(addr(1), acc);
        let snap = db.fork();
        let mut acc2 = Account::new();
        acc2.nonce = 9;
        db.set_account(addr(1), acc2);
        assert_eq!(snap.get_account(&addr(1)).unwrap().nonce, 1);
        assert_eq!(db.get_account(&addr(1)).unwrap().nonce, 9);
    }

    #[test]
    fn reset_from_overwrites_live_handle() {
        let live = StateDB::new();
        let snap = StateDB::new();
        let mut acc = Account::new();
        acc.balance = U256::from(5u64);
        snap.set_account(addr(1), acc);
        live.reset_from(&snap);
        assert_eq!(
            live.get_account(&addr(1)).unwrap().balance,
            U256::from(5u64)
        );
        let mut other = Account::new();
        other.balance = U256::from(1u64);
        snap.set_account(addr(1), other);
        assert_eq!(
            live.get_account(&addr(1)).unwrap().balance,
            U256::from(5u64)
        );
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
