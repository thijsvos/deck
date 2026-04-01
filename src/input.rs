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
    GoToBackspace,
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
            KeyCode::Backspace => Action::GoToBackspace,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn next_keys() {
        assert!(matches!(map_key(key(KeyCode::Right), false), Action::Next));
        assert!(matches!(
            map_key(key(KeyCode::Char(' ')), false),
            Action::Next
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('l')), false),
            Action::Next
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('j')), false),
            Action::Next
        ));
    }

    #[test]
    fn prev_keys() {
        assert!(matches!(map_key(key(KeyCode::Left), false), Action::Prev));
        assert!(matches!(
            map_key(key(KeyCode::Char('h')), false),
            Action::Prev
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('k')), false),
            Action::Prev
        ));
    }

    #[test]
    fn quit_keys() {
        assert!(matches!(
            map_key(key(KeyCode::Char('q')), false),
            Action::Quit
        ));
        assert!(matches!(map_key(key(KeyCode::Esc), false), Action::Quit));
    }

    #[test]
    fn goto_mode() {
        assert!(matches!(
            map_key(key(KeyCode::Char(':')), false),
            Action::StartGoTo
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('5')), true),
            Action::GoToDigit('5')
        ));
        assert!(matches!(
            map_key(key(KeyCode::Enter), true),
            Action::GoToConfirm
        ));
        assert!(matches!(
            map_key(key(KeyCode::Esc), true),
            Action::GoToCancel
        ));
    }

    #[test]
    fn goto_ignores_non_digits() {
        assert!(matches!(
            map_key(key(KeyCode::Char('a')), true),
            Action::None
        ));
    }

    #[test]
    fn control_keys() {
        assert!(matches!(
            map_key(key(KeyCode::Char('p')), false),
            Action::TogglePresenter
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('?')), false),
            Action::ToggleHelp
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('r')), false),
            Action::ResetTimer
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('g')), false),
            Action::First
        ));
        assert!(matches!(
            map_key(key(KeyCode::Char('G')), false),
            Action::Last
        ));
    }
}
