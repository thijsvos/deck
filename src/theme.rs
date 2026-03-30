use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Hacker,
    Minimal,
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
}

impl Theme {
    pub fn from_name(name: &ThemeName) -> Self {
        match name {
            ThemeName::Hacker => Self::hacker(),
            ThemeName::Minimal => Self::minimal(),
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
