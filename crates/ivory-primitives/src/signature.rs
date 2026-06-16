// crates/ivory-primitives/src/signature.rs

//! Cryptographic primitive types.

use core::fmt;
use crate::H512;

/// A cryptographic signature.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Signature(pub H512);

impl Signature {
    /// Create from H512
    pub fn from_h512(h: H512) -> Self {
        Self(h)
    }
}

/// A cryptographic public key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PublicKey(pub [u8; 32]);

/// A cryptographic secret key.
pub struct SecretKey(pub [u8; 32]);

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature({:?})", self.0)
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey(0x{})", hex::encode(self.0))
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey(***)")
    }
}
