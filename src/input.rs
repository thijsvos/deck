use crossterm::event::{KeyCode, KeyEvent};

pub enum Action {
    Next,
    Prev,
    First,
    Last,
    TogglePresenter,
    ToggleHelp,
    StartGoTo,
    GoToConfirm,
    GoToDigit(char),
    GoToCancel,
    ResetTimer,
    Quit,
    None,
}

pub fn map_key(key: KeyEvent, in_goto: bool) -> Action {
    if in_goto {
        return match key.code {
            KeyCode::Enter => Action::GoToConfirm,
            KeyCode::Esc => Action::GoToCancel,
            KeyCode::Char(c) if c.is_ascii_digit() => Action::GoToDigit(c),
            KeyCode::Backspace => Action::GoToDigit('\x08'),
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Right | KeyCode::Down | KeyCode::Char(' ') | KeyCode::Enter => Action::Next,
        KeyCode::Char('l') | KeyCode::Char('j') => Action::Next,
        KeyCode::Left | KeyCode::Up | KeyCode::Backspace => Action::Prev,
        KeyCode::Char('h') | KeyCode::Char('k') => Action::Prev,
        KeyCode::Char('g') => Action::First,
        KeyCode::Char('G') => Action::Last,
        KeyCode::Char('p') => Action::TogglePresenter,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char(':') => Action::StartGoTo,
        KeyCode::Char('r') => Action::ResetTimer,
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        _ => Action::None,
    }
}
