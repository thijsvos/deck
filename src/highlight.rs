use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{FontStyle, Theme as SynTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Syntax highlighter backed by syntect. Maps highlighted ranges to ratatui styles.
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme: SynTheme,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Self { syntax_set, theme }
    }

    /// Returns the syntect theme's background color for code block backgrounds.
    pub fn bg_color(&self) -> Option<Color> {
        self.theme
            .settings
            .background
            .map(|c| Color::Rgb(c.r, c.g, c.b))
    }

    /// Highlight source code and return one `Line` per source line.
    pub fn highlight<'a>(&self, code: &str, lang: &str) -> Vec<Line<'a>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = syntect::easy::HighlightLines::new(syntax, &self.theme);

        code.lines()
            .map(|line| {
                let ranges = h.highlight_line(line, &self.syntax_set).unwrap_or_default();
                let spans: Vec<Span<'a>> = ranges
                    .into_iter()
                    .map(|(style, text)| Span::styled(text.to_string(), to_ratatui(style)))
                    .collect();
                Line::from(spans)
            })
            .collect()
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
        let h = Highlighter::new();
        let lines = h.highlight("fn main() {}", "rs");
        assert_eq!(lines.len(), 1);
        // Should have multiple styled spans (keyword, ident, punctuation)
        assert!(
            lines[0].spans.len() > 1,
            "expected multiple spans, got {}",
            lines[0].spans.len()
        );
    }

    #[test]
    fn highlight_unknown_lang_falls_back_to_plain() {
        let h = Highlighter::new();
        let lines = h.highlight("hello world", "nonexistent_language_xyz");
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn highlight_multiline() {
        let h = Highlighter::new();
        let lines = h.highlight("line1\nline2\nline3", "txt");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_empty_code() {
        let h = Highlighter::new();
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
        let h = Highlighter::new();
        let lines = h.highlight("fn main() {}", "rs");
        // "fn" keyword should have a non-default foreground color
        let first_span = &lines[0].spans[0];
        assert!(
            matches!(first_span.style.fg, Some(Color::Rgb(_, _, _))),
            "expected RGB color on keyword"
        );
    }

    #[test]
    fn highlight_comment_produces_styled_span() {
        let h = Highlighter::new();
        let lines = h.highlight("// a comment", "rs");
        assert!(!lines[0].spans.is_empty());
        // Comment should have some foreground color
        assert!(lines[0].spans[0].style.fg.is_some());
    }
}
