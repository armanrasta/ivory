//! Chain store and block production errors.

use ivory_consensus::ConsensusError;
use ivory_core::BlockError;
use ivory_executor::ExecutionError;
use thiserror::Error;

/// Errors from the in-memory chain index and block production.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// Core header validation failed (gas, parent, timestamp).
    #[error(transparent)]
    Block(#[from] BlockError),
    /// Consensus rejected the header.
    #[error(transparent)]
    Consensus(#[from] ConsensusError),
    /// Parent hash is not in the store.
    #[error("unknown parent")]
    UnknownParent,
    /// Block hash is already stored.
    #[error("duplicate block")]
    DuplicateBlock,
    /// `number` is not `parent.number + 1` (or genesis is not height 0).
    #[error("invalid block number: expected {expected}, got {got}")]
    InvalidBlockNumber {
        /// Expected height.
        expected: u64,
        /// Height on the submitted block.
        got: u64,
    },
    /// Genesis must have `parent_hash = ZERO` and `number = 0`.
    #[error("invalid genesis")]
    InvalidGenesis,
    /// Transaction execution failed while producing a block.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// Header `state_root` does not match execution on the parent snapshot.
    #[error("invalid state root")]
    InvalidStateRoot,
    /// Parent block is indexed but has no recorded post-state snapshot.
    #[error("unknown parent state")]
    UnknownParentState,
}
