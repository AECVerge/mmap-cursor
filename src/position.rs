//! Byte-position primitives: [`BytePos`] and [`ByteRange`].
//!
//! [`BytePos`] is a `usize` byte offset from the start of a snapshot;
//! [`ByteRange`] is a half-open `[start, end)` pair of positions. Using `usize`
//! makes positions directly indexable into the memory-mapped snapshot on the
//! current platform, so the largest addressable file is bounded by the platform's
//! `usize` (on 32-bit targets that means smaller files are supported).
//! [`BytePos`] is totally ordered; both types are `Copy`, `Eq` and `Hash`, and
//! derive `serde::Serialize` under the `serde` feature.
//!
//! Arithmetic on positions is plain integer arithmetic: [`BytePos`] implements
//! `Add`/`Sub` and both panic on overflow instead of wrapping (see the operators'
//! `# Panics` sections). Reading through a snapshot never panics — out-of-range
//! and overflowing positions are reported as errors instead.

use std::fmt;
use std::io;

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
    pub const fn new(n: usize) -> Self {
        BytePos(n)
    }

    /// Return the raw `usize` offset.
    #[inline]
    pub const fn to_usize(self) -> usize {
        self.0
    }

    /// Collapse this position into an empty [`ByteRange`] at the same offset.
    #[inline]
    pub fn as_range(self) -> ByteRange {
        ByteRange {
            start: self,
            end: self,
        }
    }

    /// Saturating addition, for "advance by `rhs`, clamped at the top of the
    /// address space" arithmetic. Internal: callers clamp to the snapshot's end
    /// on top of this, so no position arithmetic inside the crate can panic.
    #[inline]
    pub(crate) const fn saturating_add(self, rhs: BytePos) -> BytePos {
        BytePos(self.0.saturating_add(rhs.0))
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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    start: BytePos,
    end: BytePos,
}

impl ByteRange {
    /// Construct a range from a start and end position.
    ///
    /// Because the fields are private, a valid range is only built through this
    /// constructor, its fallible sibling [`ByteRange::try_new`],
    /// [`ByteRange::EMPTY`] or [`BytePos::as_range`], so every `ByteRange` value
    /// obeys `start <= end` by construction.
    ///
    /// # Panics
    ///
    /// Panics if `start` is greater than `end`. Use [`ByteRange::try_new`] when
    /// `start`/`end` may be untrusted and you want to handle the error rather
    /// than panic.
    #[inline]
    pub fn new(start: BytePos, end: BytePos) -> Self {
        assert!(start <= end, "ByteRange: start > end");
        Self { start, end }
    }

    /// Fallible variant of [`ByteRange::new`].
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] of kind
    /// [`InvalidInput`](io::ErrorKind::InvalidInput) if `start` is greater than
    /// `end`, instead of panicking.
    #[inline]
    pub fn try_new(start: BytePos, end: BytePos) -> io::Result<Self> {
        if start > end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
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
    ///
    /// Returned as a [`BytePos`] so that a length and an offset share one type;
    /// use [`BytePos::to_usize`] for the raw number.
    #[inline]
    pub fn len(&self) -> BytePos {
        self.end - self.start
    }

    /// If this is an empty range.
    #[inline]
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
        assert_eq!(r.len(), BytePos::ZERO);
    }

    // --- ByteRange ---

    #[test]
    fn byte_range_exposes_bounds_and_len_for_a_valid_range() {
        let r = ByteRange::new(BytePos::new(10), BytePos::new(25));
        assert_eq!(r.start().to_usize(), 10);
        assert_eq!(r.end().to_usize(), 25);
        assert_eq!(r.len().to_usize(), 15);
        assert!(!r.is_empty());
    }

    #[test]
    fn byte_range_zero_length_is_empty_and_spans_a_single_point() {
        let at = BytePos::new(50);
        let r = ByteRange::new(at, at);
        assert!(r.is_empty());
        assert_eq!(r.len(), BytePos::ZERO);
        assert_eq!(r.start(), at);
        assert_eq!(r.end(), at);
    }

    #[test]
    fn byte_range_try_new_accepts_a_valid_range() {
        let r = ByteRange::try_new(BytePos::new(10), BytePos::new(25)).unwrap();
        assert_eq!(r.start().to_usize(), 10);
        assert_eq!(r.end().to_usize(), 25);
        assert_eq!(r.len().to_usize(), 15);
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
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(err.to_string(), "ByteRange: start > end");
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
        assert_eq!(r.len(), BytePos::ZERO);
        // The cross-constructor invariant: `EMPTY` is exactly `ZERO.as_range()`,
        // so every route to an empty range agrees.
        assert_eq!(r, BytePos::ZERO.as_range());
    }

    #[test]
    fn byte_range_spans_the_whole_usize_space() {
        // A half-open range spanning the entire addressable domain is
        // representable: positions are pure offsets into the snapshot.
        let hi = BytePos::new(usize::MAX);
        let r = ByteRange::new(BytePos::ZERO, hi);
        assert_eq!(r.len(), hi);
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
