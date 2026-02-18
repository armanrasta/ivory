// crates/ivory-primitives/src/hash.rs

//! Fixed-size hash types.
//!
//! Provides [`H128`], [`H160`], [`H256`], [`H512`], [`H520`] types
//! for representing fixed-size byte arrays commonly used in blockchains.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::str::FromStr;
use core::ops::{Deref, DerefMut, BitXor, BitAnd, BitOr, Index, IndexMut};
use core::hash::{Hash, Hasher};
use core::cmp::Ordering;

use crate::error::PrimitiveError;

// ─────────────────────────────────────────────────────────────────────────────
// Macro for generating hash types
// ─────────────────────────────────────────────────────────────────────────────

macro_rules! impl_hash {
    ($name:ident, $size:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Default)]
        #[repr(transparent)]
        pub struct $name(pub [u8; $size]);

        impl $name {
            /// Size in bytes
            pub const SIZE: usize = $size;
            
            /// Zero value (all bytes are 0)
            pub const ZERO: Self = Self([0u8; $size]);
            
            /// Create a new hash with all bytes set to zero
            #[inline]
            pub const fn zero() -> Self {
                Self::ZERO
            }
            
            /// Create from a byte array
            #[inline]
            pub const fn from_bytes(bytes: [u8; $size]) -> Self {
                Self(bytes)
            }
            
            /// Create from a byte slice
            ///
            /// Returns `None` if the slice length doesn't match
            #[inline]
            pub fn from_slice(slice: &[u8]) -> Option<Self> {
                if slice.len() != $size {
                    return None;
                }
                let mut arr = [0u8; $size];
                arr.copy_from_slice(slice);
                Some(Self(arr))
            }
            
            /// Create from a hex string (with or without "0x" prefix)
            ///
            /// # Errors
            ///
            /// Returns error if the string is invalid hex or wrong length
            pub fn from_hex(s: &str) -> Result<Self, PrimitiveError> {
                let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
                
                if s.len() != $size * 2 {
                    return Err(PrimitiveError::InvalidLength {
                        expected: $size * 2,
                        got: s.len(),
                    });
                }
                
                let bytes = hex::decode(s).map_err(|_| PrimitiveError::InvalidHex)?;
                
                Self::from_slice(&bytes).ok_or(PrimitiveError::InvalidLength {
                    expected: $size,
                    got: bytes.len(),
                })
            }
            
            /// Get as byte slice
            #[inline]
            pub const fn as_bytes(&self) -> &[u8; $size] {
                &self.0
            }
            
            /// Get as mutable byte slice
            #[inline]
            pub fn as_bytes_mut(&mut self) -> &mut [u8; $size] {
                &mut self.0
            }
            
            /// Get as raw byte slice
            #[inline]
            pub fn as_slice(&self) -> &[u8] {
                &self.0
            }
            
            /// Convert to owned byte array
            #[inline]
            pub const fn to_bytes(self) -> [u8; $size] {
                self.0
            }
            
            /// Convert to Vec
            #[inline]
            pub fn to_vec(&self) -> Vec<u8> {
                self.0.to_vec()
            }
            
            /// Check if all bytes are zero
            #[inline]
            pub fn is_zero(&self) -> bool {
                self.0.iter().all(|&b| b == 0)
            }
            
            /// Convert to hex string with "0x" prefix
            #[inline]
            pub fn to_hex(&self) -> String {
                alloc::format!("0x{}", hex::encode(self.0))
            }
            
            /// Convert to hex string without prefix
            #[inline]
            pub fn to_hex_raw(&self) -> String {
                hex::encode(self.0)
            }
            
            /// Reverse the bytes (useful for endianness conversion)
            #[inline]
            pub fn reverse(&mut self) {
                self.0.reverse();
            }
            
            /// Return a reversed copy
            #[inline]
            pub fn reversed(mut self) -> Self {
                self.reverse();
                self
            }
            
            /// XOR with another hash
            #[inline]
            pub fn xor(&self, other: &Self) -> Self {
                let mut result = [0u8; $size];
                for i in 0..$size {
                    result[i] = self.0[i] ^ other.0[i];
                }
                Self(result)
            }
            
            /// Count leading zeros
            #[inline]
            pub fn leading_zeros(&self) -> u32 {
                let mut count = 0u32;
                for byte in &self.0 {
                    if *byte == 0 {
                        count += 8;
                    } else {
                        count += byte.leading_zeros();
                        break;
                    }
                }
                count
            }
            
            /// Count trailing zeros
            #[inline]
            pub fn trailing_zeros(&self) -> u32 {
                let mut count = 0u32;
                for byte in self.0.iter().rev() {
                    if *byte == 0 {
                        count += 8;
                    } else {
                        count += byte.trailing_zeros();
                        break;
                    }
                }
                count
            }
            
            /// Create from a u64 (right-aligned, big-endian)
            pub fn from_u64(v: u64) -> Self {
                let mut result = Self::ZERO;
                let bytes = v.to_be_bytes();
                let start = $size.saturating_sub(8);
                let copy_len = 8.min($size);
                result.0[start..start + copy_len].copy_from_slice(&bytes[8 - copy_len..]);
                result
            }
        }
        
        // ─────────────────────────────────────────────────────────────────────
        // Trait Implementations
        // ─────────────────────────────────────────────────────────────────────
        
        impl PartialEq for $name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        
        impl Eq for $name {}
        
        impl PartialOrd for $name {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        
        impl Ord for $name {
            #[inline]
            fn cmp(&self, other: &Self) -> Ordering {
                self.0.cmp(&other.0)
            }
        }
        
        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        
        impl Deref for $name {
            type Target = [u8; $size];
            
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        
        impl DerefMut for $name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
        
        impl AsRef<[u8]> for $name {
            #[inline]
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }
        
        impl AsMut<[u8]> for $name {
            #[inline]
            fn as_mut(&mut self) -> &mut [u8] {
                &mut self.0
            }
        }
        
        impl From<[u8; $size]> for $name {
            #[inline]
            fn from(arr: [u8; $size]) -> Self {
                Self(arr)
            }
        }
        
        impl From<$name> for [u8; $size] {
            #[inline]
            fn from(hash: $name) -> Self {
                hash.0
            }
        }
        
        impl TryFrom<&[u8]> for $name {
            type Error = PrimitiveError;
            
            fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
                Self::from_slice(slice).ok_or(PrimitiveError::InvalidLength {
                    expected: $size,
                    got: slice.len(),
                })
            }
        }
        
        impl TryFrom<Vec<u8>> for $name {
            type Error = PrimitiveError;
            
            fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
                Self::try_from(vec.as_slice())
            }
        }
        
        impl Index<usize> for $name {
            type Output = u8;
            
            #[inline]
            fn index(&self, index: usize) -> &Self::Output {
                &self.0[index]
            }
        }
        
        impl IndexMut<usize> for $name {
            #[inline]
            fn index_mut(&mut self, index: usize) -> &mut Self::Output {
                &mut self.0[index]
            }
        }
        
        impl Index<core::ops::Range<usize>> for $name {
            type Output = [u8];
            
            #[inline]
            fn index(&self, index: core::ops::Range<usize>) -> &Self::Output {
                &self.0[index]
            }
        }
        
        impl BitXor for $name {
            type Output = Self;
            
            #[inline]
            fn bitxor(self, rhs: Self) -> Self::Output {
                self.xor(&rhs)
            }
        }
        
        impl BitAnd for $name {
            type Output = Self;
            
            fn bitand(self, rhs: Self) -> Self::Output {
                let mut result = [0u8; $size];
                for i in 0..$size {
                    result[i] = self.0[i] & rhs.0[i];
                }
                Self(result)
            }
        }
        
        impl BitOr for $name {
            type Output = Self;
            
            fn bitor(self, rhs: Self) -> Self::Output {
                let mut result = [0u8; $size];
                for i in 0..$size {
                    result[i] = self.0[i] | rhs.0[i];
                }
                Self(result)
            }
        }
        
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.to_hex())
            }
        }
        
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if $size <= 8 {
                    // Show full hex for small hashes
                    write!(f, "{}", self.to_hex())
                } else {
                    // Show abbreviated for large hashes
                    write!(f, "0x{}…{}", 
                        hex::encode(&self.0[..4]),
                        hex::encode(&self.0[$size - 4..])
                    )
                }
            }
        }
        
        impl fmt::LowerHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if f.alternate() {
                    write!(f, "0x")?;
                }
                for byte in &self.0 {
                    write!(f, "{:02x}", byte)?;
                }
                Ok(())
            }
        }
        
        impl fmt::UpperHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if f.alternate() {
                    write!(f, "0x")?;
                }
                for byte in &self.0 {
                    write!(f, "{:02X}", byte)?;
                }
                Ok(())
            }
        }
        
        impl FromStr for $name {
            type Err = PrimitiveError;
            
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_hex(s)
            }
        }
        
        // Serde support
        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&self.to_hex())
                } else {
                    serializer.serialize_bytes(&self.0)
                }
            }
        }
        
        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                if deserializer.is_human_readable() {
                    let s = <alloc::string::String as serde::Deserialize>::deserialize(deserializer)?;
                    Self::from_hex(&s).map_err(serde::de::Error::custom)
                } else {
                    let bytes = <[u8; $size] as serde::Deserialize>::deserialize(deserializer)?;
                    Ok(Self(bytes))
                }
            }
        }
        
        // Arbitrary support for fuzzing
        #[cfg(feature = "arbitrary")]
        impl<'a> arbitrary::Arbitrary<'a> for $name {
            fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
                let bytes: [u8; $size] = u.arbitrary()?;
                Ok(Self(bytes))
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Hash Type Definitions
// ─────────────────────────────────────────────────────────────────────────────

impl_hash!(H128, 16, "128-bit (16 byte) fixed-size hash type.");
impl_hash!(H160, 20, "160-bit (20 byte) fixed-size hash type. Used for addresses.");
impl_hash!(H256, 32, "256-bit (32 byte) fixed-size hash type. Primary hash type for blocks, transactions, etc.");
impl_hash!(H512, 64, "512-bit (64 byte) fixed-size hash type. Used for signatures.");
impl_hash!(H520, 65, "520-bit (65 byte) fixed-size hash type. Used for recoverable signatures.");

// ─────────────────────────────────────────────────────────────────────────────
// Additional H256 methods (most commonly used)
// ─────────────────────────────────────────────────────────────────────────────

impl H256 {
    /// Create from H160 (left-padded with zeros)
    pub fn from_h160(h: H160) -> Self {
        let mut result = Self::ZERO;
        result.0[12..32].copy_from_slice(&h.0);
        result
    }
    
    /// Extract H160 from last 20 bytes
    pub fn to_h160(&self) -> H160 {
        let mut result = H160::ZERO;
        result.0.copy_from_slice(&self.0[12..32]);
        result
    }
    
    /// Create from U256 (big-endian)
    pub fn from_u256(u: crate::U256) -> Self {
        Self(u.to_be_bytes())
    }
    
    /// Convert to U256
    pub fn to_u256(&self) -> crate::U256 {
        crate::U256::from_be_bytes(self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_h256_zero() {
        let h = H256::zero();
        assert!(h.is_zero());
        assert_eq!(h, H256::ZERO);
    }
    
    #[test]
    fn test_h256_from_hex() {
        let hex_str = "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let h = H256::from_hex(hex_str).unwrap();
        assert_eq!(h.0[0], 0x01);
        assert_eq!(h.0[31], 0x20);
        assert!(!h.is_zero());
    }
    
    #[test]
    fn test_h256_from_hex_no_prefix() {
        let hex_str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let h = H256::from_hex(hex_str).unwrap();
        assert_eq!(h.0[0], 0x01);
    }
    
    #[test]
    fn test_h256_from_hex_invalid() {
        assert!(H256::from_hex("0x123").is_err()); // Too short
        assert!(H256::from_hex("0xgg").is_err()); // Invalid char
    }
    
    #[test]
    fn test_h256_display() {
        let h = H256::from_hex("0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20").unwrap();
        let display = format!("{}", h);
        assert!(display.starts_with("0x01020304"));
        assert!(display.contains("…"));
        assert!(display.ends_with("1d1e1f20"));
    }
    
    #[test]
    fn test_h256_to_hex_roundtrip() {
        let original = "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let h = H256::from_hex(original).unwrap();
        let back = h.to_hex();
        assert_eq!(original, back);
    }
    
    #[test]
    fn test_h256_from_str() {
        let h: H256 = "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".parse().unwrap();
        assert_eq!(h.0[0], 0x01);
    }
    
    #[test]
    fn test_h256_xor() {
        let a = H256::from_hex("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
        let b = H256::from_hex("0x0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f").unwrap();
        let c = a.xor(&b);
        assert_eq!(c.0[0], 0xf0);
    }
    
    #[test]
    fn test_h256_leading_zeros() {
        let h = H256::ZERO;
        assert_eq!(h.leading_zeros(), 256);
        
        let h = H256::from_hex("0x0100000000000000000000000000000000000000000000000000000000000000").unwrap();
        assert_eq!(h.leading_zeros(), 7);
    }
    
    #[test]
    fn test_h256_ord() {
        let a = H256::from_hex("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let b = H256::from_hex("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        assert!(a < b);
    }
    
    #[test]
    fn test_h160_size() {
        assert_eq!(H160::SIZE, 20);
        let h = H160::ZERO;
        assert_eq!(h.as_bytes().len(), 20);
    }
    
    #[test]
    fn test_h256_from_u64() {
        let h = H256::from_u64(255);
        assert_eq!(h.0[31], 255);
        assert_eq!(h.0[30], 0);
    }
    
    #[cfg(feature = "serde")]
    #[test]
    fn test_h256_serde_json() {
        let original = H256::from_hex("0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains("0x0102030405060708"));
        
        let decoded: H256 = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }
    
    #[cfg(feature = "serde")]
    #[test]
    fn test_h256_serde_bincode() {
        let original = H256::from_hex("0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20").unwrap();
        let encoded = bincode::serialize(&original).unwrap();
        assert_eq!(encoded.len(), 32); // Binary format should be compact
        
        let decoded: H256 = bincode::deserialize(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}