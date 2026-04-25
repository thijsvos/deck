use std::collections::HashMap;
use std::sync::Arc;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{FontStyle, Theme as SynTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Syntax highlighter backed by syntect. Caches results per (code, lang) pair.
///
/// Cache values are wrapped in `Arc` so cache hits are a refcount bump
/// rather than a deep clone of every line and span.
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme: SynTheme,
    cache: HashMap<(String, String), Arc<Vec<Line<'static>>>>,
}

impl Highlighter {
    /// Construct a highlighter with the bundled syntax set and the
    /// `base16-ocean.dark` color theme.
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Self {
            syntax_set,
            theme,
            cache: HashMap::new(),
        }
    }

    /// Returns the syntect theme's background color for code block backgrounds.
    pub fn bg_color(&self) -> Option<Color> {
        self.theme
            .settings
            .background
            .map(|c| Color::Rgb(c.r, c.g, c.b))
    }

    /// Highlight source code and return one `Line` per source line.
    /// Results are cached per `(code, lang)`. Returns an `Arc` so repeated
    /// calls (typewriter animation, multiple frames) are cheap refcount bumps.
    pub fn highlight(&mut self, code: &str, lang: &str) -> Arc<Vec<Line<'static>>> {
        let key = (code.to_string(), lang.to_string());
        if let Some(cached) = self.cache.get(&key) {
            return Arc::clone(cached);
        }

        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = syntect::easy::HighlightLines::new(syntax, &self.theme);

        let lines: Vec<Line<'static>> = code
            .lines()
            .map(|line| {
                let ranges = h.highlight_line(line, &self.syntax_set).unwrap_or_default();
                let spans: Vec<Span<'static>> = ranges
                    .into_iter()
                    .map(|(style, text)| Span::styled(text.to_string(), to_ratatui(style)))
                    .collect();
                Line::from(spans)
            })
            .collect();

        let arc = Arc::new(lines);
        self.cache.insert(key, Arc::clone(&arc));
        arc
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

fn to_ratatui(style: syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let bg = Color::Rgb(style.background.r, style.background.g, style.background.b);
    let mut s = Style::default().fg(fg).bg(bg);
    if style.font_style.contains(FontStyle::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        s = s.add_modifier(Modifier::ITALIC);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_produces_colored_spans() {
        let mut h = Highlighter::new();
        let lines = h.highlight("fn main() {}", "rs");
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].spans.len() > 1,
            "expected multiple spans, got {}",
            lines[0].spans.len()
        );
    }

    #[test]
    fn highlight_unknown_lang_falls_back_to_plain() {
        let mut h = Highlighter::new();
        let lines = h.highlight("hello world", "nonexistent_language_xyz");
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn highlight_multiline() {
        let mut h = Highlighter::new();
        let lines = h.highlight("line1\nline2\nline3", "txt");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_empty_code() {
        let mut h = Highlighter::new();
        let lines = h.highlight("", "rs");
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn bg_color_returns_some() {
        let h = Highlighter::new();
        assert!(h.bg_color().is_some());
    }

    #[test]
    fn highlight_rust_keywords_have_color() {
        let mut h = Highlighter::new();
        let lines = h.highlight("fn main() {}", "rs");
        let first_span = &lines[0].spans[0];
        assert!(
            matches!(first_span.style.fg, Some(Color::Rgb(_, _, _))),
            "expected RGB color on keyword"
        );
    }

    #[test]
    fn highlight_comment_produces_styled_span() {
        let mut h = Highlighter::new();
        let lines = h.highlight("// a comment", "rs");
        assert!(!lines[0].spans.is_empty());
        assert!(lines[0].spans[0].style.fg.is_some());
    }

    #[test]
    fn highlight_caches_results() {
        let mut h = Highlighter::new();
        let lines1 = h.highlight("fn foo() {}", "rs");
        let lines2 = h.highlight("fn foo() {}", "rs");
        assert_eq!(lines1.len(), lines2.len());
        assert_eq!(h.cache.len(), 1);
        // Both handles point at the same Arc — refcount-bump on hit.
        assert!(Arc::ptr_eq(&lines1, &lines2));
    }

    #[test]
    fn highlight_distinct_code_distinct_cache_entries() {
        let mut h = Highlighter::new();
        let _ = h.highlight("fn a() {}", "rs");
        let _ = h.highlight("fn b() {}", "rs");
        assert_eq!(h.cache.len(), 2);
    }
}
