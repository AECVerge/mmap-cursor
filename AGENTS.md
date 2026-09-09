# AGENTS.md

Guidance for coding agents working on this crate. It is a small, format-agnostic,
zero-copy **byte-position reader** for large files, motivated by the IFC/STEP
(ISO 10303-21) "load then read" scenario. Keep it *minimal*.

## What this crate is

- Read-only access to the bytes of a file by byte position.
- Zero-copy reads backed by an mmap snapshot.
- Positions can be converted to `(line, column)` for diagnostics and reference.
- Format-agnostic: it must not know or care about the format of the bytes.

## What it is explicitly NOT (do not expand into these)

- Not a parser, tokenizer, or lexer.
- No entity semantics, schema, or validation.
- No reference resolution, object graph, or geometry.
- No writing, locking, or concurrency control. File replacement and write
  safety are the caller's (harness) responsibility.

## Safety contract (single source of truth; do not weaken)

- The crate never mutates files.
- Opening a file produces a **stable snapshot**: all positions, slices, and
  line/column values are relative to that snapshot's content.
- The caller guarantees the backing file is not truncated while it is mapped
  (the convention is atomic `rename` for writes). Under that guarantee,
  map-backed reads never observe a partial file and never SIGBUS.
- `unsafe` is confined to the single mmap call and must carry a SAFETY comment
  restating the guarantee above. Do not add `unsafe` elsewhere.
- Reads never panic: out-of-range or overflowing positions return `io::Error`
  (or `None` where documented). Keep all reads bounds-checked.

## Conventions

- Byte positions are `u64` offsets from the start of the snapshot.
- All positions are relative to the opened snapshot; they become invalid once
  the file is replaced, so re-open to re-read.
- Line/column are 1-based. The index splits on `\n`, `\r\n`, or a lone `\r` as
  a line terminator; a `\r` inside a `\r\n` pair is counted as a column of the
  line it terminates.
- Prefer returning errors over panicking, and keep any public `&[u8]` borrow
  tied to the snapshot's lifetime.

## Feature flags

- `generation` (default ON): detect whether the file was replaced after it was
  opened, for "read after update". This is the only code that stats the path.
- `simd` (default ON): use `memchr` for a SIMD-accelerated newline scan while
  building the line index. Without it (e.g. `--no-default-features`) a
  byte-scanning fallback is used; the index and the public API are identical
  either way.
- `--no-default-features` builds a pure byte reader that neither stats the path
  nor pulls the `memchr` dependency.

## Build / test / lint / docs

- `cargo build`
- `cargo test`
- `cargo test --all-targets --no-default-features`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo doc --no-deps --all-features`

## When changing the crate

- Keep the public API minimal and additive; do not remove or rename public
  items without a semver-major bump.
- Any change to the safety contract or the atomic-rename assumption must update
  the crate-level docs **and** this file.
- Keep tests covering: the empty file, out-of-range and overflowing positions,
  `\n`/`\r\n`/lone-`\r` line handling, and generation staleness.
