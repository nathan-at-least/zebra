//! Block difficulty data structures and calculations
//!
//! The block difficulty "target threshold" is stored in the block header as a
//! 32-bit `CompactDifficulty`. The `block::Hash` must be less than or equal
//! to the `ExpandedDifficulty` threshold, when represented as a 256-bit integer
//! in little-endian order.
//!
//! The target threshold is also used to calculate the `Work` for each block.
//! The block work is used to find the chain with the greatest total work. Each
//! block's work value depends on the fixed threshold in the block header, not
//! the actual work represented by the block header hash.
#![allow(clippy::unit_arg)]

use crate::block;

use std::cmp::{Ordering, PartialEq, PartialOrd};
use std::{fmt, ops::Add, ops::AddAssign};

use primitive_types::U256;

#[cfg(test)]
use proptest_derive::Arbitrary;
#[cfg(test)]
mod tests;

/// A 32-bit "compact bits" value, which represents the difficulty threshold for
/// a block header.
///
/// Used for:
///   - checking the `difficulty_threshold` value in the block header,
///   - calculating the 256-bit `ExpandedDifficulty` threshold, for comparison
///     with the block header hash, and
///   - calculating the block work.
///
/// Details:
///
/// This is a floating-point encoding, with a 24-bit signed mantissa,
/// an 8-bit exponent, an offset of 3, and a radix of 256.
/// (IEEE 754 32-bit floating-point values use a separate sign bit, an implicit
/// leading mantissa bit, an offset of 127, and a radix of 2.)
///
/// The precise bit pattern of a `CompactDifficulty` value is
/// consensus-critical, because it is used for the `difficulty_threshold` field,
/// which is:
///   - part of the `BlockHeader`, which is used to create the
///     `block::Hash`, and
///   - bitwise equal to the median `ExpandedDifficulty` value of recent blocks,
///     when encoded to `CompactDifficulty` using the specified conversion
///     function.
///
/// Without these consensus rules, some `ExpandedDifficulty` values would have
/// multiple equivalent `CompactDifficulty` values, due to redundancy in the
/// floating-point format.
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct CompactDifficulty(pub u32);

impl fmt::Debug for CompactDifficulty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("CompactDifficulty")
            // Use hex, because it's a float
            .field(&format_args!("{:#010x}", self.0))
            .finish()
    }
}

/// A 256-bit unsigned "expanded difficulty" value.
///
/// Used as a target threshold for the difficulty of a `block::Hash`.
///
/// Details:
///
/// The precise bit pattern of an `ExpandedDifficulty` value is
/// consensus-critical, because it is compared with the `block::Hash`.
///
/// Note that each `CompactDifficulty` value represents a range of
/// `ExpandedDifficulty` values, because the precision of the
/// floating-point format requires rounding on conversion.
///
/// Therefore, consensus-critical code must perform the specified
/// conversions to `CompactDifficulty`, even if the original
/// `ExpandedDifficulty` values are known.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct ExpandedDifficulty(U256);

impl fmt::Debug for ExpandedDifficulty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0; 32];
        // Use the same byte order as block::Hash
        self.0.to_little_endian(&mut buf);
        f.debug_tuple("ExpandedDifficulty")
            .field(&hex::encode(&buf))
            .finish()
    }
}

/// A 128-bit unsigned "Work" value.
///
/// Used to calculate the total work for each chain of blocks.
///
/// Details:
///
/// The relative value of `Work` is consensus-critical, because it is used to
/// choose the best chain. But its precise value and bit pattern are not
/// consensus-critical.
///
/// We calculate work values according to the Zcash specification, but store
/// them as u128, rather than the implied u256. We don't expect the total chain
/// work to ever exceed 2^128. The current total chain work for Zcash is 2^58,
/// and Bitcoin adds around 2^91 work per year. (Each extra bit represents twice
/// as much work.)
#[derive(Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Work(u128);

impl fmt::Debug for Work {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // There isn't a standard way to represent alternate formats for the
        // same value.
        f.debug_tuple("Work")
            // Use hex, because expanded difficulty is in hex.
            .field(&format_args!("{:#x}", self.0))
            // Use decimal, to compare with zcashd
            .field(&format_args!("{}", self.0))
            // Use log2, to compare with zcashd
            .field(&format_args!("{:.5}", (self.0 as f64).log2()))
            .finish()
    }
}

impl CompactDifficulty {
    /// CompactDifficulty exponent base.
    const BASE: u32 = 256;

    /// CompactDifficulty exponent offset.
    const OFFSET: i32 = 3;

    /// CompactDifficulty floating-point precision.
    const PRECISION: u32 = 24;

    /// CompactDifficulty sign bit, part of the signed mantissa.
    const SIGN_BIT: u32 = 1 << (CompactDifficulty::PRECISION - 1);

    /// CompactDifficulty unsigned mantissa mask.
    ///
    /// Also the maximum unsigned mantissa value.
    const UNSIGNED_MANTISSA_MASK: u32 = CompactDifficulty::SIGN_BIT - 1;

    /// Calculate the ExpandedDifficulty for a compact representation.
    ///
    /// See `ToTarget()` in the Zcash Specification, and `CheckProofOfWork()` in
    /// zcashd.
    ///
    /// Returns None for negative, zero, and overflow values. (zcashd rejects
    /// these values, before comparing the hash.)
    pub fn to_expanded(&self) -> Option<ExpandedDifficulty> {
        // The constants for this floating-point representation.
        // Alias the struct constants here, so the code is easier to read.
        const BASE: u32 = CompactDifficulty::BASE;
        const OFFSET: i32 = CompactDifficulty::OFFSET;
        const PRECISION: u32 = CompactDifficulty::PRECISION;
        const SIGN_BIT: u32 = CompactDifficulty::SIGN_BIT;
        const UNSIGNED_MANTISSA_MASK: u32 = CompactDifficulty::UNSIGNED_MANTISSA_MASK;

        // Negative values in this floating-point representation.
        // 0 if (x & 2^23 == 2^23)
        // zcashd rejects negative values without comparing the hash.
        if self.0 & SIGN_BIT == SIGN_BIT {
            return None;
        }

        // The components of the result
        // The fractional part of the floating-point number
        // x & (2^23 - 1)
        let mantissa = self.0 & UNSIGNED_MANTISSA_MASK;

        // The exponent for the multiplier in the floating-point number
        // 256^(floor(x/(2^24)) - 3)
        // The i32 conversion is safe, because we've just divided self by 2^24.
        let exponent = ((self.0 >> PRECISION) as i32) - OFFSET;

        // Normalise the mantissa and exponent before multiplying.
        //
        // zcashd rejects non-zero overflow values, but accepts overflows where
        // all the overflowing bits are zero. It also allows underflows.
        let (mantissa, exponent) = match (mantissa, exponent) {
            // Overflow: check for non-zero overflow bits
            //
            // If m is non-zero, overflow. If m is zero, invalid.
            (_, e) if (e >= 32) => return None,
            // If m is larger than the remaining bytes, overflow.
            // Otherwise, avoid overflows in base^exponent.
            (m, e) if (e == 31 && m > u8::MAX.into()) => return None,
            (m, e) if (e == 31 && m <= u8::MAX.into()) => (m << 16, e - 2),
            (m, e) if (e == 30 && m > u16::MAX.into()) => return None,
            (m, e) if (e == 30 && m <= u16::MAX.into()) => (m << 8, e - 1),

            // Underflow: perform the right shift.
            // The abs is safe, because we've just divided by 2^24, and offset
            // is small.
            (m, e) if (e < 0) => (m >> ((e.abs() * 8) as u32), 0),
            (m, e) => (m, e),
        };

        // Now calculate the result: mantissa*base^exponent
        // Earlier code should make sure all these values are in range.
        let mantissa: U256 = mantissa.into();
        let base: U256 = BASE.into();
        let exponent: U256 = exponent.into();
        let result = mantissa * base.pow(exponent);

        if result == U256::zero() {
            // zcashd rejects zero values, without comparing the hash
            None
        } else {
            Some(ExpandedDifficulty(result))
        }
    }

    /// Calculate the Work for a compact representation.
    ///
    /// See `Definition of Work` in the [Zcash Specification], and
    /// `GetBlockProof()` in zcashd.
    ///
    /// Returns None if the corresponding ExpandedDifficulty is None.
    /// Also returns None on Work overflow, which should be impossible on a
    /// valid chain.
    ///
    /// [Zcash Specification]: https://zips.z.cash/protocol/canopy.pdf#workdef
    pub fn to_work(&self) -> Option<Work> {
        let expanded = self.to_expanded()?;

        // We need to compute `2^256 / (expanded + 1)`, but we can't represent
        // 2^256, as it's too large for a u256. However, as 2^256 is at least as
        // large as `expanded + 1`, it is equal to
        // `((2^256 - expanded - 1) / (expanded + 1)) + 1`, or
        let result = (!expanded.0 / (expanded.0 + 1)) + 1;
        if result <= u128::MAX.into() {
            return Some(Work(result.as_u128()));
        }

        None
    }
}

impl ExpandedDifficulty {
    /// Returns the difficulty of the hash.
    ///
    /// Used to implement comparisons between difficulties and hashes.
    ///
    /// Usage:
    ///
    /// Compare the hash with the calculated difficulty value, using Rust's
    /// standard comparison operators.
    ///
    /// Hashes are not used to calculate the difficulties of future blocks, so
    /// users of this module should avoid converting hashes into difficulties.
    fn from_hash(hash: &block::Hash) -> ExpandedDifficulty {
        ExpandedDifficulty(U256::from_little_endian(&hash.0))
    }
}

impl PartialEq<block::Hash> for ExpandedDifficulty {
    /// Is `self` equal to `other`?
    ///
    /// See `partial_cmp` for details.
    fn eq(&self, other: &block::Hash) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd<block::Hash> for ExpandedDifficulty {
    /// `block::Hash`es are compared with `ExpandedDifficulty` thresholds by
    /// converting the hash to a 256-bit integer in little-endian order.
    fn partial_cmp(&self, other: &block::Hash) -> Option<Ordering> {
        self.partial_cmp(&ExpandedDifficulty::from_hash(other))
    }
}

impl PartialEq<ExpandedDifficulty> for block::Hash {
    /// Is `self` equal to `other`?
    ///
    /// See `partial_cmp` for details.
    fn eq(&self, other: &ExpandedDifficulty) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<ExpandedDifficulty> for block::Hash {
    /// `block::Hash`es are compared with `ExpandedDifficulty` thresholds by
    /// converting the hash to a 256-bit integer in little-endian order.
    fn partial_cmp(&self, other: &ExpandedDifficulty) -> Option<Ordering> {
        use Ordering::*;

        // Use the base implementation, but reverse the order.
        match other.partial_cmp(self) {
            Some(Less) => Some(Greater),
            Some(Greater) => Some(Less),
            Some(Equal) => Some(Equal),
            None => unreachable!(
                "Unexpected incomparable values: difficulties and hashes have a total order."
            ),
        }
    }
}

impl Add for Work {
    type Output = Self;

    fn add(self, rhs: Work) -> Self {
        let result = self
            .0
            .checked_add(rhs.0)
            .expect("Work values do not overflow");
        Work(result)
    }
}

impl AddAssign for Work {
    fn add_assign(&mut self, rhs: Work) {
        *self = *self + rhs;
    }
}
