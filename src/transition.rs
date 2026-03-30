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

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f64(&mut self) -> f64 {
        (self.next() % 10000) as f64 / 10000.0
    }
}

const GLITCH_CHARS: &[char] = &[
    '!', '@', '#', '$', '%', '^', '&', '*', '<', '>', '{', '}', '[', ']', '|', '/', '\\', '~',
    '░', '▒', '▓', '█', '▄', '▀', '▌', '▐',
];

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
        TransitionKind::None => {}
    }
}
