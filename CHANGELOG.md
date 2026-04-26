# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.2.1] - 2026-04-26

### Security
- `image_renderer::resolve_and_validate` sandbox-checks every image path
  (relative *and* absolute) and refuses symlinks via `symlink_metadata`;
  the `validated` cache (which had a stale-validation hazard) is gone
- 16 MB cap on deck file reads; 256 B cap on sync-file reads — defuses
  a symlink-to-`/dev/zero` DoS
- Frontmatter end detection is line-anchored, so `\n---` inside TOML
  triple-quoted strings no longer terminates the frontmatter early
- Saturating `u16` arithmetic on every row computation in `render.rs`
  (release profile has `overflow-checks` off); `saturating_mul` for
  typewriter duration; `u16::try_from` for resized image dims instead
  of `as u16` truncation
- `Block::HorizontalRule` writes `─` cells directly into the buffer —
  drops the `"─".repeat(width)` allocation/DoS surface

### Performance
- `Highlighter` and the new `BigtextCache` rekey on an `fnv1a` hash so
  cache hits no longer allocate a `String` per frame
- `spans_to_line` returns `Line<'a>` borrowing from the parsed slide
  instead of cloning each span's `String` per frame
- `theme.bullet_prefix` is built once at theme construction; bullet
  rendering deduplicated into a single helper
- Typewriter slicing is byte-accurate via `char_indices`; the full-line
  `String` allocation that existed solely for counting is gone
- Code-block path reuses the cached `Arc<Vec<Line>>` once the
  typewriter is finished, skipping the per-frame `Vec<Line>` rebuild
- H1 path emits a single `Paragraph::new(Vec<Line>)` instead of one
  widget per row
- Animated-background loops fold the `is_empty` check into the
  `cell_mut` borrow — one buffer lookup per cell instead of two
- Kitty base64 chunks iterated directly without a `Vec<&[u8]>` collect
- ASCII fast path in `estimate_line_height` skips the East-Asian-Width
  walk for English text

### Internal
- `render_block` split into eight per-arm helpers; dispatcher is now
  ~12 lines, and `(u16, Option<Rect>)` is replaced by a named
  `BlockRender { height, entrance_target }` struct
- `render_help` / `render_goto` moved out of `app.rs` into
  `render::help_overlay` / `goto_overlay`
- `count_bullets` / `initial_reveal` are now `Slide::bullet_count` /
  `Slide::initial_reveal` methods
- Main-loop tick durations promoted to named `ANIMATION_TICK`,
  `BACKGROUND_TICK`, `IDLE_TICK` constants
- 8-arg `render_status_bar` replaced by a `StatusBar<'a>` config
  struct (kills the `#[allow(clippy::too_many_arguments)]` band-aid)
- `EntranceTracker` key widened to `(slide, block, sub)`; cascade items
  use `sub = item_i + 1`, removing the `block_idx * 10_000 + item_i`
  magic encoding

## [0.2.0] - 2026-04-25

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

### Documentation
- Public API surface (App, Deck, Theme, render, transitions, input, etc.) now
  carries rustdoc; existing docstrings updated to match post-refactor behavior
  (sync error handling, image cache tiers, entrance-tracker semantics)

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
