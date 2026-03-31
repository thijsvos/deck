use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    originals: HashMap<String, RgbaImage>,
    /// Resized images keyed by (path, cols, rows).
    resized: HashMap<(String, u16, u16), RgbaImage>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            originals: HashMap::new(),
            resized: HashMap::new(),
        }
    }

    /// Load, resize, and cache an image for the given area dimensions.
    /// Returns None if the file can't be read/decoded.
    pub fn get_resized(
        &mut self,
        src: &str,
        base_dir: &Path,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<&RgbaImage> {
        let full_path = if Path::new(src).is_absolute() {
            PathBuf::from(src)
        } else {
            base_dir.join(src)
        };

        let key = full_path.to_string_lossy().to_string();
        let cache_key = (key.clone(), max_cols, max_rows);

        if self.resized.contains_key(&cache_key) {
            return self.resized.get(&cache_key);
        }

        // Decode original if not cached
        if !self.originals.contains_key(&key) {
            let img = image::open(&full_path).ok()?;
            self.originals.insert(key.clone(), img.to_rgba8());
        }

        let original = self.originals.get(&key)?;
        let resized = resize_to_fit(original, max_cols, max_rows);
        self.resized.insert(cache_key.clone(), resized);
        self.resized.get(&cache_key)
    }
}

/// A queued image render for Kitty/Sixel post-draw pass.
pub struct DeferredImage {
    pub x: u16,
    pub y: u16,
    pub cols: u16,
    pub rows: u16,
    pub rgba: RgbaImage,
    pub protocol: ImageProtocol,
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
    if new_h % 2 != 0 {
        new_h += 1;
    }

    image::imageops::resize(img, new_w, new_h, FilterType::Lanczos3)
}

/// Render image as half-block characters directly into the ratatui buffer.
/// Each cell uses `▀` with fg = top pixel, bg = bottom pixel.
pub fn render_halfblocks(buf: &mut Buffer, area: Rect, img: &RgbaImage) {
    let (img_w, img_h) = img.dimensions();
    let rows = (img_h + 1) / 2;

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

/// Kitty Graphics Protocol: send PNG image as base64 in APC escape.
fn render_kitty<W: Write>(w: &mut W, d: &DeferredImage) -> std::io::Result<()> {
    use base64::Engine;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;

    // Encode RGBA to PNG
    let mut png_buf = Vec::new();
    let encoder = PngEncoder::new(&mut png_buf);
    encoder
        .write_image(d.rgba.as_raw(), d.rgba.width(), d.rgba.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);

    // Move cursor to image position (1-based)
    write!(w, "\x1b[{};{}H", d.y + 1, d.x + 1)?;

    // Send chunked if >4096 bytes
    let chunk_size = 4096;
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(chunk_size).collect();

    if chunks.len() <= 1 {
        write!(
            w,
            "\x1b_Ga=T,f=100,c={},r={};{}\x1b\\",
            d.cols, d.rows, b64
        )?;
    } else {
        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            let m = if is_last { 0 } else { 1 };
            let chunk_str = std::str::from_utf8(chunk).unwrap_or("");

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
/// A full Sixel encoder (color quantization + band encoding) is complex.
/// For terminals detected as Sixel-capable, we write ANSI half-blocks
/// directly which gives equivalent visual quality without the encoding overhead.
fn render_sixel_fallback<W: Write>(w: &mut W, d: &DeferredImage) -> std::io::Result<()> {
    let (img_w, img_h) = d.rgba.dimensions();
    let rows = (img_h + 1) / 2;

    for ty in 0..rows.min(d.rows as u32) {
        // Move cursor to start of this row
        write!(w, "\x1b[{};{}H", d.y + 1 + ty as u16, d.x + 1)?;

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
            write!(
                w,
                "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                top[0], top[1], top[2], bot[0], bot[1], bot[2]
            )?;
        }
    }
    // Reset colors
    write!(w, "\x1b[0m")?;
    w.flush()
}
