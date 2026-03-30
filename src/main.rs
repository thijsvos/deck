mod app;
mod background;
mod bigtext;
mod input;
mod markdown;
mod parse;
mod render;
mod render_presenter;
mod theme;
mod transition;

use std::io;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::parse::parse_deck;
use crate::theme::{Theme, ThemeName};

#[derive(Parser)]
#[command(name = "deck", version, about = "Terminal presentations with style")]
struct Cli {
    /// Markdown file to present
    file: String,

    /// Theme: hacker (default) or minimal
    #[arg(long)]
    theme: Option<String>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let content = std::fs::read_to_string(&cli.file).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: {}", cli.file, e),
        )
    })?;

    let deck = parse_deck(&content);

    if deck.slides.is_empty() {
        eprintln!("No slides found in {}", cli.file);
        return Ok(());
    }

    let theme_name = cli
        .theme
        .as_deref()
        .map(|t| match t {
            "minimal" => ThemeName::Minimal,
            _ => ThemeName::Hacker,
        })
        .unwrap_or(deck.meta.theme.clone());

    let theme = Theme::from_name(&theme_name);
    let mut app = App::new(deck, theme);

    // Clean terminal restore on panic
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        default_panic(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| app.draw(frame))?;

        let timeout = if app.transition.is_some() {
            Duration::from_millis(16)  // 60fps during transitions
        } else if app.has_active_background() {
            Duration::from_millis(33)  // ~30fps for background animation
        } else {
            Duration::from_millis(100) // low CPU when idle
        };

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }

        app.tick();
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)?;

    Ok(())
}
