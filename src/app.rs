use std::path::PathBuf;
use std::time::Instant;

use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Clear, Paragraph},
    Frame,
};

use crate::background;
use crate::entrance::EntranceTracker;
use crate::highlight::Highlighter;
use crate::image_renderer::{DeferredImage, ImageCache, ImageProtocol};
use crate::input::{map_key, Action};
use crate::markdown::Block;
use crate::parse::{Deck, Slide};
use crate::render;
use crate::render_presenter;
use crate::sync::SyncFile;
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
    pub sync: Option<SyncFile>,
    pub is_follower: bool,
    pub protocol: ImageProtocol,
    pub image_cache: ImageCache,
    pub deferred_images: Vec<DeferredImage>,
    pub base_dir: PathBuf,
    pub highlighter: Highlighter,
    pub entrances: EntranceTracker,
}

impl App {
    pub fn new(
        deck: Deck,
        theme: Theme,
        sync: Option<SyncFile>,
        is_follower: bool,
        protocol: ImageProtocol,
        base_dir: PathBuf,
    ) -> Self {
        assert!(!deck.slides.is_empty(), "deck must have at least one slide");
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
            sync,
            is_follower,
            protocol,
            image_cache: ImageCache::new(),
            deferred_images: Vec::new(),
            base_dir,
            highlighter: Highlighter::new(),
            entrances: EntranceTracker::new(),
        }
    }

    pub fn has_active_background(&self) -> bool {
        self.deck.slides[self.slide_index].background.is_some()
    }

    pub fn cleanup_sync(&self) {
        // Only the presenter cleans up the sync file
        if let Some(ref sync) = self.sync {
            if !self.is_follower {
                sync.cleanup();
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        self.deferred_images.clear();
        self.entrances.on_slide_change(self.slide_index);

        let area = frame.area();

        let chunks = RLayout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let main_area = chunks[0];
        let status_area = chunks[1];

        let mut ctx = render::RenderCtx {
            protocol: self.protocol,
            image_cache: &mut self.image_cache,
            deferred: &mut self.deferred_images,
            base_dir: &self.base_dir,
            highlighter: &self.highlighter,
            entrances: &mut self.entrances,
            slide_index: self.slide_index,
        };

        // Render slide
        match self.mode {
            Mode::Normal => {
                let slide = &self.deck.slides[self.slide_index];
                render::render_slide(
                    frame,
                    main_area,
                    slide,
                    &self.theme,
                    self.reveal_count,
                    &mut ctx,
                );

                // Animated background fills empty cells around content
                if let Some(ref bg) = slide.background {
                    let anim_time = self.start.elapsed().as_secs_f64();
                    background::apply_background(frame, main_area, bg, anim_time, &self.theme);
                }
            }
            Mode::Presenter => {
                render_presenter::render_presenter(
                    frame,
                    main_area,
                    &self.deck,
                    self.slide_index,
                    self.reveal_count,
                    &self.theme,
                    &self.timer,
                    &mut ctx,
                );
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
            self.deck.meta.author.as_deref(),
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
        // Follower mode: only allow quit
        if self.is_follower {
            let action = map_key(key, false);
            return matches!(action, Action::Quit);
        }

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
                self.goto_input.push(c);
            }
            Action::GoToBackspace => {
                self.goto_input.pop();
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

        // Follower: read sync file and update position
        if self.is_follower {
            if let Some(ref sync) = self.sync {
                if let Some((slide, reveal)) = sync.read() {
                    let slide = slide.min(self.deck.slides.len().saturating_sub(1));
                    if slide != self.slide_index {
                        self.slide_index = slide;
                        self.reveal_count = reveal;
                        self.start_transition();
                    } else if reveal != self.reveal_count {
                        self.reveal_count = reveal;
                    }
                }
            }
        }
    }

    fn write_sync(&self) {
        if let Some(ref sync) = self.sync {
            if !self.is_follower {
                sync.write(self.slide_index, self.reveal_count);
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
        self.write_sync();
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
        self.write_sync();
    }

    fn go_to(&mut self, index: usize) {
        let new_index = index.min(self.deck.slides.len() - 1);
        if new_index != self.slide_index {
            self.slide_index = new_index;
            self.reveal_count = initial_reveal(&self.deck.slides[self.slide_index]);
            self.start_transition();
        }
        self.write_sync();
    }

    fn start_transition(&mut self) {
        // Use frontmatter transition if explicitly set, otherwise theme default
        let kind = match self.deck.meta.transition {
            TransitionKind::None => self.theme.default_transition.clone(),
            ref explicit => explicit.clone(),
        };
        if !matches!(kind, TransitionKind::None) {
            self.transition = Some(TransitionState::new(kind));
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
    if total > 0 {
        0
    } else {
        usize::MAX
    }
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

    let text = format!(":{input}_");
    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" Go to slide ");
    let paragraph = Paragraph::new(RSpan::styled(text, theme.body_style())).block(block);
    frame.render_widget(paragraph, rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::{Block, ListItem, Span};
    use crate::parse::{Deck, DeckMeta, Layout, Slide};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn dummy_slide(blocks: Vec<Block>) -> Slide {
        Slide {
            blocks,
            layout: Layout::Default,
            columns: None,
            notes: vec![],
            background: None,
        }
    }

    fn bullet_list(n: usize) -> Block {
        Block::BulletList {
            items: (0..n)
                .map(|i| ListItem {
                    spans: vec![Span::Plain(format!("item {i}"))],
                })
                .collect(),
        }
    }

    fn make_deck(slides: Vec<Slide>) -> Deck {
        Deck {
            meta: DeckMeta::default(),
            slides,
        }
    }

    fn make_app(slides: Vec<Slide>) -> App {
        let deck = make_deck(slides);
        let theme = Theme::from_name(&crate::theme::ThemeName::Hacker);
        App::new(
            deck,
            theme,
            None,
            false,
            ImageProtocol::HalfBlocks,
            std::path::PathBuf::from("."),
        )
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // T1-T3: count_bullets
    #[test]
    fn count_bullets_empty_slide() {
        let slide = dummy_slide(vec![]);
        assert_eq!(count_bullets(&slide), 0);
    }

    #[test]
    fn count_bullets_ignores_numbered_lists() {
        let slide = dummy_slide(vec![
            bullet_list(3),
            Block::NumberedList {
                items: vec![ListItem {
                    spans: vec![Span::Plain("x".into())],
                }],
            },
        ]);
        assert_eq!(count_bullets(&slide), 3);
    }

    #[test]
    fn count_bullets_sums_multiple_lists() {
        let slide = dummy_slide(vec![bullet_list(2), bullet_list(3)]);
        assert_eq!(count_bullets(&slide), 5);
    }

    // T4-T5: initial_reveal
    #[test]
    fn initial_reveal_with_bullets_returns_zero() {
        let slide = dummy_slide(vec![bullet_list(3)]);
        assert_eq!(initial_reveal(&slide), 0);
    }

    #[test]
    fn initial_reveal_without_bullets_returns_max() {
        let slide = dummy_slide(vec![Block::Heading {
            level: 1,
            text: "Hi".into(),
        }]);
        assert_eq!(initial_reveal(&slide), usize::MAX);
    }

    // T6-T8: advance
    #[test]
    fn advance_reveals_next_bullet() {
        let mut app = make_app(vec![dummy_slide(vec![bullet_list(3)])]);
        assert_eq!(app.reveal_count, 0);
        app.advance();
        assert_eq!(app.reveal_count, 1);
        app.advance();
        assert_eq!(app.reveal_count, 2);
    }

    #[test]
    fn advance_moves_to_next_slide_when_all_revealed() {
        let mut app = make_app(vec![
            dummy_slide(vec![bullet_list(1)]),
            dummy_slide(vec![Block::Heading {
                level: 1,
                text: "Two".into(),
            }]),
        ]);
        app.advance(); // reveal bullet
        assert_eq!(app.slide_index, 0);
        app.advance(); // all revealed -> next slide
        assert_eq!(app.slide_index, 1);
    }

    #[test]
    fn advance_stays_on_last_slide() {
        let mut app = make_app(vec![dummy_slide(vec![Block::Heading {
            level: 1,
            text: "Only".into(),
        }])]);
        app.advance();
        assert_eq!(app.slide_index, 0);
    }

    // T9-T11: go_back
    #[test]
    fn go_back_hides_last_bullet() {
        let mut app = make_app(vec![dummy_slide(vec![bullet_list(3)])]);
        app.advance();
        app.advance();
        assert_eq!(app.reveal_count, 2);
        app.go_back();
        assert_eq!(app.reveal_count, 1);
    }

    #[test]
    fn go_back_moves_to_previous_slide() {
        let mut app = make_app(vec![
            dummy_slide(vec![bullet_list(1)]),
            dummy_slide(vec![Block::Heading {
                level: 1,
                text: "Two".into(),
            }]),
        ]);
        app.advance(); // reveal bullet
        app.advance(); // next slide
        assert_eq!(app.slide_index, 1);
        app.go_back();
        assert_eq!(app.slide_index, 0);
        assert_eq!(app.reveal_count, 1); // previous slide fully revealed
    }

    #[test]
    fn go_back_stays_on_first_slide() {
        let mut app = make_app(vec![dummy_slide(vec![Block::Heading {
            level: 1,
            text: "First".into(),
        }])]);
        app.go_back();
        assert_eq!(app.slide_index, 0);
    }

    // T12-T14: go_to
    #[test]
    fn go_to_jumps_to_valid_index() {
        let mut app = make_app(vec![
            dummy_slide(vec![]),
            dummy_slide(vec![]),
            dummy_slide(vec![]),
        ]);
        app.go_to(2);
        assert_eq!(app.slide_index, 2);
    }

    #[test]
    fn go_to_clamps_out_of_bounds() {
        let mut app = make_app(vec![dummy_slide(vec![]), dummy_slide(vec![])]);
        app.go_to(100);
        assert_eq!(app.slide_index, 1);
    }

    #[test]
    fn go_to_noop_when_already_there() {
        let mut app = make_app(vec![dummy_slide(vec![]), dummy_slide(vec![])]);
        app.go_to(0);
        assert!(app.transition.is_none()); // no transition for same-slide
    }

    // T15-T17: handle_key
    #[test]
    fn handle_key_quit_returns_true() {
        let mut app = make_app(vec![dummy_slide(vec![])]);
        assert!(app.handle_key(key(KeyCode::Char('q'))));
    }

    #[test]
    fn handle_key_quit_closes_help_first() {
        let mut app = make_app(vec![dummy_slide(vec![])]);
        app.show_help = true;
        assert!(!app.handle_key(key(KeyCode::Char('q')))); // closes help, doesn't quit
        assert!(!app.show_help);
    }

    #[test]
    fn handle_key_follower_ignores_navigation() {
        let mut app = make_app(vec![dummy_slide(vec![]), dummy_slide(vec![])]);
        app.is_follower = true;
        assert!(!app.handle_key(key(KeyCode::Right))); // ignored
        assert_eq!(app.slide_index, 0); // didn't move
    }

    #[test]
    fn presenter_writes_sync_follower_reads() {
        use crate::sync::SyncFile;

        let sync_path = "/__deck_test_sync_presenter_follower__";

        let mk_slides = || {
            vec![
                dummy_slide(vec![Block::Heading {
                    level: 1,
                    text: "One".into(),
                }]),
                dummy_slide(vec![Block::Heading {
                    level: 1,
                    text: "Two".into(),
                }]),
                dummy_slide(vec![Block::Heading {
                    level: 1,
                    text: "Three".into(),
                }]),
            ]
        };

        let presenter_sync = SyncFile::for_file(sync_path);
        let theme = Theme::from_name(&crate::theme::ThemeName::Hacker);
        let mut presenter = App::new(
            make_deck(mk_slides()),
            theme,
            Some(SyncFile::for_file(sync_path)),
            false,
            ImageProtocol::HalfBlocks,
            std::path::PathBuf::from("."),
        );
        let theme2 = Theme::from_name(&crate::theme::ThemeName::Hacker);
        let mut follower = App::new(
            make_deck(mk_slides()),
            theme2,
            Some(SyncFile::for_file(sync_path)),
            true,
            ImageProtocol::HalfBlocks,
            std::path::PathBuf::from("."),
        );

        assert_eq!(presenter.slide_index, 0);
        assert_eq!(follower.slide_index, 0);

        // Presenter advances to slide 1
        presenter.advance();
        assert_eq!(presenter.slide_index, 1);
        follower.tick();
        assert_eq!(follower.slide_index, 1, "follower should sync to slide 1");

        // Presenter advances to slide 2
        presenter.advance();
        assert_eq!(presenter.slide_index, 2);
        follower.tick();
        assert_eq!(follower.slide_index, 2, "follower should sync to slide 2");

        presenter_sync.cleanup();
    }

    #[test]
    fn presenter_sync_with_bullet_reveals() {
        use crate::sync::SyncFile;

        let sync_path = "/__deck_test_sync_bullet_reveal__";

        let mk_slides = || {
            vec![
                dummy_slide(vec![Block::Heading {
                    level: 1,
                    text: "Title".into(),
                }]),
                dummy_slide(vec![
                    Block::Heading {
                        level: 2,
                        text: "Bullets".into(),
                    },
                    bullet_list(3),
                ]),
                dummy_slide(vec![Block::Heading {
                    level: 1,
                    text: "End".into(),
                }]),
            ]
        };

        let presenter_sync = SyncFile::for_file(sync_path);
        let theme = Theme::from_name(&crate::theme::ThemeName::Hacker);
        let mut presenter = App::new(
            make_deck(mk_slides()),
            theme,
            Some(SyncFile::for_file(sync_path)),
            false,
            ImageProtocol::HalfBlocks,
            std::path::PathBuf::from("."),
        );
        let theme2 = Theme::from_name(&crate::theme::ThemeName::Hacker);
        let mut follower = App::new(
            make_deck(mk_slides()),
            theme2,
            Some(SyncFile::for_file(sync_path)),
            true,
            ImageProtocol::HalfBlocks,
            std::path::PathBuf::from("."),
        );

        // Advance from title to bullets slide
        presenter.advance();
        assert_eq!(presenter.slide_index, 1);
        assert_eq!(presenter.reveal_count, 0); // bullets hidden initially
        follower.tick();
        assert_eq!(follower.slide_index, 1);
        assert_eq!(follower.reveal_count, 0);

        // Reveal bullet 1
        presenter.advance();
        assert_eq!(presenter.slide_index, 1);
        assert_eq!(presenter.reveal_count, 1);
        follower.tick();
        assert_eq!(follower.reveal_count, 1, "follower should sync reveal to 1");

        // Reveal bullet 2
        presenter.advance();
        assert_eq!(presenter.reveal_count, 2);
        follower.tick();
        assert_eq!(follower.reveal_count, 2, "follower should sync reveal to 2");

        // Reveal bullet 3
        presenter.advance();
        assert_eq!(presenter.reveal_count, 3);
        follower.tick();
        assert_eq!(follower.reveal_count, 3, "follower should sync reveal to 3");

        // Advance to next slide (all bullets revealed)
        presenter.advance();
        assert_eq!(presenter.slide_index, 2);
        follower.tick();
        assert_eq!(follower.slide_index, 2, "follower should sync to slide 2");

        presenter_sync.cleanup();
    }
}
