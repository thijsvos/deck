use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Paragraph, Wrap},
    Frame,
};

use crate::bigtext;
use crate::image_renderer::{self, DeferredImage, ImageCache, ImageProtocol};
use crate::markdown::{Block, Span};
use crate::parse::{Layout, Slide};
use crate::theme::Theme;

pub fn render_slide(
    frame: &mut Frame,
    area: Rect,
    slide: &Slide,
    theme: &Theme,
    reveal: usize,
    protocol: ImageProtocol,
    image_cache: &mut ImageCache,
    deferred: &mut Vec<DeferredImage>,
    base_dir: &Path,
) {
    // Fill background
    let bg = WidgetBlock::default().style(theme.body_style());
    frame.render_widget(bg, area);

    let content = padded(area, 3, 2);

    match slide.layout {
        Layout::Default => {
            render_blocks(frame, content, &slide.blocks, theme, reveal, protocol, image_cache, deferred, base_dir);
        }
        Layout::Center => {
            let total_height = estimate_height(&slide.blocks, content.width);
            let y_off = content.height.saturating_sub(total_height) / 2;
            let centered = Rect {
                y: content.y + y_off,
                height: total_height.min(content.height),
                ..content
            };
            render_blocks(frame, centered, &slide.blocks, theme, reveal, protocol, image_cache, deferred, base_dir);
        }
        Layout::Columns => {
            let mut y = content.y;
            for block in &slide.blocks {
                let remaining = Rect::new(
                    content.x,
                    y,
                    content.width,
                    content.height.saturating_sub(y - content.y),
                );
                let h = render_block(frame, remaining, block, theme, protocol, image_cache, deferred, base_dir);
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

                render_blocks(frame, halves[0], &cols.left, theme, usize::MAX, protocol, image_cache, deferred, base_dir);
                render_blocks(frame, halves[1], &cols.right, theme, usize::MAX, protocol, image_cache, deferred, base_dir);
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

    let left = format!(" {} ", title);
    let center = format!(" {}/{} ", slide_num, total);
    let right = format!(" {:02}:{:02} ", mins, secs);

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
    protocol: ImageProtocol,
    image_cache: &mut ImageCache,
    deferred: &mut Vec<DeferredImage>,
    base_dir: &Path,
) {
    let mut y = area.y;
    let mut bullet_count: usize = 0;

    for block in blocks {
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
                for item in items {
                    if bullet_count >= reveal {
                        return;
                    }
                    let line = spans_to_line(&item.spans, theme);
                    let mut spans_vec =
                        vec![RSpan::styled(format!("  {} ", theme.bullet), theme.bullet_style())];
                    spans_vec.extend(line.spans);
                    let combined = Line::from(spans_vec);
                    let h = estimate_line_height(&combined, remaining.width);
                    if y + h > area.y + area.height {
                        return;
                    }
                    frame.render_widget(
                        Paragraph::new(combined).wrap(Wrap { trim: false }),
                        Rect::new(remaining.x, y, remaining.width, h),
                    );
                    y += h;
                    bullet_count += 1;
                }
            }
            _ => {
                let h = render_block(frame, remaining, block, theme, protocol, image_cache, deferred, base_dir);
                y += h;
            }
        }
    }
}

fn render_block(
    frame: &mut Frame,
    area: Rect,
    block: &Block,
    theme: &Theme,
    protocol: ImageProtocol,
    image_cache: &mut ImageCache,
    deferred: &mut Vec<DeferredImage>,
    base_dir: &Path,
) -> u16 {
    match block {
        Block::Heading { level: 1, text } => {
            let big = bigtext::render(text, theme.font);
            let height = big.len() as u16;
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
            height + 1
        }
        Block::Heading { text, .. } => {
            let line = Line::from(vec![RSpan::styled(text.clone(), theme.heading_style())]);
            frame.render_widget(Paragraph::new(line), Rect::new(area.x, area.y, area.width, 1));
            2
        }
        Block::Paragraph { spans } => {
            let line = spans_to_line(spans, theme);
            let h = estimate_line_height(&line, area.width);
            frame.render_widget(
                Paragraph::new(line).wrap(Wrap { trim: false }),
                Rect::new(area.x, area.y, area.width, h),
            );
            h + 1
        }
        Block::BulletList { items } => {
            let mut h = 0u16;
            for item in items {
                let line = spans_to_line(&item.spans, theme);
                let mut spans_vec =
                    vec![RSpan::styled(format!("  {} ", theme.bullet), theme.bullet_style())];
                spans_vec.extend(line.spans);
                let combined = Line::from(spans_vec);
                let lh = estimate_line_height(&combined, area.width);
                frame.render_widget(
                    Paragraph::new(combined).wrap(Wrap { trim: false }),
                    Rect::new(area.x, area.y + h, area.width, lh),
                );
                h += lh;
            }
            h
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
            h
        }
        Block::CodeBlock { lang, code } => {
            let title = lang.as_deref().unwrap_or("");
            let inner_height = code.lines().count() as u16;
            let total_height = inner_height + 2;
            let block_widget = WidgetBlock::default()
                .borders(Borders::ALL)
                .border_style(theme.code_border())
                .title(title);
            let paragraph = Paragraph::new(code.as_str())
                .style(theme.code_style())
                .block(block_widget);
            frame.render_widget(
                paragraph,
                Rect::new(area.x, area.y, area.width, total_height.min(area.height)),
            );
            total_height + 1
        }
        Block::HorizontalRule => {
            let rule = "─".repeat(area.width as usize);
            frame.render_widget(
                Paragraph::new(RSpan::styled(rule, theme.rule_style())),
                Rect::new(area.x, area.y, area.width, 1),
            );
            2
        }
        Block::Image { path, alt } => {
            // Load and resize image
            let img = match image_cache.load(path, base_dir) {
                Some(img) => img.clone(),
                None => {
                    // Fallback: show alt text
                    let label = if alt.is_empty() {
                        format!("[Image: {}]", path)
                    } else {
                        format!("[Image: {}]", alt)
                    };
                    let line = Line::from(RSpan::styled(label, theme.rule_style()));
                    frame.render_widget(Paragraph::new(line), Rect::new(area.x, area.y, area.width, 1));
                    return 2;
                }
            };

            let resized = image_renderer::resize_to_fit(&img, area.width, area.height);
            let (px_w, px_h) = resized.dimensions();
            let consumed_rows = ((px_h + 1) / 2) as u16;
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
            if matches!(protocol, ImageProtocol::Kitty | ImageProtocol::Sixel) {
                deferred.push(DeferredImage {
                    x: img_area.x,
                    y: img_area.y,
                    cols: consumed_cols,
                    rows: consumed_rows.min(area.height),
                    rgba: resized,
                    protocol,
                });
            }

            consumed_rows + 1
        }
        Block::Blank => 1,
    }
}

fn spans_to_line(spans: &[Span], theme: &Theme) -> Line<'static> {
    let rspans: Vec<RSpan<'static>> = spans
        .iter()
        .map(|s| match s {
            Span::Plain(t) => RSpan::styled(t.clone(), theme.body_style()),
            Span::Bold(t) => RSpan::styled(t.clone(), theme.bold_style()),
            Span::Italic(t) => RSpan::styled(t.clone(), theme.italic_style()),
            Span::Code(t) => RSpan::styled(format!("`{}`", t), theme.code_style()),
        })
        .collect();
    Line::from(rspans)
}

fn estimate_line_height(line: &Line, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let text_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
    ((text_width as u16).saturating_add(width - 1) / width).max(1)
}

fn estimate_height(blocks: &[Block], width: u16) -> u16 {
    blocks
        .iter()
        .map(|b| match b {
            Block::Heading { level: 1, .. } => 6,
            Block::Heading { .. } => 2,
            Block::Paragraph { spans } => {
                let text_len: usize = spans
                    .iter()
                    .map(|s| match s {
                        Span::Plain(t) | Span::Bold(t) | Span::Italic(t) | Span::Code(t) => {
                            t.len()
                        }
                    })
                    .sum();
                let w = width.max(1) as usize;
                ((text_len + w - 1) / w).max(1) as u16 + 1
            }
            Block::BulletList { items } => items.len() as u16,
            Block::NumberedList { items } => items.len() as u16,
            Block::CodeBlock { code, .. } => code.lines().count() as u16 + 3,
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
