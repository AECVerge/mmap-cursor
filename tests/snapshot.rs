//! The snapshot contract, exercised against real files.
//!
//! Opening a [`ByteFile`] must produce a *stable* snapshot: positions, slices
//! and line/column results keep describing the bytes of the file as it was when
//! it was opened, whatever happens to the path afterwards. The crate never
//! re-stats the path it was opened from, so noticing a replacement and
//! re-opening is the caller's job.
//!
//! # Deliberately not tested here
//!
//! - **Truncating a file that is still mapped.** The safety model calls this a
//!   caller contract violation, and on the offending platforms the consequence
//!   is a `SIGBUS`/access violation rather than a value. There is no test for it
//!   that would not be a flaky crash.
//! - **Overwriting the mapped bytes in place.** A writer that mutates the mapped
//!   inode rather than renaming over it is outside the documented convention, so
//!   what a snapshot observes is unspecified and not asserted.

mod support;

use std::fs;

use mmap_cursor::{ByteFile, BytePos, Error};
use support::TempDir;

/// The documented write convention: replace the path, never the mapped inode.
#[test]
fn an_atomic_rename_leaves_an_open_snapshot_untouched() {
    let dir = TempDir::new("rename-length");
    let original = b"first version\n";
    let path = dir.write("data.bin", original);

    let file = ByteFile::open(&path).unwrap();
    assert_eq!(file.bytes(), original);

    // A different length on purpose: even a re-stat could see the change, and
    // the snapshot must still not follow it.
    dir.replace_atomically(&path, b"second\n");

    assert_eq!(file.len(), original.len());
    assert_eq!(file.bytes(), original);
    assert_eq!(file.cursor().rest(), Some(&original[..]));
    assert_eq!(file.line_index().num_lines(), 2);

    // Re-opening is how a caller picks up the new version.
    let reopened = ByteFile::open(&path).unwrap();
    assert_eq!(reopened.bytes(), b"second\n");
    assert_eq!(reopened.len(), 7);
}

#[test]
fn a_same_length_replacement_is_not_observed_either() {
    let dir = TempDir::new("rename-same-length");
    let path = dir.write("data.bin", b"aaaa");

    let file = ByteFile::open(&path).unwrap();
    dir.replace_atomically(&path, b"bbbb");

    // Same length, so nothing about the new file is detectable from the
    // snapshot — the mapping still refers to the old inode.
    assert_eq!(file.len(), 4);
    assert_eq!(file.bytes(), b"aaaa");
    assert_eq!(ByteFile::open(&path).unwrap().bytes(), b"bbbb");
}

#[test]
fn two_generations_of_the_same_path_coexist() {
    let dir = TempDir::new("two-generations");
    let path = dir.write("data.bin", b"gen-0\n");
    let first = ByteFile::open(&path).unwrap();

    dir.replace_atomically(&path, b"generation number one\n");
    let second = ByteFile::open(&path).unwrap();

    // Reading through both, interleaved, must not cross the generations.
    assert_eq!(first.bytes(), b"gen-0\n");
    assert_eq!(second.bytes(), b"generation number one\n");
    assert_eq!(first.line_index().line_column(BytePos::ZERO), Some((1, 1)));
    assert_eq!(second.line_index().num_lines(), 2);
    assert_eq!(first.bytes(), b"gen-0\n");
}

#[test]
fn a_snapshot_survives_the_removal_of_its_path() {
    let dir = TempDir::new("unlink");
    let content = b"kept alive by the mapping\n";
    let path = dir.write("data.bin", content);

    let file = ByteFile::open(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(!path.exists());

    // The path is gone; the snapshot is intact and still fully readable.
    assert_eq!(file.len(), content.len());
    assert_eq!(file.bytes(), content);
    assert_eq!(file.cursor().rest(), Some(&content[..]));
    assert_eq!(file.line_index().line_column(BytePos::ZERO), Some((1, 1)));

    // ...and the crate never notices the path is gone, because it never looks.
    assert!(matches!(ByteFile::open(&path), Err(Error::Io(_))));
}
