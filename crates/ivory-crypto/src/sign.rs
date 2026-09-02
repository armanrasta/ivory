//! Ed25519 sign and verify.

use ed25519_dalek::{Signer, Verifier};
use ivory_primitives::{PublicKey, SecretKey, Signature};

use crate::error::CryptoError;

/// Sign `message` with `sk`.
#[must_use]
pub fn sign(message: &[u8], sk: &SecretKey) -> Signature {
    let signing = ed25519_dalek::SigningKey::from_bytes(sk.as_bytes());
    let sig = signing.sign(message);
    Signature::from_bytes(sig.to_bytes())
}

/// Verify `signature` over `message` with `pk`.
///
/// # Errors
///
/// [`CryptoError::InvalidPublicKey`] or [`CryptoError::InvalidSignature`].
pub fn verify(message: &[u8], signature: &Signature, pk: &PublicKey) -> Result<(), CryptoError> {
    let verifying = ed25519_dalek::VerifyingKey::from_bytes(pk.as_bytes())
        .map_err(|_| CryptoError::InvalidPublicKey)?;
    let sig = ed25519_dalek::Signature::from_bytes(signature.as_bytes());
    verifying
        .verify(message, &sig)
        .map_err(|_| CryptoError::InvalidSignature)
}
