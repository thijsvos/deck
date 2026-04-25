use std::collections::{HashMap, VecDeque};
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::imageops::FilterType;
use image::RgbaImage;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// Best image protocol the terminal supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageProtocol {
    Kitty,
    Sixel,
    HalfBlocks,
}

/// Cap on decoded image dimensions and allocation, to defuse decompression
/// bombs in attacker-controlled image files.
const MAX_IMAGE_DIM: u32 = 8192;

/// FIFO-evicted cache. Insertion order is tracked so we never thrash the
/// whole cache on overflow. On `insert`, oldest entries are popped one at a
/// time until `len < capacity`; in steady state with a fixed capacity this
/// drops exactly one entry per insert.
struct FifoCache<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
    capacity: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V> FifoCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&self, k: &K) -> Option<&V> {
        self.map.get(k)
    }

    fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    fn insert(&mut self, k: K, v: V) {
        while self.map.len() >= self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            } else {
                break;
            }
        }
        self.order.push_back(k.clone());
        self.map.insert(k, v);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

impl<V> FifoCache<String, V> {
    /// Borrowed-key lookup for `String`-keyed caches. Avoids the per-call
    /// `to_string()` that callers would otherwise need to construct an owned
    /// key for `HashMap::contains_key`.
    fn contains_key_str(&self, k: &str) -> bool {
        self.map.contains_key(k)
    }

    fn get_str(&self, k: &str) -> Option<&V> {
        self.map.get(k)
    }
}

/// Cache key keyed by the raw `src` string. Avoids canonicalizing the path
/// on every frame's lookup; canonicalize only runs on cache miss to validate
/// the path.
type ResizedKey = (String, u16, u16);

/// Multi-tier image cache.
///
/// - `originals`: decoded RGBA images keyed by raw `src`
/// - `resized`: resized images keyed by `(src, cols, rows)`
/// - `encoded`: pre-encoded Kitty base64 PNG keyed by `(src, cols, rows)`
///
/// Each tier is FIFO-evicted independently. Decode + resize + encode each
/// happen at most once per unique key. Sandbox validation (`resolve_and_validate`)
/// runs on every cache miss; there is no per-`src` validation cache because the
/// canonicalize syscall is cheap relative to image decode and a stale validation
/// cache would silently keep accepting paths after `base_dir` moved.
pub struct ImageCache {
    originals: FifoCache<String, Arc<RgbaImage>>,
    resized: FifoCache<ResizedKey, Arc<RgbaImage>>,
    encoded: FifoCache<ResizedKey, String>,
}

const DEFAULT_MAX_RESIZED: usize = 64;
const DEFAULT_MAX_ORIGINALS: usize = 16;
const DEFAULT_MAX_ENCODED: usize = 64;

impl ImageCache {
    /// Construct an empty cache with the default capacities (16 originals,
    /// 64 resized, 64 encoded).
    pub fn new() -> Self {
        Self {
            originals: FifoCache::new(DEFAULT_MAX_ORIGINALS),
            resized: FifoCache::new(DEFAULT_MAX_RESIZED),
            encoded: FifoCache::new(DEFAULT_MAX_ENCODED),
        }
    }

    /// Resolve and sandbox-check `src`. Returns the canonicalized path on success.
    ///
    /// The check applies to both relative and absolute paths from the markdown:
    /// the resolved path must live under `base_dir`, even when the markdown writes
    /// an absolute `src`. Symlinks under `base_dir` are refused outright — without
    /// this, a symlink at e.g. `./innocent.png -> /home/user/.ssh/id_rsa` would
    /// `canonicalize()` to a target outside `base_dir` and fall through to a
    /// decode error that still leaks file existence via timing.
    fn resolve_and_validate(&self, src: &str, base_dir: &Path) -> Option<PathBuf> {
        let resolved = resolve(src, base_dir)?;
        let base = base_dir.canonicalize().ok()?;
        if !resolved.starts_with(&base) {
            return None;
        }
        // Refuse symlinks (resolved is already canonicalized, but the original
        // src may have been a symlink whose canonical target happens to land
        // inside base_dir; symlink_metadata sees the link itself).
        let original = if Path::new(src).is_absolute() {
            PathBuf::from(src)
        } else {
            base_dir.join(src)
        };
        if std::fs::symlink_metadata(&original)
            .ok()?
            .file_type()
            .is_symlink()
        {
            return None;
        }
        Some(resolved)
    }

    /// Load, resize, and cache an image for the given area dimensions.
    /// Returns `None` if the file can't be read or decoded.
    ///
    /// Relative `src` paths are sandboxed to `base_dir`; absolute paths are
    /// trusted (user-typed) and bypass the sandbox check.
    pub fn get_resized(
        &mut self,
        src: &str,
        base_dir: &Path,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<&Arc<RgbaImage>> {
        let key: ResizedKey = (src.to_string(), max_cols, max_rows);

        if self.resized.contains(&key) {
            return self.resized.get(&key);
        }

        // Cache miss: validate path, decode if needed, resize.
        let resolved = self.resolve_and_validate(src, base_dir)?;

        if !self.originals.contains_key_str(src) {
            let img = decode_with_limits(&resolved)?;
            self.originals.insert(src.to_string(), Arc::new(img));
        }

        let original = self.originals.get_str(src)?.clone();
        let resized = resize_to_fit(&original, max_cols, max_rows);
        self.resized.insert(key.clone(), Arc::new(resized));
        self.resized.get(&key)
    }

    /// Get pre-encoded Kitty base64 PNG for a cached resized image.
    /// Computes and caches on first call for each `(src, cols, rows)`.
    /// Returns `None` if no resized image has been cached for this key
    /// (caller must invoke `get_resized` first), or if PNG encoding fails.
    /// `_base_dir` is unused — kept for symmetry with `get_resized`.
    pub fn get_encoded_kitty(
        &mut self,
        src: &str,
        _base_dir: &Path,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<&str> {
        let key: ResizedKey = (src.to_string(), max_cols, max_rows);

        if self.encoded.contains(&key) {
            return self.encoded.get(&key).map(|s| s.as_str());
        }

        let img = self.resized.get(&key)?;
        let b64 = encode_kitty_png(img)?;
        self.encoded.insert(key.clone(), b64);
        self.encoded.get(&key).map(|s| s.as_str())
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve(src: &str, base_dir: &Path) -> Option<PathBuf> {
    let full_path = if Path::new(src).is_absolute() {
        PathBuf::from(src)
    } else {
        base_dir.join(src)
    };
    full_path.canonicalize().ok()
}

/// Decode an image with conservative dimension limits to defuse decompression
/// bombs. Falls back to None on any decode error.
fn decode_with_limits(path: &Path) -> Option<RgbaImage> {
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIM);
    limits.max_image_height = Some(MAX_IMAGE_DIM);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    Some(img.to_rgba8())
}

/// Encode RGBA image to PNG then base64 for Kitty protocol.
fn encode_kitty_png(img: &RgbaImage) -> Option<String> {
    use base64::Engine;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;

    let mut png_buf = Vec::new();
    let encoder = PngEncoder::new(&mut png_buf);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;

    Some(base64::engine::general_purpose::STANDARD.encode(&png_buf))
}

/// A queued image render for Kitty/Sixel post-draw pass.
pub struct DeferredImage {
    pub x: u16,
    pub y: u16,
    pub cols: u16,
    pub rows: u16,
    pub rgba: Arc<RgbaImage>,
    pub protocol: ImageProtocol,
    /// Pre-encoded base64 PNG for Kitty (avoids re-encoding per frame).
    pub kitty_b64: Option<String>,
}

/// Detect the best image protocol by checking environment variables.
pub fn detect_protocol() -> ImageProtocol {
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_lowercase();
    let term = std::env::var("TERM").unwrap_or_default();

    // Kitty protocol terminals
    if term == "xterm-kitty" || std::env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageProtocol::Kitty;
    }
    if term_program == "ghostty" || std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
        return ImageProtocol::Kitty;
    }
    if term_program == "wezterm" {
        return ImageProtocol::Kitty;
    }
    if term_program == "konsole" {
        return ImageProtocol::Kitty;
    }

    // Sixel terminals
    if term_program.contains("iterm") || std::env::var("ITERM_SESSION_ID").is_ok() {
        return ImageProtocol::Sixel;
    }
    if term_program == "foot" || term.starts_with("foot") {
        return ImageProtocol::Sixel;
    }

    ImageProtocol::HalfBlocks
}

/// Resize image to fit within terminal cell area, preserving aspect ratio.
/// Each cell row = 2 vertical pixels (half-block technique).
pub fn resize_to_fit(img: &RgbaImage, max_cols: u16, max_rows: u16) -> RgbaImage {
    let max_px_w = max_cols as u32;
    let max_px_h = max_rows as u32 * 2;

    let (orig_w, orig_h) = img.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return img.clone();
    }

    let scale_w = max_px_w as f64 / orig_w as f64;
    let scale_h = max_px_h as f64 / orig_h as f64;
    let scale = scale_w.min(scale_h); // allow upscaling to fill the slide

    let new_w = ((orig_w as f64 * scale).round() as u32).max(1);
    let mut new_h = ((orig_h as f64 * scale).round() as u32).max(1);

    // Even height for clean half-block pairing
    if !new_h.is_multiple_of(2) {
        new_h += 1;
    }

    image::imageops::resize(img, new_w, new_h, FilterType::Lanczos3)
}

/// Render image as half-block characters directly into the ratatui buffer.
/// Each cell uses `▀` with fg = top pixel, bg = bottom pixel.
pub fn render_halfblocks(buf: &mut Buffer, area: Rect, img: &RgbaImage) {
    let (img_w, img_h) = img.dimensions();
    let rows = img_h.div_ceil(2);

    for ty in 0..rows.min(area.height as u32) {
        for tx in 0..img_w.min(area.width as u32) {
            let top_py = ty * 2;
            let bot_py = top_py + 1;

            let top = img.get_pixel(tx, top_py);
            let bot = if bot_py < img_h {
                img.get_pixel(tx, bot_py)
            } else {
                top
            };

            let fg = Color::Rgb(top[0], top[1], top[2]);
            let bg = Color::Rgb(bot[0], bot[1], bot[2]);

            let pos = (area.x + tx as u16, area.y + ty as u16);
            if let Some(cell) = buf.cell_mut(pos) {
                cell.set_char('▀');
                cell.set_style(Style::default().fg(fg).bg(bg));
            }
        }
    }
}

/// Flush deferred Kitty/Sixel images after terminal.draw().
pub fn flush_deferred<W: Write>(w: &mut W, images: &[DeferredImage]) -> std::io::Result<()> {
    for d in images {
        match d.protocol {
            ImageProtocol::Kitty => render_kitty(w, d)?,
            ImageProtocol::Sixel => render_sixel_fallback(w, d)?,
            ImageProtocol::HalfBlocks => {} // already in buffer
        }
    }
    Ok(())
}

/// Kitty Graphics Protocol: send pre-encoded base64 PNG in an APC escape.
/// No-op (returns Ok) if `d.kitty_b64` is `None` — caller is expected to
/// have populated it via [`ImageCache::get_encoded_kitty`].
fn render_kitty<W: Write>(w: &mut W, d: &DeferredImage) -> std::io::Result<()> {
    let b64 = match d.kitty_b64 {
        Some(ref s) => s,
        None => return Ok(()), // no encoded data available
    };

    // Move cursor to image position (1-based)
    write!(w, "\x1b[{};{}H", d.y + 1, d.x + 1)?;

    // Send chunked if >4096 bytes
    let chunk_size = 4096;
    let bytes = b64.as_bytes();
    let total_chunks = bytes.len().div_ceil(chunk_size).max(1);

    if total_chunks <= 1 {
        write!(w, "\x1b_Ga=T,f=100,c={},r={};{}\x1b\\", d.cols, d.rows, b64)?;
    } else {
        for (i, chunk) in bytes.chunks(chunk_size).enumerate() {
            let is_last = i + 1 == total_chunks;
            let m = if is_last { 0 } else { 1 };
            let chunk_str =
                std::str::from_utf8(chunk).expect("base64 output is always valid ASCII/UTF-8");

            if i == 0 {
                write!(
                    w,
                    "\x1b_Ga=T,f=100,c={},r={},m={};{}\x1b\\",
                    d.cols, d.rows, m, chunk_str
                )?;
            } else {
                write!(w, "\x1b_Gm={m};{chunk_str}\x1b\\")?;
            }
        }
    }

    w.flush()
}

/// Sixel fallback: re-render as half-blocks via escape sequences.
/// Buffers each row into a String to reduce write call overhead.
fn render_sixel_fallback<W: Write>(w: &mut W, d: &DeferredImage) -> std::io::Result<()> {
    let (img_w, img_h) = d.rgba.dimensions();
    let rows = img_h.div_ceil(2);

    let mut row_buf = String::with_capacity(img_w as usize * 40);

    for ty in 0..rows.min(d.rows as u32) {
        row_buf.clear();
        // Move cursor to start of this row
        let _ = write!(row_buf, "\x1b[{};{}H", d.y + 1 + ty as u16, d.x + 1);

        for tx in 0..img_w.min(d.cols as u32) {
            let top_py = ty * 2;
            let bot_py = top_py + 1;

            let top = d.rgba.get_pixel(tx, top_py);
            let bot = if bot_py < img_h {
                d.rgba.get_pixel(tx, bot_py)
            } else {
                top
            };

            // ANSI 24-bit color: fg=top, bg=bottom, char=▀
            let _ = write!(
                row_buf,
                "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                top[0], top[1], top[2], bot[0], bot[1], bot[2]
            );
        }

        w.write_all(row_buf.as_bytes())?;
    }
    // Reset colors
    write!(w, "\x1b[0m")?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image(w: u32, h: u32) -> RgbaImage {
        RgbaImage::new(w, h)
    }

    #[test]
    fn resize_wide_image_scales_down() {
        let img = make_image(200, 50);
        let resized = resize_to_fit(&img, 100, 50);
        assert!(resized.width() <= 100);
    }

    #[test]
    fn resize_tall_image_scales_down() {
        let img = make_image(50, 200);
        let resized = resize_to_fit(&img, 100, 25); // 25 rows = 50px tall
        assert!(resized.height() <= 50);
    }

    #[test]
    fn resize_preserves_aspect_ratio() {
        let img = make_image(200, 100);
        let resized = resize_to_fit(&img, 100, 100);
        // 200:100 = 2:1 ratio. Fitting into 100 cols x 200 px tall -> limited by width
        // Result: 100 x 50 (maintaining 2:1)
        let ratio_orig = 200.0 / 100.0;
        let ratio_new = resized.width() as f64 / resized.height() as f64;
        assert!((ratio_orig - ratio_new).abs() < 0.1);
    }

    #[test]
    fn resize_forces_even_height() {
        let img = make_image(100, 51);
        let resized = resize_to_fit(&img, 100, 50);
        assert_eq!(resized.height() % 2, 0);
    }

    #[test]
    fn resize_zero_dimension_returns_clone() {
        let img = make_image(0, 0);
        let resized = resize_to_fit(&img, 100, 50);
        assert_eq!(resized.dimensions(), (0, 0));
    }

    #[test]
    fn resize_small_image_upscales() {
        let img = make_image(10, 10);
        let resized = resize_to_fit(&img, 100, 50);
        assert!(resized.width() > 10);
    }

    #[test]
    fn detect_protocol_fallback() {
        // In test environment, none of the terminal-specific env vars should be set
        // unless running inside Kitty/Ghostty/etc. Just verify it doesn't panic.
        let _protocol = detect_protocol();
    }

    #[test]
    fn image_cache_new_is_empty() {
        let cache = ImageCache::new();
        assert_eq!(cache.originals.len(), 0);
        assert_eq!(cache.resized.len(), 0);
    }

    #[test]
    fn image_cache_missing_file_returns_none() {
        let mut cache = ImageCache::new();
        assert!(cache
            .get_resized("nonexistent.png", Path::new("."), 80, 24)
            .is_none());
    }

    #[test]
    fn path_traversal_blocked() {
        let mut cache = ImageCache::new();
        // Trying to escape base_dir should return None
        assert!(cache
            .get_resized("../../../etc/passwd", Path::new("."), 80, 24)
            .is_none());
    }

    #[test]
    fn encode_kitty_png_produces_base64() {
        let img = make_image(4, 4);
        let b64 = encode_kitty_png(&img);
        assert!(b64.is_some());
        let s = b64.unwrap();
        assert!(!s.is_empty());
        // Base64 should only contain valid characters
        assert!(s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='));
    }

    #[test]
    fn resize_1x1_upscales_with_even_height() {
        let img = make_image(1, 1);
        let resized = resize_to_fit(&img, 100, 50);
        assert!(resized.width() > 1);
        assert!(resized.height() > 1);
        assert!(resized.height().is_multiple_of(2));
    }

    #[test]
    fn render_halfblocks_small_image() {
        use ratatui::buffer::Buffer;
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // top-left red
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 255])); // top-right green
        img.put_pixel(0, 1, image::Rgba([0, 0, 255, 255])); // bot-left blue
        img.put_pixel(1, 1, image::Rgba([255, 255, 0, 255])); // bot-right yellow

        let area = Rect::new(0, 0, 2, 1); // 2 cols x 1 row = 2x2 pixels
        let mut buf = Buffer::empty(area);
        render_halfblocks(&mut buf, area, &img);

        // Both cells should be '▀'
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "▀");
        assert_eq!(buf.cell((1, 0)).unwrap().symbol(), "▀");
    }

    #[test]
    fn flush_deferred_halfblocks_is_noop() {
        let images = vec![DeferredImage {
            x: 0,
            y: 0,
            cols: 2,
            rows: 1,
            rgba: std::sync::Arc::new(make_image(2, 2)),
            protocol: ImageProtocol::HalfBlocks,
            kitty_b64: None,
        }];
        let mut output = Vec::new();
        flush_deferred(&mut output, &images).unwrap();
        assert!(output.is_empty()); // HalfBlocks writes nothing
    }

    #[test]
    fn fifo_cache_evicts_oldest() {
        let mut c: FifoCache<u32, &'static str> = FifoCache::new(2);
        c.insert(1, "a");
        c.insert(2, "b");
        c.insert(3, "c"); // evicts 1
        assert!(c.get(&1).is_none());
        assert_eq!(c.get(&2), Some(&"b"));
        assert_eq!(c.get(&3), Some(&"c"));
    }
}
