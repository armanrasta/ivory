//! Mempool admission limits.

/// Tunables for transaction-pool admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolConfig {
    /// Maximum pending transactions in the pool.
    pub max_pending: usize,
    /// Maximum pending transactions from a single sender.
    pub max_per_sender: usize,
    /// Minimum `tx.gas` accepted (typically the intrinsic base cost).
    pub min_gas: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_pending: 4096,
            max_per_sender: 64,
            min_gas: 21_000,
        }
    }
}

impl PoolConfig {
    /// Restrictive config for unit tests that hit capacity limits.
    #[must_use]
    pub fn tiny() -> Self {
        Self {
            max_pending: 2,
            max_per_sender: 1,
            min_gas: 21_000,
        }
    }
}
