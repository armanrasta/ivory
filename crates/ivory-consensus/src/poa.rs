//! Proof-of-Authority consensus.

use ivory_core::BlockHeader;
use ivory_primitives::Address;

use crate::engine::ConsensusEngine;
use crate::error::ConsensusError;
use crate::seal::{encode_seals, verify_seal};

/// Validator set and seal policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoAConfig {
    /// Authorized miners.
    pub validators: Vec<Address>,
    /// Number of 64-byte placeholder seals required in `extra_data`.
    pub required_signatures: usize,
}

impl PoAConfig {
    /// Single-validator set requiring one seal.
    #[must_use]
    pub fn single(validator: Address) -> Self {
        Self {
            validators: vec![validator],
            required_signatures: 1,
        }
    }
}

/// In-memory PoA engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoAConsensus {
    config: PoAConfig,
}

impl PoAConsensus {
    /// Build from an explicit config.
    ///
    /// # Errors
    ///
    /// [`ConsensusError::EmptyValidatorSet`] or [`ConsensusError::InvalidSeal`]
    /// if `required_signatures` is zero.
    pub fn new(config: PoAConfig) -> Result<Self, ConsensusError> {
        if config.validators.is_empty() {
            return Err(ConsensusError::EmptyValidatorSet);
        }
        if config.required_signatures == 0 {
            return Err(ConsensusError::InvalidSeal);
        }
        Ok(Self { config })
    }

    /// Convenience: one validator, one seal.
    ///
    /// # Errors
    ///
    /// Same as [`Self::new`].
    pub fn with_validator(validator: Address) -> Result<Self, ConsensusError> {
        Self::new(PoAConfig::single(validator))
    }

    /// Configured validator addresses.
    #[must_use]
    pub fn validators(&self) -> &[Address] {
        &self.config.validators
    }

    /// Required placeholder seals.
    #[must_use]
    pub fn required_signatures(&self) -> usize {
        self.config.required_signatures
    }
}

impl ConsensusEngine for PoAConsensus {
    fn is_validator(&self, addr: &Address) -> bool {
        self.config.validators.contains(addr)
    }

    fn validate_header(
        &self,
        header: &BlockHeader,
        parent: Option<&BlockHeader>,
    ) -> Result<(), ConsensusError> {
        if !self.is_validator(&header.miner) {
            return Err(ConsensusError::NotValidator);
        }
        if let Some(parent) = parent
            && header.timestamp < parent.timestamp
        {
            return Err(ConsensusError::InvalidTimestamp);
        }
        verify_seal(&header.extra_data, self.config.required_signatures)
    }

    fn seal_header(&self, header: &mut BlockHeader, miner: &Address) -> Result<(), ConsensusError> {
        if !self.is_validator(miner) {
            return Err(ConsensusError::NotValidator);
        }
        header.miner = *miner;
        header.extra_data = encode_seals(self.config.required_signatures)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ivory_core::BlockHeader;
    use ivory_primitives::{Address, Bytes, H256, U256};

    use super::*;
    use crate::seal::encode_seals;

    fn addr(b: u8) -> Address {
        Address::from_bytes([b; 20])
    }

    fn header(miner: Address, ts: u64, extra: Bytes) -> BlockHeader {
        BlockHeader {
            number: 1,
            parent_hash: H256::ZERO,
            timestamp: ts,
            miner,
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: extra,
        }
    }

    #[test]
    fn single_validator_is_member() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        assert!(poa.is_validator(&addr(1)));
        assert!(!poa.is_validator(&addr(2)));
        assert_eq!(poa.required_signatures(), 1);
        assert_eq!(poa.validators(), &[addr(1)]);
    }

    #[test]
    fn empty_validator_set_rejected() {
        assert_eq!(
            PoAConsensus::new(PoAConfig {
                validators: vec![],
                required_signatures: 1,
            }),
            Err(ConsensusError::EmptyValidatorSet)
        );
    }

    #[test]
    fn zero_required_signatures_rejected() {
        assert_eq!(
            PoAConsensus::new(PoAConfig {
                validators: vec![addr(1)],
                required_signatures: 0,
            }),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn seal_header_writes_extra_data() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let mut h = header(Address::zero(), 10, Bytes::new());
        poa.seal_header(&mut h, &addr(1)).unwrap();
        assert_eq!(h.miner, addr(1));
        assert_eq!(h.extra_data.as_slice().len(), 64);
        poa.validate_header(&h, None).unwrap();
    }

    #[test]
    fn seal_header_rejects_non_validator() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let mut h = header(addr(1), 10, Bytes::new());
        assert_eq!(
            poa.seal_header(&mut h, &addr(9)),
            Err(ConsensusError::NotValidator)
        );
    }

    #[test]
    fn validate_rejects_unknown_miner() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let extra = encode_seals(1).unwrap();
        let h = header(addr(2), 10, extra);
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::NotValidator)
        );
    }

    #[test]
    fn validate_rejects_short_extra_data() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let h = header(addr(1), 10, Bytes::from_slice(&[0u8; 32]));
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn validate_rejects_empty_extra_data() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let h = header(addr(1), 10, Bytes::new());
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn timestamp_must_not_go_backwards() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let extra = encode_seals(1).unwrap();
        let parent = header(addr(1), 100, extra.clone());
        let child = header(addr(1), 99, extra);
        assert_eq!(
            poa.validate_header(&child, Some(&parent)),
            Err(ConsensusError::InvalidTimestamp)
        );
    }

    #[test]
    fn timestamp_equal_to_parent_ok() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let extra = encode_seals(1).unwrap();
        let parent = header(addr(1), 100, extra.clone());
        let child = header(addr(1), 100, extra);
        poa.validate_header(&child, Some(&parent)).unwrap();
    }

    #[test]
    fn timestamp_after_parent_ok() {
        let poa = PoAConsensus::with_validator(addr(1)).unwrap();
        let extra = encode_seals(1).unwrap();
        let parent = header(addr(1), 100, extra.clone());
        let child = header(addr(1), 101, extra);
        poa.validate_header(&child, Some(&parent)).unwrap();
    }

    #[test]
    fn two_required_signatures() {
        let poa = PoAConsensus::new(PoAConfig {
            validators: vec![addr(1), addr(2)],
            required_signatures: 2,
        })
        .unwrap();
        let extra = encode_seals(2).unwrap();
        let h = header(addr(1), 10, extra);
        poa.validate_header(&h, None).unwrap();
        let short = header(addr(1), 10, encode_seals(1).unwrap());
        assert_eq!(
            poa.validate_header(&short, None),
            Err(ConsensusError::InsufficientSeals {
                expected: 2,
                got: 1
            })
        );
    }

    #[test]
    fn two_validators_either_may_mine() {
        let poa = PoAConsensus::new(PoAConfig {
            validators: vec![addr(1), addr(2)],
            required_signatures: 1,
        })
        .unwrap();
        let extra = encode_seals(1).unwrap();
        poa.validate_header(&header(addr(1), 1, extra.clone()), None)
            .unwrap();
        poa.validate_header(&header(addr(2), 1, extra), None)
            .unwrap();
        assert!(poa.is_validator(&addr(1)));
        assert!(poa.is_validator(&addr(2)));
        assert!(!poa.is_validator(&addr(3)));
    }

    #[test]
    fn config_single_helper() {
        let cfg = PoAConfig::single(addr(7));
        assert_eq!(cfg.validators.len(), 1);
        assert_eq!(cfg.required_signatures, 1);
    }
}
