//! Intrinsic gas and per-transaction metering.

use ivory_core::Transaction;

use crate::error::ExecutionError;

/// Gas schedule for intrinsic costs and the block cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GasConfig {
    /// Base cost charged for every transaction.
    pub tx_gas_cost: u64,
    /// Cost per byte of `tx.data`.
    pub data_gas_cost: u64,
    /// Maximum cumulative gas in a block.
    pub max_gas_per_block: u64,
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            tx_gas_cost: 21_000,
            data_gas_cost: 16,
            max_gas_per_block: 30_000_000,
        }
    }
}

/// Per-transaction gas remaining after intrinsic purchase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GasMeter {
    /// Transaction gas limit.
    pub limit: u64,
    /// Gas still available for execution (after intrinsic).
    pub remaining: u64,
    /// Intrinsic gas charged up front.
    pub intrinsic: u64,
}

impl GasMeter {
    /// Buy intrinsic gas from `limit`.
    ///
    /// # Errors
    ///
    /// [`ExecutionError::OutOfGas`] if `intrinsic > limit`.
    pub fn new(limit: u64, intrinsic: u64) -> Result<Self, ExecutionError> {
        let remaining = limit
            .checked_sub(intrinsic)
            .ok_or(ExecutionError::OutOfGas)?;
        Ok(Self {
            limit,
            remaining,
            intrinsic,
        })
    }

    /// Gas consumed so far (`limit - remaining`).
    #[must_use]
    pub fn gas_used(&self) -> u64 {
        self.limit.saturating_sub(self.remaining)
    }

    /// Unused gas that can be refunded (`remaining`).
    #[must_use]
    pub fn refund_gas(&self) -> u64 {
        self.remaining
    }

    /// Spend `amount` from remaining gas (reserved for the VM in #7).
    ///
    /// # Errors
    ///
    /// [`ExecutionError::OutOfGas`] if `amount` exceeds remaining.
    pub fn spend(&mut self, amount: u64) -> Result<(), ExecutionError> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(ExecutionError::OutOfGas)?;
        Ok(())
    }
}

/// Intrinsic gas: base cost plus per-byte calldata.
#[must_use]
pub fn compute_intrinsic_gas(tx: &Transaction, cfg: &GasConfig) -> u64 {
    compute_intrinsic_gas_len(tx.data_len(), cfg)
}

/// Intrinsic gas from calldata length (simulation without a signed tx).
#[must_use]
pub fn compute_intrinsic_gas_len(data_len: usize, cfg: &GasConfig) -> u64 {
    let data_cost = (data_len as u64).saturating_mul(cfg.data_gas_cost);
    cfg.tx_gas_cost.saturating_add(data_cost)
}

#[cfg(test)]
mod tests {
    use ivory_primitives::{Address, Bytes, PublicKey, Signature, U256};

    use super::*;

    fn tx_with_data(data: &[u8], gas: u64) -> Transaction {
        Transaction {
            from: Address::from_bytes([1u8; 20]),
            to: Some(Address::from_bytes([2u8; 20])),
            value: U256::ZERO,
            data: Bytes::from_slice(data),
            gas_price: U256::ONE,
            gas,
            nonce: 0,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        }
    }

    #[test]
    fn intrinsic_base_only() {
        let cfg = GasConfig::default();
        let tx = tx_with_data(&[], 21_000);
        assert_eq!(compute_intrinsic_gas(&tx, &cfg), 21_000);
    }

    #[test]
    fn intrinsic_includes_data() {
        let cfg = GasConfig::default();
        let tx = tx_with_data(&[0xaa, 0xbb], 30_000);
        assert_eq!(compute_intrinsic_gas(&tx, &cfg), 21_000 + 32);
    }

    #[test]
    fn meter_rejects_intrinsic_above_limit() {
        assert_eq!(GasMeter::new(21_000, 21_001), Err(ExecutionError::OutOfGas));
    }

    #[test]
    fn meter_used_and_refund() {
        let meter = GasMeter::new(21_032, 21_032).unwrap();
        assert_eq!(meter.gas_used(), 21_032);
        assert_eq!(meter.refund_gas(), 0);
        let meter = GasMeter::new(30_000, 21_000).unwrap();
        assert_eq!(meter.gas_used(), 21_000);
        assert_eq!(meter.refund_gas(), 9_000);
    }

    #[test]
    fn meter_spend_reduces_remaining() {
        let mut meter = GasMeter::new(30_000, 21_000).unwrap();
        meter.spend(1_000).unwrap();
        assert_eq!(meter.gas_used(), 22_000);
        assert_eq!(meter.refund_gas(), 8_000);
        assert_eq!(meter.spend(8_001), Err(ExecutionError::OutOfGas));
    }
}
