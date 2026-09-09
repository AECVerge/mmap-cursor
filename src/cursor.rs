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

    /// Peek at the current byte without advancing.
    /// Returns `None` at EOF.
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.source.get(self.pos.to_usize()).copied()
    }

    /// Peek at the nth byte ahead (0 = current).
    #[inline]
    pub fn peek_n_ahead(&self, n: usize) -> Option<u8> {
        self.source.get(self.pos.to_usize() + n).copied()
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
        let end = self.eof.min(self.pos + BytePos::new(n));
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

    /// Move the cursor to an absolute byte position.
    #[inline]
    pub fn seek(&mut self, pos: BytePos) {
        if pos > self.eof {
            return;
        }
        self.pos = pos;
    }

    /// Advance one byte and return the consumed byte.
    #[inline]
    pub fn advance(&mut self) {
        self.pos = self.eof.min(self.pos + BytePos::new(1));
    }

    /// Advance `n` bytes. Panics if past EOF in debug.
    #[inline]
    pub fn advance_n_ahead(&mut self, n: usize) {
        self.pos = self.eof.min(self.pos + BytePos::new(n));
    }

    /// Skip given something. Return if succeeded.
    pub fn advance_sth(&mut self, sth: &[u8]) -> bool {
        if self.rest().is_some_and(|r| r.starts_with(sth)) {
            self.pos = self.eof.min(self.pos + BytePos::new(sth.len()));
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

    /// Skip anything until meeting stop.
    ///
    /// Use `include_stop` to specify whether stop after `stop`.
    /// Set [`true`] if advancing a quoted value, set [`false`] if
    /// advancing a corrupted token until ',', ')', or ';'.
    pub fn advance_until_stop(&mut self, stop: u8, include_stop: bool) {
        while self.peek().is_some_and(|p| p != stop) {
            self.advance();
        }
        if include_stop {
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
    fn cursor_advance_consumes_and_returns() {
        let mut c = cursor(b"abc");
        c.advance();
        assert_eq!(c.position().to_usize(), 1);
        c.advance();
        assert_eq!(c.position().to_usize(), 2);
    }

    #[test]
    fn cursor_advance_at_eof() {
        let mut c = cursor(b"a");
        c.advance(); // consume 'src'
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
}
