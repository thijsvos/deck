use std::time::{Duration, Instant};

use ratatui::{layout::Rect, style::Style, Frame};
use serde::Deserialize;

use crate::theme::Theme;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionKind {
    #[default]
    None,
    Glitch,
    Fade,
    Wipe,
    Dissolve,
}

pub struct TransitionState {
    pub kind: TransitionKind,
    pub started: Instant,
    pub duration: Duration,
}

impl TransitionState {
    pub fn new(kind: TransitionKind) -> Self {
        Self {
            kind,
            started: Instant::now(),
            duration: Duration::from_millis(400),
        }
    }

    pub fn is_done(&self) -> bool {
        self.started.elapsed() >= self.duration
    }
}

use crate::util::{Rng, GLITCH_CHARS};

/// Apply transition overlay on top of already-rendered slide content.
pub fn apply_transition(frame: &mut Frame, area: Rect, state: &TransitionState, theme: &Theme) {
    let elapsed = state.started.elapsed().as_secs_f64();
    let t = (elapsed / state.duration.as_secs_f64()).min(1.0);

    let buf = frame.buffer_mut();
    let mut rng = Rng::new(elapsed.to_bits());

    match state.kind {
        TransitionKind::Glitch => {
            // Random characters gradually resolve into the slide content
            for y in 0..area.height {
                for x in 0..area.width {
                    if rng.next_f64() > t {
                        let ch = GLITCH_CHARS[rng.next() as usize % GLITCH_CHARS.len()];
                        if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                            cell.set_char(ch);
                            cell.set_style(Style::default().fg(theme.accent).bg(theme.bg));
                        }
                    }
                }
            }
        }
        TransitionKind::Fade => {
            // Shade blocks fade out to reveal slide content
            let shades = ['█', '▓', '▒', '░'];
            let shade_idx = (t * shades.len() as f64) as usize;
            if shade_idx < shades.len() {
                for y in 0..area.height {
                    for x in 0..area.width {
                        if rng.next_f64() > t {
                            let ch = shades[shade_idx.min(shades.len() - 1)];
                            if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                                cell.set_char(ch);
                                cell.set_style(Style::default().fg(theme.dim).bg(theme.bg));
                            }
                        }
                    }
                }
            }
        }
        TransitionKind::Wipe => {
            // Clean left-to-right wipe revealing the new slide
            // A vertical bar sweeps across; cells left of it are revealed, right are blanked
            let progress_col = (t * area.width as f64) as u16;
            let edge_width = 3u16; // soft edge gradient

            for y in 0..area.height {
                for x in 0..area.width {
                    if x > progress_col + edge_width {
                        // Not yet revealed: blank
                        if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                            cell.set_char(' ');
                            cell.set_style(Style::default().bg(theme.bg));
                        }
                    } else if x > progress_col {
                        // Soft edge: thin line
                        let ch = '│';
                        if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                            cell.set_char(ch);
                            cell.set_style(Style::default().fg(theme.dim).bg(theme.bg));
                        }
                    }
                    // x <= progress_col: already revealed (slide content shows through)
                }
            }
        }
        TransitionKind::Dissolve => {
            // Soft pixel dissolve — cells randomly appear with easing
            // Uses smoothstep for a gentler feel than linear random
            let eased = t * t * (3.0 - 2.0 * t); // smoothstep
            for y in 0..area.height {
                for x in 0..area.width {
                    if rng.next_f64() > eased {
                        // Not yet dissolved: show a soft dot
                        let ch = '·';
                        if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                            cell.set_char(ch);
                            cell.set_style(Style::default().fg(theme.dim).bg(theme.bg));
                        }
                    }
                }
            }
        }
        TransitionKind::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_state_new_stores_kind() {
        let t = TransitionState::new(TransitionKind::Glitch);
        assert!(matches!(t.kind, TransitionKind::Glitch));
        assert_eq!(t.duration, Duration::from_millis(400));
    }

    #[test]
    fn transition_not_done_immediately() {
        let t = TransitionState::new(TransitionKind::Fade);
        assert!(!t.is_done());
    }

    #[test]
    fn transition_done_after_zero_duration() {
        let mut t = TransitionState::new(TransitionKind::Wipe);
        t.duration = Duration::ZERO;
        assert!(t.is_done());
    }

    #[test]
    fn rng_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next(), b.next());
        }
    }

    #[test]
    fn rng_different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next(), b.next());
    }

    #[test]
    fn rng_next_f64_in_range() {
        let mut rng = Rng::new(123);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v < 1.0, "got {}", v);
        }
    }

    #[test]
    fn rng_zero_seed_not_stuck() {
        let mut rng = Rng::new(0);
        let first = rng.next();
        let second = rng.next();
        assert_ne!(first, 0);
        assert_ne!(first, second);
    }
}
