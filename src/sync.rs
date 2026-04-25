use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::util::fnv1a;

/// Maximum bytes read from the sync file. The payload is a tiny ASCII
/// `"<slide> <reveal>"`, so 256 bytes is generous and caps the worst case
/// where a local attacker swaps the sync file for a symlink to `/dev/zero` or
/// a multi-GB file.
const MAX_SYNC_BYTES: u64 = 256;

/// Tiny file-based sync between presenter and follower instances.
/// The presenter writes `slide_index reveal_count` to a temp file.
/// The follower polls it.
pub struct SyncFile {
    path: PathBuf,
}

impl SyncFile {
    /// Derive a deterministic sync file path from the presentation file path.
    /// Canonicalizes first so `--present talk.md` and `--follow ./talk.md`
    /// (or one absolute, one relative) connect to the same sync file.
    /// Falls back to the raw path string if canonicalize fails for any reason
    /// (file not yet created, permission error, etc.).
    pub fn for_file(input_path: &str) -> Self {
        let canonical = fs::canonicalize(input_path)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| input_path.to_string());
        let hash = fnv1a(&canonical);
        let dir = sync_dir();
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("deck-{hash:016x}.sync"));
        Self { path }
    }

    /// Write current state. Uses write-then-rename for atomicity.
    /// All I/O errors are silently ignored — the sync channel is best-effort
    /// and a failed write recovers on the next call. The temp file
    /// (`<path>.tmp.<pid>`) is best-effort cleaned up on write failure.
    pub fn write(&self, slide: usize, reveal: usize) {
        let tmp = self.path.with_extension({
            let pid = std::process::id();
            format!("tmp.{pid}")
        });
        if let Ok(mut f) = fs::File::create(&tmp) {
            if writeln!(f, "{slide} {reveal}").is_ok() {
                let _ = f.flush();
                let _ = fs::rename(&tmp, &self.path);
            } else {
                let _ = fs::remove_file(&tmp);
            }
        }
    }

    /// Read current state from the sync file. Returns `None` if the file is
    /// missing, unreadable, malformed (fewer than two whitespace-separated
    /// tokens), or contains values that don't parse as `usize`.
    pub fn read(&self) -> Option<(usize, usize)> {
        let mut content = String::with_capacity(64);
        fs::File::open(&self.path)
            .ok()?
            .take(MAX_SYNC_BYTES)
            .read_to_string(&mut content)
            .ok()?;
        let mut parts = content.split_whitespace();
        let slide = parts.next()?.parse().ok()?;
        let reveal = parts.next()?.parse().ok()?;
        Some((slide, reveal))
    }

    /// Remove the sync file.
    pub fn cleanup(&self) {
        let _ = fs::remove_file(&self.path);
        // Clean up any leftover tmp file from this process
        let tmp = self.path.with_extension({
            let pid = std::process::id();
            format!("tmp.{pid}")
        });
        let _ = fs::remove_file(&tmp);
    }
}

/// User-private directory for sync files.
fn sync_dir() -> PathBuf {
    // Prefer XDG_RUNTIME_DIR (Linux, user-private, tmpfs)
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("deck");
    }
    // Fallback: user cache/config dir
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("deck");
    }
    // Last resort: temp dir (less secure but functional)
    std::env::temp_dir().join("deck")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Per-test unique key so parallel `cargo test` runs do not collide.
    fn unique_key(label: &str) -> String {
        format!(
            "/__deck_test__{}__{}__{}__{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            line!(),
        )
    }

    #[test]
    fn write_read_roundtrip() {
        let key = unique_key("roundtrip");
        let sync = SyncFile::for_file(&key);
        sync.write(5, 3);
        let result = sync.read();
        assert_eq!(result, Some((5, 3)));
        sync.cleanup();
    }

    #[test]
    fn read_missing_file_returns_none() {
        let key = unique_key("missing");
        let sync = SyncFile::for_file(&key);
        sync.cleanup(); // ensure clean state
        assert_eq!(sync.read(), None);
    }

    #[test]
    fn deterministic_path() {
        let a = SyncFile::for_file("talk.md");
        let b = SyncFile::for_file("talk.md");
        assert_eq!(a.path, b.path);
    }

    #[test]
    fn different_files_different_paths() {
        let a = SyncFile::for_file("a.md");
        let b = SyncFile::for_file("b.md");
        assert_ne!(a.path, b.path);
    }

    #[test]
    fn cleanup_removes_files() {
        let key = unique_key("cleanup");
        let sync = SyncFile::for_file(&key);
        sync.write(0, 0);
        assert!(sync.path.exists());
        sync.cleanup();
        assert!(!sync.path.exists());
    }

    #[test]
    fn sync_dir_is_not_tmp_root() {
        let dir = sync_dir();
        // Should be inside a "deck" subdirectory, not directly in /tmp
        assert!(dir.ends_with("deck"));
    }

    #[test]
    fn relative_and_absolute_resolve_to_same_path_when_canonicalizable() {
        // Use a file that actually exists (Cargo.toml at the project root)
        // so canonicalize() succeeds and equates the two forms.
        let cwd = std::env::current_dir().expect("cwd");
        let abs = cwd.join("Cargo.toml");
        if abs.exists() {
            let a = SyncFile::for_file("Cargo.toml");
            let b = SyncFile::for_file(abs.to_str().unwrap());
            assert_eq!(a.path, b.path, "canonicalize should equate paths");
        }
    }
}
