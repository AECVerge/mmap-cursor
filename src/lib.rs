//! A zero-copy byte-position reader for parsers and lexers of large files.
//!
//! [`ByteFile`] opens a file into a read-only, memory-mapped snapshot. You then
//! move a [`Cursor`] around and read `&[u8]` slices by byte position, or convert
//! positions into `(line, column)` with [`LineIndex`]. No tokens are parsed; the
//! crate is format-agnostic and designed around the "load a large file once, then
//! read byte positions from it" scenario.
//!
//! # Safety / concurrency model
//!
//! - The crate never mutates files.
//! - Opening produces a **stable snapshot**: positions, slices and line/column
//!   results are relative to that snapshot's content.
//! - The caller must ensure no external writer truncates the mapped inode while
//!   it is mapped. The standard convention is **atomic `rename` for writes**;
//!   under that guarantee a map-based read never observes a partial file and
//!   never SIGBUSes.
//! - Reads never panic: an out-of-range or overflowing position comes back as
//!   `None`, never as a panic or a substituted value. Arithmetic *on* positions
//!   — `BytePos + BytePos` — is the caller's own computation and does panic on
//!   overflow; see [`BytePos`]'s operators.
//!
//! The crate does not track the path it was opened from: noticing that the file
//! was replaced, and re-opening to read the new version, is the caller's
//! responsibility.
//!
//! # Conventions
//!
//! - A position is a `usize` byte offset from the start of its snapshot, and is
//!   directly indexable into it; the largest addressable file is therefore
//!   bounded by the platform's `usize` (smaller on 32-bit targets).
//! - `(line, column)` values are **1-based**; [`LineIndex::line_for_offset`] is
//!   the 0-based line index behind them.
//! - Lines are terminated by `\n`, `\r\n`, or a lone `\r`. A `\r` belonging to a
//!   `\r\n` pair counts as a column of the line it terminates.
//!
//! # Errors
//!
//! Only [`ByteFile::open`] and [`ByteRange::try_new`] can fail, and both report
//! an [`Error`]. A position outside the snapshot is an ordinary outcome rather
//! than an error: those reads answer `None`. [`Error`] documents the complete
//! inventory of failure modes — including the conditions that are deliberately
//! reported as `None`, and the ones that cannot be reported as a value at all.
//!
//! # Optional features
//!
//! - `serde` (off by default) derives `serde::Serialize` on the [`BytePos`]
//!   and [`ByteRange`] position types.
//! - `simd` (default on) accelerates the newline scan with `memchr`.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

mod bytefile;
mod cursor;
mod error;
mod line_index;
mod position;

pub use crate::bytefile::ByteFile;
pub use crate::cursor::Cursor;
pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::line_index::LineIndex;
pub use crate::position::BytePos;
pub use crate::position::ByteRange;
