use std::time::Instant;

use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Clear, Paragraph},
    Frame,
};

use crate::background;
use crate::input::{map_key, Action};
use crate::markdown::Block;
use crate::parse::{Deck, Slide};
use crate::render;
use crate::render_presenter;
use crate::theme::Theme;
use crate::transition::{self, TransitionKind, TransitionState};

pub enum Mode {
    Normal,
    Presenter,
}

pub struct App {
    pub deck: Deck,
    pub slide_index: usize,
    pub reveal_count: usize,
    pub mode: Mode,
    pub show_help: bool,
    pub goto_input: String,
    pub in_goto: bool,
    pub transition: Option<TransitionState>,
    pub start: Instant,
    pub timer: Instant,
    pub theme: Theme,
}

impl App {
    pub fn new(deck: Deck, theme: Theme) -> Self {
        let reveal = initial_reveal(&deck.slides[0]);
        let now = Instant::now();
        Self {
            deck,
            slide_index: 0,
            reveal_count: reveal,
            mode: Mode::Normal,
            show_help: false,
            goto_input: String::new(),
            in_goto: false,
            transition: None,
            start: now,
            timer: now,
            theme,
        }
    }

    pub fn has_active_background(&self) -> bool {
        self.deck.slides[self.slide_index].background.is_some()
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = RLayout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let main_area = chunks[0];
        let status_area = chunks[1];

        // Render slide
        match self.mode {
            Mode::Normal => {
                let slide = &self.deck.slides[self.slide_index];
                render::render_slide(frame, main_area, slide, &self.theme, self.reveal_count);

                // Animated background fills empty cells around content
                if let Some(ref bg) = slide.background {
                    let anim_time = self.start.elapsed().as_secs_f64();
                    background::apply_background(frame, main_area, bg, anim_time, &self.theme);
                }
            }
            Mode::Presenter => {
                render_presenter::render_presenter(frame, main_area, self);
            }
        }

        // Apply transition overlay
        if let Some(ref trans) = self.transition {
            transition::apply_transition(frame, main_area, trans, &self.theme);
        }

        // Status bar
        let elapsed = self.timer.elapsed().as_secs();
        render::render_status_bar(
            frame,
            status_area,
            &self.deck.meta.title,
            self.slide_index + 1,
            self.deck.slides.len(),
            elapsed,
            &self.theme,
        );

        // Overlays
        if self.show_help {
            render_help(frame, area, &self.theme);
        }
        if self.in_goto {
            render_goto(frame, area, &self.goto_input, &self.theme);
        }
    }

    /// Returns true if the app should quit.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let action = map_key(key, self.in_goto);

        match action {
            Action::Quit => {
                if self.show_help {
                    self.show_help = false;
                } else if self.in_goto {
                    self.in_goto = false;
                    self.goto_input.clear();
                } else {
                    return true;
                }
            }
            Action::Next => self.advance(),
            Action::Prev => self.go_back(),
            Action::First => self.go_to(0),
            Action::Last => self.go_to(self.deck.slides.len() - 1),
            Action::TogglePresenter => {
                self.mode = match self.mode {
                    Mode::Normal => Mode::Presenter,
                    Mode::Presenter => Mode::Normal,
                };
            }
            Action::ToggleHelp => self.show_help = !self.show_help,
            Action::StartGoTo => {
                self.in_goto = true;
                self.goto_input.clear();
            }
            Action::GoToDigit(c) => {
                if c == '\x08' {
                    self.goto_input.pop();
                } else {
                    self.goto_input.push(c);
                }
            }
            Action::GoToConfirm => {
                if let Ok(n) = self.goto_input.parse::<usize>() {
                    let target = n.saturating_sub(1).min(self.deck.slides.len() - 1);
                    self.go_to(target);
                }
                self.in_goto = false;
                self.goto_input.clear();
            }
            Action::GoToCancel => {
                self.in_goto = false;
                self.goto_input.clear();
            }
            Action::ResetTimer => {
                self.timer = Instant::now();
            }
            Action::None => {}
        }

        false
    }

    pub fn tick(&mut self) {
        if let Some(ref trans) = self.transition {
            if trans.is_done() {
                self.transition = None;
            }
        }
    }

    fn advance(&mut self) {
        let total_bullets = count_bullets(&self.deck.slides[self.slide_index]);
        if total_bullets > 0 && self.reveal_count < total_bullets {
            self.reveal_count += 1;
        } else if self.slide_index < self.deck.slides.len() - 1 {
            self.slide_index += 1;
            self.reveal_count = initial_reveal(&self.deck.slides[self.slide_index]);
            self.start_transition();
        }
    }

    fn go_back(&mut self) {
        let total_bullets = count_bullets(&self.deck.slides[self.slide_index]);
        if total_bullets > 0 && self.reveal_count > 0 {
            self.reveal_count -= 1;
        } else if self.slide_index > 0 {
            self.slide_index -= 1;
            let prev_bullets = count_bullets(&self.deck.slides[self.slide_index]);
            self.reveal_count = if prev_bullets > 0 {
                prev_bullets
            } else {
                usize::MAX
            };
            self.start_transition();
        }
    }

    fn go_to(&mut self, index: usize) {
        let new_index = index.min(self.deck.slides.len() - 1);
        if new_index != self.slide_index {
            self.slide_index = new_index;
            self.reveal_count = initial_reveal(&self.deck.slides[self.slide_index]);
            self.start_transition();
        }
    }

    fn start_transition(&mut self) {
        if !matches!(self.deck.meta.transition, TransitionKind::None) {
            self.transition = Some(TransitionState::new(self.deck.meta.transition.clone()));
        }
    }
}

fn count_bullets(slide: &Slide) -> usize {
    slide
        .blocks
        .iter()
        .map(|b| match b {
            Block::BulletList { items } => items.len(),
            _ => 0,
        })
        .sum()
}

fn initial_reveal(slide: &Slide) -> usize {
    let total = count_bullets(slide);
    if total > 0 { 0 } else { usize::MAX }
}

fn render_help(frame: &mut Frame, area: Rect, theme: &Theme) {
    let help_lines = [
        "",
        "  Navigation",
        "  ─────────────────────────",
        "  →/Space/Enter/l/j  Next",
        "  ←/Backspace/h/k    Previous",
        "  g                  First slide",
        "  G                  Last slide",
        "  :N Enter           Go to slide N",
        "",
        "  Controls",
        "  ─────────────────────────",
        "  p                  Presenter mode",
        "  r                  Reset timer",
        "  ?                  Toggle help",
        "  q/Esc              Quit",
        "",
    ];

    let width = 38u16;
    let height = help_lines.len() as u16 + 2;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);

    let lines: Vec<Line> = help_lines
        .iter()
        .map(|s| Line::from(RSpan::styled(s.to_string(), theme.body_style())))
        .collect();

    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" ? Help ");

    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

fn render_goto(frame: &mut Frame, area: Rect, input: &str, theme: &Theme) {
    let width = 22u16;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height / 2;
    let rect = Rect::new(x, y, width, 3);

    frame.render_widget(Clear, rect);

    let text = format!(":{}_", input);
    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" Go to slide ");
    let paragraph = Paragraph::new(RSpan::styled(text, theme.body_style())).block(block);
    frame.render_widget(paragraph, rect);
}
