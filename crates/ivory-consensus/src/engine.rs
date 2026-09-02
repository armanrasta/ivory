//! Consensus engine trait.

use ivory_core::BlockHeader;
use ivory_primitives::{Address, SecretKey};

use crate::error::ConsensusError;

/// Header sealing and validation for a consensus algorithm.
pub trait ConsensusEngine {
    /// Whether `addr` may propose / seal blocks.
    fn is_validator(&self, addr: &Address) -> bool;

    /// Validate `header` against an optional parent (genesis has `None`).
    ///
    /// # Errors
    ///
    /// [`ConsensusError`] when miner, timestamp, or seal checks fail.
    fn validate_header(
        &self,
        header: &BlockHeader,
        parent: Option<&BlockHeader>,
    ) -> Result<(), ConsensusError>;

    /// Sign the header (empty `extra_data` hash) and write seals into `extra_data`.
    ///
    /// # Errors
    ///
    /// [`ConsensusError::NotValidator`] if `miner` is not authorized or does not
    /// match `secret`.
    fn seal_header(
        &self,
        header: &mut BlockHeader,
        miner: &Address,
        secret: &SecretKey,
    ) -> Result<(), ConsensusError>;
}
