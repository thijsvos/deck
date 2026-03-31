use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

use crate::bigtext::FontStyle;
use crate::transition::TransitionKind;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Hacker,
    Minimal,
    Corporate,
    Catppuccin,
}

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
    pub bullet: &'static str,
    pub font: FontStyle,
    pub default_transition: TransitionKind,
}

impl Theme {
    pub fn from_name(name: &ThemeName) -> Self {
        match name {
            ThemeName::Hacker => Self::hacker(),
            ThemeName::Minimal => Self::minimal(),
            ThemeName::Corporate => Self::corporate(),
            ThemeName::Catppuccin => Self::catppuccin(),
        }
    }

    fn hacker() -> Self {
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
            bullet: ">",
            font: FontStyle::Block,
            default_transition: TransitionKind::Glitch,
        }
    }

    fn catppuccin() -> Self {
        // Catppuccin Mocha palette
        Self {
            bg: Color::Rgb(30, 30, 46),       // Base
            fg: Color::Rgb(205, 214, 244),     // Text
            heading: Color::Rgb(137, 180, 250), // Blue
            accent: Color::Rgb(245, 194, 231),  // Pink
            dim: Color::Rgb(88, 91, 112),      // Overlay0
            code_bg: Color::Rgb(49, 50, 68),   // Surface0
            code_fg: Color::Rgb(166, 173, 200), // Subtext0
            bold: Color::Rgb(250, 179, 135),    // Peach
            italic: Color::Rgb(180, 190, 254),  // Lavender
            bullet: "◆",
            font: FontStyle::Block,
            default_transition: TransitionKind::Dissolve,
        }
    }

    fn corporate() -> Self {
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
            bullet: "•",
            font: FontStyle::Large,
            default_transition: TransitionKind::Wipe,
        }
    }

    fn minimal() -> Self {
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
            bullet: "·",
            font: FontStyle::Large,
            default_transition: TransitionKind::Fade,
        }
    }

    pub fn h1_style(&self) -> Style {
        Style::default().fg(self.heading).add_modifier(Modifier::BOLD)
    }

    pub fn heading_style(&self) -> Style {
        Style::default().fg(self.heading).add_modifier(Modifier::BOLD)
    }

    pub fn body_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn code_style(&self) -> Style {
        Style::default().fg(self.code_fg).bg(self.code_bg)
    }

    pub fn code_border(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn bullet_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn bold_style(&self) -> Style {
        Style::default().fg(self.bold).bg(self.bg).add_modifier(Modifier::BOLD)
    }

    pub fn italic_style(&self) -> Style {
        Style::default().fg(self.italic).bg(self.bg).add_modifier(Modifier::ITALIC)
    }

    pub fn rule_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn status_style(&self) -> Style {
        Style::default().fg(self.dim).bg(self.bg)
    }

    pub fn status_accent(&self) -> Style {
        Style::default().fg(self.accent).bg(self.bg)
    }
}
