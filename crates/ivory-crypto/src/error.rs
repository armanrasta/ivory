//! Cryptographic errors.

use thiserror::Error;

/// Failures from key parsing, signing, or verification.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// Ed25519 signature did not verify under the given public key.
    #[error("invalid signature")]
    InvalidSignature,
    /// Public key bytes are not a valid Ed25519 key.
    #[error("invalid public key")]
    InvalidPublicKey,
    /// Secret key bytes are not a valid Ed25519 key.
    #[error("invalid secret key")]
    InvalidSecretKey,
    /// Derived address does not match `tx.from`.
    #[error("key does not match sender")]
    KeyMismatch,
}
