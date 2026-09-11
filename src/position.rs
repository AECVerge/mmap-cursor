//! Byte-position primitives: [`BytePos`] and [`ByteRange`].
//!
//! [`BytePos`] is a `usize` byte offset from the start of a snapshot;
//! [`ByteRange`] is a half-open `[start, end)` pair of positions. Using `usize`
//! makes positions directly indexable into the memory-mapped snapshot on the
//! current platform, so the largest addressable file is bounded by the platform's
//! `usize` (on 32-bit targets that means smaller files are supported).

use std::fmt;

use crate::{Error, Result};

#[cfg(feature = "serde")]
use serde::Serialize;

/// A byte offset into source text, stored as a `usize`.
#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytePos(usize);

impl BytePos {
    /// The zero byte offset.
    pub const ZERO: Self = BytePos(0);

    /// Construct a [`BytePos`] from a `usize` offset.
    #[inline]
    #[must_use]
    pub const fn new(n: usize) -> Self {
        BytePos(n)
    }

    /// Return the raw `usize` offset.
    #[inline]
    #[must_use]
    pub const fn to_usize(self) -> usize {
        self.0
    }

    /// Collapse this position into an empty [`ByteRange`] at the same offset.
    #[inline]
    #[must_use]
    pub fn as_range(self) -> ByteRange {
        ByteRange {
            start: self,
            end: self,
        }
    }

    /// Checked addition: `None` if the sum would overflow this platform's
    /// `usize`, where [`+`](std::ops::Add) panics.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, rhs: BytePos) -> Option<BytePos> {
        match self.0.checked_add(rhs.0) {
            Some(sum) => Some(BytePos(sum)),
            None => None,
        }
    }

    /// Checked subtraction: `None` if `rhs` is greater than `self`, where
    /// [`-`](std::ops::Sub) panics.
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, rhs: BytePos) -> Option<BytePos> {
        match self.0.checked_sub(rhs.0) {
            Some(difference) => Some(BytePos(difference)),
            None => None,
        }
    }

    /// Saturating addition: clamps at the top of the address space instead of
    /// panicking.
    ///
    /// Use this for "advance by `rhs`, clamped at the top" arithmetic, and
    /// [`checked_add`](Self::checked_add) when the clamp must be detectable.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: BytePos) -> BytePos {
        BytePos(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction: clamps at [`ZERO`](Self::ZERO) instead of
    /// panicking.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: BytePos) -> BytePos {
        BytePos(self.0.saturating_sub(rhs.0))
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

    /// # Panics
    ///
    /// Panics if the sum overflows this platform's `usize`. Unlike plain `usize`
    /// arithmetic, this panics in **every** profile, release included.
    #[inline]
    fn add(self, rhs: BytePos) -> BytePos {
        BytePos(
            self.0
                .checked_add(rhs.0)
                .expect("BytePos addition overflowed"),
        )
    }
}

impl std::ops::Sub<BytePos> for BytePos {
    type Output = BytePos;

    /// # Panics
    ///
    /// Panics if `rhs` is greater than `self`. Unlike plain `usize` arithmetic,
    /// this panics in **every** profile, release included.
    #[inline]
    fn sub(self, rhs: BytePos) -> BytePos {
        BytePos(
            self.0
                .checked_sub(rhs.0)
                .expect("BytePos subtraction underflowed"),
        )
    }
}

/// A half-open byte range: `[start, end)`.
#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ByteRange {
    start: BytePos,
    end: BytePos,
}

impl ByteRange {
    /// Construct a range from a start and end position.
    ///
    /// A valid range is only built through this constructor, its fallible
    /// sibling [`ByteRange::try_new`], [`ByteRange::EMPTY`] or
    /// [`BytePos::as_range`], so every `ByteRange` value obeys `start <= end`
    /// by construction.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `end`. Use [`ByteRange::try_new`] when
    /// `start`/`end` may be untrusted and you want to handle the error rather
    /// than panic.
    #[inline]
    #[must_use]
    pub fn new(start: BytePos, end: BytePos) -> Self {
        assert!(start <= end, "ByteRange: start > end");
        Self { start, end }
    }

    /// Fallible variant of [`ByteRange::new`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReversedRange`] if `start` is greater than `end`, instead
    /// of panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// use mmap_cursor::{BytePos, ByteRange, Error};
    ///
    /// let range = ByteRange::try_new(BytePos::new(3), BytePos::new(7)).unwrap();
    /// assert_eq!(range.len().to_usize(), 4);
    ///
    /// let err = ByteRange::try_new(BytePos::new(7), BytePos::new(3)).unwrap_err();
    /// assert!(matches!(err, Error::ReversedRange { .. }));
    /// ```
    #[inline]
    pub fn try_new(start: BytePos, end: BytePos) -> Result<Self> {
        if start > end {
            return Err(Error::ReversedRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// Start of the range in bytes.
    #[inline]
    #[must_use]
    pub fn start(&self) -> BytePos {
        self.start
    }

    /// End of the range in bytes.
    #[inline]
    #[must_use]
    pub fn end(&self) -> BytePos {
        self.end
    }

    /// Length of the range in bytes returned as a usize.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start).to_usize()
    }

    /// If this is an empty range.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The empty range at offset zero, equivalent to [`BytePos::ZERO`]`.as_range()`.
    /// For an empty range at an arbitrary offset use [`BytePos::as_range`].
    pub const EMPTY: Self = ByteRange {
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
    fn byte_pos_new_and_to_usize_round_trip() {
        for n in [0usize, 1, 42, usize::MAX - 1] {
            assert_eq!(BytePos::new(n).to_usize(), n);
        }
    }

    #[test]
    fn byte_pos_zero_default_and_equality() {
        assert_eq!(BytePos::ZERO.to_usize(), 0);
        assert_eq!(BytePos::default(), BytePos::ZERO);
        assert_eq!(BytePos::new(0), BytePos::ZERO);
        assert_ne!(BytePos::new(1), BytePos::ZERO);
    }

    #[test]
    fn byte_pos_total_order_matches_the_usize_value() {
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
        assert_eq!((base + delta).to_usize(), 130);
        assert_eq!((base - delta).to_usize(), 70);
        assert_eq!(((base + delta) - delta), base);
        assert_eq!((base + BytePos::ZERO), base);
        assert_eq!((base - BytePos::ZERO), base);
    }

    #[test]
    fn byte_pos_saturating_add_clamps_at_the_top_of_the_address_space() {
        assert_eq!(
            BytePos::new(5).saturating_add(BytePos::new(7)),
            BytePos::new(12)
        );
        assert_eq!(
            BytePos::new(usize::MAX).saturating_add(BytePos::new(1)),
            BytePos::new(usize::MAX)
        );
    }

    #[test]
    fn byte_pos_checked_arithmetic_reports_overflow_as_none() {
        assert_eq!(
            BytePos::new(5).checked_add(BytePos::new(7)),
            Some(BytePos::new(12))
        );
        assert_eq!(BytePos::new(usize::MAX).checked_add(BytePos::new(1)), None);
        assert_eq!(
            BytePos::new(usize::MAX).checked_add(BytePos::ZERO),
            Some(BytePos::new(usize::MAX))
        );
        assert_eq!(BytePos::new(5).checked_sub(BytePos::new(7)), None);
        assert_eq!(
            BytePos::new(5).checked_sub(BytePos::new(5)),
            Some(BytePos::ZERO)
        );
        assert_eq!(
            BytePos::ZERO.checked_sub(BytePos::ZERO),
            Some(BytePos::ZERO)
        );
    }

    #[test]
    fn byte_pos_checked_arithmetic_agrees_with_the_panicking_operators() {
        let cases: [(usize, usize); 8] = [
            (0, 0),
            (0, 1),
            (1, 0),
            (7, 9),
            (9, 7),
            (usize::MAX, 0),
            (usize::MAX - 1, 1),
            (1, usize::MAX - 1),
        ];
        for (a, b) in cases {
            let (a, b) = (BytePos::new(a), BytePos::new(b));
            if let Some(sum) = a.checked_add(b) {
                assert_eq!(sum, a + b, "checked_add disagrees for {a} + {b}");
            }
            if let Some(difference) = a.checked_sub(b) {
                assert_eq!(difference, a - b, "checked_sub disagrees for {a} - {b}");
            }
        }
    }

    #[test]
    fn byte_pos_saturating_sub_clamps_at_zero() {
        assert_eq!(
            BytePos::new(9).saturating_sub(BytePos::new(3)),
            BytePos::new(6)
        );
        assert_eq!(
            BytePos::new(3).saturating_sub(BytePos::new(3)),
            BytePos::ZERO
        );
        assert_eq!(
            BytePos::new(3).saturating_sub(BytePos::new(9)),
            BytePos::ZERO
        );
        assert_eq!(
            BytePos::ZERO.saturating_sub(BytePos::new(usize::MAX)),
            BytePos::ZERO
        );
    }

    #[test]
    #[should_panic(expected = "BytePos addition overflowed")]
    fn byte_pos_addition_overflows() {
        let _ = BytePos::new(usize::MAX) + BytePos::new(1);
    }

    #[test]
    #[should_panic(expected = "BytePos subtraction underflowed")]
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
        assert_eq!(r.len(), 0);
    }

    // --- ByteRange ---

    #[test]
    fn byte_range_exposes_bounds_and_len_for_a_valid_range() {
        let r = ByteRange::new(BytePos::new(10), BytePos::new(25));
        assert_eq!(r.start().to_usize(), 10);
        assert_eq!(r.end().to_usize(), 25);
        assert_eq!(r.len(), 15);
        assert!(!r.is_empty());
    }

    #[test]
    fn byte_range_zero_length_is_empty_and_spans_a_single_point() {
        let at = BytePos::new(50);
        let r = ByteRange::new(at, at);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.start(), at);
        assert_eq!(r.end(), at);
    }

    #[test]
    fn byte_range_try_new_accepts_a_valid_range() {
        let r = ByteRange::try_new(BytePos::new(10), BytePos::new(25)).unwrap();
        assert_eq!(r.start().to_usize(), 10);
        assert_eq!(r.end().to_usize(), 25);
        assert_eq!(r.len(), 15);
    }

    #[test]
    fn byte_range_try_new_accepts_a_zero_length_range() {
        let at = BytePos::new(10);
        let r = ByteRange::try_new(at, at).unwrap();
        assert!(r.is_empty());
        assert_eq!(r.start(), at);
        assert_eq!(r.end(), at);
    }

    #[test]
    fn byte_range_try_new_rejects_reversed_bounds() {
        let err = ByteRange::try_new(BytePos::new(10), BytePos::new(5)).unwrap_err();
        assert_eq!(err.to_string(), "byte range start 10 is greater than end 5");
        match err {
            Error::ReversedRange { start, end } => {
                assert_eq!(start, BytePos::new(10));
                assert_eq!(end, BytePos::new(5));
            }
            other => panic!("expected ReversedRange, got {other:?}"),
        }
    }

    #[test]
    fn byte_range_try_new_accepts_the_bounds_that_new_accepts() {
        // The two constructors must agree on every case that does not panic.
        let cases = [
            (0usize, 0usize),
            (0, 1),
            (7, 7),
            (3, 9),
            (usize::MAX, usize::MAX),
        ];
        for (start, end) in cases {
            let checked = ByteRange::try_new(BytePos::new(start), BytePos::new(end));
            assert_eq!(
                checked.ok(),
                Some(ByteRange::new(BytePos::new(start), BytePos::new(end))),
                "{start}..{end}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "ByteRange: start > end")]
    fn byte_range_new_panics_on_reversed_bounds() {
        let _ = ByteRange::new(BytePos::new(10), BytePos::new(5));
    }

    #[test]
    fn byte_range_empty_is_the_empty_zero_range() {
        let r = ByteRange::EMPTY;
        assert_eq!(r.start(), BytePos::ZERO);
        assert_eq!(r.end(), BytePos::ZERO);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        // The cross-constructor invariant: `EMPTY` is exactly `ZERO.as_range()`,
        // so every route to an empty range agrees.
        assert_eq!(r, BytePos::ZERO.as_range());
    }

    #[test]
    fn byte_range_default_is_the_empty_zero_range() {
        assert_eq!(ByteRange::default(), ByteRange::EMPTY);
        assert_eq!(ByteRange::default(), BytePos::ZERO.as_range());
        assert!(ByteRange::default().is_empty());
        assert_eq!(ByteRange::default().len(), 0);
    }

    #[test]
    fn byte_range_spans_the_whole_usize_space() {
        // A half-open range spanning the entire addressable domain is
        // representable: positions are pure offsets into the snapshot.
        let hi = BytePos::new(usize::MAX);
        let r = ByteRange::new(BytePos::ZERO, hi);
        assert_eq!(r.len(), hi.to_usize());
        assert_eq!(r.start(), BytePos::ZERO);
        assert_eq!(r.end(), hi);
        assert!(!r.is_empty());
    }

    #[test]
    fn byte_range_debug_prints_start_to_end() {
        let r = ByteRange::new(BytePos::new(3), BytePos::new(7));
        assert_eq!(format!("{r:?}"), "3..7");
    }

    #[test]
    fn byte_range_is_hashable_and_equatable() {
        use std::collections::HashSet;
        let a = ByteRange::new(BytePos::new(1), BytePos::new(4));
        let b = ByteRange::new(BytePos::new(1), BytePos::new(4));
        let c = ByteRange::new(BytePos::new(1), BytePos::new(5));
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
