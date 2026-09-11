//! Scenario: the fixture's `DATA` section is closed by an `ENDSEC` that forgot
//! its `;`. This example finds it, copies the offending line and its neighbours
//! out of the snapshot, **drops the snapshot**, and only then renders a
//! rustc-style message.
//!
//! Dropping the source first is the point: a [`BytePos`] and a [`LineIndex`]
//! stay meaningful after the bytes are gone, so a parser can hand positions to a
//! diagnostic layer and let the file go.
//!
//! ```text
//! cargo run --example diagnostic
//! ```

use std::path::Path;

use mmap_cursor::{ByteFile, BytePos, Cursor, Error, LineIndex};

/// The fixture, named as the message would print it.
const SOURCE: &str = "sample.ifc";

/// What the message needs, copied out of the snapshot so that it outlives it.
struct Finding {
    /// Where the missing `;` should have gone.
    at: BytePos,
    before: Option<String>,
    offending: String,
    after: Option<String>,
}

fn main() -> Result<(), Error> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/fixtures")
        .join(SOURCE);

    let file = ByteFile::open(&path)?;

    // Find the unterminated `ENDSEC`, copying out the text the message
    // needs while the snapshot is still there to read.
    let index = file.line_index();
    let Some(finding) = find(&file, &index) else {
        println!("no missing `;` found in {SOURCE}");
        return Ok(());
    };

    // The snapshot has done its job.
    drop(file);

    // Render. The bytes are gone, but the position and the index still
    // resolve to a line and a column.
    let Some((line, column)) = index.line_column(finding.at) else {
        return Ok(());
    };
    let width = line.to_string().len();
    let gutter = " ".repeat(width);
    println!("error: missing `;` at the end of ENDSEC");
    println!("{gutter}--> {SOURCE}:{line}:{column}");
    println!("{gutter} |");
    if let Some(text) = &finding.before {
        println!("{number:>width$} | {text}", number = line - 1);
    }
    let text = &finding.offending;
    println!("{line:>width$} | {text}");
    println!("{gutter} | {}^ expected `;` here", " ".repeat(column - 1));
    if let Some(text) = &finding.after {
        println!("{number:>width$} | {text}", number = line + 1);
    }
    println!("{gutter} |");
    println!("note: rendered after `drop(file)`; only the position and the index survived");

    Ok(())
}

/// The first line that opens an `ENDSEC` without terminating it, plus the text a
/// message needs to show it in context. Everything read here is gone once the
/// snapshot is dropped.
fn find(file: &ByteFile, index: &LineIndex) -> Option<Finding> {
    let cursor = file.cursor();
    for line in 0..index.num_lines() {
        let Some(range) = index.line_range(line) else {
            continue;
        };
        let Some(bytes) = cursor.slice(range) else {
            continue;
        };
        // A line's range includes its terminator, which the message does not
        // want, so trim it before looking at the statement.
        let content = bytes.trim_ascii_end();
        if !content.starts_with(b"ENDSEC") || content.ends_with(b";") {
            continue;
        }
        // The `;` belongs just past the last byte of the statement.
        return Some(Finding {
            at: range.start() + BytePos::new(content.len()),
            before: line_text(&cursor, index, line.checked_sub(1)),
            offending: String::from_utf8_lossy(content).into_owned(),
            after: line_text(&cursor, index, Some(line + 1)),
        });
    }
    None
}

/// A neighbouring line's text, without its terminator, or `None` when there is
/// no such line (the first and last line have only one neighbour).
fn line_text(cursor: &Cursor<'_>, index: &LineIndex, line: Option<usize>) -> Option<String> {
    let range = index.line_range(line?)?;
    let text = cursor.slice(range)?.trim_ascii_end();
    Some(String::from_utf8_lossy(text).into_owned())
}
