use std::collections::HashMap;
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

/// Cached images keyed by (path, target_cols, target_rows).
/// Stores already-resized images so resize only happens once per size.
pub struct ImageCache {
    /// Original decoded images keyed by path.
    originals: HashMap<String, Arc<RgbaImage>>,
    /// Resized images keyed by (path, cols, rows).
    resized: HashMap<(String, u16, u16), Arc<RgbaImage>>,
    /// Pre-encoded Kitty base64 PNG keyed by (path, cols, rows).
    encoded: HashMap<(String, u16, u16), String>,
    /// Maximum number of resized entries before eviction.
    max_resized: usize,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            originals: HashMap::new(),
            resized: HashMap::new(),
            encoded: HashMap::new(),
            max_resized: 64,
        }
    }

    /// Load, resize, and cache an image for the given area dimensions.
    /// Returns None if the file can't be read/decoded or path escapes base_dir.
    pub fn get_resized(
        &mut self,
        src: &str,
        base_dir: &Path,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<&Arc<RgbaImage>> {
        let full_path = if Path::new(src).is_absolute() {
            PathBuf::from(src)
        } else {
            base_dir.join(src)
        };

        // Path traversal guard: resolved path must stay within base_dir
        let resolved = full_path.canonicalize().ok()?;
        let base = base_dir.canonicalize().ok()?;
        if !resolved.starts_with(&base) {
            return None;
        }

        let key = resolved.to_string_lossy().to_string();
        let cache_key = (key.clone(), max_cols, max_rows);

        if self.resized.contains_key(&cache_key) {
            return self.resized.get(&cache_key);
        }

        // Evict if cache is too large
        if self.resized.len() >= self.max_resized {
            self.resized.clear();
            self.encoded.clear();
        }

        // Decode original if not cached
        if !self.originals.contains_key(&key) {
            let img = image::open(&resolved).ok()?;
            self.originals.insert(key.clone(), Arc::new(img.to_rgba8()));
        }

        let original = self.originals.get(&key)?;
        let resized = resize_to_fit(original, max_cols, max_rows);
        self.resized.insert(cache_key.clone(), Arc::new(resized));
        self.resized.get(&cache_key)
    }

    /// Get pre-encoded Kitty base64 PNG for a cached resized image.
    /// Computes and caches on first call for each (path, cols, rows).
    pub fn get_encoded_kitty(
        &mut self,
        src: &str,
        base_dir: &Path,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<&str> {
        let full_path = if Path::new(src).is_absolute() {
            PathBuf::from(src)
        } else {
            base_dir.join(src)
        };
        let resolved = full_path.canonicalize().ok()?;
        let key = resolved.to_string_lossy().to_string();
        let cache_key = (key, max_cols, max_rows);

        if self.encoded.contains_key(&cache_key) {
            return self.encoded.get(&cache_key).map(|s| s.as_str());
        }

        let img = self.resized.get(&cache_key)?;
        let b64 = encode_kitty_png(img)?;
        self.encoded.insert(cache_key.clone(), b64);
        self.encoded.get(&cache_key).map(|s| s.as_str())
    }
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

/// Kitty Graphics Protocol: send pre-encoded base64 PNG in APC escape.
fn render_kitty<W: Write>(w: &mut W, d: &DeferredImage) -> std::io::Result<()> {
    let b64 = match d.kitty_b64 {
        Some(ref s) => s,
        None => return Ok(()), // no encoded data available
    };

    // Move cursor to image position (1-based)
    write!(w, "\x1b[{};{}H", d.y + 1, d.x + 1)?;

    // Send chunked if >4096 bytes
    let chunk_size = 4096;
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(chunk_size).collect();

    if chunks.len() <= 1 {
        write!(w, "\x1b_Ga=T,f=100,c={},r={};{}\x1b\\", d.cols, d.rows, b64)?;
    } else {
        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
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
                write!(w, "\x1b_Gm={};{}\x1b\\", m, chunk_str)?;
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
        assert!(cache.originals.is_empty());
        assert!(cache.resized.is_empty());
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
}
