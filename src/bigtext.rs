/// Font style for H1 big text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStyle {
    /// Heavy block characters (█), 5 rows — hacker/catppuccin
    #[default]
    Block,
    /// Large clean block characters, 7 rows — corporate/minimal
    Large,
}

/// Cache for `render` results so an unchanging H1 isn't re-rasterized every frame.
///
/// Keyed by an `fnv1a(text) ^ font_bits` hash so cache lookups don't allocate
/// on the hot path. Hash collisions just trigger a recompute, which matches the
/// policy in `Highlighter` (see `util::fnv1a`).
pub struct BigtextCache {
    entries: std::collections::HashMap<u64, Vec<String>>,
}

impl BigtextCache {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Returns the rendered rows for `(text, font)`, computing and caching on miss.
    pub fn render(&mut self, text: &str, font: FontStyle) -> &[String] {
        let key = cache_key(text, font);
        self.entries
            .entry(key)
            .or_insert_with(|| render(text, font))
    }
}

impl Default for BigtextCache {
    fn default() -> Self {
        Self::new()
    }
}

fn cache_key(text: &str, font: FontStyle) -> u64 {
    let font_bits: u64 = match font {
        FontStyle::Block => 0,
        FontStyle::Large => 1,
    };
    crate::util::fnv1a(text) ^ font_bits.wrapping_mul(0x9E3779B97F4A7C15)
}

pub fn glyph_block(c: char) -> Option<[&'static str; 5]> {
    Some(match c.to_ascii_uppercase() {
        'A' => [" ██ ", "█  █", "████", "█  █", "█  █"],
        'B' => ["███ ", "█  █", "███ ", "█  █", "███ "],
        'C' => [" ███", "█   ", "█   ", "█   ", " ███"],
        'D' => ["███ ", "█  █", "█  █", "█  █", "███ "],
        'E' => ["████", "█   ", "███ ", "█   ", "████"],
        'F' => ["████", "█   ", "███ ", "█   ", "█   "],
        'G' => [" ███", "█   ", "█ ██", "█  █", " ███"],
        'H' => ["█  █", "█  █", "████", "█  █", "█  █"],
        'I' => ["███", " █ ", " █ ", " █ ", "███"],
        'J' => [" ███", "   █", "   █", "█  █", " ██ "],
        'K' => ["█  █", "█ █ ", "██  ", "█ █ ", "█  █"],
        'L' => ["█   ", "█   ", "█   ", "█   ", "████"],
        'M' => ["█   █", "██ ██", "█ █ █", "█   █", "█   █"],
        'N' => ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
        'O' => [" ██ ", "█  █", "█  █", "█  █", " ██ "],
        'P' => ["███ ", "█  █", "███ ", "█   ", "█   "],
        'Q' => [" ██ ", "█  █", "█  █", "█ ██", " ███"],
        'R' => ["███ ", "█  █", "███ ", "█ █ ", "█  █"],
        'S' => [" ███", "█   ", " ██ ", "   █", "███ "],
        'T' => ["████", " █  ", " █  ", " █  ", " █  "],
        'U' => ["█  █", "█  █", "█  █", "█  █", " ██ "],
        'V' => ["█  █", "█  █", "█  █", " ██ ", " ██ "],
        'W' => ["█   █", "█   █", "█ █ █", "█ █ █", " █ █ "],
        'X' => ["█  █", "█  █", " ██ ", "█  █", "█  █"],
        'Y' => ["█  █", "█  █", " ██ ", " █  ", " █  "],
        'Z' => ["████", "  █ ", " █  ", "█   ", "████"],
        '0' => [" ██ ", "█  █", "█  █", "█  █", " ██ "],
        '1' => [" █ ", "██ ", " █ ", " █ ", "███"],
        '2' => [" ██ ", "█  █", "  █ ", " █  ", "████"],
        '3' => ["████", "   █", " ██ ", "   █", "████"],
        '4' => ["█  █", "█  █", "████", "   █", "   █"],
        '5' => ["████", "█   ", "███ ", "   █", "███ "],
        '6' => [" ██ ", "█   ", "███ ", "█  █", " ██ "],
        '7' => ["████", "   █", "  █ ", " █  ", "█   "],
        '8' => [" ██ ", "█  █", " ██ ", "█  █", " ██ "],
        '9' => [" ██ ", "█  █", " ███", "   █", " ██ "],
        ' ' => ["    ", "    ", "    ", "    ", "    "],
        '.' => ["  ", "  ", "  ", "  ", "██"],
        '!' => ["██", "██", "██", "  ", "██"],
        '?' => [" ██ ", "█  █", "  █ ", "    ", " █  "],
        '-' => ["    ", "    ", "████", "    ", "    "],
        ':' => ["  ", "██", "  ", "██", "  "],
        '\'' => ["██", "█ ", "  ", "  ", "  "],
        '&' => [" █  ", "█ █ ", " ██ ", "█ █ ", " ███"],
        '/' => ["   █", "  █ ", " █  ", "█   ", "█   "],
        '@' => [" ██ ", "█  █", "█ ██", "█   ", " ███"],
        '+' => ["    ", " █  ", "████", " █  ", "    "],
        _ => return None,
    })
}

pub fn glyph_large(c: char) -> Option<[&'static str; 7]> {
    Some(match c.to_ascii_uppercase() {
        'A' => [
            "  ████  ",
            " ██  ██ ",
            "██    ██",
            "████████",
            "██    ██",
            "██    ██",
            "██    ██",
        ],
        'B' => [
            "██████  ",
            "██   ██ ",
            "██   ██ ",
            "██████  ",
            "██   ██ ",
            "██   ██ ",
            "██████  ",
        ],
        'C' => [
            " ██████ ",
            "██      ",
            "██      ",
            "██      ",
            "██      ",
            "██      ",
            " ██████ ",
        ],
        'D' => [
            "██████  ",
            "██   ██ ",
            "██    ██",
            "██    ██",
            "██    ██",
            "██   ██ ",
            "██████  ",
        ],
        'E' => [
            "████████",
            "██      ",
            "██      ",
            "██████  ",
            "██      ",
            "██      ",
            "████████",
        ],
        'F' => [
            "████████",
            "██      ",
            "██      ",
            "██████  ",
            "██      ",
            "██      ",
            "██      ",
        ],
        'G' => [
            " ██████ ",
            "██      ",
            "██      ",
            "██  ████",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        'H' => [
            "██    ██",
            "██    ██",
            "██    ██",
            "████████",
            "██    ██",
            "██    ██",
            "██    ██",
        ],
        'I' => [
            "██████",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "  ██  ",
            "██████",
        ],
        'J' => [
            "  ██████",
            "      ██",
            "      ██",
            "      ██",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        'K' => [
            "██   ██ ",
            "██  ██  ",
            "██ ██   ",
            "████    ",
            "██ ██   ",
            "██  ██  ",
            "██   ██ ",
        ],
        'L' => [
            "██      ",
            "██      ",
            "██      ",
            "██      ",
            "██      ",
            "██      ",
            "████████",
        ],
        'M' => [
            "██    ██",
            "███  ███",
            "████████",
            "██ ██ ██",
            "██    ██",
            "██    ██",
            "██    ██",
        ],
        'N' => [
            "██    ██",
            "███   ██",
            "████  ██",
            "██ ██ ██",
            "██  ████",
            "██   ███",
            "██    ██",
        ],
        'O' => [
            " ██████ ",
            "██    ██",
            "██    ██",
            "██    ██",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        'P' => [
            "██████  ",
            "██   ██ ",
            "██   ██ ",
            "██████  ",
            "██      ",
            "██      ",
            "██      ",
        ],
        'Q' => [
            " ██████ ",
            "██    ██",
            "██    ██",
            "██    ██",
            "██  ████",
            "██   ██ ",
            " █████ █",
        ],
        'R' => [
            "██████  ",
            "██   ██ ",
            "██   ██ ",
            "██████  ",
            "██ ██   ",
            "██  ██  ",
            "██   ██ ",
        ],
        'S' => [
            " ██████ ",
            "██      ",
            "██      ",
            " ██████ ",
            "      ██",
            "      ██",
            " ██████ ",
        ],
        'T' => [
            "████████",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
        ],
        'U' => [
            "██    ██",
            "██    ██",
            "██    ██",
            "██    ██",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        'V' => [
            "██    ██",
            "██    ██",
            "██    ██",
            " ██  ██ ",
            " ██  ██ ",
            "  ████  ",
            "   ██   ",
        ],
        'W' => [
            "██    ██",
            "██    ██",
            "██    ██",
            "██ ██ ██",
            "████████",
            "███  ███",
            "██    ██",
        ],
        'X' => [
            "██    ██",
            " ██  ██ ",
            "  ████  ",
            "   ██   ",
            "  ████  ",
            " ██  ██ ",
            "██    ██",
        ],
        'Y' => [
            "██    ██",
            " ██  ██ ",
            "  ████  ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
        ],
        'Z' => [
            "████████",
            "     ██ ",
            "    ██  ",
            "   ██   ",
            "  ██    ",
            " ██     ",
            "████████",
        ],
        '0' => [
            " ██████ ",
            "██    ██",
            "██   ███",
            "██  ████",
            "██ ██ ██",
            "███   ██",
            " ██████ ",
        ],
        '1' => [
            "   ██   ",
            " ████   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "   ██   ",
            "████████",
        ],
        '2' => [
            " ██████ ",
            "██    ██",
            "      ██",
            "  ████  ",
            "██      ",
            "██      ",
            "████████",
        ],
        '3' => [
            "████████",
            "      ██",
            "      ██",
            "  ████  ",
            "      ██",
            "      ██",
            "████████",
        ],
        '4' => [
            "██    ██",
            "██    ██",
            "██    ██",
            "████████",
            "      ██",
            "      ██",
            "      ██",
        ],
        '5' => [
            "████████",
            "██      ",
            "██      ",
            "██████  ",
            "      ██",
            "      ██",
            "██████  ",
        ],
        '6' => [
            " ██████ ",
            "██      ",
            "██      ",
            "██████  ",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        '7' => [
            "████████",
            "      ██",
            "     ██ ",
            "    ██  ",
            "   ██   ",
            "  ██    ",
            " ██     ",
        ],
        '8' => [
            " ██████ ",
            "██    ██",
            "██    ██",
            " ██████ ",
            "██    ██",
            "██    ██",
            " ██████ ",
        ],
        '9' => [
            " ██████ ",
            "██    ██",
            "██    ██",
            " ███████",
            "      ██",
            "      ██",
            " ██████ ",
        ],
        ' ' => [
            "        ", "        ", "        ", "        ", "        ", "        ", "        ",
        ],
        '.' => ["    ", "    ", "    ", "    ", "    ", "    ", " ██ "],
        '!' => [" ██ ", " ██ ", " ██ ", " ██ ", " ██ ", "    ", " ██ "],
        '?' => [
            " ██████ ",
            "██    ██",
            "      ██",
            "    ██  ",
            "   ██   ",
            "        ",
            "   ██   ",
        ],
        '-' => [
            "        ",
            "        ",
            "        ",
            "██████  ",
            "        ",
            "        ",
            "        ",
        ],
        ':' => ["    ", "    ", " ██ ", "    ", " ██ ", "    ", "    "],
        '/' => [
            "      ██",
            "     ██ ",
            "    ██  ",
            "   ██   ",
            "  ██    ",
            " ██     ",
            "██      ",
        ],
        _ => return None,
    })
}

pub fn render(text: &str, style: FontStyle) -> Vec<String> {
    match style {
        FontStyle::Block => render_with(text, glyph_block, 5),
        FontStyle::Large => render_with(text, glyph_large, 7),
    }
}

fn render_with<const N: usize>(
    text: &str,
    glyph_fn: fn(char) -> Option<[&'static str; N]>,
    rows: usize,
) -> Vec<String> {
    // Collect with `map` (not `filter_map`) so a single missing glyph forces
    // the whole title into the plain-text fallback. Otherwise we'd silently
    // drop unsupported chars (e.g. "café" → "cafe", missing the é).
    let glyphs: Option<Vec<[&str; N]>> = text.chars().map(glyph_fn).collect();
    let glyphs = match glyphs {
        Some(g) if !g.is_empty() => g,
        _ => return vec![text.to_uppercase()],
    };

    (0..rows)
        .map(|row| glyphs.iter().map(|g| g[row]).collect::<Vec<_>>().join(" "))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_font_renders_5_rows() {
        let lines = render("A", FontStyle::Block);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn large_font_renders_7_rows() {
        let lines = render("A", FontStyle::Large);
        assert_eq!(lines.len(), 7);
    }

    #[test]
    fn unknown_chars_fallback() {
        let lines = render("🎉", FontStyle::Block);
        assert_eq!(lines, vec!["🎉"]);
    }

    #[test]
    fn space_produces_empty_glyph() {
        assert!(glyph_block(' ').is_some());
        assert!(glyph_large(' ').is_some());
    }

    #[test]
    fn lowercase_uppercased() {
        let lower = render("abc", FontStyle::Block);
        let upper = render("ABC", FontStyle::Block);
        assert_eq!(lower, upper);
    }

    #[test]
    fn multi_char_joins_with_space() {
        let lines = render("HI", FontStyle::Block);
        // Each line should contain a space separator between H and I glyphs
        assert!(lines[0].contains(' '));
    }

    #[test]
    fn glyph_block_all_digits() {
        for d in '0'..='9' {
            assert!(glyph_block(d).is_some(), "glyph_block missing digit '{d}'");
        }
    }

    #[test]
    fn glyph_large_all_digits() {
        for d in '0'..='9' {
            assert!(glyph_large(d).is_some(), "glyph_large missing digit '{d}'");
        }
    }

    #[test]
    fn unsupported_char_returns_none() {
        assert!(glyph_block('~').is_none());
        assert!(glyph_block('(').is_none());
        assert!(glyph_large('~').is_none());
    }

    #[test]
    fn partial_unsupported_falls_back_to_plain() {
        // Previously: `café` rendered as `CAFE` with the é silently dropped.
        // Now: any unmapped char trips the plain-text fallback.
        let lines = render("café", FontStyle::Block);
        assert_eq!(lines, vec!["CAFÉ"]);
    }
}
