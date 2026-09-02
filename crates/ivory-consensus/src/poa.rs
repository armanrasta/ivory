//! Proof-of-Authority consensus.

use ivory_core::BlockHeader;
use ivory_crypto::{address_from_secret, public_key_from_secret, sign, verify};
use ivory_primitives::{Address, PublicKey, SecretKey};

use crate::engine::ConsensusEngine;
use crate::error::ConsensusError;
use crate::seal::{decode_seals, encode_signatures, seal_hash, verify_seal};

/// Authorized miner with the public key used to check seals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    /// Account address derived from `public_key`.
    pub address: Address,
    /// Ed25519 verifying key.
    pub public_key: PublicKey,
}

impl Validator {
    /// Build from a secret key.
    #[must_use]
    pub fn from_secret(sk: &SecretKey) -> Self {
        let public_key = public_key_from_secret(sk);
        Self {
            address: ivory_crypto::address_from_public_key(&public_key),
            public_key,
        }
    }
}

/// Validator set and seal policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoAConfig {
    /// Authorized miners.
    pub validators: Vec<Validator>,
    /// Number of Ed25519 seals required in `extra_data`.
    pub required_signatures: usize,
}

impl PoAConfig {
    /// Single-validator set requiring one seal.
    #[must_use]
    pub fn single(validator: Validator) -> Self {
        Self {
            validators: vec![validator],
            required_signatures: 1,
        }
    }

    /// Single validator derived from `sk`.
    #[must_use]
    pub fn from_secret(sk: &SecretKey) -> Self {
        Self::single(Validator::from_secret(sk))
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

    /// Convenience: one validator from `sk`, one seal.
    ///
    /// # Errors
    ///
    /// Same as [`Self::new`].
    pub fn from_secret(sk: &SecretKey) -> Result<Self, ConsensusError> {
        Self::new(PoAConfig::from_secret(sk))
    }

    /// Configured validators.
    #[must_use]
    pub fn validators(&self) -> &[Validator] {
        &self.config.validators
    }

    /// Required Ed25519 seals.
    #[must_use]
    pub fn required_signatures(&self) -> usize {
        self.config.required_signatures
    }

    fn verify_crypto_seals(&self, header: &BlockHeader) -> Result<(), ConsensusError> {
        verify_seal(&header.extra_data, self.config.required_signatures)?;
        let seals = decode_seals(&header.extra_data)?;
        let hash = seal_hash(header);
        let message = hash.as_bytes();
        let mut unique: Vec<Address> = Vec::new();
        for seal in &seals {
            let mut matched: Option<Address> = None;
            for v in &self.config.validators {
                if verify(message, seal, &v.public_key).is_ok() {
                    matched = Some(v.address);
                    break;
                }
            }
            let Some(addr) = matched else {
                return Err(ConsensusError::InvalidSeal);
            };
            if !unique.contains(&addr) {
                unique.push(addr);
            }
        }
        if unique.len() < self.config.required_signatures {
            return Err(ConsensusError::InsufficientSeals {
                expected: self.config.required_signatures,
                got: unique.len(),
            });
        }
        Ok(())
    }
}

impl ConsensusEngine for PoAConsensus {
    fn is_validator(&self, addr: &Address) -> bool {
        self.config.validators.iter().any(|v| v.address == *addr)
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
        self.verify_crypto_seals(header)
    }

    fn seal_header(
        &self,
        header: &mut BlockHeader,
        miner: &Address,
        secret: &SecretKey,
    ) -> Result<(), ConsensusError> {
        if !self.is_validator(miner) {
            return Err(ConsensusError::NotValidator);
        }
        if address_from_secret(secret) != *miner {
            return Err(ConsensusError::NotValidator);
        }
        header.miner = *miner;
        header.extra_data = ivory_primitives::Bytes::new();
        let sig = sign(seal_hash(header).as_bytes(), secret);
        header.extra_data = encode_signatures(&[sig])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ivory_core::BlockHeader;
    use ivory_crypto::{keypair_from_byte, sign};
    use ivory_primitives::{Address, Bytes, H256, U256};

    use super::*;
    use crate::seal::{encode_seals, encode_signatures, seal_hash};

    fn sk(b: u8) -> SecretKey {
        keypair_from_byte(b).0
    }

    fn addr(b: u8) -> Address {
        keypair_from_byte(b).2
    }

    fn val(b: u8) -> Validator {
        Validator::from_secret(&sk(b))
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

    fn signed_header(miner_seed: u8, ts: u64) -> BlockHeader {
        let mut h = header(addr(miner_seed), ts, Bytes::new());
        PoAConsensus::from_secret(&sk(miner_seed))
            .unwrap()
            .seal_header(&mut h, &addr(miner_seed), &sk(miner_seed))
            .unwrap();
        h
    }

    #[test]
    fn single_validator_is_member() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        assert!(poa.is_validator(&addr(1)));
        assert!(!poa.is_validator(&addr(2)));
        assert_eq!(poa.required_signatures(), 1);
        assert_eq!(poa.validators()[0].address, addr(1));
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
                validators: vec![val(1)],
                required_signatures: 0,
            }),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn seal_header_writes_extra_data() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let mut h = header(Address::zero(), 10, Bytes::new());
        poa.seal_header(&mut h, &addr(1), &sk(1)).unwrap();
        assert_eq!(h.miner, addr(1));
        assert_eq!(h.extra_data.as_slice().len(), 64);
        poa.validate_header(&h, None).unwrap();
    }

    #[test]
    fn seal_header_rejects_non_validator() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let mut h = header(addr(1), 10, Bytes::new());
        assert_eq!(
            poa.seal_header(&mut h, &addr(9), &sk(9)),
            Err(ConsensusError::NotValidator)
        );
    }

    #[test]
    fn seal_header_rejects_key_mismatch() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let mut h = header(addr(1), 10, Bytes::new());
        assert_eq!(
            poa.seal_header(&mut h, &addr(1), &sk(2)),
            Err(ConsensusError::NotValidator)
        );
    }

    #[test]
    fn validate_rejects_unknown_miner() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let h = signed_header(2, 10);
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::NotValidator)
        );
    }

    #[test]
    fn validate_rejects_short_extra_data() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let h = header(addr(1), 10, Bytes::from_slice(&[0u8; 32]));
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn validate_rejects_empty_extra_data() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let h = header(addr(1), 10, Bytes::new());
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn validate_rejects_placeholder_zeros() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let h = header(addr(1), 10, encode_seals(1).unwrap());
        assert_eq!(
            poa.validate_header(&h, None),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn timestamp_must_not_go_backwards() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let parent = signed_header(1, 100);
        let mut child = header(addr(1), 99, Bytes::new());
        poa.seal_header(&mut child, &addr(1), &sk(1)).unwrap();
        assert_eq!(
            poa.validate_header(&child, Some(&parent)),
            Err(ConsensusError::InvalidTimestamp)
        );
    }

    #[test]
    fn timestamp_equal_to_parent_ok() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let parent = signed_header(1, 100);
        let mut child = header(addr(1), 100, Bytes::new());
        poa.seal_header(&mut child, &addr(1), &sk(1)).unwrap();
        poa.validate_header(&child, Some(&parent)).unwrap();
    }

    #[test]
    fn timestamp_after_parent_ok() {
        let poa = PoAConsensus::from_secret(&sk(1)).unwrap();
        let parent = signed_header(1, 100);
        let mut child = header(addr(1), 101, Bytes::new());
        poa.seal_header(&mut child, &addr(1), &sk(1)).unwrap();
        poa.validate_header(&child, Some(&parent)).unwrap();
    }

    #[test]
    fn two_required_signatures() {
        let poa = PoAConsensus::new(PoAConfig {
            validators: vec![val(1), val(2)],
            required_signatures: 2,
        })
        .unwrap();
        let mut h = header(addr(1), 10, Bytes::new());
        let hash = seal_hash(&h);
        let s1 = sign(hash.as_bytes(), &sk(1));
        let s2 = sign(hash.as_bytes(), &sk(2));
        h.extra_data = encode_signatures(&[s1, s2]).unwrap();
        poa.validate_header(&h, None).unwrap();

        let mut short = header(addr(1), 10, Bytes::new());
        poa.seal_header(&mut short, &addr(1), &sk(1)).unwrap();
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
            validators: vec![val(1), val(2)],
            required_signatures: 1,
        })
        .unwrap();
        let mut h1 = header(addr(1), 1, Bytes::new());
        poa.seal_header(&mut h1, &addr(1), &sk(1)).unwrap();
        poa.validate_header(&h1, None).unwrap();
        let mut h2 = header(addr(2), 1, Bytes::new());
        poa.seal_header(&mut h2, &addr(2), &sk(2)).unwrap();
        poa.validate_header(&h2, None).unwrap();
        assert!(poa.is_validator(&addr(1)));
        assert!(poa.is_validator(&addr(2)));
        assert!(!poa.is_validator(&addr(3)));
    }

    #[test]
    fn config_single_helper() {
        let cfg = PoAConfig::from_secret(&sk(7));
        assert_eq!(cfg.validators.len(), 1);
        assert_eq!(cfg.required_signatures, 1);
        assert_eq!(cfg.validators[0].address, addr(7));
    }
}
