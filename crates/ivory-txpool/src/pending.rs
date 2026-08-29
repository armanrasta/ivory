//! Pending transaction entries.

use ivory_core::Transaction;
use ivory_primitives::H256;

/// How a transaction entered the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxOrigin {
    /// Submitted by a local RPC / node operator.
    Local,
    /// Gossiped from a peer.
    Remote,
}

/// Cached pending entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingTx {
    /// Transaction hash (bincode + blake3 placeholder).
    pub hash: H256,
    /// The queued transaction.
    pub tx: Transaction,
    /// Admission origin.
    pub origin: TxOrigin,
    /// Wall-clock milliseconds when added; `0` in unit tests is fine.
    pub added_at_ms: u64,
}
