//! Byte-position primitives: [`BytePos`] and [`ByteRange`].
//!
//! [`BytePos`] is a `u64` byte offset from the start of a snapshot; [`ByteRange`]
//! is a half-open `[start, end)` pair of positions. Both are `Copy`, comparable
//! and hashable, and derive `serde::Serialize` under the `serde` feature.

use std::fmt;
use std::io::{self, Error, ErrorKind};

#[cfg(feature = "serde")]
use serde::Serialize;

/// A byte offset into source text, stored as a `u64`.
#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytePos(u64);

impl BytePos {
    /// The zero byte offset.
    pub const ZERO: Self = BytePos(0);

    /// Construct a [`BytePos`] from a `u64` offset.
    #[inline]
    pub const fn new(n: u64) -> Self {
        BytePos(n)
    }

    /// Return the raw `u64` offset.
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self.0
    }

    /// Collapse this position into an empty [`ByteRange`] at the same offset.
    pub fn as_range(self) -> ByteRange {
        ByteRange {
            start: self,
            end: self,
        }
    }
}

impl fmt::Debug for BytePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for BytePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add<BytePos> for BytePos {
    type Output = BytePos;
    #[inline]
    fn add(self, rhs: BytePos) -> BytePos {
        BytePos(self.0 + rhs.0)
    }
}

impl std::ops::Sub<BytePos> for BytePos {
    type Output = BytePos;
    #[inline]
    fn sub(self, rhs: BytePos) -> BytePos {
        BytePos(self.0 - rhs.0)
    }
}

/// A half-open byte range: `[start, end)`.
#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    start: BytePos,
    end: BytePos,
}

impl ByteRange {
    /// Construct a range from a start and end position.
    ///
    /// Returns an [`io::Error`] if `start` is greater than `end`.
    #[inline]
    pub fn new(start: BytePos, end: BytePos) -> io::Result<Self> {
        if start > end {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ByteRange: start > end",
            ));
        }
        Ok(Self { start, end })
    }

    /// Start of the range in bytes.
    #[inline]
    pub fn start(&self) -> BytePos {
        self.start
    }

    /// End of the range in bytes.
    #[inline]
    pub fn end(&self) -> BytePos {
        self.end
    }

    /// Length of the range in bytes.
    #[inline]
    pub fn len(&self) -> BytePos {
        self.end - self.start
    }

    /// If this is an empty range.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The empty range at offset zero.
    pub const NULL: Self = ByteRange {
        start: BytePos::ZERO,
        end: BytePos::ZERO,
    };
}

impl fmt::Debug for ByteRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_of<T: std::hash::Hash>(v: &T) -> u64 {
        use std::hash::{DefaultHasher, Hasher};
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    // --- BytePos ---

    #[test]
    fn byte_pos_new_and_to_u64_round_trip() {
        for n in [0u64, 1, 42, u64::MAX - 1] {
            assert_eq!(BytePos::new(n).to_u64(), n);
        }
    }

    #[test]
    fn byte_pos_zero_default_and_equality() {
        assert_eq!(BytePos::ZERO.to_u64(), 0);
        assert_eq!(BytePos::default(), BytePos::ZERO);
        assert_eq!(BytePos::new(0), BytePos::ZERO);
        assert_ne!(BytePos::new(1), BytePos::ZERO);
    }

    #[test]
    fn byte_pos_total_order_matches_the_u64_value() {
        use std::cmp::Ordering;
        let lo = BytePos::new(10);
        let hi = BytePos::new(20);
        assert_eq!(lo.cmp(&hi), Ordering::Less);
        assert_eq!(hi.cmp(&lo), Ordering::Greater);
        assert_eq!(lo.cmp(&BytePos::new(10)), Ordering::Equal);
        assert!(lo < hi);
        assert!(hi > lo);
        assert!(lo <= BytePos::new(10));
        assert!(hi >= BytePos::new(20));
        assert_eq!(lo.min(hi), lo);
        assert_eq!(lo.max(hi), hi);
    }

    #[test]
    fn byte_pos_hash_and_eq_agree() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BytePos::new(5));
        set.insert(BytePos::new(5)); // duplicate: same hash + eq
        set.insert(BytePos::new(6));
        assert_eq!(set.len(), 2);
        assert!(set.contains(&BytePos::new(5)));
        assert!(!set.contains(&BytePos::new(7)));
        assert_eq!(hash_of(&BytePos::new(9)), hash_of(&BytePos::new(9)));
    }

    #[test]
    fn byte_pos_display_and_debug_print_the_offset() {
        let p = BytePos::new(123_456);
        assert_eq!(p.to_string(), "123456");
        assert_eq!(format!("{p:?}"), "123456");
    }

    #[test]
    fn byte_pos_addition_and_subtraction_compose() {
        let base = BytePos::new(100);
        let delta = BytePos::new(30);
        assert_eq!((base + delta).to_u64(), 130);
        assert_eq!((base - delta).to_u64(), 70);
        assert_eq!(((base + delta) - delta), base);
        assert_eq!((base + BytePos::ZERO), base);
        assert_eq!((base - BytePos::ZERO), base);
    }

    #[test]
    #[should_panic(expected = "attempt to add with overflow")]
    fn byte_pos_addition_overflows() {
        let _ = BytePos::new(u64::MAX) + BytePos::new(1);
    }

    #[test]
    #[should_panic(expected = "attempt to subtract with overflow")]
    fn byte_pos_subtraction_underflows() {
        let _ = BytePos::new(0) - BytePos::new(1);
    }

    #[test]
    fn byte_pos_as_range_is_empty_at_the_same_offset() {
        let at = BytePos::new(999);
        let r = at.as_range();
        assert_eq!(r.start(), at);
        assert_eq!(r.end(), at);
        assert!(r.is_empty());
        assert_eq!(r.len(), BytePos::ZERO);
    }

    // --- ByteRange ---

    #[test]
    fn byte_range_exposes_bounds_and_len_for_a_valid_range() {
        let r = ByteRange::new(BytePos::new(10), BytePos::new(25)).unwrap();
        assert_eq!(r.start().to_u64(), 10);
        assert_eq!(r.end().to_u64(), 25);
        assert_eq!(r.len().to_u64(), 15);
        assert!(!r.is_empty());
    }

    #[test]
    fn byte_range_zero_length_is_empty_and_spans_a_single_point() {
        let at = BytePos::new(50);
        let r = ByteRange::new(at, at).unwrap();
        assert!(r.is_empty());
        assert_eq!(r.len(), BytePos::ZERO);
        assert_eq!(r.start(), at);
        assert_eq!(r.end(), at);
    }

    #[test]
    fn byte_range_rejects_reversed_bounds_with_invalid_input() {
        let err = ByteRange::new(BytePos::new(10), BytePos::new(5)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(err.to_string(), "ByteRange: start > end");
    }

    #[test]
    fn byte_range_null_is_the_empty_zero_range() {
        let r = ByteRange::NULL;
        assert_eq!(r.start(), BytePos::ZERO);
        assert_eq!(r.end(), BytePos::ZERO);
        assert!(r.is_empty());
        assert_eq!(r.len(), BytePos::ZERO);
    }

    #[test]
    fn byte_range_survives_the_full_u64_space_edge() {
        // A half-open range spanning the entire u64 domain is representable:
        // positions are pure offsets, so this is valid even though no real file
        // is that large.
        let hi = BytePos::new(u64::MAX);
        let r = ByteRange::new(BytePos::ZERO, hi).unwrap();
        assert_eq!(r.len(), hi);
        assert_eq!(r.start(), BytePos::ZERO);
        assert_eq!(r.end(), hi);
        assert!(!r.is_empty());
    }

    #[test]
    fn byte_range_debug_prints_start_to_end() {
        let r = ByteRange::new(BytePos::new(3), BytePos::new(7)).unwrap();
        assert_eq!(format!("{r:?}"), "3..7");
    }

    #[test]
    fn byte_range_is_hashable_and_equatable() {
        use std::collections::HashSet;
        let a = ByteRange::new(BytePos::new(1), BytePos::new(4)).unwrap();
        let b = ByteRange::new(BytePos::new(1), BytePos::new(4)).unwrap();
        let c = ByteRange::new(BytePos::new(1), BytePos::new(5)).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b); // duplicate
        set.insert(c);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&a));
        assert!(set.contains(&c));
    }

    // --- serde (feature-gated) ---

    #[cfg(feature = "serde")]
    #[test]
    fn position_types_derive_serialize_when_the_feature_is_enabled() {
        fn assert_serialize<T: serde::Serialize>() {}
        assert_serialize::<BytePos>();
        assert_serialize::<ByteRange>();
    }
}
