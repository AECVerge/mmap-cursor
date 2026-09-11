//! Scenario: a Part 21 parser has to skip `/* ... */` remarks before it can
//! read the records between them. This example locates every remark as a byte
//! range, then reports it as a zero-copy slice and as a `(line, column)`.
//!
//! The crate knows nothing about comments: `/*` is format knowledge and lives
//! here, in the caller. What the crate supplies is the byte position, the slice
//! and the line/column lookup.
//!
//! ```text
//! cargo run --example comments
//! ```

use std::path::PathBuf;

use mmap_cursor::{ByteFile, ByteRange, Error};

fn main() -> Result<(), Error> {
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/fixtures/sample.ifc"
    ));

    let file = ByteFile::open(&path)?;
    let index = file.line_index();
    let cursor = file.cursor();

    let ranges = comments(&file);

    // `slice` takes absolute positions rather than the cursor's own, so the
    // text comes straight out of the snapshot with no copy.
    println!("==sample.ifc==");
    println!("- comments: {}", ranges.len());
    for (n, range) in ranges.iter().enumerate() {
        let Some(text) = cursor.slice(*range) else {
            continue;
        };
        if let Some((line, column)) = index.line_column(range.start()) {
            println!(
                "- #{n}: bytes {range:?} at line {line}, column {column}: {}",
                String::from_utf8_lossy(text)
            );
        }
    }
    let commented: usize = ranges.iter().map(ByteRange::len).sum();
    println!("- comment bytes: {commented} of {}", file.len());

    Ok(())
}

/// The byte ranges of every `/* ... */` remark, in order.
///
/// This is caller-side scanning, not parsing: the crate has no idea what a
/// comment is. A `*` inside a remark is harmless, because only a `*` followed
/// by `/` closes one; and a `/*` with no `*/` ends up as a range that reaches
/// the end of the file, which is what an unterminated remark looks like.
fn comments(file: &ByteFile) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut cursor = file.cursor();

    while !cursor.is_eof() {
        // Stop in front of the next `/`, where a `/*` could start.
        cursor.advance_to(b'/');
        if cursor.is_eof() {
            break;
        }
        // A lone `/` opens nothing: step over it and keep looking.
        if cursor.peek_n_ahead(1) != Some(b'*') {
            cursor.advance();
            continue;
        }

        let start = cursor.position();
        cursor.advance_n_ahead(2); // past the opening `/*`
        loop {
            cursor.advance_to(b'*');
            if cursor.is_eof() {
                break;
            }
            cursor.advance(); // past the `*` we stopped on
            if cursor.peek() == Some(b'/') {
                cursor.advance(); // past the closing `/`
                break;
            }
        }
        // The cursor only ever moves forward, so `start <= cursor.position()`.
        ranges.push(ByteRange::new(start, cursor.position()));
    }

    ranges
}
