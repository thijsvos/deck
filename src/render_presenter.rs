use std::time::Instant;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout as RLayout, Rect},
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Paragraph, Wrap},
    Frame,
};

use crate::parse::Deck;
use crate::render::{self, RenderCtx};
use crate::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render_presenter(
    frame: &mut Frame,
    area: Rect,
    deck: &Deck,
    slide_index: usize,
    reveal_count: usize,
    theme: &Theme,
    timer: &Instant,
    ctx: &mut RenderCtx,
) {
    // Top 70% for slides, bottom 30% for notes/timer
    let rows = RLayout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Top: current (65%) + next preview (35%)
    let cols = RLayout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(rows[0]);

    // Bottom: notes + timer
    let bottom = RLayout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(14)])
        .split(rows[1]);

    // -- Current slide --
    let current_border = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" Current ");
    let inner = current_border.inner(cols[0]);
    frame.render_widget(current_border, cols[0]);

    let slide = &deck.slides[slide_index];
    render::render_slide(frame, inner, slide, theme, reveal_count, ctx);

    // -- Next slide preview --
    let next_border = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.code_border())
        .title(" Next ");
    let inner = next_border.inner(cols[1]);
    frame.render_widget(next_border, cols[1]);

    if let Some(next_slide) = deck.slides.get(slide_index + 1) {
        render::render_slide(frame, inner, next_slide, theme, usize::MAX, ctx);
    } else {
        let msg = Line::from(RSpan::styled("  End of deck", theme.rule_style()));
        frame.render_widget(Paragraph::new(msg), inner);
    }

    // -- Speaker notes --
    let notes_border = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.code_border())
        .title(" Notes ");
    let inner = notes_border.inner(bottom[0]);
    frame.render_widget(notes_border, bottom[0]);

    let notes_text = slide.notes.join("\n");
    if notes_text.is_empty() {
        let msg = Line::from(RSpan::styled("  No notes", theme.rule_style()));
        frame.render_widget(Paragraph::new(msg), inner);
    } else {
        frame.render_widget(
            Paragraph::new(notes_text)
                .style(theme.body_style())
                .wrap(Wrap { trim: false }),
            inner,
        );
    }

    // -- Timer --
    let elapsed = timer.elapsed().as_secs();
    let mins = elapsed / 60;
    let secs = elapsed % 60;
    let timer_text = format!("{mins:02}:{secs:02}");

    let timer_border = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.code_border())
        .title(" Timer ");
    let paragraph = Paragraph::new(Line::from(RSpan::styled(timer_text, theme.status_accent())))
        .alignment(Alignment::Center)
        .block(timer_border);
    frame.render_widget(paragraph, bottom[1]);
}
