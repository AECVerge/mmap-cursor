# mmap-cursor

[![crates.io](https://img.shields.io/crates/v/mmap-cursor.svg)](https://crates.io/crates/mmap-cursor)
[![docs.rs](https://docs.rs/mmap-cursor/badge.svg)](https://docs.rs/mmap-cursor)
[![CI](https://github.com/AECVerge/mmap-cursor/actions/workflows/ci.yml/badge.svg)](https://github.com/AECVerge/mmap-cursor/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#minimum-supported-rust-version)
[![license](https://img.shields.io/crates/l/mmap-cursor.svg)](#license)

A zero-copy byte-position reader for parsers and lexers of large files. Open a
file into a read-only memory-mapped snapshot, move a byte cursor, read `&[u8]`
slices by byte position, and convert any position into `(line, column)`.

## Why

Parsers, lexers and linters tend to bring their own cursor over an in-memory
buffer. That works while the input is small, because the whole file can be read
into a `String` or a `Vec<u8>` first.

The files this crate is aimed at are not small — hundreds of megabytes, and
occasionally gigabytes. Reading one of those into memory means paying for a full
copy before any work starts.

`mmap-cursor` maps the file once, read-only, and hands out positions into that
mapping. A slice is a borrow of the mapping rather than a copy, and a single
snapshot answers both "what bytes are at this position" and "which line and
column does this position fall on". `Cursor` is deliberately a broad API for
walking those bytes — seek, peek ahead, slice, and advance by one byte, by a
count, to a delimiter, past a delimiter, or until a predicate says stop —
because composing those calls is how you build the scanning loop your format
needs.

The crate is format-agnostic and parses nothing. It has no idea what a token, a
comment or a record is: `/*` is format knowledge, and it stays in your code.
What the crate supplies is the position, the slice, and the line/column lookup
— the infrastructure a parser is built on.

## Quick start

```toml
[dependencies]
mmap-cursor = "0.1"
```

```rust,no_run
use mmap_cursor::{ByteFile, Error};

fn main() -> Result<(), Error> {
    // Opening maps the file read-only and copies nothing. The snapshot stays
    // stable for as long as `file` lives, whatever happens to the path after.
    let file = ByteFile::open("large.ifc")?;

    // One snapshot, two views: a line index for looking positions up, and a
    // cursor for walking bytes in order.
    let index = file.line_index();
    let mut cursor = file.cursor();

    // `advance_to` stops *in front of* the byte, leaving it to be inspected.
    cursor.advance_to(b'\n');
    let position = cursor.position();
    if let Some((line, column)) = index.line_column(position) {
        println!("first newline at {line}:{column} (byte {position})");
    }

    Ok(())
}
```

Three runnable programs in `examples/` go further:

| Example | Shows |
| --- | --- |
| `basic` | Open, index, walk, and report a position as `line:column`. |
| `comments` | Caller-side scanning: find every `/* ... */` remark as a byte range, and print it as a zero-copy slice. |
| `diagnostic` | Read what a message needs, **drop the snapshot**, then render a rustc-style diagnostic from the position and index that outlived it. |

```console
cargo run --example basic
cargo run --example comments
cargo run --example diagnostic
```

## Safety / concurrency model

`ByteFile::open` produces a **stable snapshot**: every position, slice and
`(line, column)` value is relative to that snapshot's content, and goes on
describing it however the path changes afterwards.

- The crate never mutates a file.
- The caller must ensure no external writer **truncates the mapped file** while
  it is mapped. Under that guarantee a map-backed read never observes a partial
  file, and never faults.
- The convention is to stage new contents in a sibling file and **rename it
  over** the old path: the mapped inode is left alone, so an open snapshot keeps
  reading the old bytes, while a reader that wants the new version re-opens the
  path.
- Truncating a mapped file, or overwriting it in place, is outside that
  convention; what a snapshot observes then is undefined.

The crate does not track the path it was opened from. Noticing that a file was
replaced, and deciding when to re-open, is the caller's job.

## Reading by byte position

A position is a `usize` offset from the start of the snapshot, so it indexes the
mapping directly. The largest file a snapshot can address is therefore bounded
by the platform's `usize`.

- Reads never panic. A position at or past the end, or one whose arithmetic
  overflows, comes back as `None` — never a panic, and never a substituted
  value.
- `(line, column)` is 1-based, and a column is counted in **bytes**, not
  characters.
- Lines are terminated by `\n`, `\r\n`, or a lone `\r`. The `\r` of a `\r\n`
  pair counts as a column of the line it terminates.
- Bytes are never decoded: non-UTF-8 sequences and NUL bytes pass through
  unchanged.
- `BytePos`'s `+` and `-` panic on overflow in **every** profile, release
  included. The checked and saturating methods report the same conditions as
  values, for callers who would rather not panic.

## API

| Type | Purpose |
| --- | --- |
| `ByteFile` | A read-only mapping of one file: `bytes`, `cursor`, `line_index`, `len`, `is_empty`. |
| `Cursor` | Moves over a snapshot: `seek`, `position`, `peek`, `peek_n_ahead`, `slice`, `slice_n_ahead`, `rest`, and the `advance*` methods. |
| `LineIndex` | Position and line lookups: `line_column`, `line_for_offset`, `line_start`, `line_range`, `num_lines`. |
| `BytePos` | A byte offset from the start of a snapshot. |
| `ByteRange` | A half-open `[start, end)` pair of positions. |
| `Error`, `Result` | The crate's error type and its result alias. |

Only two operations can fail: `ByteFile::open` reports I/O errors and a file
length the platform cannot address, and `ByteRange::try_new` reports reversed
bounds. Everything else that can go wrong is an ordinary `None`. Full signatures
and per-item documentation are on [docs.rs](https://docs.rs/mmap-cursor).

## Feature flags

| Feature | Default | Effect |
| --- | --- | --- |
| `simd` | on | Scans for newlines with `memchr`. Without it, a byte-scanning fallback produces an identical index. |
| `serde` | off | Derives `Serialize` on `BytePos` and `ByteRange`. |

`--no-default-features` builds a pure byte reader that pulls in no optional
dependency. With `serde` enabled, a position serializes at the width of the
platform's `usize`, as it is in memory.

## Minimum supported Rust version

1.85, which `edition = "2024"` requires. CI has a job pinned to exactly that
version, so the declared MSRV cannot drift from the one that is tested. Raising
it is recorded in the [changelog](CHANGELOG.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
