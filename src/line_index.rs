//! Precomputed line-start byte offsets for `pos -> (line, column)` mapping.
//!
//! The index is built in a single pass over a snapshot's bytes, and a lookup is
//! a binary search. No tokenisation happens; this is format-agnostic.
//!
//! With the `simd` feature (default), the newline scan uses `memchr` for a
//! SIMD-accelerated pass; without it, an equivalent byte-scanning fallback is
//! used. The resulting index and the public API are identical either way.

#[cfg(feature = "simd")]
use memchr::memchr2_iter;

use crate::BytePos;

/// Precomputed line-start byte offsets for `pos -> (line, column)` mapping.
///
/// Building is a one-pass scan of a snapshot's bytes; each lookup is a binary
/// search. Positions and results are relative to the snapshot that the index
/// was built from.
pub struct LineIndex {
    line_starts: Vec<u64>,
    len: u64,
}

impl LineIndex {
    /// Build line starts from a snapshot's bytes.
    ///
    /// `\n`, `\r\n`, and a lone `\r` are all treated as line terminators. A
    /// trailing terminator yields a final (possibly empty) line. A `\r` inside
    /// a `\r\n` pair is counted as a column of the line it terminates.
    pub fn new(bytes: &[u8]) -> Self {
        let mut line_starts = Vec::with_capacity(bytes.len() / 88 + 1);
        line_starts.push(0u64);

        #[cfg(feature = "simd")]
        {
            for i in memchr2_iter(b'\n', b'\r', bytes) {
                if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    // A `\r` belonging to a `\r\n` pair — the following `\n`
                    // terminates the line, so skip this index.
                    continue;
                }
                line_starts.push(i as u64 + 1);
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
                            line_starts.push(i as u64 + 2);
                            i += 2;
                            continue;
                        }
                        line_starts.push(i as u64 + 1);
                    }
                    b'\n' => line_starts.push(i as u64 + 1),
                    _ => {}
                }
                i += 1;
            }
        }

        Self {
            line_starts,
            len: bytes.len() as u64,
        }
    }

    /// Number of lines implied by the index.
    pub fn num_lines(&self) -> usize {
        self.line_starts.len()
    }

    /// 0-based line index containing `offset`, or `None` if `offset` is past
    /// EOF.
    pub fn line_for_offset(&self, offset: BytePos) -> Option<usize> {
        let offset = offset.to_u64();
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
    pub fn line_column(&self, offset: BytePos) -> Option<(u64, u64)> {
        let line = self.line_for_offset(offset)?;
        let start = self.line_starts[line];
        let column = offset.to_u64().saturating_sub(start);
        Some((line as u64 + 1, column + 1))
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
}
