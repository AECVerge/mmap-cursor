#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

mod bytefile;
mod cursor;
mod line_index;
mod position;

pub use crate::bytefile::ByteFile;
pub use crate::cursor::Cursor;
pub use crate::line_index::LineIndex;
pub use crate::position::BytePos;
pub use crate::position::ByteRange;
