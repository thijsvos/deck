# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Security
- Cap decoded image dimensions and allocations to defuse decompression-bomb images
  in shared decks

### Fixed
- Sync files now connect across CWDs and absolute/relative path forms by
  canonicalizing the input path before hashing
- Image references with absolute paths (e.g. `/usr/share/icons/foo.png`) are no
  longer silently rejected by the path-traversal sandbox
- Frontmatter parser handles CRLF line endings (Windows-authored markdown)
- Decrypt entrance no longer corrupts wide-grapheme continuation cells in titles
- Big-text rendering falls back to plain text when any character lacks a glyph,
  rather than silently dropping the unsupported chars (e.g. "café" → "CAFÉ")

### Performance
- Highlighted code blocks cached behind `Arc` — cache hits are a refcount bump
  instead of a deep clone of every line and span
- Image cache keyed by the raw `src` string so repeated frames skip
  per-frame `canonicalize` syscalls
- Image cache uses FIFO eviction (one entry at a time) instead of clearing
  the whole cache on overflow; originals are now bounded too

### Internal
- Shared FNV-1a `fnv1a` helper in `util.rs`, replacing duplicated impls
- `Rng::next_f64` now uses the full 53-bit f64 mantissa (no modulo bias)
- Sync and app tests use unique per-test paths so parallel `cargo test` runs
  do not collide
- Defensive `saturating_sub(1)` on `slides.len()` at navigation call sites
- Exhaustive `EntranceKind` match in render — adding a new variant is a
  compile error, not a silent no-op

## [0.1.0] - 2026-04-01

### Added
- Markdown slides with TOML frontmatter and `---` separators
- Big ASCII text rendering for H1 headings (Block and Large font styles)
- Progressive bullet point reveal (one-at-a-time, forward and backward)
- Column layouts with `::: columns` syntax
- Centered slides with `<!-- layout: center -->` directive
- Speaker notes with `<!-- note: ... -->` syntax
- Inline images with auto-detection of Kitty, Sixel, and half-block protocols
- Syntax highlighting for ~50 languages via syntect (base16-ocean.dark theme)
- Cinematic block entrance animations: heading decrypt, slide-in, code typewriter, bullet cascade, paragraph/image fade-in
- Presenter mode with current slide, next preview, notes, and timer
- Dual-screen mode: `--present` on your laptop, `--follow` on the projector (file-based sync, no networking)
- 10 animated mathematical backgrounds: aurora, matrix, plasma, lissajous, spiral, wave, rain, noise, lattice, orbit
- Per-slide background directives with `<!-- background: name -->`
- 4 slide transitions: glitch, fade, wipe, dissolve (theme-linked defaults)
- 4 color themes: hacker (default), corporate, catppuccin, minimal
- Status bar with title, author, slide count, and elapsed time
- Help overlay (`?`) and go-to-slide input (`:N Enter`)
- Vim-style keyboard navigation (hjkl, g/G, Space, Enter, arrows)
- Adaptive frame rate: 60fps during animations, 30fps for backgrounds, 100ms idle
- Image path traversal guard (images must stay within presentation directory)
- Sync files stored in user-private directory (`$XDG_RUNTIME_DIR` or `~/.cache`)
- Panic hook for clean terminal restore
- Cross-platform CI (Linux, macOS, Windows) with clippy, fmt, and tests
- Release workflow with prebuilt binaries for 4 targets
