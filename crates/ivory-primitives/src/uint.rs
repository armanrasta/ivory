// crates/ivory-primitives/src/uint.rs

//! Large unsigned integer types.
//!
//! Provides [`U128`], [`U256`], and [`U512`] for handling large numbers
//! commonly needed in blockchain applications (balances, gas, etc.).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::str::FromStr;
use core::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr, Not};
use core::cmp::Ordering;

use crate::error::PrimitiveError;

// ─────────────────────────────────────────────────────────────────────────────
// U256 Implementation
// ─────────────────────────────────────────────────────────────────────────────

/// 256-bit unsigned integer.
///
/// Stored as 4 x u64 in little-endian order (limbs[0] is least significant).
///
/// # Example
///
/// ```rust
/// use ivory_primitives::U256;
///
/// let a = U256::from(100u64);
/// let b = U256::from(200u64);
/// let c = a.checked_add(b).unwrap();
/// assert_eq!(c, U256::from(300u64));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U256(pub [u64; 4]);

impl U256 {
    /// Number of bits
    pub const BITS: usize = 256;
    
    /// Number of bytes
    pub const BYTES: usize = 32;
    
    /// Zero value
    pub const ZERO: Self = U256([0, 0, 0, 0]);
    
    /// One value
    pub const ONE: Self = U256([1, 0, 0, 0]);
    
    /// Maximum value (all bits set)
    pub const MAX: Self = U256([u64::MAX, u64::MAX, u64::MAX, u64::MAX]);
    
    /// Create zero value
    #[inline]
    pub const fn zero() -> Self {
        Self::ZERO
    }
    
    /// Create one value
    #[inline]
    pub const fn one() -> Self {
        Self::ONE
    }
    
    /// Create from u64
    #[inline]
    pub const fn from_u64(v: u64) -> Self {
        U256([v, 0, 0, 0])
    }
    
    /// Create from u128
    #[inline]
    pub const fn from_u128(v: u128) -> Self {
        U256([v as u64, (v >> 64) as u64, 0, 0])
    }
    
    /// Create from big-endian bytes
    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for i in 0..4 {
            let offset = (3 - i) * 8;
            limbs[i] = u64::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
        }
        U256(limbs)
    }
    
    /// Create from little-endian bytes
    pub fn from_le_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for i in 0..4 {
            let offset = i * 8;
            limbs[i] = u64::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
        }
        U256(limbs)
    }
    
    /// Create from hex string
    pub fn from_hex(s: &str) -> Result<Self, PrimitiveError> {
        let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
        
        if s.is_empty() {
            return Ok(Self::ZERO);
        }
        
        if s.len() > 64 {
            return Err(PrimitiveError::Overflow);
        }
        
        // Pad to 64 characters
        let padded = format!("{:0>64}", s);
        let bytes = hex::decode(&padded).map_err(|_| PrimitiveError::InvalidHex)?;
        
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_be_bytes(arr))
    }
    
    /// Convert to big-endian bytes
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for i in 0..4 {
            let limb_bytes = self.0[3 - i].to_be_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
        }
        bytes
    }
    
    /// Convert to little-endian bytes
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for i in 0..4 {
            let limb_bytes = self.0[i].to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
        }
        bytes
    }
    
    /// Convert to hex string with 0x prefix
    pub fn to_hex(&self) -> String {
        if self.is_zero() {
            return alloc::string::String::from("0x0");
        }
        
        let bytes = self.to_be_bytes();
        let hex = hex::encode(bytes);
        let trimmed = hex.trim_start_matches('0');
        
        if trimmed.is_empty() {
            alloc::string::String::from("0x0")
        } else {
            alloc::format!("0x{}", trimmed)
        }
    }
    
    /// Get low 64 bits
    #[inline]
    pub const fn low_u64(&self) -> u64 {
        self.0[0]
    }
    
    /// Get low 128 bits
    #[inline]
    pub const fn low_u128(&self) -> u128 {
        (self.0[1] as u128) << 64 | self.0[0] as u128
    }
    
    /// Check if zero
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }
    
    /// Count leading zeros
    pub fn leading_zeros(&self) -> u32 {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return (3 - i as u32) * 64 + self.0[i].leading_zeros();
            }
        }
        256
    }
    
    /// Count trailing zeros
    pub fn trailing_zeros(&self) -> u32 {
        for i in 0..4 {
            if self.0[i] != 0 {
                return i as u32 * 64 + self.0[i].trailing_zeros();
            }
        }
        256
    }
    
    /// Get number of bits needed to represent this value
    pub fn bits(&self) -> u32 {
        256 - self.leading_zeros()
    }
    
    /// Checked addition
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let mut result = [0u64; 4];
        let mut carry = 0u64;
        
        for i in 0..4 {
            let (sum1, c1) = self.0[i].overflowing_add(rhs.0[i]);
            let (sum2, c2) = sum1.overflowing_add(carry);
            result[i] = sum2;
            carry = u64::from(c1) + u64::from(c2);
        }
        
        if carry > 0 {
            None
        } else {
            Some(U256(result))
        }
    }
    
    /// Checked subtraction
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        if self < rhs {
            return None;
        }
        
        let mut result = [0u64; 4];
        let mut borrow = 0u64;
        
        for i in 0..4 {
            let (diff1, b1) = self.0[i].overflowing_sub(rhs.0[i]);
            let (diff2, b2) = diff1.overflowing_sub(borrow);
            result[i] = diff2;
            borrow = u64::from(b1) + u64::from(b2);
        }
        
        Some(U256(result))
    }
    
    /// Checked multiplication
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // Check for simple cases
        if self.is_zero() || rhs.is_zero() {
            return Some(Self::ZERO);
        }
        
        if self == Self::ONE {
            return Some(rhs);
        }
        
        if rhs == Self::ONE {
            return Some(self);
        }
        
        // Check if result would overflow
        let self_bits = self.bits();
        let rhs_bits = rhs.bits();
        if self_bits + rhs_bits > 257 {
            return None;
        }
        
        // Perform multiplication
        let mut result = [0u64; 4];
        
        for i in 0..4 {
            if self.0[i] == 0 {
                continue;
            }
            
            let mut carry = 0u128;
            
            for j in 0..4 {
                if i + j >= 4 {
                    if self.0[i] as u128 * rhs.0[j] as u128 + carry > 0 {
                        return None; // Overflow
                    }
                    break;
                }
                
                let product = self.0[i] as u128 * rhs.0[j] as u128 + result[i + j] as u128 + carry;
                result[i + j] = product as u64;
                carry = product >> 64;
            }
            
            if carry > 0 && i + 4 <= 4 {
                return None; // Overflow
            }
        }
        
        Some(U256(result))
    }
    
    /// Checked division
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        
        if self.is_zero() {
            return Some(Self::ZERO);
        }
        
        if self < rhs {
            return Some(Self::ZERO);
        }
        
        if rhs == Self::ONE {
            return Some(self);
        }
        
        // Simple long division for now
        // TODO: Optimize with better algorithm
        let mut quotient = Self::ZERO;
        let mut remainder = self;
        
        let shift = rhs.leading_zeros() - self.leading_zeros();
        let mut divisor = rhs << shift;
        
        for i in (0..=shift).rev() {
            if remainder >= divisor {
                remainder = remainder.checked_sub(divisor)?;
                quotient = quotient | (Self::ONE << i);
            }
            divisor = divisor >> 1;
        }
        
        Some(quotient)
    }
    
    /// Checked remainder
    pub fn checked_rem(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        
        let quotient = self.checked_div(rhs)?;
        let product = quotient.checked_mul(rhs)?;
        self.checked_sub(product)
    }
    
    /// Saturating addition
    #[inline]
    pub fn saturating_add(self, rhs: Self) -> Self {
        self.checked_add(rhs).unwrap_or(Self::MAX)
    }
    
    /// Saturating subtraction
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        self.checked_sub(rhs).unwrap_or(Self::ZERO)
    }
    
    /// Saturating multiplication
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs).unwrap_or(Self::MAX)
    }
    
    /// Wrapping addition
    pub fn wrapping_add(self, rhs: Self) -> Self {
        let mut result = [0u64; 4];
        let mut carry = 0u64;
        
        for i in 0..4 {
            let (sum1, c1) = self.0[i].overflowing_add(rhs.0[i]);
            let (sum2, c2) = sum1.overflowing_add(carry);
            result[i] = sum2;
            carry = u64::from(c1) + u64::from(c2);
        }
        
        U256(result)
    }
    
    /// Wrapping subtraction
    pub fn wrapping_sub(self, rhs: Self) -> Self {
        let mut result = [0u64; 4];
        let mut borrow = 0u64;
        
        for i in 0..4 {
            let (diff1, b1) = self.0[i].overflowing_sub(rhs.0[i]);
            let (diff2, b2) = diff1.overflowing_sub(borrow);
            result[i] = diff2;
            borrow = u64::from(b1) + u64::from(b2);
        }
        
        U256(result)
    }
    
    /// Exponentiation
    pub fn checked_pow(self, exp: u32) -> Option<Self> {
        if exp == 0 {
            return Some(Self::ONE);
        }
        
        let mut base = self;
        let mut exp = exp;
        let mut result = Self::ONE;
        
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.checked_mul(base)?;
            }
            exp >>= 1;
            if exp > 0 {
                base = base.checked_mul(base)?;
            }
        }
        
        Some(result)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Comparison
// ─────────────────────────────────────────────────────────────────────────────

impl PartialOrd for U256 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare from most significant to least significant
        for i in (0..4).rev() {
            match self.0[i].cmp(&other.0[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bit Operations
// ─────────────────────────────────────────────────────────────────────────────

impl BitAnd for U256 {
    type Output = Self;
    
    fn bitand(self, rhs: Self) -> Self::Output {
        U256([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl BitOr for U256 {
    type Output = Self;
    
    fn bitor(self, rhs: Self) -> Self::Output {
        U256([
            self.0[0] | rhs.0[0],
            self.0[1] | rhs.0[1],
            self.0[2] | rhs.0[2],
            self.0[3] | rhs.0[3],
        ])
    }
}

impl BitXor for U256 {
    type Output = Self;
    
    fn bitxor(self, rhs: Self) -> Self::Output {
        U256([
            self.0[0] ^ rhs.0[0],
            self.0[1] ^ rhs.0[1],
            self.0[2] ^ rhs.0[2],
            self.0[3] ^ rhs.0[3],
        ])
    }
}

impl Not for U256 {
    type Output = Self;
    
    fn not(self) -> Self::Output {
        U256([
            !self.0[0],
            !self.0[1],
            !self.0[2],
            !self.0[3],
        ])
    }
}

impl Shl<u32> for U256 {
    type Output = Self;
    
    fn shl(self, shift: u32) -> Self::Output {
        if shift >= 256 {
            return Self::ZERO;
        }
        
        if shift == 0 {
            return self;
        }
        
        let limb_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        
        let mut result = [0u64; 4];
        
        if bit_shift == 0 {
            for i in limb_shift..4 {
                result[i] = self.0[i - limb_shift];
            }
        } else {
            for i in limb_shift..4 {
                result[i] = self.0[i - limb_shift] << bit_shift;
                if i > limb_shift {
                    result[i] |= self.0[i - limb_shift - 1] >> (64 - bit_shift);
                }
            }
        }
        
        U256(result)
    }
}

impl Shr<u32> for U256 {
    type Output = Self;
    
    fn shr(self, shift: u32) -> Self::Output {
        if shift >= 256 {
            return Self::ZERO;
        }
        
        if shift == 0 {
            return self;
        }
        
        let limb_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        
        let mut result = [0u64; 4];
        
        if bit_shift == 0 {
            for i in 0..(4 - limb_shift) {
                result[i] = self.0[i + limb_shift];
            }
        } else {
            for i in 0..(4 - limb_shift) {
                result[i] = self.0[i + limb_shift] >> bit_shift;
                if i + limb_shift + 1 < 4 {
                    result[i] |= self.0[i + limb_shift + 1] << (64 - bit_shift);
                }
            }
        }
        
        U256(result)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From implementations
// ─────────────────────────────────────────────────────────────────────────────

impl From<u8> for U256 {
    #[inline]
    fn from(v: u8) -> Self {
        Self::from_u64(v as u64)
    }
}

impl From<u16> for U256 {
    #[inline]
    fn from(v: u16) -> Self {
        Self::from_u64(v as u64)
    }
}

impl From<u32> for U256 {
    #[inline]
    fn from(v: u32) -> Self {
        Self::from_u64(v as u64)
    }
}

impl From<u64> for U256 {
    #[inline]
    fn from(v: u64) -> Self {
        Self::from_u64(v)
    }
}

impl From<u128> for U256 {
    #[inline]
    fn from(v: u128) -> Self {
        Self::from_u128(v)
    }
}

impl From<bool> for U256 {
    #[inline]
    fn from(v: bool) -> Self {
        if v { Self::ONE } else { Self::ZERO }
    }
}

impl TryFrom<U256> for u64 {
    type Error = PrimitiveError;
    
    fn try_from(value: U256) -> Result<Self, Self::Error> {
        if value.0[1] != 0 || value.0[2] != 0 || value.0[3] != 0 {
            Err(PrimitiveError::Overflow)
        } else {
            Ok(value.0[0])
        }
    }
}

impl TryFrom<U256> for u128 {
    type Error = PrimitiveError;
    
    fn try_from(value: U256) -> Result<Self, Self::Error> {
        if value.0[2] != 0 || value.0[3] != 0 {
            Err(PrimitiveError::Overflow)
        } else {
            Ok(value.low_u128())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Display and Debug
// ─────────────────────────────────────────────────────────────────────────────

impl fmt::Debug for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U256({})", self.to_hex())
    }
}

impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = self.to_hex();
        let hex = hex.strip_prefix("0x").unwrap_or(&hex);
        if f.alternate() {
            write!(f, "0x{}", hex)
        } else {
            write!(f, "{}", hex)
        }
    }
}

impl FromStr for U256 {
    type Err = PrimitiveError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Serde
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "serde")]
impl serde::Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_hex())
        } else {
            let bytes = self.to_be_bytes();
            serializer.serialize_bytes(&bytes)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = <alloc::string::String as serde::Deserialize>::deserialize(deserializer)?;
            Self::from_hex(&s).map_err(serde::de::Error::custom)
        } else {
            let bytes = <[u8; 32] as serde::Deserialize>::deserialize(deserializer)?;
            Ok(Self::from_be_bytes(bytes))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// U128 (simplified, delegates to u128)
// ─────────────────────────────────────────────────────────────────────────────

/// 128-bit unsigned integer wrapper.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct U128(pub u128);

impl U128 {
    pub const ZERO: Self = U128(0);
    pub const ONE: Self = U128(1);
    pub const MAX: Self = U128(u128::MAX);
    
    #[inline]
    pub const fn from_u64(v: u64) -> Self {
        U128(v as u128)
    }
    
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl From<u64> for U128 {
    #[inline]
    fn from(v: u64) -> Self {
        U128(v as u128)
    }
}

impl From<u128> for U128 {
    #[inline]
    fn from(v: u128) -> Self {
        U128(v)
    }
}

impl fmt::Debug for U128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U128({:#x})", self.0)
    }
}

impl fmt::Display for U128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// U512 (for extended operations)
// ─────────────────────────────────────────────────────────────────────────────

/// 512-bit unsigned integer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U512(pub [u64; 8]);

impl U512 {
    pub const ZERO: Self = U512([0; 8]);
    pub const ONE: Self = U512([1, 0, 0, 0, 0, 0, 0, 0]);
    
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
            && self.0[4] == 0 && self.0[5] == 0 && self.0[6] == 0 && self.0[7] == 0
    }
    
    /// Convert to U256 (takes low 256 bits)
    pub fn to_u256(&self) -> U256 {
        U256([self.0[0], self.0[1], self.0[2], self.0[3]])
    }
    
    /// Create from U256
    pub fn from_u256(v: U256) -> Self {
        U512([v.0[0], v.0[1], v.0[2], v.0[3], 0, 0, 0, 0])
    }
}

impl From<U256> for U512 {
    fn from(v: U256) -> Self {
        Self::from_u256(v)
    }
}

impl fmt::Debug for U512 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U512([{:?}, {:?}])", 
            U256([self.0[0], self.0[1], self.0[2], self.0[3]]),
            U256([self.0[4], self.0[5], self.0[6], self.0[7]]))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_u256_zero() {
        let u = U256::zero();
        assert!(u.is_zero());
        assert_eq!(u, U256::ZERO);
    }
    
    #[test]
    fn test_u256_one() {
        let u = U256::one();
        assert!(!u.is_zero());
        assert_eq!(u.low_u64(), 1);
    }
    
    #[test]
    fn test_u256_from_u64() {
        let u = U256::from(12345u64);
        assert_eq!(u.low_u64(), 12345);
    }
    
    #[test]
    fn test_u256_from_u128() {
        let v: u128 = 1 << 100;
        let u = U256::from(v);
        assert_eq!(u.low_u128(), v);
    }
    
    #[test]
    fn test_u256_add() {
        let a = U256::from(100u64);
        let b = U256::from(200u64);
        let c = a.checked_add(b).unwrap();
        assert_eq!(c.low_u64(), 300);
    }
    
    #[test]
    fn test_u256_add_overflow() {
        let a = U256::MAX;
        let b = U256::ONE;
        assert!(a.checked_add(b).is_none());
    }
    
    #[test]
    fn test_u256_sub() {
        let a = U256::from(300u64);
        let b = U256::from(100u64);
        let c = a.checked_sub(b).unwrap();
        assert_eq!(c.low_u64(), 200);
    }
    
    #[test]
    fn test_u256_sub_underflow() {
        let a = U256::from(100u64);
        let b = U256::from(200u64);
        assert!(a.checked_sub(b).is_none());
    }
    
    #[test]
    fn test_u256_mul() {
        let a = U256::from(100u64);
        let b = U256::from(200u64);
        let c = a.checked_mul(b).unwrap();
        assert_eq!(c.low_u64(), 20000);
    }
    
    #[test]
    fn test_u256_mul_overflow() {
        let a = U256::MAX;
        let b = U256::from(2u64);
        assert!(a.checked_mul(b).is_none());
    }
    
    #[test]
    fn test_u256_div() {
        let a = U256::from(1000u64);
        let b = U256::from(3u64);
        let c = a.checked_div(b).unwrap();
        assert_eq!(c.low_u64(), 333);
    }
    
    #[test]
    fn test_u256_div_by_zero() {
        let a = U256::from(100u64);
        let b = U256::ZERO;
        assert!(a.checked_div(b).is_none());
    }
    
    #[test]
    fn test_u256_rem() {
        let a = U256::from(1000u64);
        let b = U256::from(3u64);
        let c = a.checked_rem(b).unwrap();
        assert_eq!(c.low_u64(), 1);
    }
    
    #[test]
    fn test_u256_ord() {
        let a = U256::from(100u64);
        let b = U256::from(200u64);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
    }
    
    #[test]
    fn test_u256_shift_left() {
        let a = U256::ONE;
        let b = a << 64;
        assert_eq!(b.0[0], 0);
        assert_eq!(b.0[1], 1);
    }
    
    #[test]
    fn test_u256_shift_right() {
        let mut a = U256::ZERO;
        a.0[1] = 1;
        let b = a >> 64;
        assert_eq!(b.0[0], 1);
        assert_eq!(b.0[1], 0);
    }
    
    #[test]
    fn test_u256_from_hex() {
        let u = U256::from_hex("0x100").unwrap();
        assert_eq!(u.low_u64(), 256);
        
        let u = U256::from_hex("0xff").unwrap();
        assert_eq!(u.low_u64(), 255);
    }
    
    #[test]
    fn test_u256_to_hex() {
        let u = U256::from(255u64);
        assert_eq!(u.to_hex(), "0xff");
        
        let u = U256::ZERO;
        assert_eq!(u.to_hex(), "0x0");
    }
    
    #[test]
    fn test_u256_bytes_roundtrip() {
        let original = U256::from_hex("0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20").unwrap();
        let bytes = original.to_be_bytes();
        let decoded = U256::from_be_bytes(bytes);
        assert_eq!(original, decoded);
    }
    
    #[test]
    fn test_u256_bits() {
        assert_eq!(U256::ZERO.bits(), 0);
        assert_eq!(U256::ONE.bits(), 1);
        assert_eq!((U256::ONE << 255).bits(), 256);
    }
}   
