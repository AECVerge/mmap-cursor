mod cursor;
mod line_index;
mod position;
/// A one-directional byte-position cursor over a snapshot.
pub use crate::cursor::Cursor;
/// Maps byte positions to `(line, column)`.
pub use crate::line_index::LineIndex;
/// A byte offset into source text.
pub use crate::position::BytePos;
/// A half-open byte range `[start, end)`.
pub use crate::position::ByteRange;
