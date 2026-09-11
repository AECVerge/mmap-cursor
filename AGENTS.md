# AGENTS.md

Guidance for coding agents working on this crate. It is a small, format-agnostic,
zero-copy **byte-position reader** for large files, motivated by the "load a large
file once, then read byte positions from it" scenario. Keep it *minimal*.

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
- No change, staleness, or generation detection: noticing that a file was
  replaced and re-opening to read the new version is the caller's job. The crate
  never re-stats the path it was opened from.

## Safety contract (single source of truth; do not weaken)

- The crate never mutates files.
- Opening a file produces a **stable snapshot**: all positions, slices, and
  line/column values are relative to that snapshot's content.
- The caller guarantees the backing file is not truncated while it is mapped
  (the convention is atomic `rename` for writes). Under that guarantee,
  map-backed reads never observe a partial file and never SIGBUS.
- `unsafe` is confined to the single mmap call and must carry a SAFETY comment
  restating the guarantee above. Do not add `unsafe` elsewhere.
- Reads never panic: an out-of-range or overflowing position is reported as
  `None`, never as a panic or a substituted value. Keep all reads bounds-checked.
- Arithmetic on positions is the caller's own computation: the `BytePos`
  operators panic on overflow in **every** profile (implemented over
  `checked_add`/`checked_sub`, not `usize`'s debug-only check) and document that.
  No code inside the crate uses panicking arithmetic — internal advances use
  `BytePos::saturating_add`, clamped to EOF.

## Conventions

- Byte positions are `usize` offsets from the start of the snapshot. This keeps
  them directly indexable into the memory-mapped snapshot; the largest file a
  snapshot can address is therefore bounded by the platform's `usize`. The
  serialized width of a position (via the `serde` feature) is likewise
  platform-dependent.
- `BytePos` for offset, absolute/ relative position; `ByteRange` for a range 
  marked by 2 `BytePos`es; `usize` for raw index, length of `ByteRange`, `&[u8]`,
  user given values(steps to advance), etc.
- All positions are relative to the opened snapshot; they become invalid once
  the file is replaced, so re-open to re-read.
- Line/column are 1-based. The index splits on `\n`, `\r\n`, or a lone `\r` as
  a line terminator; a `\r` inside a `\r\n` pair is counted as a column of the
  line it terminates.
- Prefer returning errors over panicking, and keep any public `&[u8]` borrow
  tied to the snapshot's lifetime.

## Feature flags

- `serde` (default OFF): derive `Serialize` on the position types
  (`BytePos`, `ByteRange`). Opt-in so consumers that never serialize positions
  don't pull in `serde`.
- `simd` (default ON): use `memchr` for a SIMD-accelerated newline scan while
  building the line index. Without it (e.g. `--no-default-features`) a
  byte-scanning fallback is used; the index and the public API are identical
  either way.
- `--no-default-features` builds a pure byte reader that pulls no optional
  dependency and never touches the file path after `open`.

## Build / test / lint / docs

- `cargo build`
- `cargo test`
- `cargo test --doc` (doctests — note that `--all-targets` does **not** run them)
- `cargo test --all-targets --no-default-features`
- `cargo test --release --all-targets` (overflow checks are off in release by
  default, so this catches profile-dependent panic/overflow behaviour)
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo doc --no-deps --all-features`

## MSRV

- `rust-version` in `Cargo.toml` is the oldest toolchain this crate promises to
  build on, and it is **tested, not assumed**: CI's `msrv` job is pinned to
  exactly that version. The lower bound is currently set by `edition = "2024"`,
  which requires 1.85.
- It is a public contract. Raising it means bumping `Cargo.toml` and the `msrv`
  job together, and recording the change in `CHANGELOG.md`.
- `edition = "2024"` implies `resolver = "3"` (MSRV-aware), so this value also
  steers dependency selection: keep it at the true floor, not at an aspirational
  version that nothing ever compiles against.

## Testing layout

- `ByteFile` is the crate's filesystem entry point, and its contract lives in
  `tests/`: opening real files, mmap behaviour, OS errors and the
  snapshot/replacement semantics cannot be unit-tested. **Do not add a
  `#[cfg(test)]` module to `src/bytefile.rs`.** It is covered by
  `tests/bytefile_open.rs` (`open` success and failure paths),
  `tests/snapshot.rs` (stable snapshot, atomic `rename`, removal) and
  `tests/end_to_end.rs` (`ByteFile` × `Cursor` × `LineIndex`).
- Keep the pure logic unit-testable and unit-tested in-module: `Cursor`,
  `LineIndex`, `BytePos`/`ByteRange` and `Error`.
- `tests/support/` holds the shared helper (temporary directories, atomic
  replacement). It is hand-rolled on purpose — no dev-dependency is added for it.
- **Never check in a fixture whose exact bytes matter.** `.gitattributes` sets
  `* text=auto`, so a committed `\r\n` or lone-`\r` fixture is rewritten on
  commit and the test would silently assert the wrong bytes. Build byte-exact
  input at runtime with byte literals (`b"a\r\nb"`, `b"\xC3\xA9"`) and write it
  into a temporary directory.
- `Error::FileTooLarge` is unreachable through `open` on a 64-bit target; its
  formatting and conversions are covered by the `src/error.rs` unit tests. Do not
  fake an integration test for it, and do not claim that `open` is fully covered.

## When changing the crate

- Keep the public API minimal and additive; do not remove or rename public
  items without a semver-major bump.
- Any change to the safety contract or the atomic-rename assumption must update
  the crate-level docs **and** this file.
- Keep tests covering: the empty file, out-of-range and overflowing positions,
  and `\n`/`\r\n`/lone-`\r` line handling. See *Testing layout* for where each
  kind of test belongs.
