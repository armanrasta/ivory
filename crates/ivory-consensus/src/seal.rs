//! PoA seal encoding in [`ivory_core::BlockHeader::extra_data`].
//!
//! Each seal is a 64-byte Ed25519 signature over [`seal_hash`] (header hash with
//! empty `extra_data`). Length checks live here; cryptographic verify is in
//! [`crate::poa`].

use ivory_core::BlockHeader;
use ivory_primitives::{Bytes, H256, Signature};

use crate::error::ConsensusError;

/// Encoded length of one seal.
pub const SEAL_LEN: usize = Signature::SIZE;

/// Hash used as the Ed25519 message for PoA seals.
///
/// `extra_data` is cleared so the signature is not taken over itself.
#[must_use]
pub fn seal_hash(header: &BlockHeader) -> H256 {
    let mut unsigned = header.clone();
    unsigned.extra_data = Bytes::new();
    unsigned.hash()
}

/// Concatenate `count` zero seals (length helper / tests).
///
/// # Errors
///
/// [`ConsensusError::InvalidSeal`] if `count` is zero.
pub fn encode_seals(count: usize) -> Result<Bytes, ConsensusError> {
    if count == 0 {
        return Err(ConsensusError::InvalidSeal);
    }
    encode_signatures(&vec![Signature::zero(); count])
}

/// Concatenate real Ed25519 seals.
///
/// # Errors
///
/// [`ConsensusError::InvalidSeal`] if `seals` is empty.
pub fn encode_signatures(seals: &[Signature]) -> Result<Bytes, ConsensusError> {
    if seals.is_empty() {
        return Err(ConsensusError::InvalidSeal);
    }
    let mut buf = Vec::with_capacity(seals.len().saturating_mul(SEAL_LEN));
    for seal in seals {
        buf.extend_from_slice(seal.as_bytes());
    }
    Ok(Bytes::from_vec(buf))
}

/// Decode concatenated 64-byte seals from `extra_data`.
///
/// # Errors
///
/// [`ConsensusError::InvalidSeal`] if the buffer length is not a multiple of 64
/// or is empty.
pub fn decode_seals(extra_data: &Bytes) -> Result<Vec<Signature>, ConsensusError> {
    let bytes = extra_data.as_slice();
    if bytes.is_empty() || !bytes.len().is_multiple_of(SEAL_LEN) {
        return Err(ConsensusError::InvalidSeal);
    }
    Ok(bytes
        .as_chunks::<SEAL_LEN>()
        .0
        .iter()
        .map(|arr| Signature::from_bytes(*arr))
        .collect())
}

/// Number of seals encoded in `extra_data`.
///
/// # Errors
///
/// Same as [`decode_seals`].
pub fn seal_count(extra_data: &Bytes) -> Result<usize, ConsensusError> {
    Ok(decode_seals(extra_data)?.len())
}

/// Check that `extra_data` holds at least `required` seals (length only).
///
/// # Errors
///
/// [`ConsensusError::InsufficientSeals`] or [`ConsensusError::InvalidSeal`].
pub fn verify_seal(extra_data: &Bytes, required: usize) -> Result<(), ConsensusError> {
    if required == 0 {
        return Err(ConsensusError::InvalidSeal);
    }
    let got = seal_count(extra_data)?;
    if got < required {
        return Err(ConsensusError::InsufficientSeals {
            expected: required,
            got,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ivory_core::BlockHeader;
    use ivory_primitives::{Address, H256, U256};

    use super::*;

    fn header() -> BlockHeader {
        BlockHeader {
            number: 1,
            parent_hash: H256::ZERO,
            timestamp: 10,
            miner: Address::zero(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        }
    }

    #[test]
    fn encode_one_seal_is_64_bytes() {
        let bytes = encode_seals(1).unwrap();
        assert_eq!(bytes.as_slice().len(), 64);
        assert!(bytes.as_slice().iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_three_seals_is_192_bytes() {
        let bytes = encode_seals(3).unwrap();
        assert_eq!(bytes.as_slice().len(), 192);
        assert_eq!(seal_count(&bytes).unwrap(), 3);
    }

    #[test]
    fn encode_zero_is_invalid() {
        assert_eq!(encode_seals(0), Err(ConsensusError::InvalidSeal));
        assert_eq!(encode_signatures(&[]), Err(ConsensusError::InvalidSeal));
    }

    #[test]
    fn decode_roundtrip() {
        let encoded = encode_seals(2).unwrap();
        let seals = decode_seals(&encoded).unwrap();
        assert_eq!(seals.len(), 2);
        assert!(seals.iter().all(|s| s.is_zero()));
    }

    #[test]
    fn decode_empty_is_invalid() {
        assert_eq!(
            decode_seals(&Bytes::new()),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn decode_odd_length_is_invalid() {
        assert_eq!(
            decode_seals(&Bytes::from_slice(&[0u8; 63])),
            Err(ConsensusError::InvalidSeal)
        );
    }

    #[test]
    fn verify_seal_ok() {
        let extra = encode_seals(2).unwrap();
        verify_seal(&extra, 2).unwrap();
        verify_seal(&extra, 1).unwrap();
    }

    #[test]
    fn verify_seal_too_few() {
        let extra = encode_seals(1).unwrap();
        assert_eq!(
            verify_seal(&extra, 2),
            Err(ConsensusError::InsufficientSeals {
                expected: 2,
                got: 1
            })
        );
    }

    #[test]
    fn verify_required_zero_is_invalid() {
        let extra = encode_seals(1).unwrap();
        assert_eq!(verify_seal(&extra, 0), Err(ConsensusError::InvalidSeal));
    }

    #[test]
    fn seal_hash_ignores_extra_data() {
        let mut a = header();
        let b = a.clone();
        a.extra_data = encode_seals(1).unwrap();
        assert_eq!(seal_hash(&a), seal_hash(&b));
        assert_ne!(a.hash(), b.hash());
    }
}
