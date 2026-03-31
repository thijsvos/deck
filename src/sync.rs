use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Tiny file-based sync between presenter and follower instances.
/// The presenter writes `slide_index reveal_count` to a temp file.
/// The follower polls it.
pub struct SyncFile {
    path: PathBuf,
}

impl SyncFile {
    /// Derive a deterministic sync file path from the presentation file path.
    pub fn for_file(input_path: &str) -> Self {
        let hash = simple_hash(input_path);
        let path = std::env::temp_dir().join(format!("deck-{:016x}.sync", hash));
        Self { path }
    }

    /// Write current state. Uses write-then-rename for atomicity.
    pub fn write(&self, slide: usize, reveal: usize) {
        let tmp = self.path.with_extension("tmp");
        if let Ok(mut f) = fs::File::create(&tmp) {
            let _ = writeln!(f, "{} {}", slide, reveal);
            let _ = fs::rename(&tmp, &self.path);
        }
    }

    /// Read current state from the sync file.
    pub fn read(&self) -> Option<(usize, usize)> {
        let content = fs::read_to_string(&self.path).ok()?;
        let mut parts = content.trim().split_whitespace();
        let slide = parts.next()?.parse().ok()?;
        let reveal = parts.next()?.parse().ok()?;
        Some((slide, reveal))
    }

    /// Remove the sync file.
    pub fn cleanup(&self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("tmp"));
    }
}

fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3); // FNV prime
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_roundtrip() {
        let sync = SyncFile::for_file("/tmp/test-deck-roundtrip.md");
        sync.write(5, 3);
        let result = sync.read();
        assert_eq!(result, Some((5, 3)));
        sync.cleanup();
    }

    #[test]
    fn read_missing_file_returns_none() {
        let sync = SyncFile::for_file("/tmp/nonexistent-deck-test.md");
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
        let sync = SyncFile::for_file("/tmp/test-deck-cleanup.md");
        sync.write(0, 0);
        assert!(sync.path.exists());
        sync.cleanup();
        assert!(!sync.path.exists());
    }
}
