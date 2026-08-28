// crates/ivory-primitives/src/signature.rs

//! Cryptographic signature types.
//!
//! Provides [`Signature`], [`PublicKey`], and [`SecretKey`] types
//! for Ed25519 signatures.

use alloc::string::String;
use core::fmt;

use crate::{H256, H512};

/// Ed25519 signature (64 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub H512);

impl Signature {
    /// Size in bytes
    pub const SIZE: usize = 64;

    /// Zero signature
    pub const ZERO: Self = Signature(H512::ZERO);

    /// Create zero signature
    #[inline]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Create from byte array
    #[inline]
    pub const fn from_bytes(bytes: [u8; 64]) -> Self {
        Signature(H512(bytes))
    }

    /// Create from byte slice
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        H512::from_slice(slice).map(Signature)
    }

    /// Get as byte slice
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 64] {
        self.0.as_bytes()
    }

    /// Convert to byte array
    #[inline]
    pub const fn to_bytes(self) -> [u8; 64] {
        self.0.to_bytes()
    }

    /// Check if zero signature
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Convert to hex string
    #[inline]
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Signature(0x{}...)",
            hex::encode(&self.0.as_bytes()[..8])
        )
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        H512::deserialize(deserializer).map(Signature)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Signature {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        H512::arbitrary(u).map(Signature)
    }
}

/// Ed25519 public key (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey(pub H256);

impl PublicKey {
    /// Size in bytes
    pub const SIZE: usize = 32;

    /// Zero public key
    pub const ZERO: Self = PublicKey(H256::ZERO);

    /// Create zero public key
    #[inline]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Create from byte array
    #[inline]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        PublicKey(H256(bytes))
    }

    /// Create from byte slice
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        H256::from_slice(slice).map(PublicKey)
    }

    /// Get as byte slice
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Convert to byte array
    #[inline]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Check if zero public key
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Convert to hex string
    #[inline]
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl Default for PublicKey {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.0.to_hex())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        H256::deserialize(deserializer).map(PublicKey)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for PublicKey {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        H256::arbitrary(u).map(PublicKey)
    }
}

/// Ed25519 secret key (32 bytes).
#[derive(Clone)]
pub struct SecretKey(pub H256);

impl SecretKey {
    /// Size in bytes
    pub const SIZE: usize = 32;

    /// Create from byte array
    #[inline]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        SecretKey(H256(bytes))
    }

    /// Create from byte slice
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        H256::from_slice(slice).map(SecretKey)
    }

    /// Get as byte slice
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Convert to byte array
    #[inline]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Convert to hex string
    #[inline]
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Don't show actual secret key contents
        write!(f, "SecretKey([REDACTED])")
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SecretKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Don't serialize secret key by default
        serializer.serialize_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_zero() {
        let sig = Signature::zero();
        assert!(sig.is_zero());
        assert_eq!(sig, Signature::ZERO);
    }

    #[test]
    fn test_public_key_zero() {
        let pk = PublicKey::zero();
        assert!(pk.is_zero());
        assert_eq!(pk, PublicKey::ZERO);
    }

    #[test]
    fn test_secret_key_debug() {
        let sk = SecretKey::from_bytes([1u8; 32]);
        let debug = format!("{:?}", sk);
        assert_eq!(debug, "SecretKey([REDACTED])");
    }
}
