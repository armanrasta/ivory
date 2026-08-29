//! Block-level execution context.

use ivory_primitives::Address;

/// Cumulative gas and header fields for a block being executed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionContext {
    /// Block number of the enclosing block.
    pub block_number: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Gas used by transactions already applied in this block.
    pub gas_used: u64,
    /// Miner / beneficiary (unused until base fees are paid out).
    pub beneficiary: Address,
}

impl ExecutionContext {
    /// Context for tests at genesis-like defaults.
    #[must_use]
    pub fn new(block_number: u64, timestamp: u64) -> Self {
        Self {
            block_number,
            timestamp,
            gas_used: 0,
            beneficiary: Address::zero(),
        }
    }
}
