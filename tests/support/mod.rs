//! Shared helpers for the integration tests.
//!
//! [`ByteFile`](mmap_cursor::ByteFile) is the crate's filesystem entry point, so
//! its tests belong here rather than in a unit-test module: the contract is
//! about real files, real mmaps and real operating-system errors, none of which
//! an in-module test can exercise.
//!
//! Every test binary compiles its own copy of this module, so a helper that one
//! binary does not happen to use must not warn as dead code.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Process-wide counter keeping temporary paths unique under parallel tests.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A per-test temporary directory, removed on drop.
///
/// The name embeds the process id and a counter, so test binaries running
/// concurrently (and tests running in parallel inside one binary) never share a
/// directory. This is hand-rolled on purpose: the crate's dependency list is
/// deliberately minimal and a dev-dependency for a dozen lines is not worth it.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a fresh, unique, empty directory tagged with `tag`.
    pub fn new(tag: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mmap-cursor-test-{}-{id}-{tag}",
            std::process::id()
        ));
        // A directory left behind by an earlier crashed run is reused, so start
        // from a known-empty one.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the temporary test directory");
        Self { path }
    }

    /// The directory itself.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write `bytes` to `name` inside the directory and return the full path.
    pub fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, bytes).expect("write the temporary test file");
        path
    }

    /// A path inside the directory that does not exist yet.
    pub fn missing(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Replace `target` by staging the new bytes in a sibling file and renaming
    /// it over `target` — the atomic-`rename` write convention the crate's
    /// safety model assumes.
    pub fn replace_atomically(&self, target: &Path, bytes: &[u8]) {
        let staged = self.path.join(format!(
            "staged-{}",
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&staged, bytes).expect("write the staged replacement");
        fs::rename(&staged, target).expect("rename the staged file over the target");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a cleanup failure must never fail a test. Removing files
        // that are still mapped works on both platforms — the mapping inherited
        // the default `FILE_SHARE_DELETE` sharing mode on Windows.
        let _ = fs::remove_dir_all(&self.path);
    }
}
