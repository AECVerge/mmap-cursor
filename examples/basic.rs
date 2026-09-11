//! Minimal demo: open a file, read bytes by position, and report positions as
//! (line, column). No tokenisation — this just walks bytes and locates `#`-led
//! record markers, which is the kind of "byte-position" reading this crate is
//! designed for (IFC/STEP files) without parsing anything.
//!
//! Usage: `cargo run --example basic -- path/to/file.ifc`

use filecursor::{ByteFile, BytePos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: basic <path-to-file>");

    let bf = ByteFile::open(&path)?;
    println!("len = {} bytes", bf.len());

    let index = bf.line_index();
    let mut cursor = bf.cursor();

    // Chunk through the snapshot; record where each '#' record marker is.
    let mut offset = 0usize;
    let mut found = 0usize;
    while let Some(chunk) = cursor.slice_n_ahead(4096) {
        // Operate purely on bytes: report the (line, column) of each '#'
        // that we see. This is a byte search, not a token stream.
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'#' {
                let pos = offset + i;
                if let Some((line, col)) = index.line_column(BytePos::new(pos)) {
                    println!("marker@{pos} -> line {line}, col {col}");
                    found += 1;
                }
            }
        }
        offset += chunk.len();
    }

    println!("total '#' markers: {found}");

    Ok(())
}
