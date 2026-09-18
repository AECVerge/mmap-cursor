//! `ByteFile::open` against real files: the crate's only filesystem entry point,
//! and therefore the only place real paths, real mappings and real OS errors can
//! be exercised. In-module unit tests cannot reach any of this.
//!
//! # Coverage gap: `Error::FileTooLarge`
//!
//! `open` reports [`Error::FileTooLarge`] only when the file's length does not
//! fit the platform's `usize`. On a 64-bit target that condition needs a file of
//! more than 16 EiB, so **`open` cannot produce this variant here** and no test
//! in this directory claims to. The variant's own formatting and conversions are
//! covered by the unit tests in `src/error.rs`; exercising it end to end needs a
//! 32-bit target and a multi-gigabyte file, which this project's CI does not run.

mod support;

use std::fs;
use std::io;

use mmap_cursor::{ByteFile, BytePos, ByteRange, Error};
use support::TempDir;

// --- success paths ---------------------------------------------------------

#[test]
fn opens_a_file_and_exposes_its_bytes() {
    let dir = TempDir::new("text");
    let content = b"ISO-10303-21;\nHEADER;\nENDSEC;\n";
    let path = dir.write("data.ifc", content);

    let file = ByteFile::open(&path).unwrap();

    assert_eq!(file.len(), content.len());
    assert!(!file.is_empty());
    assert_eq!(file.bytes(), content);
}

#[test]
fn preserves_binary_content_that_is_not_utf8() {
    // The crate is format-agnostic: NUL bytes and broken UTF-8 sequences are
    // just bytes, and must survive the round trip byte for byte.
    let content: Vec<u8> =
        vec![0x00, 0xFF, 0xFE, 0xC3, 0x28, 0x80, b'\n', 0x00];
    assert!(
        std::str::from_utf8(&content).is_err(),
        "the fixture is meant to be invalid UTF-8"
    );

    let dir = TempDir::new("binary");
    let path = dir.write("data.bin", &content);
    let file = ByteFile::open(&path).unwrap();

    assert_eq!(file.len(), content.len());
    assert_eq!(file.bytes(), content.as_slice());
    assert_eq!(file.cursor().peek(), Some(0x00));
    // No lossy conversion anywhere along the way.
    assert_eq!(file.bytes()[3], 0xC3);
    assert_eq!(file.bytes()[4], 0x28);
}

#[test]
fn an_empty_file_is_a_zero_length_snapshot_not_an_error() {
    let dir = TempDir::new("empty");
    let path = dir.write("empty.bin", b"");

    let file = ByteFile::open(&path).unwrap();

    assert_eq!(file.len(), 0);
    assert!(file.is_empty());
    assert_eq!(file.bytes(), b"");

    let mut cursor = file.cursor();
    assert!(cursor.is_eof());
    assert_eq!(cursor.position(), BytePos::ZERO);
    assert_eq!(cursor.remaining_len(), 0);
    assert_eq!(cursor.peek(), None);
    assert_eq!(cursor.peek_n_ahead(0), None);
    assert_eq!(cursor.peek_n_ahead(usize::MAX), None);
    assert_eq!(cursor.rest(), None);
    // Nothing can be read, and nothing is invented: a short read at EOF is
    // `None` even for a zero-length request.
    assert_eq!(cursor.slice_n_ahead(0), None);
    assert_eq!(cursor.slice_n_ahead(1), None);
    assert_eq!(cursor.slice_n_ahead(usize::MAX), None);
    // An empty range is inside every snapshot, including this one.
    assert_eq!(cursor.slice(ByteRange::EMPTY), Some(&b""[..]));

    let index = file.line_index();
    assert_eq!(index.num_lines(), 1);
    assert_eq!(index.line_for_offset(BytePos::ZERO), Some(0));
    assert_eq!(index.line_start(0), Some(BytePos::ZERO));
    assert_eq!(index.line_range(0), Some(ByteRange::EMPTY));
    assert_eq!(index.line_column(BytePos::ZERO), Some((1, 1)));
    assert_eq!(index.line_column(BytePos::new(1)), None);
}

#[test]
fn snapshots_at_and_around_a_page_boundary_are_exact() {
    // The OS maps whole pages and zero-fills the tail of the last one, so a
    // snapshot that reported the mapping's length instead of the file's would
    // show up at lengths that are not page multiples: 4095 and 4097 expose it on
    // any page size, 4096 only where the page is 4 KiB. The last byte of the file
    // must be the last byte of the snapshot, and nothing past it readable.
    for len in [0usize, 1, 2, 4095, 4096, 4097, 8193] {
        let dir = TempDir::new(&format!("page-{len}"));
        let content: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let path = dir.write("data.bin", &content);

        let file = ByteFile::open(&path).unwrap();
        assert_eq!(file.len(), len, "len {len}");
        assert_eq!(file.bytes(), content.as_slice(), "len {len}");
        assert_eq!(file.is_empty(), len == 0, "len {len}");

        if len > 0 {
            let last = ((len - 1) % 251) as u8;
            assert_eq!(file.bytes()[len - 1], last, "len {len}");
            let mut cursor = file.cursor();
            cursor.seek(BytePos::new(len - 1));
            assert_eq!(cursor.peek(), Some(last), "len {len}");
            assert_eq!(cursor.peek_n_ahead(1), None, "len {len}");
        }
    }
}

#[test]
fn a_large_snapshot_matches_the_file_on_disk() {
    // One MiB is already many pages wide. The multi-megabyte seek walk lives in
    // `end_to_end.rs`, so this test only has to show that `open` reports the
    // file's exact length rather than the mapping's rounded-up one.
    const LEN: usize = 1024 * 1024 + 7;
    let content: Vec<u8> = (0..LEN).map(|i| (i % 251) as u8).collect();
    let dir = TempDir::new("large");
    let path = dir.write("large.bin", &content);

    let file = ByteFile::open(&path).unwrap();
    assert_eq!(file.len(), LEN);
    assert_eq!(file.len(), fs::metadata(&path).unwrap().len() as usize);

    // Spot-check across pages instead of materialising a second copy.
    for offset in [0, 1, 4095, 4096, 4097, LEN / 2, LEN - 1] {
        assert_eq!(file.bytes()[offset], content[offset], "offset {offset}");
    }

    let mut cursor = file.cursor();
    cursor.seek(BytePos::new(LEN - 16));
    assert_eq!(cursor.slice_n_ahead(16), Some(&content[LEN - 16..]));
}

#[test]
fn accepts_any_as_ref_path() {
    let dir = TempDir::new("paths");
    let content = b"path shapes\n";
    let path = dir.write("data.bin", content);
    // A path with redundant `.` components resolves without any working-directory
    // games, which would be process-global state and unsafe under parallel tests.
    let with_dot = path.parent().unwrap().join(".").join("data.bin");

    assert_eq!(ByteFile::open(path.as_path()).unwrap().bytes(), content); // &Path
    assert_eq!(ByteFile::open(path.clone()).unwrap().bytes(), content); // owned PathBuf
    assert_eq!(ByteFile::open(with_dot).unwrap().bytes(), content); // with `.`
}

#[test]
fn two_snapshots_of_the_same_path_are_independent() {
    let dir = TempDir::new("two-opens");
    let path = dir.write("data.bin", b"shared\n");

    let first = ByteFile::open(&path).unwrap();
    let second = ByteFile::open(&path).unwrap();
    assert_eq!(first.bytes(), second.bytes());

    // No lock and no shared state: dropping one snapshot cannot disturb another.
    drop(first);
    assert_eq!(second.len(), 7);
    assert_eq!(second.bytes(), b"shared\n");
    assert_eq!(second.line_index().num_lines(), 2);
}

#[test]
fn debug_reports_the_length_and_never_the_content() {
    let dir = TempDir::new("debug");
    let secret = b"SECRET-MARKER-0123456789";
    let path = dir.write("data.bin", secret);

    let debug = format!("{:?}", ByteFile::open(&path).unwrap());
    assert_eq!(debug, format!("ByteFile {{ len: {} }}", secret.len()));
    assert!(!debug.contains("SECRET"), "{debug}");

    // Debug must stay O(1): a megabyte of bytes must not become a megabyte of log.
    let big = dir.write("big.bin", &vec![b'S'; 1 << 20]);
    assert!(format!("{:?}", ByteFile::open(&big).unwrap()).len() < 64);
}

#[test]
fn opening_and_reading_never_modifies_the_file() {
    let dir = TempDir::new("untouched");
    let content = b"do not touch\n";
    let path = dir.write("data.bin", content);
    let before = fs::metadata(&path).unwrap();

    let file = ByteFile::open(&path).unwrap();
    // Read through every entry point the snapshot offers.
    assert_eq!(file.bytes(), content);
    let mut cursor = file.cursor();
    while cursor.slice_n_ahead(4).is_some() {}
    let _ = file.line_index();

    let after = fs::metadata(&path).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(after.permissions().mode(), before.permissions().mode());
    }
    // The crate never writes: the file on disk is bit-for-bit what we wrote.
    assert_eq!(fs::read(&path).unwrap(), content);
}

// --- failure paths ---------------------------------------------------------

#[test]
fn a_missing_path_is_an_io_error_and_creates_nothing() {
    let dir = TempDir::new("missing");
    let path = dir.missing("nope.bin");

    match ByteFile::open(&path) {
        Err(Error::Io(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::NotFound)
        }
        other => panic!("expected Error::Io(NotFound), got {other:?}"),
    }

    // A failed read-only open has no side effects at all.
    assert!(!path.exists(), "a failed open must not create the file");
}

#[test]
fn a_directory_is_rejected_with_an_io_error() {
    let dir = TempDir::new("directory");
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();

    match ByteFile::open(&sub) {
        Err(Error::Io(_)) => {}
        Err(other) => panic!("expected Error::Io, got {other:?}"),
        // Which call fails is a platform detail: Windows rejects the directory
        // in `File::open`, Unix accepts it there and fails in `mmap` — unless
        // the filesystem reports a zero length for the directory entry, in
        // which case the documented empty-file path takes over and the snapshot
        // is legitimately empty. Either way `open` never panics and never hands
        // back a directory's invented content.
        Ok(file) => assert!(
            file.is_empty(),
            "a directory snapshot may only be the empty one"
        ),
    }
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_is_an_io_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("permission");
    let path = dir.write("data.bin", b"locked\n");
    let locked = fs::Permissions::from_mode(0o000);
    let readable = fs::Permissions::from_mode(0o600);
    fs::set_permissions(&path, locked).unwrap();

    if fs::read(&path).is_ok() {
        // Running as root (or on a filesystem that ignores the mode bits): the
        // bits do not deny anything, so there is no error to assert.
        fs::set_permissions(&path, readable).unwrap();
        return;
    }

    match ByteFile::open(&path) {
        Err(Error::Io(error)) => {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied)
        }
        other => panic!("expected Error::Io(PermissionDenied), got {other:?}"),
    }

    // Restore the mode so the temporary directory stays removable.
    fs::set_permissions(&path, readable).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"locked\n");
}

// Every Unix except macOS accepts arbitrary bytes in a filename. macOS requires
// names to be valid UTF-8, so the fixture below cannot even be created there:
// `fs::write` fails with `EILSEQ` (errno 92) before `ByteFile::open` is ever
// reached. That is a property of the platform, not of this crate, so the test is
// confined to the platforms where the case is representable — do not widen it
// back to a bare `#[cfg(unix)]`.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn a_non_utf8_path_can_be_opened() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = TempDir::new("non-utf8-path");
    let path = dir
        .path()
        .join(OsString::from_vec(b"non-utf8-\xFF.bin".to_vec()));
    fs::write(&path, b"opaque name\n").unwrap();

    // The path is not valid UTF-8, and that must not matter.
    assert!(path.to_str().is_none());

    let file = ByteFile::open(&path).unwrap();
    assert_eq!(file.bytes(), b"opaque name\n");
}

#[cfg(windows)]
#[test]
fn a_file_that_denies_sharing_is_rejected_with_an_io_error() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    let dir = TempDir::new("share-mode");
    let path = dir.write("data.bin", b"held\n");

    // `share_mode(0)` denies every other open of the path, including ours.
    let _holder = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();

    match ByteFile::open(&path) {
        Err(Error::Io(_)) => {}
        other => panic!("expected Error::Io, got {other:?}"),
    }
}
