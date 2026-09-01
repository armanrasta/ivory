//! PoA seal encoding in [`ivory_core::BlockHeader::extra_data`].
//!
//! Placeholder: each seal is 64 zero bytes ([`Signature::zero`]). Crypto recovery
//! lands in #28 / #16.

use ivory_primitives::{Bytes, Signature};

use crate::error::ConsensusError;

/// Encoded length of one seal.
pub const SEAL_LEN: usize = Signature::SIZE;

/// Concatenate `count` placeholder seals.
///
/// # Errors
///
/// [`ConsensusError::InvalidSeal`] if `count` is zero.
pub fn encode_seals(count: usize) -> Result<Bytes, ConsensusError> {
    if count == 0 {
        return Err(ConsensusError::InvalidSeal);
    }
    let mut buf = Vec::with_capacity(count.saturating_mul(SEAL_LEN));
    for _ in 0..count {
        buf.extend_from_slice(Signature::zero().as_bytes());
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
    let mut seals = Vec::with_capacity(bytes.len() / SEAL_LEN);
    for chunk in bytes.chunks_exact(SEAL_LEN) {
        let arr: [u8; SEAL_LEN] = chunk.try_into().map_err(|_| ConsensusError::InvalidSeal)?;
        seals.push(Signature::from_bytes(arr));
    }
    Ok(seals)
}

/// Number of seals encoded in `extra_data`.
///
/// # Errors
///
/// Same as [`decode_seals`].
pub fn seal_count(extra_data: &Bytes) -> Result<usize, ConsensusError> {
    Ok(decode_seals(extra_data)?.len())
}

/// Check that `extra_data` holds at least `required` seals.
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
    use super::*;

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
}
