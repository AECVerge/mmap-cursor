use std::fmt;

use crate::{BytePos, ByteRange};

/// A one-directional byte cursor over a [`ByteFile`] snapshot.
///
/// Reading advances the internal position; seeking moves it explicitly. Reads
/// are short (they never fail on reaching EOF — they return what's available).
pub struct Cursor<'src> {
    source: &'src [u8],
    pos: BytePos,
    eof: BytePos,
}

impl<'src> Cursor<'src> {
    /// Create a cursor from source bytes.
    pub fn new(source: &'src [u8]) -> Self {
        Self {
            source,
            pos: BytePos::ZERO,
            eof: BytePos::new(source.len()),
        }
    }

    /// Current cursor position in bytes.
    #[inline]
    pub fn position(&self) -> BytePos {
        self.pos
    }

    /// True if we've consumed all bytes.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.pos >= self.eof
    }

    /// Number of bytes between the cursor and EOF.
    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.eof.to_usize().saturating_sub(self.pos.to_usize())
    }

    /// Peek at the current byte without advancing.
    /// Returns `None` at EOF.
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.source.get(self.pos.to_usize()).copied()
    }

    /// Peek at the nth byte ahead (0 = current).
    #[inline]
    pub fn peek_n_ahead(&self, n: usize) -> Option<u8> {
        self.source
            .get(self.pos.to_usize().saturating_add(n))
            .copied()
    }

    /// Slice the source from `start` to `end` (both absolute byte positions).
    pub fn slice(&self, range: ByteRange) -> Option<&'src [u8]> {
        if range.end() > self.eof {
            return None;
        };
        Some(&self.source[range.start().to_usize()..range.end().to_usize()])
    }

    /// Read up to `size` bytes starting at the cursor position and advance past
    /// them.
    ///
    /// Returns `None` if the cursor is already at EOF. Otherwise returns the
    /// bytes read (fewer than `size` when near EOF, empty for `size == 0`) and
    /// advances the cursor past them.
    pub fn slice_n_ahead(&mut self, n: usize) -> Option<&'src [u8]> {
        if self.is_eof() {
            return None;
        }
        let end = self.pos.saturating_add(BytePos::new(n)).min(self.eof);
        let slice = self
            .slice(ByteRange::new(self.pos, end))
            .expect("byte range is within the snapshot");
        self.pos = end;
        Some(slice)
    }

    /// Return the remaining source from the current position to EOF.
    #[inline]
    pub fn rest(&self) -> Option<&'src [u8]> {
        if self.is_eof() {
            return None;
        };
        Some(&self.source[self.pos.to_usize()..])
    }

    /// Move the cursor to an absolute byte position, clamping at EOF.
    ///
    /// A position past the end of the snapshot moves the cursor to EOF rather
    /// than leaving it where it was, so `seek` follows the same rule as the
    /// `advance*` methods. A caller that wants to know whether clamping happened
    /// can compare [`position`](Self::position) with the requested position
    /// afterwards.
    #[inline]
    pub fn seek(&mut self, pos: BytePos) {
        self.pos = pos.min(self.eof);
    }

    /// Advance one byte, clamping at EOF.
    #[inline]
    pub fn advance(&mut self) {
        self.pos = self.pos.saturating_add(BytePos::new(1)).min(self.eof);
    }

    /// Advance `n` bytes, clamping at EOF.
    #[inline]
    pub fn advance_n_ahead(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(BytePos::new(n)).min(self.eof);
    }

    /// If the upcoming bytes start with `prefix`, advance past it and return
    /// `true`; otherwise leave the cursor where it is and return `false`.
    ///
    /// At EOF there are no upcoming bytes, so this is `false` there — even for an
    /// empty `prefix`.
    pub fn advance_if_starts_with(&mut self, prefix: &[u8]) -> bool {
        if self.rest().is_some_and(|r| r.starts_with(prefix)) {
            self.pos = self
                .pos
                .saturating_add(BytePos::new(prefix.len()))
                .min(self.eof);
            return true;
        }
        false
    }

    /// Advance past bytes while `f` returns `true`, stopping at the first byte
    /// for which `f` returns `false`, or at EOF. The byte that stops the scan is
    /// left unconsumed.
    pub fn advance_until(&mut self, f: impl Fn(u8) -> bool) {
        while self.peek().is_some_and(&f) {
            self.advance();
        }
    }

    /// Advance to the first `stop` byte, or to EOF if there is none, leaving the
    /// `stop` byte unconsumed.
    pub fn advance_to(&mut self, stop: u8) {
        self.advance_until(|b| b != stop);
    }

    /// Advance to the first `stop` byte and past it, or to EOF if there is none.
    ///
    /// Use this to consume a delimited value's terminator; use
    /// [`advance_to`](Self::advance_to) to stop in front of a delimiter you want
    /// to inspect first.
    pub fn advance_through(&mut self, stop: u8) {
        self.advance_to(stop);
        if self.peek() == Some(stop) {
            self.advance();
        }
    }
}

impl<'src> fmt::Debug for Cursor<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_eof() {
            write!(f, "Cursor(pos={}, EOF)", self.pos)
        } else {
            let tail = &self.source[self.pos.to_usize()..];
            let snippet = String::from_utf8_lossy(&tail[..20.min(tail.len())]);
            write!(f, "Cursor(pos={}, next={:?})", self.pos, snippet)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(data: &[u8]) -> Cursor<'_> {
        Cursor::new(data)
    }

    #[test]
    fn cursor_new_pos_zero() {
        let c = cursor(b"abc");
        assert_eq!(c.position().to_usize(), 0);
    }

    #[test]
    fn cursor_peek_first_byte() {
        let c = cursor(b"abc");
        assert_eq!(c.peek(), Some(b'a'));
        assert_eq!(c.position().to_usize(), 0); // peek does not advance
    }

    #[test]
    fn cursor_peek_n() {
        let c = cursor(b"abc");
        assert_eq!(c.peek_n_ahead(0), Some(b'a'));
        assert_eq!(c.peek_n_ahead(1), Some(b'b'));
        assert_eq!(c.peek_n_ahead(2), Some(b'c'));
        assert_eq!(c.peek_n_ahead(3), None);
    }

    #[test]
    fn cursor_advance_moves_by_one_byte() {
        let mut c = cursor(b"abc");
        c.advance();
        assert_eq!(c.position().to_usize(), 1);
        c.advance();
        assert_eq!(c.position().to_usize(), 2);
    }

    #[test]
    fn cursor_advance_at_eof() {
        let mut c = cursor(b"a");
        c.advance(); // consume the only byte
        assert!(c.is_eof());
        c.advance();
        assert_eq!(c.position().to_usize(), c.eof.to_usize());
    }

    #[test]
    fn cursor_advance_n_basic() {
        let mut c = cursor(b"abcdef");
        c.advance_n_ahead(3);
        assert_eq!(c.position().to_usize(), 3);
        assert_eq!(c.peek(), Some(b'd'));
    }

    #[test]
    fn cursor_slice_correct_range() {
        let c = cursor(b"hello world");
        let range = ByteRange::new(BytePos::new(0), BytePos::new(5));
        assert_eq!(c.slice(range).unwrap(), b"hello");
    }

    #[test]
    fn cursor_rest_from_position() {
        let mut c = cursor(b"hello world");
        c.advance_n_ahead(6); // past "hello "
        assert_eq!(c.rest().unwrap(), b"world");
    }

    #[test]
    fn cursor_is_eof() {
        let mut c = cursor(b"ab");
        assert!(!c.is_eof());
        c.advance_n_ahead(2);
        assert!(c.is_eof());
    }

    #[test]
    fn cursor_is_eof_empty_source() {
        let c = cursor(b"");
        assert!(c.is_eof());
    }

    #[test]
    fn cursor_peek_at_eof() {
        let c = cursor(b"");
        assert_eq!(c.peek(), None);
    }

    #[test]
    fn cursor_debug_mid() {
        let c = cursor(b"hello world");
        let s = format!("{:?}", c);
        assert!(s.contains("pos=0"));
        assert!(s.contains("hello"));
    }

    #[test]
    fn cursor_debug_eof() {
        let c = cursor(b"");
        let s = format!("{:?}", c);
        assert!(s.contains("EOF"));
    }

    #[test]
    fn cursor_advance_until_stops_at_first_non_matching_byte() {
        let mut c = cursor(b"ab,cd");
        // Advance while the byte isn't a comma; stop at the comma, unconsumed.
        c.advance_until(|b| b != b',');
        assert_eq!(c.position().to_usize(), 2);
        assert_eq!(c.peek(), Some(b','));
    }

    #[test]
    fn cursor_advance_until_consumes_a_matching_run() {
        let mut c = cursor(b"aaaab");
        c.advance_until(|b| b == b'a');
        assert_eq!(c.position().to_usize(), 4);
        assert_eq!(c.peek(), Some(b'b'));
    }

    #[test]
    fn cursor_advance_until_runs_to_eof_when_every_byte_matches() {
        let mut c = cursor(b"aaaa");
        c.advance_until(|b| b == b'a');
        assert!(c.is_eof());
        assert_eq!(c.position().to_usize(), c.eof.to_usize());
        assert_eq!(c.peek(), None);
    }

    #[test]
    fn cursor_advance_until_no_advance_when_first_byte_fails_predicate() {
        let mut c = cursor(b"xyz");
        c.advance_until(|b| b == b'a');
        assert_eq!(c.position().to_usize(), 0);
        assert_eq!(c.peek(), Some(b'x'));
    }

    #[test]
    fn cursor_advance_until_empty_source_is_noop() {
        let mut c = cursor(b"");
        c.advance_until(|b| b == b'a');
        assert!(c.is_eof());
        assert_eq!(c.peek(), None);
    }

    #[test]
    fn cursor_peek_n_ahead_past_eof_is_none() {
        let c = cursor(b"abc");
        assert_eq!(c.peek_n_ahead(3), None); // one past the last byte
        assert_eq!(c.peek_n_ahead(usize::MAX), None);
    }

    #[test]
    fn cursor_peek_n_ahead_from_a_non_zero_position_does_not_wrap() {
        // A bare `pos + n` used to wrap in release and hand back the byte at the
        // wrapped offset.
        let mut c = cursor(b"abc");
        c.advance();
        assert_eq!(c.peek_n_ahead(1), Some(b'c'));
        assert_eq!(c.peek_n_ahead(2), None);
        assert_eq!(c.peek_n_ahead(usize::MAX), None);
    }

    #[test]
    fn cursor_slice_out_of_range_is_none() {
        let c = cursor(b"abc");
        assert_eq!(
            c.slice(ByteRange::new(BytePos::ZERO, BytePos::new(4))),
            None
        );
        assert_eq!(
            c.slice(ByteRange::new(BytePos::new(3), BytePos::new(4))),
            None
        );
        assert_eq!(
            c.slice(ByteRange::new(BytePos::ZERO, BytePos::new(3))),
            Some(&b"abc"[..])
        );
    }

    #[test]
    fn cursor_slice_n_ahead_reads_and_advances() {
        let mut c = cursor(b"hello");
        assert_eq!(c.slice_n_ahead(2), Some(&b"he"[..]));
        assert_eq!(c.position().to_usize(), 2);
        assert_eq!(c.slice_n_ahead(99), Some(&b"llo"[..])); // clamped at EOF
        assert!(c.is_eof());
        assert_eq!(c.slice_n_ahead(1), None); // already at EOF
    }

    #[test]
    fn cursor_slice_n_ahead_with_zero_is_empty_and_leaves_the_position() {
        let mut c = cursor(b"abc");
        assert_eq!(c.slice_n_ahead(0), Some(&b""[..]));
        assert_eq!(c.position().to_usize(), 0);
    }

    #[test]
    fn cursor_rest_is_none_at_eof() {
        assert_eq!(cursor(b"").rest(), None);
        let mut c = cursor(b"ab");
        c.advance_n_ahead(2);
        assert_eq!(c.rest(), None);
        assert_eq!(c.rest().map_or(0, <[u8]>::len), c.remaining_len());
    }

    #[test]
    fn cursor_seek_moves_the_position_and_rewinds() {
        let mut c = cursor(b"abcdef");
        c.seek(BytePos::new(4));
        assert_eq!(c.position().to_usize(), 4);
        assert_eq!(c.peek(), Some(b'e'));
        c.seek(BytePos::new(1)); // backwards is allowed
        assert_eq!(c.peek(), Some(b'b'));
        c.seek(BytePos::ZERO);
        assert_eq!(c.position().to_usize(), 0);
    }

    #[test]
    fn cursor_seek_past_eof_clamps_to_eof() {
        let mut c = cursor(b"abc");
        let requested = BytePos::new(99);
        c.seek(requested);
        assert_eq!(c.position().to_usize(), 3);
        assert!(c.is_eof());
        // Clamping is detectable by comparing with the requested position.
        assert_ne!(c.position(), requested);
        c.seek(BytePos::new(usize::MAX));
        assert_eq!(c.position().to_usize(), 3);
    }

    #[test]
    fn cursor_remaining_len_counts_down_to_zero() {
        let mut c = cursor(b"hello");
        assert_eq!(c.remaining_len(), 5);
        c.advance_n_ahead(2);
        assert_eq!(c.remaining_len(), 3);
        assert_eq!(c.remaining_len(), c.rest().map_or(0, <[u8]>::len));
        c.seek(BytePos::new(5));
        assert_eq!(c.remaining_len(), 0);
        assert!(c.is_eof());
    }

    #[test]
    fn cursor_advance_if_starts_with_consumes_a_matching_prefix() {
        let mut c = cursor(b"hello world");
        assert!(c.advance_if_starts_with(b"hello"));
        assert_eq!(c.position().to_usize(), 5);
        assert_eq!(c.peek(), Some(b' '));
    }

    #[test]
    fn cursor_advance_if_starts_with_leaves_a_non_matching_prefix() {
        let mut c = cursor(b"hello");
        assert!(!c.advance_if_starts_with(b"help"));
        // A prefix longer than what is left never matches either.
        assert!(!c.advance_if_starts_with(b"hello!"));
        assert_eq!(c.position().to_usize(), 0);
    }

    #[test]
    fn cursor_advance_if_starts_with_and_the_empty_prefix() {
        let mut c = cursor(b"abc");
        assert!(c.advance_if_starts_with(b"")); // matches, consumes nothing
        assert_eq!(c.position().to_usize(), 0);
        // At EOF there are no upcoming bytes, so nothing matches.
        let mut at_eof = cursor(b"");
        assert!(!at_eof.advance_if_starts_with(b""));
        assert_eq!(at_eof.position().to_usize(), 0);
    }

    #[test]
    fn cursor_advance_to_stops_in_front_of_the_stop_byte() {
        let mut c = cursor(b"abc;def");
        c.advance_to(b';');
        assert_eq!(c.position().to_usize(), 3);
        assert_eq!(c.peek(), Some(b';'));
    }

    #[test]
    fn cursor_advance_to_runs_to_eof_when_there_is_no_stop_byte() {
        let mut c = cursor(b"abcdef");
        c.advance_to(b';');
        assert!(c.is_eof());
        assert_eq!(c.position().to_usize(), 6);
    }

    #[test]
    fn cursor_advance_through_consumes_the_stop_byte() {
        let mut c = cursor(b"abc;def");
        c.advance_through(b';');
        assert_eq!(c.position().to_usize(), 4);
        assert_eq!(c.peek(), Some(b'd'));
    }

    #[test]
    fn cursor_advance_through_a_trailing_stop_byte_lands_on_eof() {
        let mut c = cursor(b"abc;");
        c.advance_through(b';');
        assert!(c.is_eof());
        assert_eq!(c.position().to_usize(), 4);
    }

    #[test]
    fn cursor_advance_through_without_a_stop_byte_stops_at_eof() {
        let mut c = cursor(b"abc");
        c.advance_through(b';');
        assert!(c.is_eof());
        assert_eq!(c.position().to_usize(), 3);
        // Still a no-op once already at EOF.
        c.advance_through(b';');
        assert_eq!(c.position().to_usize(), 3);
    }

    #[test]
    fn cursor_advance_to_and_through_on_an_empty_source_are_noops() {
        let mut c = cursor(b"");
        c.advance_to(b';');
        assert!(c.is_eof());
        c.advance_through(b';');
        assert!(c.is_eof());
        assert_eq!(c.position().to_usize(), 0);
    }
}
