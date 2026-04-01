mod app;
mod background;
mod bigtext;
mod image_renderer;
mod input;
mod markdown;
mod parse;
mod render;
mod render_presenter;
mod sync;
mod theme;
mod transition;

use std::io::{self, Write};
use std::path::Path;
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
use crate::sync::SyncFile;
use crate::theme::{Theme, ThemeName};

#[derive(Parser)]
#[command(name = "deck", version, about = "Terminal presentations with style")]
struct Cli {
    /// Markdown file to present
    file: String,

    /// Theme: hacker (default), corporate, catppuccin, or minimal
    #[arg(long, value_enum)]
    theme: Option<ThemeName>,

    /// Presenter screen: shows notes + timer, controls navigation. Syncs to --follow instances.
    #[arg(long, conflicts_with = "follow")]
    present: bool,

    /// Audience screen: full-screen slides that follow a --present instance.
    #[arg(long, conflicts_with = "present")]
    follow: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let content = std::fs::read_to_string(&cli.file)
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, format!("{}: {}", cli.file, e)))?;

    let deck = parse_deck(&content);

    if deck.slides.is_empty() {
        eprintln!("No slides found in {}", cli.file);
        return Ok(());
    }

    let theme_name = cli.theme.unwrap_or(deck.meta.theme.clone());

    let theme = Theme::from_name(&theme_name);
    let protocol = image_renderer::detect_protocol();
    let base_dir = Path::new(&cli.file)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let sync = if cli.present || cli.follow {
        Some(SyncFile::for_file(&cli.file))
    } else {
        None
    };

    let mut app = App::new(deck, theme, sync, cli.follow, protocol, base_dir);

    // Start in presenter mode when --present is used
    if cli.present {
        app.mode = crate::app::Mode::Presenter;
    }

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

        // Flush deferred Kitty/Sixel images after ratatui has written its buffer
        if !app.deferred_images.is_empty() {
            image_renderer::flush_deferred(terminal.backend_mut(), &app.deferred_images)?;
            terminal.backend_mut().flush()?;
        }

        let timeout = if app.transition.is_some() {
            Duration::from_millis(16) // 60fps during transitions
        } else if app.has_active_background() || app.is_follower {
            Duration::from_millis(33) // ~30fps for backgrounds or sync polling
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

    // Clean up sync file
    app.cleanup_sync();

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)?;

    Ok(())
}
