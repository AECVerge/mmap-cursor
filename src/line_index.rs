//! Precomputed line-start byte offsets for `pos -> (line, column)` mapping and
//! for `line -> bytes` lookup.
//!
//! The index is built in a single pass over a snapshot's bytes, and lookups are
//! a binary search. No tokenisation happens; this is format-agnostic.
//!
//! With the `simd` feature (default), the newline scan uses `memchr` for a
//! SIMD-accelerated pass; without it, an equivalent byte-scanning fallback is
//! used. The resulting index and the public API are identical either way.

#[cfg(feature = "simd")]
use memchr::memchr2_iter;

use crate::{BytePos, ByteRange};

/// Precomputed line-start byte offsets for `pos -> (line, column)` mapping.
///
/// Building is a one-pass scan of a snapshot's bytes; position lookups are a
/// binary search. The index answers both directions: a byte position to a
/// `(line, column)`, and a line to its byte range. Positions and results are
/// relative to the snapshot that the index was built from.
pub struct LineIndex {
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    /// Build line starts from a snapshot's bytes.
    ///
    /// `\n`, `\r\n`, and a lone `\r` are all treated as line terminators. A
    /// trailing terminator yields a final (possibly empty) line, and a file with
    /// no terminator at all (the empty file included) has exactly one line. A
    /// `\r` inside a `\r\n` pair is counted as a column of the line it
    /// terminates.
    pub fn new(bytes: &[u8]) -> Self {
        let mut line_starts = Vec::new();
        line_starts.push(0usize);

        #[cfg(feature = "simd")]
        {
            for i in memchr2_iter(b'\n', b'\r', bytes) {
                if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    // A `\r` belonging to a `\r\n` pair — the following `\n`
                    // terminates the line, so skip this index.
                    continue;
                }
                line_starts.push(i + 1);
            }
        }

        #[cfg(not(feature = "simd"))]
        {
            let mut i = 0usize;
            while i < bytes.len() {
                match bytes[i] {
                    b'\r' => {
                        if bytes.get(i + 1) == Some(&b'\n') {
                            // Consume the whole `\r\n` pair as one terminator.
                            line_starts.push(i + 2);
                            i += 2;
                            continue;
                        }
                        line_starts.push(i + 1);
                    }
                    b'\n' => line_starts.push(i + 1),
                    _ => {}
                }
                i += 1;
            }
        }

        Self {
            line_starts,
            len: bytes.len(),
        }
    }

    /// Number of lines implied by the index.
    ///
    /// A file with no terminator (the empty file included) has exactly one line;
    /// a file ending in a terminator has a final (possibly empty) line after it,
    /// so `num_lines` counts that one too.
    pub fn num_lines(&self) -> usize {
        self.line_starts.len()
    }

    /// Byte position where `line` starts, or `None` if `line` is out of range.
    ///
    /// `line` is 0-based, matching [`line_for_offset`](Self::line_for_offset);
    /// valid values are `0..num_lines()`.
    pub fn line_start(&self, line: usize) -> Option<BytePos> {
        self.line_starts.get(line).copied().map(BytePos::new)
    }

    /// Half-open byte range of `line`, or `None` if `line` is out of range.
    ///
    /// The range runs from this line's start to the start of the next line, so it
    /// includes this line's terminator; the last line ends at the end of the
    /// snapshot, so a file with a trailing terminator ends with an empty range.
    pub fn line_range(&self, line: usize) -> Option<ByteRange> {
        let start = self.line_start(line)?;
        let end = match self.line_starts.get(line + 1) {
            Some(&next) => BytePos::new(next),
            None => BytePos::new(self.len),
        };
        Some(ByteRange::new(start, end))
    }

    /// 0-based line index containing `offset`, or `None` if `offset` is past
    /// EOF.
    pub fn line_for_offset(&self, offset: BytePos) -> Option<usize> {
        let offset = offset.to_usize();
        if offset > self.len {
            return None;
        }
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        };
        Some(line)
    }

    /// Convert a byte position into a 1-based `(line, column)`.
    ///
    /// `column` is measured in bytes from the line start. Returns `None` for a
    /// position past EOF.
    ///
    /// # Examples
    ///
    /// ```
    /// use filecursor::{BytePos, LineIndex};
    ///
    /// let index = LineIndex::new(b"one\ntwo\n");
    /// assert_eq!(index.line_column(BytePos::new(5)), Some((2, 2))); // 1-based
    /// assert_eq!(index.line_column(BytePos::new(99)), None);
    /// ```
    pub fn line_column(&self, offset: BytePos) -> Option<(usize, usize)> {
        let line = self.line_for_offset(offset)?;
        let start = self.line_starts[line];
        let column = offset.to_usize().saturating_sub(start);
        Some((line + 1, column + 1))
    }
}

impl std::fmt::Debug for LineIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineIndex")
            .field("num_lines", &self.num_lines())
            .field("len", &self.len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_single_line() {
        let idx = LineIndex::new(b"hello");
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0));
        assert_eq!(idx.line_for_offset(BytePos::new(2)), Some(0));
        assert_eq!(idx.line_for_offset(BytePos::new(4)), Some(0));
        assert_eq!(idx.line_for_offset(BytePos::new(5)), Some(0)); // exactly EOF
        assert_eq!(idx.line_for_offset(BytePos::new(6)), None); // past EOF
        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1)));
        assert_eq!(idx.line_column(BytePos::new(4)), Some((1, 5)));
    }

    #[test]
    fn line_index_empty() {
        let idx = LineIndex::new(b"");
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0));
        assert_eq!(idx.line_for_offset(BytePos::new(1)), None); // past EOF
        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1)));
    }

    #[test]
    fn line_index_lf() {
        // "a\nbc\n"
        let idx = LineIndex::new(b"a\nbc\n");
        // Line 0: "a\n"    → bytes 0..2
        // Line 1: "bc\n"   → bytes 2..5
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0)); // 'a'
        assert_eq!(idx.line_for_offset(BytePos::new(1)), Some(0)); // '\n' belongs to line 0
        assert_eq!(idx.line_for_offset(BytePos::new(2)), Some(1)); // 'b'
        assert_eq!(idx.line_for_offset(BytePos::new(4)), Some(1)); // '\n' at end

        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1))); // 'a'
        assert_eq!(idx.line_column(BytePos::new(2)), Some((2, 1))); // 'b'
        assert_eq!(idx.line_column(BytePos::new(3)), Some((2, 2))); // 'c'
    }

    #[test]
    fn line_index_crlf() {
        // "a\r\nb"
        let idx = LineIndex::new(b"a\r\nb");
        // \r\n counts as one line break
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0)); // 'a'
        assert_eq!(idx.line_for_offset(BytePos::new(1)), Some(0)); // '\r'
        assert_eq!(idx.line_for_offset(BytePos::new(2)), Some(0)); // '\n' — still line 0
        assert_eq!(idx.line_for_offset(BytePos::new(3)), Some(1)); // 'b'

        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1)));
        assert_eq!(idx.line_column(BytePos::new(1)), Some((1, 2))); // \r as a column
        assert_eq!(idx.line_column(BytePos::new(3)), Some((2, 1)));
    }

    #[test]
    fn line_index_cr_only() {
        // "a\rb"
        let idx = LineIndex::new(b"a\rb");
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0)); // 'a'
        assert_eq!(idx.line_for_offset(BytePos::new(2)), Some(1)); // 'b'
        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1)));
        assert_eq!(idx.line_column(BytePos::new(1)), Some((1, 2))); // \r as a column
        assert_eq!(idx.line_column(BytePos::new(2)), Some((2, 1)));
    }

    #[test]
    fn line_index_multiple_lines() {
        let idx = LineIndex::new(b"line0\nline1\nline2\nline3");
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0));
        assert_eq!(idx.line_for_offset(BytePos::new(6)), Some(1)); // 'l' of line1
        assert_eq!(idx.line_for_offset(BytePos::new(12)), Some(2)); // 'l' of line2
        assert_eq!(idx.line_for_offset(BytePos::new(18)), Some(3)); // 'l' of line3
    }

    #[test]
    fn line_index_no_trailing_newline() {
        // "a\nb" — last line has no trailing \n
        let idx = LineIndex::new(b"a\nb");
        assert_eq!(idx.line_for_offset(BytePos::new(0)), Some(0)); // 'a'
        assert_eq!(idx.line_for_offset(BytePos::new(2)), Some(1)); // 'b'
        assert_eq!(idx.line_column(BytePos::new(2)), Some((2, 1)));
    }

    #[test]
    fn line_index_offset_past_end() {
        let idx = LineIndex::new(b"abc");
        // Past EOF maps to None per the documented contract, not a synthetic
        // line/column.
        assert_eq!(idx.line_for_offset(BytePos::new(999)), None);
        assert_eq!(idx.line_column(BytePos::new(999)), None);
    }

    #[test]
    fn line_index_num_lines_counts_lines_and_the_trailing_empty_one() {
        let cases: [(&[u8], usize); 8] = [
            (b"", 1),        // the empty file is one empty line
            (b"a", 1),       // no terminator at all
            (b"a\n", 2),     // trailing terminator adds a final empty line
            (b"a\r\n", 2),   // ... and a \r\n counts as one terminator
            (b"a\r", 2),     // ... so does a lone \r
            (b"a\nb", 2),    // no trailing terminator
            (b"a\nbc\n", 3), // "a", "bc", and the final empty line
            (b"\n", 2),      // one empty line, plus the final empty line
        ];
        for (bytes, expected) in cases {
            assert_eq!(LineIndex::new(bytes).num_lines(), expected, "{bytes:?}");
        }
    }

    #[test]
    fn line_index_mixed_terminators() {
        // "a\r\nb\rc\nd": lines are "a\r\n", "b\r", "c\n", "d".
        let idx = LineIndex::new(b"a\r\nb\rc\nd");
        assert_eq!(idx.num_lines(), 4);
        let expected: [(usize, usize, usize); 8] = [
            (0, 1, 1), // 'a'
            (1, 1, 2), // '\r' of the \r\n pair, as a column
            (2, 1, 3), // '\n' of the \r\n pair
            (3, 2, 1), // 'b'
            (4, 2, 2), // lone '\r'
            (5, 3, 1), // 'c'
            (6, 3, 2), // '\n'
            (7, 4, 1), // 'd'
        ];
        for (offset, line, column) in expected {
            assert_eq!(
                idx.line_column(BytePos::new(offset)),
                Some((line, column)),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn line_index_trailing_terminator_yields_a_final_empty_line() {
        let lf = LineIndex::new(b"a\n");
        assert_eq!(lf.num_lines(), 2);
        assert_eq!(lf.line_column(BytePos::new(2)), Some((2, 1))); // EOF

        let crlf = LineIndex::new(b"a\r\n");
        assert_eq!(crlf.num_lines(), 2);
        assert_eq!(crlf.line_column(BytePos::new(3)), Some((2, 1)));

        let cr = LineIndex::new(b"a\r");
        assert_eq!(cr.num_lines(), 2);
        assert_eq!(cr.line_column(BytePos::new(2)), Some((2, 1)));
    }

    #[test]
    fn line_index_lone_cr_before_a_crlf_pair_starts_a_line() {
        // "\r\r\n": the first \r is a lone terminator, the second belongs to the
        // \r\n pair — the one spot where the SIMD scan skips an index.
        let idx = LineIndex::new(b"\r\r\n");
        assert_eq!(idx.num_lines(), 3);
        assert_eq!(idx.line_column(BytePos::new(0)), Some((1, 1)));
        assert_eq!(idx.line_column(BytePos::new(1)), Some((2, 1)));
        assert_eq!(idx.line_column(BytePos::new(3)), Some((3, 1))); // final empty line
    }

    #[test]
    fn line_index_line_start_is_0_based_and_none_past_the_end() {
        let idx = LineIndex::new(b"a\r\nb\rc\nd");
        assert_eq!(idx.line_start(0), Some(BytePos::new(0)));
        assert_eq!(idx.line_start(1), Some(BytePos::new(3)));
        assert_eq!(idx.line_start(2), Some(BytePos::new(5)));
        assert_eq!(idx.line_start(3), Some(BytePos::new(7)));
        assert_eq!(idx.line_start(4), None);
        assert_eq!(idx.line_start(usize::MAX), None);
    }

    #[test]
    fn line_index_line_range_covers_the_line_including_its_terminator() {
        let idx = LineIndex::new(b"a\r\nb\rc\nd");
        // "a\r\n"
        assert_eq!(
            idx.line_range(0),
            Some(ByteRange::new(BytePos::new(0), BytePos::new(3)))
        );
        // "b\r"
        assert_eq!(
            idx.line_range(1),
            Some(ByteRange::new(BytePos::new(3), BytePos::new(5)))
        );
        // "d" — the last line ends at EOF
        assert_eq!(
            idx.line_range(3),
            Some(ByteRange::new(BytePos::new(7), BytePos::new(8)))
        );
        assert_eq!(idx.line_range(4), None);
        assert_eq!(idx.line_range(usize::MAX), None);

        // A trailing terminator leaves a final, empty line.
        let trailing = LineIndex::new(b"a\n");
        assert_eq!(
            trailing.line_range(0),
            Some(ByteRange::new(BytePos::ZERO, BytePos::new(2)))
        );
        assert_eq!(trailing.line_range(1), Some(BytePos::new(2).as_range()));
        assert!(trailing.line_range(1).unwrap().is_empty());

        // The empty snapshot is one empty line starting at zero.
        assert_eq!(LineIndex::new(b"").line_range(0), Some(ByteRange::EMPTY));
    }

    #[test]
    fn line_index_debug_reports_line_count_and_len() {
        let s = format!("{:?}", LineIndex::new(b"a\nbc\n"));
        assert!(s.contains("num_lines: 3"), "{s}");
        assert!(s.contains("len: 5"), "{s}");
    }
}
