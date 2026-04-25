use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

use crate::bigtext::FontStyle;
use crate::transition::TransitionKind;

/// Built-in theme variants.
///
/// Selectable via the `--theme` CLI flag or the `theme = "..."` frontmatter
/// field. Mapped to a concrete [`Theme`] palette by [`Theme::from_name`].
#[derive(Clone, Debug, Default, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Hacker,
    Minimal,
    Corporate,
    Catppuccin,
}

/// Color palette and font/transition defaults used by the renderer.
///
/// Build from a [`ThemeName`] via [`Theme::from_name`]. Prefer the `*_style`
/// accessors over reading the raw color fields directly; they bake in
/// modifiers (`BOLD`, `ITALIC`) and the chosen background.
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub heading: Color,
    pub accent: Color,
    pub dim: Color,
    pub code_bg: Color,
    pub code_fg: Color,
    pub bold: Color,
    pub italic: Color,
    /// Pre-built `"  <bullet> "` prefix string, populated once at theme
    /// construction so the renderer can borrow it instead of `format!`-ing
    /// per bullet per frame.
    pub bullet_prefix: String,
    pub font: FontStyle,
    /// Slide-to-slide transition used when the deck doesn't override it.
    pub default_transition: TransitionKind,
}

impl Theme {
    /// Build one of the built-in themes by name.
    pub fn from_name(name: &ThemeName) -> Self {
        match name {
            ThemeName::Hacker => Self::hacker(),
            ThemeName::Minimal => Self::minimal(),
            ThemeName::Corporate => Self::corporate(),
            ThemeName::Catppuccin => Self::catppuccin(),
        }
    }

    fn hacker() -> Self {
        let bullet = ">";
        Self {
            bg: Color::Rgb(10, 10, 20),
            fg: Color::Rgb(0, 255, 65),
            heading: Color::Rgb(0, 255, 200),
            accent: Color::Rgb(255, 0, 100),
            dim: Color::Rgb(60, 60, 80),
            code_bg: Color::Rgb(20, 20, 35),
            code_fg: Color::Rgb(200, 200, 200),
            bold: Color::Rgb(255, 255, 100),
            italic: Color::Rgb(150, 150, 255),
            bullet_prefix: format!("  {bullet} "),
            font: FontStyle::Block,
            default_transition: TransitionKind::Glitch,
        }
    }

    fn catppuccin() -> Self {
        // Catppuccin Mocha palette
        let bullet = "◆";
        Self {
            bg: Color::Rgb(30, 30, 46),         // Base
            fg: Color::Rgb(205, 214, 244),      // Text
            heading: Color::Rgb(137, 180, 250), // Blue
            accent: Color::Rgb(245, 194, 231),  // Pink
            dim: Color::Rgb(88, 91, 112),       // Overlay0
            code_bg: Color::Rgb(49, 50, 68),    // Surface0
            code_fg: Color::Rgb(166, 173, 200), // Subtext0
            bold: Color::Rgb(250, 179, 135),    // Peach
            italic: Color::Rgb(180, 190, 254),  // Lavender
            bullet_prefix: format!("  {bullet} "),
            font: FontStyle::Block,
            default_transition: TransitionKind::Dissolve,
        }
    }

    fn corporate() -> Self {
        let bullet = "•";
        Self {
            bg: Color::Rgb(20, 25, 40),
            fg: Color::Rgb(215, 220, 230),
            heading: Color::Rgb(100, 170, 255),
            accent: Color::Rgb(230, 185, 55),
            dim: Color::Rgb(70, 78, 95),
            code_bg: Color::Rgb(28, 34, 52),
            code_fg: Color::Rgb(195, 200, 210),
            bold: Color::Rgb(245, 248, 255),
            italic: Color::Rgb(150, 175, 215),
            bullet_prefix: format!("  {bullet} "),
            font: FontStyle::Large,
            default_transition: TransitionKind::Wipe,
        }
    }

    fn minimal() -> Self {
        let bullet = "·";
        Self {
            bg: Color::Reset,
            fg: Color::White,
            heading: Color::White,
            accent: Color::Yellow,
            dim: Color::DarkGray,
            code_bg: Color::Rgb(40, 40, 40),
            code_fg: Color::White,
            bold: Color::White,
            italic: Color::Gray,
            bullet_prefix: format!("  {bullet} "),
            font: FontStyle::Large,
            default_transition: TransitionKind::Fade,
        }
    }

    /// Style for the H1 big-text glyphs. Currently aliases `heading_style`.
    pub fn h1_style(&self) -> Style {
        self.heading_style()
    }

    /// Style for headings (H2+): `heading` color, bold.
    pub fn heading_style(&self) -> Style {
        Style::default()
            .fg(self.heading)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for normal body text: `fg` on `bg`.
    pub fn body_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    /// Style for inline and fenced code: `code_fg` on `code_bg`.
    pub fn code_style(&self) -> Style {
        Style::default().fg(self.code_fg).bg(self.code_bg)
    }

    /// Style for the border around fenced code blocks.
    pub fn code_border(&self) -> Style {
        Style::default().fg(self.dim)
    }

    /// Style for the leading bullet glyph.
    pub fn bullet_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Style for `**bold**` runs: `bold` color with the `BOLD` modifier.
    pub fn bold_style(&self) -> Style {
        Style::default()
            .fg(self.bold)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for `*italic*` runs: `italic` color with the `ITALIC` modifier.
    pub fn italic_style(&self) -> Style {
        Style::default()
            .fg(self.italic)
            .bg(self.bg)
            .add_modifier(Modifier::ITALIC)
    }

    /// Style for horizontal rules: `dim` foreground.
    pub fn rule_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    /// Status bar base style: `dim` on `bg`.
    pub fn status_style(&self) -> Style {
        Style::default().fg(self.dim).bg(self.bg)
    }

    /// Accent style used inside the status bar (title and timer): `accent` on `bg`.
    pub fn status_accent(&self) -> Style {
        Style::default().fg(self.accent).bg(self.bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hacker_theme_properties() {
        let t = Theme::from_name(&ThemeName::Hacker);
        assert_eq!(t.bullet_prefix, "  > ");
        assert!(matches!(t.font, FontStyle::Block));
        assert!(matches!(t.default_transition, TransitionKind::Glitch));
        assert!(matches!(t.bg, Color::Rgb(10, 10, 20)));
    }

    #[test]
    fn corporate_theme_properties() {
        let t = Theme::from_name(&ThemeName::Corporate);
        assert_eq!(t.bullet_prefix, "  • ");
        assert!(matches!(t.font, FontStyle::Large));
        assert!(matches!(t.default_transition, TransitionKind::Wipe));
    }

    #[test]
    fn catppuccin_theme_properties() {
        let t = Theme::from_name(&ThemeName::Catppuccin);
        assert_eq!(t.bullet_prefix, "  ◆ ");
        assert!(matches!(t.font, FontStyle::Block));
        assert!(matches!(t.default_transition, TransitionKind::Dissolve));
    }

    #[test]
    fn minimal_theme_properties() {
        let t = Theme::from_name(&ThemeName::Minimal);
        assert_eq!(t.bullet_prefix, "  · ");
        assert!(matches!(t.font, FontStyle::Large));
        assert!(matches!(t.default_transition, TransitionKind::Fade));
        assert!(matches!(t.bg, Color::Reset));
    }

    #[test]
    fn h1_style_is_bold() {
        let t = Theme::from_name(&ThemeName::Hacker);
        let style = t.h1_style();
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn body_style_has_fg_and_bg() {
        let t = Theme::from_name(&ThemeName::Hacker);
        let style = t.body_style();
        assert_eq!(style.fg, Some(t.fg));
        assert_eq!(style.bg, Some(t.bg));
    }

    #[test]
    fn bold_style_has_bold_modifier() {
        let t = Theme::from_name(&ThemeName::Hacker);
        assert!(t.bold_style().add_modifier.contains(Modifier::BOLD));
        assert_eq!(t.bold_style().fg, Some(t.bold));
    }

    #[test]
    fn italic_style_has_italic_modifier() {
        let t = Theme::from_name(&ThemeName::Hacker);
        assert!(t.italic_style().add_modifier.contains(Modifier::ITALIC));
        assert_eq!(t.italic_style().fg, Some(t.italic));
    }

    #[test]
    fn code_style_uses_code_colors() {
        let t = Theme::from_name(&ThemeName::Hacker);
        assert_eq!(t.code_style().fg, Some(t.code_fg));
        assert_eq!(t.code_style().bg, Some(t.code_bg));
    }

    #[test]
    fn status_accent_uses_accent() {
        let t = Theme::from_name(&ThemeName::Hacker);
        assert_eq!(t.status_accent().fg, Some(t.accent));
        assert_eq!(t.status_accent().bg, Some(t.bg));
    }
}
