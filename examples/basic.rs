//! Minimal end-to-end demo: open a file into a read-only snapshot, read its
//! bytes by position, and report positions as 1-based `(line, column)`.
//!
//! Nothing here parses or tokenises: the crate is format-agnostic, and turning
//! bytes into tokens is the caller's job.
//!
//! ```text
//! cargo run --example basic                  # uses examples/fixtures/sample.ifc
//! ```

use std::path::PathBuf;

use mmap_cursor::{ByteFile, Error};

fn main() -> Result<(), Error> {
    // Step 0: Provide file path.
    //
    // The fixture is found through `CARGO_MANIFEST_DIR` because
    // `cargo run --example` leaves the working directory where you invoked
    // cargo, which is not necessarily the crate root.
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/fixtures/sample.ifc"
    ));
    println!("==sample.ifc==");

    // Step 1: Open file.
    //
    // Opening maps the file read-only and copies nothing. The snapshot is stable
    // for as long as `file` lives, whatever happens to the path afterwards.
    let file = ByteFile::open(&path)?;
    println!("- bytes: {}", file.bytes().len());

    // Step 2: Build line index
    //
    // The index answers both directions: a position to `(line, column)`, and a
    // line to the byte range it covers.
    let index = file.line_index();
    println!("- lines: {}", index.num_lines());

    let show_line = 2; // line index is 0-based
    if let Some(line) = index.line_range(show_line) {
        println!(
            "- line {} spans bytes {}..{}",
            show_line + 1,
            line.start(),
            line.end()
        );
    }

    // Step 3: Get cursor and move around
    //
    // A `Cursor` is a movable position over the same snapshot, for walking bytes
    // in order instead of indexing them. Note where `advance_to` stops: in front
    // of the newline, leaving it for the caller to inspect.
    let mut cursor = file.cursor();

    cursor.advance_to(b'\n');
    if cursor.peek() == Some(b'\n') {
        let pos = cursor.position();
        // A position taken from this snapshot always resolves, but the lookup
        // stays total: there is no substituted `(0, 0)` to fall back on.
        if let Some((line, col)) = index.line_column(pos) {
            println!("- first newline at {line}:{col} (byte {pos})");
        }
    } else {
        println!("- no newline in this file");
    }

    Ok(())
}
