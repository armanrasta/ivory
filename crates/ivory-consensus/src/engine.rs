//! Consensus engine trait.

use ivory_core::BlockHeader;
use ivory_primitives::Address;

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

    /// Write the PoA seal into `header.extra_data`.
    ///
    /// # Errors
    ///
    /// [`ConsensusError::NotValidator`] if `miner` is not authorized.
    fn seal_header(&self, header: &mut BlockHeader, miner: &Address) -> Result<(), ConsensusError>;
}
