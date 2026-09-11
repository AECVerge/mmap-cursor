// The crate's documentation is the README, included verbatim. Keeping a single
// copy means the text crates.io renders is the same text rustdoc builds and
// compiles the examples of, so a snippet here cannot drift away from the public
// API the way a hand-maintained duplicate would.
#![doc = include_str!("../README.md")]
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
