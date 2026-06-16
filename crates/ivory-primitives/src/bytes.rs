// crates/ivory-primitives/src/bytes.rs

//! Dynamic byte arrays.

use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};

/// Dynamic byte array.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    /// Create a new empty Bytes
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a Vec<u8>
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self(vec)
    }

    /// Create from a byte slice
    pub fn copy_from_slice(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Bytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Self {
        Self(vec)
    }
}

impl From<&[u8]> for Bytes {
    fn from(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bytes(0x{})", hex::encode(&self.0))
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&alloc::format!("0x{}", hex::encode(&self.0)))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = <alloc::string::String as serde::Deserialize>::deserialize(deserializer)?;
            let s = s.strip_prefix("0x").unwrap_or(&s);
            let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
            Ok(Bytes(bytes))
        } else {
            let bytes = <Vec<u8> as serde::Deserialize>::deserialize(deserializer)?;
            Ok(Bytes(bytes))
        }
    }
}
