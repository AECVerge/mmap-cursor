# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-09-18

### Added

- **`Cursor::eof`** — the position one past the last byte of the snapshot, as a
  `BytePos`. It is the cursor's own limit: constant for the life of a cursor,
  never less than `position`, the position every `seek` and `advance*` clamps to,
  and exactly where `is_eof` starts returning `true`. `eof().to_usize()` is the
  snapshot length, which `remaining_len` reports only relative to the current
  position. Purely additive.

## [0.1.0] - 2026-09-11

Initial release. The minimum supported Rust version is 1.85 (`edition = "2024"`).

### Added

- **`ByteFile`** — opens a file into a read-only, memory-mapped snapshot, and
  exposes `len`, `is_empty`, `bytes`, `cursor` and `line_index`. An empty file is
  a zero-length snapshot rather than an error, and a file whose length does not
  fit the platform's `usize` is rejected whole instead of being truncated. Its
  `Debug` reports the length only, never the contents.
- **`Cursor`** — a one-directional byte cursor over a snapshot: `position`,
  `is_eof`, `remaining_len`, `peek`, `peek_n_ahead`, `slice`, `slice_n_ahead`,
  `rest`, `seek`, `advance`, `advance_n_ahead`, `advance_if_starts_with`,
  `advance_until`, `advance_to` and `advance_through`. Reads are short at EOF
  instead of failing, and every advance clamps to EOF.
- **`LineIndex`** — a one-pass index over a snapshot, answering both directions:
  `num_lines`, `line_start` and `line_range` for a line's bytes, and
  `line_for_offset` and `line_column` for a position. It holds no borrow of the
  snapshot's bytes, so a `(line, column)` stays resolvable after they are
  dropped.
- **`BytePos`** — a `usize` byte offset from the start of a snapshot, with
  `checked_add`, `checked_sub`, `saturating_add` and `saturating_sub`. The `+`
  and `-` operators panic on overflow in every profile, release included.
- **`ByteRange`** — a half-open `[start, end)` range, built with `new` (which
  panics on reversed bounds) or the fallible `try_new`.
- **`Error` and `Result`** — I/O failures, a file length the platform cannot
  address, and reversed range bounds, with `Error::into_io` for callers whose own
  API is `io::Result`. `Error` is `#[non_exhaustive]`.
- **Optional `serde` support** for `BytePos` and `ByteRange`, off by default so
  that consumers who never serialize positions pull in no `serde`.
- **Stable snapshots** — opening produces a snapshot whose positions, slices and
  `(line, column)` values keep describing the file as it was at open time. The
  crate never mutates a file, and it never re-stats the path it was opened from:
  noticing that a path now holds a newer version, and re-opening for a fresh
  snapshot, is the caller's job. The write convention the safety model assumes is
  the atomic `rename`.
- **Reads that never panic** — a position at or past EOF, or arithmetic that
  overflows, is reported as `None`, never as a panic or a substituted value. The
  caller must not truncate the mapped file while it is mapped; under that
  guarantee, map-backed reads never observe a partial file.
- **Line handling** — lines are terminated by `\n`, `\r\n` or a lone `\r`.
  `(line, column)` is 1-based and counted in bytes, and the `\r` of a `\r\n` pair
  counts as a column of the line it terminates.
- **Format-agnostic bytes** — the crate never decodes anything, so non-UTF-8
  sequences and NUL bytes pass through unchanged; there is no lossy conversion
  anywhere along the way.
- **Feature flags** — the default `simd` feature scans for newlines with
  `memchr`, while a byte-scanning fallback produces an identical index without
  it. `--no-default-features` builds a pure byte reader that pulls no optional
  dependency and never touches the file path after opening.

[Unreleased]: https://github.com/AECVerge/mmap-cursor/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/AECVerge/mmap-cursor/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/AECVerge/mmap-cursor/releases/tag/v0.1.0
