//! The crate's error type.
//!
//! This module is private and its items are re-exported at the crate root. The
//! documentation the crate publishes lives on [`Error`] itself, so that it is
//! rendered as part of the public API.

use std::error::Error as StdError;
use std::fmt;
use std::io;

use crate::BytePos;

/// The error type of every fallible operation in this crate.
///
/// # Failure inventory
///
/// Two operations in the public API can fail. Both report an [`Error`]:
///
/// | Operation | Variant | Cause |
/// | --- | --- | --- |
/// | [`ByteFile::open`](crate::ByteFile::open) | [`Error::Io`] | opening the file, reading its metadata, or creating the mapping failed. The underlying [`io::Error`] is carried unchanged, so its [`kind`](io::ErrorKind) is still matchable. |
/// | [`ByteFile::open`](crate::ByteFile::open) | [`Error::FileTooLarge`] | the file's length does not fit this platform's `usize`, so no snapshot could address it. The file is rejected whole, never truncated. |
/// | [`ByteRange::try_new`](crate::ByteRange::try_new) | [`Error::ReversedRange`] | the requested `start` is greater than the requested `end`. |
///
/// # Conditions that are values, not errors
///
/// A position outside the snapshot is an ordinary outcome of reading a file, so
/// it is reported as `None` rather than as an [`Error`]:
///
/// - [`Cursor::peek`](crate::Cursor::peek),
///   [`peek_n_ahead`](crate::Cursor::peek_n_ahead),
///   [`slice`](crate::Cursor::slice),
///   [`slice_n_ahead`](crate::Cursor::slice_n_ahead) and
///   [`rest`](crate::Cursor::rest) return `None` at or past EOF.
/// - [`LineIndex::line_start`](crate::LineIndex::line_start),
///   [`line_range`](crate::LineIndex::line_range),
///   [`line_for_offset`](crate::LineIndex::line_for_offset) and
///   [`line_column`](crate::LineIndex::line_column) return `None` for a line or
///   position outside the snapshot.
/// - [`BytePos::checked_add`](crate::BytePos::checked_add) and
///   [`BytePos::checked_sub`](crate::BytePos::checked_sub) return `None` where
///   the `+`/`-` operators would panic.
///
/// # Conditions that are not recoverable here
///
/// - **A replaced file is invisible to an already-open snapshot.** Detecting the
///   replacement, and re-opening for the new version, is the caller's job; there
///   is no error value for "this snapshot is stale" because the crate never
///   re-stats the path it was opened from.
/// - **A file truncated under an open snapshot is a contract violation**, not an
///   error value: the caller guarantees this never happens (the atomic-`rename`
///   convention), and under that guarantee the mapped bytes are stable. See the
///   crate-level *Safety / concurrency model*.
/// - **A failed allocation aborts the process**, it does not return an [`Error`].
///   [`LineIndex::new`](crate::LineIndex::new) grows a `Vec`, and Rust's default
///   allocator aborts on allocation failure; there is no fallible variant.
///
/// # Panicking constructors and operators
///
/// [`ByteRange::new`](crate::ByteRange::new) and `BytePos`'s `+`/`-` panic on
/// their respective conditions rather than returning a value. Each documents its
/// own `# Panics` section. Their fallible counterparts —
/// [`ByteRange::try_new`](crate::ByteRange::try_new),
/// [`BytePos::checked_add`](crate::BytePos::checked_add) and
/// [`BytePos::checked_sub`](crate::BytePos::checked_sub) — report the same
/// conditions as values.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An operating-system call failed: opening the file, reading its metadata,
    /// or creating the memory mapping.
    ///
    /// The underlying [`io::Error`] is carried unchanged, so its
    /// [`kind`](io::ErrorKind) is still available for matching.
    Io(io::Error),
    /// The file is longer than this platform can address, so it cannot be
    /// memory-mapped into a snapshot.
    ///
    /// This is reachable only where the file's length does not fit the target's
    /// `usize` — typically a 32-bit target and a file of 4 GiB or more. The file
    /// is left untouched and no partial snapshot is produced.
    FileTooLarge {
        /// The file's length in bytes, as reported by the filesystem.
        len: u64,
    },
    /// A [`ByteRange`](crate::ByteRange) was requested with `start` greater than
    /// `end`.
    ///
    /// This is the error form of the condition that makes
    /// [`ByteRange::new`](crate::ByteRange::new) panic.
    ReversedRange {
        /// The requested start position.
        start: BytePos,
        /// The requested end position.
        end: BytePos,
    },
}

impl Error {
    /// Recover an [`io::Error`], for callers whose own API is `io::Result`.
    ///
    /// [`Io`](Error::Io) is returned unchanged, keeping its
    /// [`kind`](io::ErrorKind) and source. This crate's own variants have no
    /// `io::ErrorKind` of their own and become
    /// [`InvalidInput`](io::ErrorKind::InvalidInput) errors carrying this error's
    /// [`Display`](fmt::Display) text; the original value is not preserved
    /// through that conversion, so match on [`Error`] instead when the
    /// distinction matters.
    #[must_use]
    pub fn into_io(self) -> io::Error {
        match self {
            Error::Io(error) => error,
            other => {
                io::Error::new(io::ErrorKind::InvalidInput, other.to_string())
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Transparent: the OS message is the useful one.
            Error::Io(error) => fmt::Display::fmt(error, f),
            Error::FileTooLarge { len } => write!(
                f,
                "file of {len} bytes is too large to address on this platform \
                 (usize holds at most {} bytes)",
                usize::MAX
            ),
            Error::ReversedRange { start, end } => {
                write!(f, "byte range start {start} is greater than end {end}")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::FileTooLarge { .. } | Error::ReversedRange { .. } => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

/// A specialized [`Result`](std::result::Result) for this crate's fallible
/// operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_pass_through_unchanged() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "no such file");
        let error = Error::from(io_error);

        assert!(matches!(error, Error::Io(_)));
        // Display is transparent, and the io error is still the source.
        assert_eq!(error.to_string(), "no such file");
        assert!(StdError::source(&error).is_some());
        assert_eq!(error.into_io().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn file_too_large_names_the_length_and_the_platform_limit() {
        let error = Error::FileTooLarge { len: u64::MAX };
        let message = error.to_string();

        assert!(message.contains(&u64::MAX.to_string()), "{message}");
        assert!(message.contains(&usize::MAX.to_string()), "{message}");
        assert!(StdError::source(&error).is_none());
        // No io::ErrorKind of its own, so it degrades to InvalidInput.
        assert_eq!(error.into_io().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn reversed_range_reports_both_bounds() {
        let error = Error::ReversedRange {
            start: BytePos::new(10),
            end: BytePos::new(5),
        };

        assert_eq!(
            error.to_string(),
            "byte range start 10 is greater than end 5"
        );
        assert!(StdError::source(&error).is_none());
        assert_eq!(error.into_io().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn error_is_a_send_sync_std_error() {
        // Callers put this in `Box<dyn Error + Send + Sync>`, `anyhow::Error`,
        // or across threads; all three need these bounds.
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_std_error<T: StdError + 'static>() {}
        assert_send_sync::<Error>();
        assert_std_error::<Error>();
    }
}
