# mmap-cursor

A zero-copy byte-position reader for parsers and lexers of large files. Open a
file into a read-only memory-mapped snapshot, move a byte cursor, read `&[u8]`
slices by byte position, and convert any position into `(line, column)`.

## Why



## Safety / concurrency model

- This crate never mutates files.
- `ByteFile::open` produces a **stable snapshot**; all positions, slices and
  line/column results are relative to that snapshot's content.
- The caller must ensure no external writer **truncates the mapped inode** while
  it is mapped. The standard convention is atomic `rename` for writes; under
  that guarantee reads never observe a partial file and never SIGBUS.

## Quick start


## API

| Type | Purpose |
|---|---|
| `ByteFile` | Open a file into an mmap-backed snapshot; `len`, `byte`, `cursor`, `line_index`. |
| `Cursor` | One-directional byte position cursor; `seek`, `position`, `read`. |
| `LineIndex` | Map a byte position to `(line, column)`; built in one pass. |


## Feature


## License

MIT.
