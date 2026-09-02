//! Transaction pool (mempool).

use dashmap::DashMap;
use ivory_core::Transaction;
use ivory_primitives::{Address, H256};

use crate::config::PoolConfig;
use crate::error::TxPoolError;
use crate::pending::{PendingTx, TxOrigin};

/// Snapshot of pool occupancy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// Number of pending transactions.
    pub pending: usize,
    /// Number of distinct senders with a pending nonce tracker.
    pub senders: usize,
}

/// In-memory mempool with strict contiguous nonces per sender.
pub struct TransactionPool {
    config: PoolConfig,
    pending: DashMap<H256, PendingTx>,
    /// Next accepted nonce per sender.
    next_nonce: DashMap<Address, u64>,
    /// Count of pending txs per sender.
    per_sender: DashMap<Address, usize>,
}

impl Default for TransactionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionPool {
    /// Create a pool with [`PoolConfig::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(PoolConfig::default())
    }

    /// Create a pool with custom admission limits.
    #[must_use]
    pub fn with_config(config: PoolConfig) -> Self {
        Self {
            config,
            pending: DashMap::new(),
            next_nonce: DashMap::new(),
            per_sender: DashMap::new(),
        }
    }

    /// Next nonce this pool will accept for `addr` (defaults to `0`).
    #[must_use]
    pub fn expected_nonce(&self, addr: &Address) -> u64 {
        self.next_nonce.get(addr).map(|v| *v).unwrap_or(0)
    }

    /// Number of pending transactions.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Occupancy snapshot.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pending: self.pending.len(),
            senders: self.next_nonce.len(),
        }
    }

    /// Look up a pending transaction by hash.
    #[must_use]
    pub fn get(&self, hash: &H256) -> Option<Transaction> {
        self.pending.get(hash).map(|e| e.tx.clone())
    }

    /// Whether `hash` is currently pending.
    #[must_use]
    pub fn contains(&self, hash: &H256) -> bool {
        self.pending.contains_key(hash)
    }

    /// Admit a transaction after Ed25519 verification.
    ///
    /// # Errors
    ///
    /// Returns [`TxPoolError`] when signature, nonce, capacity, or gas limits fail.
    pub fn add_transaction(&self, tx: Transaction, origin: TxOrigin) -> Result<H256, TxPoolError> {
        self.add_transaction_at(tx, origin, 0)
    }

    /// Admit a transaction with an explicit timestamp (tests / RPC).
    ///
    /// # Errors
    ///
    /// Same as [`Self::add_transaction`].
    pub fn add_transaction_at(
        &self,
        tx: Transaction,
        origin: TxOrigin,
        added_at_ms: u64,
    ) -> Result<H256, TxPoolError> {
        if tx.gas < self.config.min_gas {
            return Err(TxPoolError::GasLimitTooLow {
                min: self.config.min_gas,
                got: tx.gas,
            });
        }

        ivory_crypto::recover_sender(&tx).map_err(|_| TxPoolError::InvalidSignature)?;

        let hash = tx.hash();
        if self.pending.contains_key(&hash) {
            return Err(TxPoolError::AlreadyKnown);
        }

        if self.pending.len() >= self.config.max_pending {
            return Err(TxPoolError::PoolFull);
        }

        let sender = tx.from;
        let sender_count = self.per_sender.get(&sender).map(|c| *c).unwrap_or(0);
        if sender_count >= self.config.max_per_sender {
            return Err(TxPoolError::SenderLimitReached);
        }

        let expected = self.expected_nonce(&sender);
        if tx.nonce < expected {
            return Err(TxPoolError::NonceTooLow {
                expected,
                got: tx.nonce,
            });
        }
        if tx.nonce > expected {
            return Err(TxPoolError::NonceGap {
                expected,
                got: tx.nonce,
            });
        }

        self.pending.insert(
            hash,
            PendingTx {
                hash,
                tx,
                origin,
                added_at_ms,
            },
        );
        self.next_nonce.insert(sender, expected.saturating_add(1));
        self.per_sender
            .insert(sender, sender_count.saturating_add(1));
        Ok(hash)
    }

    /// Up to `max` pending transactions. Order is unspecified.
    #[must_use]
    pub fn get_pending(&self, max: usize) -> Vec<Transaction> {
        self.get_pending_entries(max)
            .into_iter()
            .map(|e| e.tx)
            .collect()
    }

    /// Up to `max` pending entries. Order is unspecified.
    #[must_use]
    pub fn get_pending_entries(&self, max: usize) -> Vec<PendingTx> {
        self.pending
            .iter()
            .take(max)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Remove a pending transaction after inclusion. Does not rewind `next_nonce`.
    pub fn remove(&self, hash: &H256) -> Option<PendingTx> {
        let (_, entry) = self.pending.remove(hash)?;
        let sender = entry.tx.from;
        if let Some(mut count) = self.per_sender.get_mut(&sender) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                drop(count);
                self.per_sender.remove(&sender);
            }
        }
        Some(entry)
    }

    /// Drop all pending txs. Does not reset nonce trackers.
    pub fn clear_pending(&self) {
        self.pending.clear();
        self.per_sender.clear();
    }

    /// Reset pending txs and nonce trackers (tests).
    pub fn clear(&self) {
        self.pending.clear();
        self.next_nonce.clear();
        self.per_sender.clear();
    }
}

#[cfg(test)]
mod tests {
    use ivory_crypto::{
        CryptoError, keypair_from_byte, recover_sender, sign_transaction, signed_tx,
    };
    use ivory_primitives::{Address, Bytes, SecretKey, U256};

    use super::*;

    fn sk(byte: u8) -> SecretKey {
        keypair_from_byte(byte).0
    }

    fn addr_of(byte: u8) -> Address {
        keypair_from_byte(byte).2
    }

    fn tx_with_value(from_seed: u8, nonce: u64, gas: u64, value: u64) -> Transaction {
        signed_tx(
            &sk(from_seed),
            Some(addr_of(9)),
            nonce,
            U256::from(value),
            gas,
            U256::from(1u64),
            Bytes::new(),
        )
    }

    fn tx(from_seed: u8, nonce: u64, gas: u64) -> Transaction {
        tx_with_value(from_seed, nonce, gas, 1)
    }

    fn default_tx(from_seed: u8, nonce: u64) -> Transaction {
        tx(from_seed, nonce, 21_000)
    }

    #[test]
    fn accepts_nonce_zero_then_one() {
        let pool = TransactionPool::new();
        let a = addr_of(1);
        assert_eq!(
            pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
                .unwrap(),
            default_tx(1, 0).hash()
        );
        assert_eq!(pool.expected_nonce(&a), 1);
        pool.add_transaction(default_tx(1, 1), TxOrigin::Local)
            .unwrap();
        assert_eq!(pool.expected_nonce(&a), 2);
        assert_eq!(pool.pending_count(), 2);
    }

    #[test]
    fn rejects_duplicate_hash() {
        let pool = TransactionPool::new();
        let t = default_tx(1, 0);
        pool.add_transaction(t.clone(), TxOrigin::Local).unwrap();
        assert_eq!(
            pool.add_transaction(t, TxOrigin::Remote),
            Err(TxPoolError::AlreadyKnown)
        );
    }

    #[test]
    fn rejects_nonce_too_low() {
        let pool = TransactionPool::new();
        let a = addr_of(1);
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        assert_eq!(
            pool.add_transaction(tx_with_value(1, 0, 21_000, 2), TxOrigin::Local),
            Err(TxPoolError::NonceTooLow {
                expected: 1,
                got: 0
            })
        );
        assert_eq!(pool.expected_nonce(&a), 1);
    }

    #[test]
    fn rejects_nonce_gap() {
        let pool = TransactionPool::new();
        assert_eq!(
            pool.add_transaction(default_tx(1, 2), TxOrigin::Local),
            Err(TxPoolError::NonceGap {
                expected: 0,
                got: 2
            })
        );
    }

    #[test]
    fn senders_are_independent() {
        let pool = TransactionPool::new();
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(default_tx(2, 0), TxOrigin::Local)
            .unwrap();
        assert_eq!(pool.pending_count(), 2);
        assert_eq!(pool.expected_nonce(&addr_of(1)), 1);
        assert_eq!(pool.expected_nonce(&addr_of(2)), 1);
    }

    #[test]
    fn get_pending_respects_max() {
        let pool = TransactionPool::new();
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(default_tx(1, 1), TxOrigin::Local)
            .unwrap();
        assert_eq!(pool.get_pending(1).len(), 1);
        assert_eq!(pool.get_pending(10).len(), 2);
        assert!(pool.get_pending(0).is_empty());
    }

    #[test]
    fn get_pending_entries_include_origin() {
        let pool = TransactionPool::new();
        pool.add_transaction_at(default_tx(1, 0), TxOrigin::Remote, 42)
            .unwrap();
        let entries = pool.get_pending_entries(1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].origin, TxOrigin::Remote);
        assert_eq!(entries[0].added_at_ms, 42);
    }

    #[test]
    fn remove_drops_pending_not_nonce() {
        let pool = TransactionPool::new();
        let a = addr_of(1);
        let t = default_tx(1, 0);
        let hash = pool.add_transaction(t, TxOrigin::Local).unwrap();
        let removed = pool.remove(&hash).unwrap();
        assert_eq!(removed.hash, hash);
        assert_eq!(pool.pending_count(), 0);
        assert_eq!(pool.expected_nonce(&a), 1);
        assert!(!pool.contains(&hash));
        assert_eq!(
            pool.add_transaction(default_tx(1, 0), TxOrigin::Local),
            Err(TxPoolError::NonceTooLow {
                expected: 1,
                got: 0
            })
        );
    }

    #[test]
    fn remove_missing_is_none() {
        let pool = TransactionPool::new();
        assert!(pool.remove(&H256::ZERO).is_none());
    }

    #[test]
    fn contains_after_add() {
        let pool = TransactionPool::new();
        let t = default_tx(1, 0);
        let hash = t.hash();
        assert!(!pool.contains(&hash));
        pool.add_transaction(t, TxOrigin::Local).unwrap();
        assert!(pool.contains(&hash));
        assert!(pool.get(&hash).is_some());
    }

    #[test]
    fn rejects_gas_below_minimum() {
        let pool = TransactionPool::new();
        assert_eq!(
            pool.add_transaction(tx(1, 0, 20_999), TxOrigin::Local),
            Err(TxPoolError::GasLimitTooLow {
                min: 21_000,
                got: 20_999
            })
        );
    }

    #[test]
    fn rejects_when_pool_full() {
        let pool = TransactionPool::with_config(PoolConfig::tiny());
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(default_tx(2, 0), TxOrigin::Local)
            .unwrap();
        assert_eq!(
            pool.add_transaction(default_tx(3, 0), TxOrigin::Local),
            Err(TxPoolError::PoolFull)
        );
    }

    #[test]
    fn rejects_when_sender_limit_reached() {
        let pool = TransactionPool::with_config(PoolConfig::tiny());
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        assert_eq!(
            pool.add_transaction(default_tx(1, 1), TxOrigin::Local),
            Err(TxPoolError::SenderLimitReached)
        );
    }

    #[test]
    fn stats_track_pending_and_senders() {
        let pool = TransactionPool::new();
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(default_tx(2, 0), TxOrigin::Local)
            .unwrap();
        let stats = pool.stats();
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.senders, 2);
    }

    #[test]
    fn clear_resets_nonces() {
        let pool = TransactionPool::new();
        let a = addr_of(1);
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.clear();
        assert_eq!(pool.pending_count(), 0);
        assert_eq!(pool.expected_nonce(&a), 0);
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
    }

    #[test]
    fn clear_pending_keeps_nonce_tracker() {
        let pool = TransactionPool::new();
        let a = addr_of(1);
        pool.add_transaction(default_tx(1, 0), TxOrigin::Local)
            .unwrap();
        pool.clear_pending();
        assert_eq!(pool.pending_count(), 0);
        assert_eq!(pool.expected_nonce(&a), 1);
    }

    #[test]
    fn default_pool_matches_new() {
        let pool = TransactionPool::default();
        assert_eq!(pool.pending_count(), 0);
    }

    #[test]
    fn rejects_invalid_signature() {
        let pool = TransactionPool::new();
        let mut t = default_tx(1, 0);
        let mut bytes = t.signature.to_bytes();
        bytes[0] ^= 0xff;
        t.signature = ivory_primitives::Signature::from_bytes(bytes);
        assert_eq!(recover_sender(&t), Err(CryptoError::InvalidSignature));
        assert_eq!(
            pool.add_transaction(t, TxOrigin::Local),
            Err(TxPoolError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_key_mismatch() {
        let pool = TransactionPool::new();
        let mut t = default_tx(1, 0);
        t.from = addr_of(2);
        sign_transaction(&mut t, &sk(1));
        t.from = addr_of(2);
        assert_eq!(
            pool.add_transaction(t, TxOrigin::Local),
            Err(TxPoolError::InvalidSignature)
        );
    }
}
