use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    style::Style,
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Paragraph, Wrap},
    Frame,
};

use crate::bigtext;
use crate::entrance::{self, EntranceKind, EntranceTracker};
use crate::highlight::Highlighter;
use crate::image_renderer::{self, DeferredImage, ImageCache, ImageProtocol};
use crate::markdown::{Block, Span};
use crate::parse::{Layout, Slide};
use crate::theme::Theme;

/// Shared rendering context to avoid passing many parameters individually.
pub struct RenderCtx<'a> {
    pub protocol: ImageProtocol,
    pub image_cache: &'a mut ImageCache,
    pub deferred: &'a mut Vec<DeferredImage>,
    pub base_dir: &'a Path,
    pub highlighter: &'a Highlighter,
    pub entrances: &'a mut EntranceTracker,
    pub slide_index: usize,
}

pub fn render_slide(
    frame: &mut Frame,
    area: Rect,
    slide: &Slide,
    theme: &Theme,
    reveal: usize,
    ctx: &mut RenderCtx,
) {
    // Fill background
    let bg = WidgetBlock::default().style(theme.body_style());
    frame.render_widget(bg, area);

    let content = padded(area, 3, 2);

    match slide.layout {
        Layout::Default => {
            render_blocks(frame, content, &slide.blocks, theme, reveal, ctx);
        }
        Layout::Center => {
            let total_height = estimate_height(&slide.blocks, content.width);
            let y_off = content.height.saturating_sub(total_height) / 2;
            let centered = Rect {
                y: content.y + y_off,
                height: total_height.min(content.height),
                ..content
            };
            render_blocks(frame, centered, &slide.blocks, theme, reveal, ctx);
        }
        Layout::Columns => {
            let mut y = content.y;
            for (i, block) in slide.blocks.iter().enumerate() {
                let remaining = Rect::new(
                    content.x,
                    y,
                    content.width,
                    content.height.saturating_sub(y - content.y),
                );
                let (h, _) = render_block(frame, remaining, block, theme, ctx, i);
                y += h;
            }

            if let Some(cols) = &slide.columns {
                let col_area = Rect::new(
                    content.x,
                    y + 1,
                    content.width,
                    content.height.saturating_sub(y - content.y + 1),
                );
                let halves = RLayout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(48), Constraint::Percentage(48)])
                    .spacing(2)
                    .split(col_area);

                render_blocks(frame, halves[0], &cols.left, theme, usize::MAX, ctx);
                render_blocks(frame, halves[1], &cols.right, theme, usize::MAX, ctx);
            }
        }
    }
}

pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    slide_num: usize,
    total: usize,
    elapsed_secs: u64,
    theme: &Theme,
) {
    let mins = elapsed_secs / 60;
    let secs = elapsed_secs % 60;

    let left = format!(" {title} ");
    let center = format!(" {slide_num}/{total} ");
    let right = format!(" {mins:02}:{secs:02} ");

    let width = area.width as usize;
    let used = left.len() + center.len() + right.len();
    let pad = width.saturating_sub(used);
    let left_pad = pad / 2;
    let right_pad = pad.saturating_sub(left_pad);

    let line = Line::from(vec![
        RSpan::styled(left, theme.status_accent()),
        RSpan::styled("─".repeat(left_pad), theme.status_style()),
        RSpan::styled(center, theme.status_style()),
        RSpan::styled("─".repeat(right_pad), theme.status_style()),
        RSpan::styled(right, theme.status_accent()),
    ]);

    let bg = WidgetBlock::default().style(theme.status_style());
    frame.render_widget(bg, area);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_blocks(
    frame: &mut Frame,
    area: Rect,
    blocks: &[Block],
    theme: &Theme,
    reveal: usize,
    ctx: &mut RenderCtx,
) {
    let mut y = area.y;
    let mut bullet_count: usize = 0;

    for (block_idx, block) in blocks.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }

        let remaining = Rect::new(
            area.x,
            y,
            area.width,
            area.height.saturating_sub(y - area.y),
        );

        match block {
            Block::BulletList { items } => {
                for (item_i, item) in items.iter().enumerate() {
                    if bullet_count >= reveal {
                        return;
                    }
                    let line = spans_to_line(&item.spans, theme);
                    let mut spans_vec = vec![RSpan::styled(
                        format!("  {} ", theme.bullet),
                        theme.bullet_style(),
                    )];
                    spans_vec.extend(line.spans);
                    let combined = Line::from(spans_vec);
                    let h = estimate_line_height(&combined, remaining.width);
                    if y + h > area.y + area.height {
                        return;
                    }
                    let bullet_rect = Rect::new(remaining.x, y, remaining.width, h);
                    frame.render_widget(
                        Paragraph::new(combined).wrap(Wrap { trim: false }),
                        bullet_rect,
                    );

                    // Cascade entrance for each bullet item
                    let cascade_idx = block_idx * 100 + item_i;
                    let stagger = std::time::Duration::from_millis(100 * item_i as u64);
                    if let Some(state) = ctx.entrances.get_or_start(
                        ctx.slide_index,
                        cascade_idx,
                        EntranceKind::Cascade,
                        std::time::Duration::from_millis(200) + stagger,
                    ) {
                        let raw_progress = state.progress();
                        // Account for stagger: animation doesn't visually start until stagger elapses
                        let stagger_frac =
                            stagger.as_secs_f64() / (state.duration.as_secs_f64().max(0.001));
                        let visual_progress =
                            ((raw_progress - stagger_frac) / (1.0 - stagger_frac)).clamp(0.0, 1.0);
                        entrance::apply_fade_in(frame, bullet_rect, visual_progress, theme);
                    }

                    y += h;
                    bullet_count += 1;
                }
            }
            _ => {
                let (h, block_rect) = render_block(frame, remaining, block, theme, ctx, block_idx);

                // Apply entrance effects based on block type
                if let Some(block_rect) = block_rect {
                    apply_entrance_for_block(frame, block, block_rect, theme, ctx, block_idx);
                }

                y += h;
            }
        }
    }
}

fn apply_entrance_for_block(
    frame: &mut Frame,
    block: &Block,
    rect: Rect,
    theme: &Theme,
    ctx: &mut RenderCtx,
    block_idx: usize,
) {
    let (kind, duration) = match block {
        Block::Heading { level: 1, .. } => {
            (EntranceKind::Decrypt, std::time::Duration::from_millis(600))
        }
        Block::Heading { .. } => (EntranceKind::SlideIn, std::time::Duration::from_millis(300)),
        Block::Paragraph { .. } => (EntranceKind::FadeIn, std::time::Duration::from_millis(250)),
        Block::Image { .. } => (EntranceKind::FadeIn, std::time::Duration::from_millis(300)),
        // Code and bullets handled separately
        _ => return,
    };

    if let Some(state) = ctx
        .entrances
        .get_or_start(ctx.slide_index, block_idx, kind, duration)
    {
        let progress = state.progress();
        match &state.kind {
            EntranceKind::Decrypt => entrance::apply_decrypt(frame, rect, progress, theme),
            EntranceKind::SlideIn => entrance::apply_slide_in(frame, rect, progress, theme),
            EntranceKind::FadeIn => entrance::apply_fade_in(frame, rect, progress, theme),
            _ => {}
        }
    }
}

/// Renders a single block and returns (consumed_height, block_rect).
fn render_block(
    frame: &mut Frame,
    area: Rect,
    block: &Block,
    theme: &Theme,
    ctx: &mut RenderCtx,
    block_idx: usize,
) -> (u16, Option<Rect>) {
    match block {
        Block::Heading { level: 1, text } => {
            let big = bigtext::render(text, theme.font);
            let height = big.len() as u16;
            let rect = Rect::new(area.x, area.y, area.width, height.min(area.height));
            for (i, line_str) in big.iter().enumerate() {
                if area.y + i as u16 >= area.y + area.height {
                    break;
                }
                let span = RSpan::styled(line_str.clone(), theme.h1_style());
                frame.render_widget(
                    Paragraph::new(Line::from(span)),
                    Rect::new(area.x, area.y + i as u16, area.width, 1),
                );
            }
            (height + 1, Some(rect))
        }
        Block::Heading { text, .. } => {
            let rect = Rect::new(area.x, area.y, area.width, 1);
            let line = Line::from(vec![RSpan::styled(text.clone(), theme.heading_style())]);
            frame.render_widget(Paragraph::new(line), rect);
            (2, Some(rect))
        }
        Block::Paragraph { spans } => {
            let line = spans_to_line(spans, theme);
            let h = estimate_line_height(&line, area.width);
            let rect = Rect::new(area.x, area.y, area.width, h);
            frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), rect);
            (h + 1, Some(rect))
        }
        Block::BulletList { items } => {
            let mut h = 0u16;
            for item in items {
                let line = spans_to_line(&item.spans, theme);
                let mut spans_vec = vec![RSpan::styled(
                    format!("  {} ", theme.bullet),
                    theme.bullet_style(),
                )];
                spans_vec.extend(line.spans);
                let combined = Line::from(spans_vec);
                let lh = estimate_line_height(&combined, area.width);
                frame.render_widget(
                    Paragraph::new(combined).wrap(Wrap { trim: false }),
                    Rect::new(area.x, area.y + h, area.width, lh),
                );
                h += lh;
            }
            (h, None) // bullets handle their own entrances
        }
        Block::NumberedList { items } => {
            let mut h = 0u16;
            for (i, item) in items.iter().enumerate() {
                let line = spans_to_line(&item.spans, theme);
                let mut spans_vec = vec![RSpan::styled(
                    format!("  {}. ", i + 1),
                    theme.bullet_style(),
                )];
                spans_vec.extend(line.spans);
                let combined = Line::from(spans_vec);
                let lh = estimate_line_height(&combined, area.width);
                frame.render_widget(
                    Paragraph::new(combined).wrap(Wrap { trim: false }),
                    Rect::new(area.x, area.y + h, area.width, lh),
                );
                h += lh;
            }
            (h, None)
        }
        Block::Code { lang, code } => {
            let title = lang.as_deref().unwrap_or("");
            let highlighted = ctx
                .highlighter
                .highlight(code, if title.is_empty() { "txt" } else { title });
            let total_lines = highlighted.len();
            let inner_height = total_lines.max(1) as u16;
            let total_height = inner_height + 2; // borders

            // Check for typewriter entrance
            let typewriter_dur = std::time::Duration::from_millis(50 * total_lines as u64 + 200);
            let entrance = ctx.entrances.get_or_start(
                ctx.slide_index,
                block_idx,
                EntranceKind::Typewriter,
                typewriter_dur,
            );

            let (visible_lines, char_frac) = match entrance {
                Some(state) => entrance::typewriter_visible(state.progress(), total_lines),
                None => (total_lines, 1.0),
            };

            // Build visible lines with optional partial last line + cursor
            let mut lines: Vec<Line<'static>> = Vec::with_capacity(inner_height as usize);
            for (i, hl) in highlighted.into_iter().enumerate() {
                if i < visible_lines {
                    lines.push(hl);
                } else if i == visible_lines && visible_lines < total_lines {
                    // Partially typed current line
                    let full_text: String = hl.spans.iter().map(|s| s.content.as_ref()).collect();
                    let chars_to_show = (char_frac * full_text.len() as f64) as usize;
                    if chars_to_show == 0 {
                        // Just show cursor
                        lines.push(Line::from(RSpan::styled("▌", theme.status_accent())));
                    } else {
                        // Truncate spans to chars_to_show characters
                        let mut partial_spans: Vec<RSpan<'static>> = Vec::new();
                        let mut chars_left = chars_to_show;
                        for span in hl.spans {
                            let span_len = span.content.len();
                            if chars_left >= span_len {
                                partial_spans.push(span);
                                chars_left -= span_len;
                            } else {
                                let truncated: String =
                                    span.content.chars().take(chars_left).collect();
                                partial_spans.push(RSpan::styled(truncated, span.style));
                                break;
                            }
                        }
                        // Append blinking cursor
                        partial_spans.push(RSpan::styled("▌", theme.status_accent()));
                        lines.push(Line::from(partial_spans));
                    }
                } else {
                    // Not yet visible — empty line to maintain block height
                    lines.push(Line::from(""));
                }
            }

            let code_bg = ctx.highlighter.bg_color().unwrap_or(theme.code_bg);
            let block_widget = WidgetBlock::default()
                .borders(Borders::ALL)
                .border_style(theme.code_border())
                .title(title);
            let paragraph = Paragraph::new(lines)
                .style(Style::default().bg(code_bg))
                .block(block_widget);
            let code_rect = Rect::new(area.x, area.y, area.width, total_height.min(area.height));
            frame.render_widget(paragraph, code_rect);
            (total_height + 1, None) // code handles its own entrance via typewriter
        }
        Block::HorizontalRule => {
            let rule = "─".repeat(area.width as usize);
            frame.render_widget(
                Paragraph::new(RSpan::styled(rule, theme.rule_style())),
                Rect::new(area.x, area.y, area.width, 1),
            );
            (2, None)
        }
        Block::Image { path, alt } => {
            // Load, resize, and cache — only resizes once per (path, size)
            let resized =
                match ctx
                    .image_cache
                    .get_resized(path, ctx.base_dir, area.width, area.height)
                {
                    Some(img) => img.clone(),
                    None => {
                        let label = if alt.is_empty() {
                            format!("[Image: {path}]")
                        } else {
                            format!("[Image: {alt}]")
                        };
                        let line = Line::from(RSpan::styled(label, theme.rule_style()));
                        frame.render_widget(
                            Paragraph::new(line),
                            Rect::new(area.x, area.y, area.width, 1),
                        );
                        return (2, None);
                    }
                };

            let (px_w, px_h) = resized.dimensions();
            let consumed_rows = px_h.div_ceil(2) as u16;
            let consumed_cols = px_w as u16;

            // Center horizontally
            let x_offset = area.width.saturating_sub(consumed_cols) / 2;
            let img_area = Rect::new(
                area.x + x_offset,
                area.y,
                consumed_cols,
                consumed_rows.min(area.height),
            );

            // Always render half-blocks into the buffer
            let buf = frame.buffer_mut();
            image_renderer::render_halfblocks(buf, img_area, &resized);

            // Queue deferred high-res render for Kitty/Sixel
            if matches!(ctx.protocol, ImageProtocol::Kitty | ImageProtocol::Sixel) {
                let kitty_b64 = if ctx.protocol == ImageProtocol::Kitty {
                    ctx.image_cache
                        .get_encoded_kitty(path, ctx.base_dir, area.width, area.height)
                        .map(|s| s.to_string())
                } else {
                    None
                };
                ctx.deferred.push(DeferredImage {
                    x: img_area.x,
                    y: img_area.y,
                    cols: consumed_cols,
                    rows: consumed_rows.min(area.height),
                    rgba: resized,
                    protocol: ctx.protocol,
                    kitty_b64,
                });
            }

            (consumed_rows + 1, Some(img_area))
        }
        Block::Blank => (1, None),
    }
}

fn spans_to_line(spans: &[Span], theme: &Theme) -> Line<'static> {
    let rspans: Vec<RSpan<'static>> = spans
        .iter()
        .map(|s| match s {
            Span::Plain(t) => RSpan::styled(t.clone(), theme.body_style()),
            Span::Bold(t) => RSpan::styled(t.clone(), theme.bold_style()),
            Span::Italic(t) => RSpan::styled(t.clone(), theme.italic_style()),
            Span::Code(t) => RSpan::styled(format!("`{t}`"), theme.code_style()),
        })
        .collect();
    Line::from(rspans)
}

fn estimate_line_height(line: &Line, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let text_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
    let w = width as usize;
    text_width.div_ceil(w).max(1).min(u16::MAX as usize) as u16
}

fn estimate_height(blocks: &[Block], width: u16) -> u16 {
    blocks
        .iter()
        .map(|b| match b {
            Block::Heading { level: 1, .. } => 8, // 5-7 row font + spacing
            Block::Heading { .. } => 2,
            Block::Paragraph { spans } => {
                let text_len: usize = spans
                    .iter()
                    .map(|s| match s {
                        Span::Plain(t) | Span::Bold(t) | Span::Italic(t) | Span::Code(t) => t.len(),
                    })
                    .sum();
                let w = width.max(1) as usize;
                text_len.div_ceil(w).max(1) as u16 + 1
            }
            Block::BulletList { items } => items.len() as u16,
            Block::NumberedList { items } => items.len() as u16,
            Block::Code { code, .. } => code.lines().count() as u16 + 3,
            Block::HorizontalRule => 2,
            Block::Image { .. } => 8,
            Block::Blank => 1,
        })
        .sum()
}

fn padded(area: Rect, h_pad: u16, v_pad: u16) -> Rect {
    Rect {
        x: area.x + h_pad,
        y: area.y + v_pad,
        width: area.width.saturating_sub(h_pad * 2),
        height: area.height.saturating_sub(v_pad * 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_line_height_zero_width() {
        let line = Line::from("hello");
        assert_eq!(estimate_line_height(&line, 0), 1);
    }

    #[test]
    fn estimate_line_height_fits_in_one_row() {
        let line = Line::from("hello");
        assert_eq!(estimate_line_height(&line, 80), 1);
    }

    #[test]
    fn estimate_line_height_wraps() {
        let line = Line::from("a".repeat(200));
        assert_eq!(estimate_line_height(&line, 80), 3); // ceil(200/80) = 3
    }

    #[test]
    fn estimate_line_height_exact_fit() {
        let line = Line::from("a".repeat(80));
        assert_eq!(estimate_line_height(&line, 80), 1);
    }

    #[test]
    fn estimate_line_height_empty() {
        let line = Line::from("");
        assert_eq!(estimate_line_height(&line, 80), 1);
    }

    #[test]
    fn padded_shrinks_area() {
        let area = Rect::new(0, 0, 100, 50);
        let p = padded(area, 3, 2);
        assert_eq!(p.x, 3);
        assert_eq!(p.y, 2);
        assert_eq!(p.width, 94);
        assert_eq!(p.height, 46);
    }

    #[test]
    fn padded_handles_small_area() {
        let area = Rect::new(0, 0, 4, 2);
        let p = padded(area, 3, 2);
        assert_eq!(p.width, 0);
        assert_eq!(p.height, 0);
    }

    #[test]
    fn estimate_height_heading() {
        let blocks = vec![Block::Heading {
            level: 1,
            text: "Hi".into(),
        }];
        assert_eq!(estimate_height(&blocks, 80), 8);
    }

    #[test]
    fn estimate_height_paragraph() {
        let blocks = vec![Block::Paragraph {
            spans: vec![Span::Plain("hello world".into())],
        }];
        // ceil(11/80) = 1, +1 spacing = 2
        assert_eq!(estimate_height(&blocks, 80), 2);
    }

    #[test]
    fn estimate_height_empty() {
        let blocks: Vec<Block> = vec![];
        assert_eq!(estimate_height(&blocks, 80), 0);
    }

    #[test]
    fn spans_to_line_maps_styles() {
        let theme = crate::theme::Theme::from_name(&crate::theme::ThemeName::Hacker);
        let spans = vec![Span::Plain("hello ".into()), Span::Bold("world".into())];
        let line = spans_to_line(&spans, &theme);
        assert_eq!(line.spans.len(), 2);
    }
}
