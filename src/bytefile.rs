use std::fs::File;
use std::io;
use std::path::Path;

use memmap2::{Mmap, MmapOptions};

use crate::{Cursor, LineIndex};

/// A zero-copy, read-only snapshot of a file's bytes.
///
/// Opening a file yields a stable snapshot: all positions, slices and
/// line/column results are relative to *this* snapshot's content. If the file
/// is later replaced (the write side is expected to use the atomic-`rename`
/// convention), a previously opened snapshot is left untouched.
///
/// Detecting that the path now holds a newer version, and deciding when to
/// re-open for a fresh snapshot, is the caller's responsibility.
///
/// # Safety model
///
/// This crate never mutates the file. The caller must guarantee that no external
/// writer truncates the mapped inode while it is mapped. Under that guarantee,
/// the mmap-backed snapshot is stable and reads can never observe a partial
/// file (and therefore never SIGBUS on the truncation-of-a-mapped-file case).
///
/// This type hands out the snapshot's bytes and the views built over them;
/// reading itself lives in [`Cursor`] and position lookup in [`LineIndex`],
/// where an out-of-range position is reported as `None` rather than panicking.
pub struct ByteFile {
    mmap: Option<Mmap>,
}

impl ByteFile {
    /// Open `path` and map it into a read-only snapshot.
    ///
    /// An empty file is supported; it maps to a zero-length snapshot. A file
    /// that cannot be addressed by this platform's `usize` (e.g. too large on a
    /// 32-bit target) is rejected rather than truncated.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from opening the file or reading its metadata, an
    /// [`InvalidInput`](io::ErrorKind::InvalidInput) error when the file is too
    /// large to address on this platform, or whatever the memory-mapping call
    /// reports.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use filecursor::ByteFile;
    ///
    /// let file = ByteFile::open("data.bin")?;
    /// let index = file.line_index();
    /// let mut cursor = file.cursor();
    ///
    /// let head = cursor.slice_n_ahead(64).unwrap_or(&[]);
    /// assert!(head.len() <= 64);
    /// assert!(index.num_lines() >= 1);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(&path)?;
        let metadata = file.metadata()?;
        let len = usize::try_from(metadata.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "file is too large to memory-map on this platform",
            )
        })?;

        let mmap = if len == 0 {
            None
        } else {
            // SAFETY: this crate only ever reads through the mapping, and the
            // caller guarantees the backing inode is not truncated (atomic
            // rename writes) while the mapping is alive. memmap2 handles the
            // OS-specific mapping details.
            let m = unsafe { MmapOptions::new().len(len).map(&file)? };
            Some(m)
        };

        Ok(Self { mmap })
    }

    /// Total length of the snapshot in bytes.
    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The snapshot's bytes as a slice (empty for an empty file).
    ///
    /// Positions into this slice are the crate's byte positions. Use this to
    /// build a [`Cursor`] or [`LineIndex`] over the snapshot.
    pub fn bytes(&self) -> &[u8] {
        self.mmap.as_deref().unwrap_or(&[])
    }

    /// Create a [`Cursor`] over the whole snapshot, positioned at its start.
    pub fn cursor(&self) -> Cursor<'_> {
        Cursor::new(self.bytes())
    }

    /// Build a [`LineIndex`] over the whole snapshot for `pos -> (line, column)`
    /// conversion.
    pub fn line_index(&self) -> LineIndex {
        LineIndex::new(self.bytes())
    }
}

impl std::fmt::Debug for ByteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByteFile")
            .field("len", &self.len())
            .finish()
    }
}
