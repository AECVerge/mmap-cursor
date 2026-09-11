//! End-to-end reading: a real file, opened into a snapshot, walked with a
//! [`Cursor`] and resolved with a [`LineIndex`].
//!
//! Every document here is built in memory and written to a temporary file at
//! test time — never checked in. That is deliberate. Line terminators are the
//! subject of this crate, and a fixture committed through git is subject to
//! `* text=auto` normalisation (see `.gitattributes`), so a checked-in file
//! cannot be trusted to keep the exact `\r\n` or lone-`\r` bytes that these
//! assertions depend on. Byte-exact input has to be produced at runtime.

mod support;

use mmap_cursor::{ByteFile, BytePos, ByteRange};
use support::TempDir;

/// The mixed-terminator document the tests below share.
///
/// | line | bytes | terminator |
/// | --- | --- | --- |
/// | 1 | `alpha` | `\n` |
/// | 2 | `b` `é` `ta` | `\r\n` |
/// | 3 | *(empty)* | lone `\r` |
/// | 4 | `gamma` | lone `\r` |
/// | 5 | `delta` | none |
///
/// `é` is written as its two UTF-8 bytes on purpose: the crate counts columns in
/// bytes, and no editor or checkout filter gets to reinterpret them.
fn mixed_document() -> Vec<u8> {
    let mut doc = Vec::new();
    doc.extend_from_slice(b"alpha\n"); //          line 1: LF
    doc.extend_from_slice(b"b\xC3\xA9ta\r\n"); //  line 2: CRLF, with 2-byte é
    doc.extend_from_slice(b"\r"); //               line 3: empty, lone CR
    doc.extend_from_slice(b"gamma\r"); //          line 4: lone CR
    doc.extend_from_slice(b"delta"); //            line 5: no terminator
    // The byte offsets written by hand throughout this file assume exactly these
    // 25 bytes; keep the document and the tables impossible to desynchronise.
    assert_eq!(doc.len(), 25, "the hand-written offsets assume 25 bytes");
    doc
}

/// Write `bytes` to a fresh temporary file and open a snapshot of it.
///
/// Keep the returned [`TempDir`] alive while reading: dropping it removes the
/// file. The snapshot itself would survive that (that is the point of a
/// mapping), but a caller normally keeps the file around.
fn open_document(tag: &str, bytes: &[u8]) -> (TempDir, ByteFile) {
    let dir = TempDir::new(tag);
    let path = dir.write("document.txt", bytes);
    let file = ByteFile::open(&path).unwrap();
    (dir, file)
}

#[test]
fn a_cursor_over_a_snapshot_starts_at_the_beginning() {
    let doc = mixed_document();
    let (_dir, file) = open_document("cursor-start", &doc);

    let mut cursor = file.cursor();
    assert_eq!(cursor.position(), BytePos::ZERO);
    assert_eq!(cursor.peek(), Some(b'a'));
    assert_eq!(cursor.peek_n_ahead(0), Some(b'a'));
    assert_eq!(cursor.remaining_len(), doc.len());
    assert_eq!(cursor.rest(), Some(doc.as_slice()));

    // Reading the whole snapshot in one call lands exactly on EOF.
    assert_eq!(cursor.slice_n_ahead(doc.len()), Some(doc.as_slice()));
    assert!(cursor.is_eof());
    assert_eq!(cursor.position().to_usize(), doc.len());
    assert_eq!(cursor.remaining_len(), 0);
    assert_eq!(cursor.rest(), None);
}

#[test]
fn reading_the_snapshot_in_order_reproduces_the_file() {
    let doc = mixed_document();
    let (_dir, file) = open_document("sequential", &doc);

    // An arbitrary chunk size that does not divide the document: the short read
    // at EOF must not lose or duplicate a byte.
    let mut cursor = file.cursor();
    let mut collected = Vec::new();
    while let Some(chunk) = cursor.slice_n_ahead(3) {
        collected.extend_from_slice(chunk);
    }

    assert_eq!(collected, doc);
    assert!(cursor.is_eof());
    assert_eq!(cursor.remaining_len(), 0);
    // Past EOF the answer stays `None` instead of panicking.
    assert_eq!(cursor.slice_n_ahead(1), None);
    assert_eq!(cursor.peek(), None);
}

#[test]
fn an_advancing_walk_reproduces_the_snapshot_byte_for_byte() {
    // The advancing API is the other half of a parser's loop: `peek` reads and
    // `advance` moves, with no slice taken at all. Over a real snapshot it must
    // reproduce the file exactly and stop cleanly at EOF.
    let doc = mixed_document();
    let (_dir, file) = open_document("advancing-walk", &doc);

    let mut cursor = file.cursor();
    let mut collected = Vec::with_capacity(doc.len());
    while let Some(byte) = cursor.peek() {
        collected.push(byte);
        cursor.advance();
    }
    assert_eq!(collected, doc);
    assert!(cursor.is_eof());
    assert_eq!(cursor.position().to_usize(), doc.len());

    // Advancing at EOF clamps to EOF instead of running past the snapshot or
    // panicking, even for a step larger than the whole address space.
    cursor.advance();
    assert_eq!(cursor.position().to_usize(), doc.len());
    cursor.advance_n_ahead(usize::MAX);
    assert_eq!(cursor.position().to_usize(), doc.len());

    // ...and the same oversized step taken from the start lands on EOF too,
    // rather than wrapping back into the snapshot.
    let mut fresh = file.cursor();
    fresh.advance_n_ahead(usize::MAX);
    assert!(fresh.is_eof());
    assert_eq!(fresh.position().to_usize(), doc.len());
}

#[test]
fn oversized_read_requests_saturate_at_eof() {
    // `peek_n_ahead` and `slice_n_ahead` add the request to the cursor position
    // before clamping. On a *non-empty* snapshot that addition is the saturating
    // path — on the empty snapshot the cursor is already at EOF and returns
    // `None` before reaching it, so the empty-file test cannot cover this.
    let doc = mixed_document();
    let (_dir, file) = open_document("oversized-reads", &doc);

    let mut cursor = file.cursor();
    cursor.seek(BytePos::new(8));
    assert_eq!(cursor.peek_n_ahead(usize::MAX), None);
    assert_eq!(cursor.position().to_usize(), 8, "peek must not move");

    // A whole-snapshot read request that cannot be represented saturates at EOF
    // and hands back everything that is left.
    let mut cursor = file.cursor();
    assert_eq!(cursor.slice_n_ahead(usize::MAX), Some(doc.as_slice()));
    assert!(cursor.is_eof());
    assert_eq!(cursor.position().to_usize(), doc.len());
    assert_eq!(cursor.remaining_len(), 0);
    assert_eq!(cursor.slice_n_ahead(usize::MAX), None); // already at EOF
}

#[test]
fn delimiter_scans_advance_through_the_snapshot() {
    // The shape a parser actually walks: a field, its `;`, a line terminator.
    let doc = b"HEADER;\nENDSEC;\nDATA;";
    let (_dir, file) = open_document("delimiter-scan", doc);
    let mut cursor = file.cursor();

    let mut fields = Vec::new();
    loop {
        // `advance_to` stops *in front of* the delimiter so it can be inspected.
        let start = cursor.position();
        cursor.advance_to(b';');
        assert_eq!(cursor.peek(), Some(b';'), "the stop byte stays unconsumed");
        fields.push(
            cursor
                .slice(ByteRange::new(start, cursor.position()))
                .expect("the scanned field is inside the snapshot")
                .to_vec(),
        );

        // `advance_through` consumes it; a conditional advance eats the newline
        // only when there is one, which is how the last record reaches EOF.
        cursor.advance_through(b';');
        if !cursor.advance_if_starts_with(b"\n") {
            break;
        }
    }

    assert!(cursor.is_eof());
    assert_eq!(
        fields,
        [b"HEADER".to_vec(), b"ENDSEC".to_vec(), b"DATA".to_vec()]
    );

    // At EOF every scan is a no-op, and nothing matches — not even the empty
    // prefix, because there are no upcoming bytes to match against.
    let at_eof = cursor.position();
    assert!(!cursor.advance_if_starts_with(b""));
    assert!(!cursor.advance_if_starts_with(b"\n"));
    cursor.advance_until(|_| true);
    cursor.advance_to(b';');
    cursor.advance_through(b';');
    assert_eq!(cursor.position(), at_eof);
}

#[test]
fn line_ranges_slice_the_actual_line_bytes() {
    let doc = mixed_document();
    let (_dir, file) = open_document("line-ranges", &doc);
    let index = file.line_index();
    let cursor = file.cursor();

    let expected: [&[u8]; 5] = [b"alpha\n", b"b\xC3\xA9ta\r\n", b"\r", b"gamma\r", b"delta"];
    assert_eq!(index.num_lines(), expected.len());

    for (line, want) in expected.iter().enumerate() {
        let range = index.line_range(line).expect("line is in range");
        assert_eq!(
            range.start(),
            index.line_start(line).unwrap(),
            "line {line}"
        );
        assert_eq!(range.len(), want.len(), "line {line}");
        // The index and the snapshot agree: a line's range slices its own bytes,
        // terminator included.
        assert_eq!(cursor.slice(range), Some(*want), "line {line}");
    }
    assert_eq!(index.line_range(expected.len()), None);

    // The per-line ranges tile the snapshot exactly: no gap, no overlap, no
    // invented byte — and every terminator stays attached to the line it ends.
    let mut rebuilt = Vec::with_capacity(doc.len());
    for line in 0..index.num_lines() {
        rebuilt.extend_from_slice(cursor.slice(index.line_range(line).unwrap()).unwrap());
    }
    assert_eq!(rebuilt, doc);
}

#[test]
fn the_line_index_over_mixed_terminators_is_fully_specified() {
    let doc = mixed_document();
    let (_dir, file) = open_document("index-parity", &doc);
    let index = file.line_index();

    // The exhaustive loop below is the point of this test: it re-derives every
    // byte's line from the byte offsets alone, so the index's binary search has
    // an independent oracle. CI runs this binary under every feature set, so the
    // `simd` scan and the byte-scanning fallback must agree here; a plain local
    // `cargo test` runs it once, with the default feature.
    //
    // The per-line `line_start` / `line_for_offset` spot checks that used to sit
    // here are covered by `line_ranges_slice_the_actual_line_bytes` above (which
    // cross-checks `line_start` against every range) and by the `src/line_index.rs`
    // unit tests.
    let starts: [usize; 5] = [0, 6, 13, 14, 20];
    assert_eq!(index.num_lines(), starts.len());

    for offset in 0..doc.len() {
        let line = index.line_for_offset(BytePos::new(offset)).unwrap();
        let start = starts[line];
        let end = starts.get(line + 1).copied().unwrap_or(doc.len());
        assert!(
            start <= offset && offset < end,
            "offset {offset} -> line {line}"
        );
    }
    assert_eq!(index.line_for_offset(BytePos::new(doc.len() + 1)), None);
}

#[test]
fn a_position_resolves_back_to_its_line_and_column() {
    // The path a diagnostic takes: a byte position produced while scanning, a
    // 1-based (line, column) for the message, and the bytes to quote back.
    let doc = mixed_document();
    let (_dir, file) = open_document("diagnostic", &doc);
    let index = file.line_index();

    let cases: &[(usize, usize, usize)] = &[
        (0, 1, 1),
        (4, 1, 5),
        (5, 1, 6), // the LF belongs to line 1
        (6, 2, 1),
        (8, 2, 3),  // the second byte of é: columns count bytes
        (11, 2, 6), // the CR of the CRLF pair ...
        (12, 2, 7), // ... and its LF both belong to line 2
        (13, 3, 1), // the empty line's lone CR
        (14, 4, 1),
        (19, 4, 6), // the lone CR terminating "gamma"
        (20, 5, 1),
        (24, 5, 5),
        (25, 5, 6), // EOF: one past the last byte, still inside line 5
    ];

    for &(offset, line, column) in cases {
        assert_eq!(
            index.line_column(BytePos::new(offset)),
            Some((line, column)),
            "offset {offset}"
        );

        // The three queries must tell one story about the same position.
        let start = index.line_start(line - 1).expect("line is in range");
        assert_eq!(offset - start.to_usize() + 1, column, "offset {offset}");
        assert_eq!(
            index.line_for_offset(BytePos::new(offset)),
            Some(line - 1),
            "offset {offset}"
        );

        // And a cursor at that position sees the bytes the index claims.
        let mut probe = file.cursor();
        probe.seek(BytePos::new(offset));
        assert_eq!(probe.peek(), doc.get(offset).copied(), "offset {offset}");
        assert_eq!(probe.remaining_len(), doc.len() - offset, "offset {offset}");
    }

    // Past the snapshot there is no line and no column to report.
    assert_eq!(index.line_column(BytePos::new(doc.len() + 1)), None);
    assert_eq!(index.line_for_offset(BytePos::new(doc.len() + 1)), None);
    assert_eq!(index.line_start(index.num_lines()), None);
}

#[test]
fn columns_are_counted_in_bytes_not_characters() {
    // U+00E9 is two bytes in UTF-8, so its second byte has a column of its own.
    let doc = b"\xC3\xA9\n";
    let (_dir, file) = open_document("utf8-columns", doc);
    let index = file.line_index();

    assert_eq!(file.len(), 3);
    assert_eq!(index.line_column(BytePos::ZERO), Some((1, 1)));
    assert_eq!(index.line_column(BytePos::new(1)), Some((1, 2)));
    assert_eq!(index.line_column(BytePos::new(2)), Some((1, 3))); // the LF
    assert_eq!(index.line_column(BytePos::new(3)), Some((2, 1))); // final empty line

    // Slices are raw bytes: the whole character comes back intact, a byte inside
    // it comes back as that byte — never a substituted or lossy decoding.
    let cursor = file.cursor();
    assert_eq!(
        cursor.slice(ByteRange::new(BytePos::ZERO, BytePos::new(2))),
        Some(&b"\xC3\xA9"[..])
    );
    assert_eq!(
        cursor.slice(ByteRange::new(BytePos::new(1), BytePos::new(2))),
        Some(&b"\xA9"[..])
    );
    assert_eq!(cursor.slice(BytePos::new(1).as_range()), Some(&b""[..]));
}

#[test]
fn seeking_around_a_multi_megabyte_snapshot_reads_the_expected_bytes() {
    const LEN: usize = 2 * 1024 * 1024 + 3;
    let doc: Vec<u8> = (0..LEN).map(|i| (i % 251) as u8).collect();
    let (_dir, file) = open_document("seek-large", &doc);

    let mut cursor = file.cursor();
    // Walk across page boundaries, forwards and backwards: a cursor may rewind,
    // and every read must still be the file's bytes at that position.
    for offset in [
        0,
        1,
        4095,
        4096,
        4097,
        LEN / 3,
        LEN / 2,
        LEN - 8,
        0,
        LEN - 1,
    ] {
        cursor.seek(BytePos::new(offset));
        assert_eq!(cursor.position(), BytePos::new(offset), "offset {offset}");

        let end = (offset + 8).min(LEN);
        assert_eq!(
            cursor.slice_n_ahead(8),
            Some(&doc[offset..end]),
            "offset {offset}"
        );
        assert_eq!(cursor.position(), BytePos::new(end), "offset {offset}");
        assert_eq!(cursor.remaining_len(), LEN - end, "offset {offset}");
    }

    // The line index over the same snapshot must count exactly the terminators
    // the raw bytes contain. This pattern emits both CR and LF bytes but never a
    // CR directly before an LF, so each is its own terminator — assert that
    // rather than assume it.
    let lf = doc.iter().filter(|&&byte| byte == b'\n').count();
    let cr = doc.iter().filter(|&&byte| byte == b'\r').count();
    let crlf = doc.windows(2).filter(|pair| *pair == b"\r\n").count();
    assert_eq!(crlf, 0, "the pattern must not form CRLF pairs here");
    assert_eq!(file.line_index().num_lines(), lf + cr + 1);

    // `Debug` must stay bounded: a multi-megabyte tail never becomes a log line.
    // The budget is deliberately loose because the 20-byte snippet is lossy-decoded
    // and then escape-printed, so one input byte can expand to several output
    // bytes; what matters is that the output does not scale with the file.
    for offset in [0, LEN / 2, LEN - 1, LEN] {
        let mut probe = file.cursor();
        probe.seek(BytePos::new(offset));
        let debug = format!("{probe:?}");
        assert!(debug.len() < 256, "offset {offset} produced {debug:?}");
    }
}
