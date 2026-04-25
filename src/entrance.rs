use std::collections::HashMap;
use std::time::{Duration, Instant};

use ratatui::{layout::Rect, style::Style, Frame};

use crate::theme::Theme;

/// What kind of entrance animation a block gets.
#[derive(Clone, Debug)]
pub enum EntranceKind {
    /// H1 headings: glitch characters resolve into real text
    Decrypt,
    /// H2+ headings: characters appear left-to-right
    SlideIn,
    /// Bullet items: staggered cascade with delay per item
    Cascade,
    /// Code blocks: line-by-line typing with cursor
    Typewriter,
    /// Paragraphs, images: cell-by-cell dissolve
    FadeIn,
}

/// Tracks animation progress for a single block entrance.
pub struct EntranceState {
    pub kind: EntranceKind,
    pub started: Instant,
    pub duration: Duration,
}

impl EntranceState {
    pub fn new(kind: EntranceKind, duration: Duration) -> Self {
        Self {
            kind,
            started: Instant::now(),
            duration,
        }
    }

    /// Animation progress 0.0..=1.0
    pub fn progress(&self) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self.started.elapsed().as_secs_f64() / self.duration.as_secs_f64()).min(1.0)
    }

    pub fn is_done(&self) -> bool {
        self.started.elapsed() >= self.duration
    }
}

/// Tracks per-block entrance animations across frames.
/// Keyed by (slide_index, block_index).
pub struct EntranceTracker {
    states: HashMap<(usize, usize), EntranceState>,
    current_slide: usize,
}

impl Default for EntranceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl EntranceTracker {
    /// Construct an empty tracker with no active slide. The first call to
    /// `on_slide_change` will not purge anything (there's nothing to purge).
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            current_slide: usize::MAX,
        }
    }

    /// Notify the tracker of the active slide.
    ///
    /// If `slide` differs from the previously tracked slide, all in-flight
    /// animations are purged. Safe to call every frame: a no-op when the
    /// slide hasn't changed.
    pub fn on_slide_change(&mut self, slide: usize) {
        if slide != self.current_slide {
            self.states.clear();
            self.current_slide = slide;
        }
    }

    /// Get entrance state for a block, creating a new animation if first seen.
    /// Returns `None` if the animation is already finished (no need to apply
    /// effects).
    ///
    /// `kind` and `duration` are only used when creating a new state; on
    /// subsequent calls for the same `(slide, block_idx)` they are ignored
    /// and the existing state is returned.
    pub fn get_or_start(
        &mut self,
        slide: usize,
        block_idx: usize,
        kind: EntranceKind,
        duration: Duration,
    ) -> Option<&EntranceState> {
        let key = (slide, block_idx);
        let state = self
            .states
            .entry(key)
            .or_insert_with(|| EntranceState::new(kind, duration));
        if state.is_done() {
            None
        } else {
            Some(state)
        }
    }

    /// True if any animation is still running (drives 60fps frame rate).
    pub fn has_active(&self) -> bool {
        self.states.values().any(|s| !s.is_done())
    }
}

use crate::util::{Rng, GLITCH_CHARS};

// ── Entrance effect implementations ──────────────────────────────

/// Decrypt effect: replace non-empty cells with glitch chars, resolving over time.
pub fn apply_decrypt(frame: &mut Frame, rect: Rect, progress: f64, theme: &Theme) {
    let buf = frame.buffer_mut();
    // Seed from rect position for deterministic pattern per frame
    let mut rng = Rng::new(progress.to_bits() ^ (rect.x as u64 * 7919 + rect.y as u64 * 104729));

    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if rng.next_f64() > progress {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    // Skip blank cells AND wide-grapheme continuation cells
                    // (whose `symbol()` is `""`) — overwriting the second cell of
                    // a wide character corrupts the rendering.
                    let sym = cell.symbol();
                    if !sym.is_empty() && sym != " " {
                        let ch = GLITCH_CHARS[rng.next() as usize % GLITCH_CHARS.len()];
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(theme.accent).bg(theme.bg));
                    }
                }
            }
        }
    }
}

/// Slide-in effect: blank cells beyond the progress column.
pub fn apply_slide_in(frame: &mut Frame, rect: Rect, progress: f64, theme: &Theme) {
    let reveal_col = (progress * rect.width as f64) as u16;
    let buf = frame.buffer_mut();

    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let local_x = x - rect.x;
            if local_x > reveal_col {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(theme.bg));
                }
            }
        }
    }
}

/// Fade-in effect: cell-by-cell dissolve using random threshold.
pub fn apply_fade_in(frame: &mut Frame, rect: Rect, progress: f64, theme: &Theme) {
    let eased = progress * progress * (3.0 - 2.0 * progress); // smoothstep
    let buf = frame.buffer_mut();
    let mut rng = Rng::new(rect.x as u64 * 31 + rect.y as u64 * 37);

    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if rng.next_f64() > eased {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(theme.bg));
                }
            }
        }
    }
}

/// Compute how many code lines + char progress for typewriter effect.
pub fn typewriter_visible(progress: f64, total_lines: usize) -> (usize, f64) {
    if total_lines == 0 {
        return (0, 1.0);
    }
    let lines_progress = progress * total_lines as f64;
    let full_lines = (lines_progress.floor() as usize).min(total_lines);
    let char_frac = lines_progress.fract();
    (full_lines, char_frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrance_state_progress() {
        let state = EntranceState::new(EntranceKind::Decrypt, Duration::from_millis(400));
        assert!(state.progress() < 0.1); // just created
        assert!(!state.is_done());
    }

    #[test]
    fn entrance_state_zero_duration() {
        let state = EntranceState::new(EntranceKind::FadeIn, Duration::ZERO);
        assert_eq!(state.progress(), 1.0);
        assert!(state.is_done());
    }

    #[test]
    fn tracker_starts_new_animation() {
        let mut tracker = EntranceTracker::new();
        tracker.on_slide_change(0);
        let state = tracker.get_or_start(0, 0, EntranceKind::Decrypt, Duration::from_millis(600));
        assert!(state.is_some());
    }

    #[test]
    fn tracker_clears_on_slide_change() {
        let mut tracker = EntranceTracker::new();
        tracker.on_slide_change(0);
        tracker.get_or_start(0, 0, EntranceKind::Decrypt, Duration::from_millis(600));
        assert!(tracker.has_active());
        tracker.on_slide_change(1);
        assert!(!tracker.has_active());
    }

    #[test]
    fn tracker_returns_none_when_done() {
        let mut tracker = EntranceTracker::new();
        tracker.on_slide_change(0);
        // Insert a state that's already done
        tracker.states.insert(
            (0, 0),
            EntranceState::new(EntranceKind::FadeIn, Duration::ZERO),
        );
        let state = tracker.get_or_start(0, 0, EntranceKind::FadeIn, Duration::from_millis(200));
        assert!(state.is_none()); // already finished
    }

    #[test]
    fn typewriter_visible_empty() {
        let (lines, _) = typewriter_visible(0.5, 0);
        assert_eq!(lines, 0);
    }

    #[test]
    fn typewriter_visible_full() {
        let (lines, frac) = typewriter_visible(1.0, 10);
        assert_eq!(lines, 10);
        assert!((frac - 0.0).abs() < 0.01);
    }

    #[test]
    fn typewriter_visible_midway() {
        let (lines, _) = typewriter_visible(0.5, 10);
        assert_eq!(lines, 5);
    }

    #[test]
    fn rng_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        assert_eq!(a.next(), b.next());
    }

    #[test]
    fn typewriter_visible_zero_progress() {
        let (lines, frac) = typewriter_visible(0.0, 10);
        assert_eq!(lines, 0);
        assert!((frac - 0.0).abs() < 0.01);
    }

    #[test]
    fn typewriter_visible_over_one_clamps() {
        let (lines, _) = typewriter_visible(1.5, 10);
        assert_eq!(lines, 10); // min(15, 10) = 10
    }

    #[test]
    fn tracker_has_active_with_running() {
        let mut tracker = EntranceTracker::new();
        tracker.on_slide_change(0);
        tracker.get_or_start(0, 0, EntranceKind::Decrypt, Duration::from_secs(10));
        assert!(tracker.has_active());
    }
}
