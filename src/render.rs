use std::path::Path;

use unicode_width::UnicodeWidthStr;

use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    style::Style,
    text::{Line, Span as RSpan},
    widgets::{Block as WidgetBlock, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::bigtext::BigtextCache;
use crate::entrance::{self, EntranceKind, EntranceTracker};
use crate::highlight::Highlighter;
use crate::image_renderer::{self, DeferredImage, ImageCache, ImageProtocol};
use crate::markdown::{Block, Span};
use crate::parse::{Layout, Slide};
use crate::theme::Theme;

/// Shared per-frame rendering context bundling all state the slide renderer
/// reads or mutates: image protocol + cache, deferred-render queue, sandbox
/// base dir, syntax highlighter, big-text cache, entrance-animation tracker,
/// and the index of the slide currently being drawn (used as the
/// entrance-tracker key).
pub struct RenderCtx<'a> {
    pub protocol: ImageProtocol,
    pub image_cache: &'a mut ImageCache,
    pub deferred: &'a mut Vec<DeferredImage>,
    pub base_dir: &'a Path,
    pub highlighter: &'a mut Highlighter,
    pub bigtext: &'a mut BigtextCache,
    pub entrances: &'a mut EntranceTracker,
    pub slide_index: usize,
}

/// Draw one slide into `area`.
///
/// Dispatches on `slide.layout` (default, vertically centered, or two-column).
/// `reveal` is the number of bullets to show; pass `usize::MAX` for previews
/// or for slides without bullets.
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
                    content.height.saturating_sub(y.saturating_sub(content.y)),
                );
                let result = render_block(frame, remaining, block, theme, ctx, i);
                y = y.saturating_add(result.height);
            }

            if let Some(cols) = &slide.columns {
                let col_area = Rect::new(
                    content.x,
                    y.saturating_add(1),
                    content.width,
                    content
                        .height
                        .saturating_sub(y.saturating_sub(content.y).saturating_add(1)),
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

/// Snapshot of state the status bar needs to render. Pass to
/// [`render_status_bar`] to avoid an 8-argument call site.
pub struct StatusBar<'a> {
    pub title: &'a str,
    pub author: Option<&'a str>,
    pub slide_num: usize,
    pub total: usize,
    pub elapsed_secs: u64,
}

/// Draw the bottom status bar.
///
/// Layout: deck title (and optional author) on the left, slide counter
/// centered, elapsed `mm:ss` timer on the right. Padding between segments is
/// filled with `─` glyphs.
pub fn render_status_bar(frame: &mut Frame, area: Rect, info: &StatusBar, theme: &Theme) {
    let mins = info.elapsed_secs / 60;
    let secs = info.elapsed_secs % 60;

    let left = match info.author {
        Some(a) if !a.is_empty() => format!(" {} — {} ", info.title, a),
        _ => format!(" {} ", info.title),
    };
    let center = format!(" {}/{} ", info.slide_num, info.total);
    let right = format!(" {mins:02}:{secs:02} ");

    let width = area.width as usize;
    let used = left.width() + center.width() + right.width();
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

/// Help overlay shown when `?` is pressed.
pub fn help_overlay(frame: &mut Frame, area: Rect, theme: &Theme) {
    let help_lines = [
        "",
        "  Navigation",
        "  ─────────────────────────",
        "  →/Space/Enter/l/j  Next",
        "  ←/Backspace/h/k    Previous",
        "  g                  First slide",
        "  G                  Last slide",
        "  :N Enter           Go to slide N",
        "",
        "  Controls",
        "  ─────────────────────────",
        "  p                  Presenter mode",
        "  r                  Reset timer",
        "  ?                  Toggle help",
        "  q/Esc              Quit",
        "",
    ];

    let width = 38u16;
    let height = help_lines.len() as u16 + 2;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);

    let lines: Vec<Line> = help_lines
        .iter()
        .map(|s| Line::from(RSpan::styled(*s, theme.body_style())))
        .collect();

    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" ? Help ");

    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

/// `:N Enter` go-to overlay accepting a slide-number buffer.
pub fn goto_overlay(frame: &mut Frame, area: Rect, input: &str, theme: &Theme) {
    let width = 22u16;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height / 2;
    let rect = Rect::new(x, y, width, 3);

    frame.render_widget(Clear, rect);

    let text = format!(":{input}_");
    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .border_style(theme.status_accent())
        .title(" Go to slide ");
    let paragraph = Paragraph::new(RSpan::styled(text, theme.body_style())).block(block);
    frame.render_widget(paragraph, rect);
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
        if y >= area.y.saturating_add(area.height) {
            break;
        }

        let remaining = Rect::new(
            area.x,
            y,
            area.width,
            area.height.saturating_sub(y.saturating_sub(area.y)),
        );

        match block {
            Block::BulletList { items } => {
                for (item_i, item) in items.iter().enumerate() {
                    if bullet_count >= reveal {
                        return;
                    }
                    let h = render_bullet_item(frame, remaining, y, &item.spans, theme);
                    if y.saturating_add(h) > area.y.saturating_add(area.height) {
                        return;
                    }
                    let bullet_rect = Rect::new(remaining.x, y, remaining.width, h);

                    // Cascade entrance: fixed 300ms animation per bullet, staggered start.
                    // sub_idx = item_i + 1 keeps each item disjoint from the block's own
                    // sub_idx = 0 entrance slot.
                    let stagger = std::time::Duration::from_millis(80 * item_i as u64);
                    let anim_dur = std::time::Duration::from_millis(300);
                    if let Some(state) = ctx.entrances.get_or_start(
                        ctx.slide_index,
                        block_idx,
                        item_i + 1,
                        EntranceKind::Cascade,
                        stagger + anim_dur,
                    ) {
                        let raw_progress = state.progress();
                        let stagger_frac =
                            stagger.as_secs_f64() / (state.duration.as_secs_f64().max(0.001));
                        let visual_progress =
                            ((raw_progress - stagger_frac) / (1.0 - stagger_frac)).clamp(0.0, 1.0);
                        entrance::apply_fade_in(frame, bullet_rect, visual_progress, theme);
                    }

                    y = y.saturating_add(h);
                    bullet_count += 1;
                }
            }
            _ => {
                let result = render_block(frame, remaining, block, theme, ctx, block_idx);

                // Apply entrance effects based on block type
                if let Some(rect) = result.entrance_target {
                    apply_entrance_for_block(frame, block, rect, theme, ctx, block_idx);
                }

                y = y.saturating_add(result.height);
            }
        }
    }
}

/// Render one bullet at `(area.x, y)` and return the height it consumed.
///
/// Used by both `render_blocks` (the cascade-entrance path) and the static
/// `BulletList` arm of `render_block`. Borrows the bullet prefix from the
/// theme to avoid `format!`-allocating per bullet per frame.
fn render_bullet_item<'a>(
    frame: &mut Frame,
    area: Rect,
    y: u16,
    spans: &'a [Span],
    theme: &'a Theme,
) -> u16 {
    let line = spans_to_line(spans, theme);
    let mut spans_vec: Vec<RSpan<'a>> = Vec::with_capacity(line.spans.len() + 1);
    spans_vec.push(RSpan::styled(
        theme.bullet_prefix.as_str(),
        theme.bullet_style(),
    ));
    spans_vec.extend(line.spans);
    let combined = Line::from(spans_vec);
    let h = estimate_line_height(&combined, area.width);
    frame.render_widget(
        Paragraph::new(combined).wrap(Wrap { trim: false }),
        Rect::new(area.x, y, area.width, h),
    );
    h
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
        .get_or_start(ctx.slide_index, block_idx, 0, kind, duration)
    {
        let progress = state.progress();
        match &state.kind {
            EntranceKind::Decrypt => entrance::apply_decrypt(frame, rect, progress, theme),
            EntranceKind::SlideIn => entrance::apply_slide_in(frame, rect, progress, theme),
            EntranceKind::FadeIn => entrance::apply_fade_in(frame, rect, progress, theme),
            // Cascade and Typewriter are dispatched by their owning render paths
            // (bullet-list and code-block). Keep the match exhaustive so adding a
            // new EntranceKind is a compile error rather than a silent no-op.
            EntranceKind::Cascade | EntranceKind::Typewriter => {}
        }
    }
}

/// Result of rendering one block: how much vertical space was consumed and,
/// if applicable, the rect to feed [`apply_entrance_for_block`] afterwards.
struct BlockRender {
    height: u16,
    /// Rect for the post-render entrance pass. `None` means the block applied
    /// its own entrance during render (typewriter for code, cascade for
    /// bullets) — do not double-apply.
    entrance_target: Option<Rect>,
}

impl BlockRender {
    fn self_entrance(height: u16) -> Self {
        Self {
            height,
            entrance_target: None,
        }
    }

    fn with_entrance(height: u16, rect: Rect) -> Self {
        Self {
            height,
            entrance_target: Some(rect),
        }
    }
}

/// Dispatches to a per-arm helper.
fn render_block(
    frame: &mut Frame,
    area: Rect,
    block: &Block,
    theme: &Theme,
    ctx: &mut RenderCtx,
    block_idx: usize,
) -> BlockRender {
    match block {
        Block::Heading { level: 1, text } => render_h1(frame, area, text, theme, ctx),
        Block::Heading { text, .. } => render_heading(frame, area, text, theme),
        Block::Paragraph { spans } => render_paragraph(frame, area, spans, theme),
        Block::BulletList { items } => render_bullet_list_static(frame, area, items, theme),
        Block::NumberedList { items } => render_numbered_list(frame, area, items, theme),
        Block::Code { lang, code } => {
            render_code(frame, area, lang.as_deref(), code, theme, ctx, block_idx)
        }
        Block::HorizontalRule => render_horizontal_rule(frame, area, theme),
        Block::Image { path, alt } => render_image(frame, area, path, alt, theme, ctx),
    }
}

fn render_h1(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    theme: &Theme,
    ctx: &mut RenderCtx,
) -> BlockRender {
    let big = ctx.bigtext.render(text, theme.font);
    let height = big.len() as u16;
    let rect = Rect::new(area.x, area.y, area.width, height.min(area.height));
    let max_rows = area.height.min(big.len() as u16);
    let lines: Vec<Line<'_>> = big
        .iter()
        .take(max_rows as usize)
        .map(|s| Line::from(RSpan::styled(s.as_str(), theme.h1_style())))
        .collect();
    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(area.x, area.y, area.width, max_rows),
    );
    BlockRender::with_entrance(height.saturating_add(1), rect)
}

fn render_heading(frame: &mut Frame, area: Rect, text: &str, theme: &Theme) -> BlockRender {
    let rect = Rect::new(area.x, area.y, area.width, 1);
    let line = Line::from(RSpan::styled(text, theme.heading_style()));
    frame.render_widget(Paragraph::new(line), rect);
    BlockRender::with_entrance(2, rect)
}

fn render_paragraph<'a>(
    frame: &mut Frame,
    area: Rect,
    spans: &'a [Span],
    theme: &'a Theme,
) -> BlockRender {
    let line = spans_to_line(spans, theme);
    let h = estimate_line_height(&line, area.width);
    let rect = Rect::new(area.x, area.y, area.width, h);
    frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), rect);
    BlockRender::with_entrance(h.saturating_add(1), rect)
}

fn render_bullet_list_static<'a>(
    frame: &mut Frame,
    area: Rect,
    items: &'a [crate::markdown::ListItem],
    theme: &'a Theme,
) -> BlockRender {
    let mut h = 0u16;
    for item in items {
        let lh = render_bullet_item(frame, area, area.y.saturating_add(h), &item.spans, theme);
        h = h.saturating_add(lh);
    }
    BlockRender::self_entrance(h) // bullets handle their own entrances
}

fn render_numbered_list<'a>(
    frame: &mut Frame,
    area: Rect,
    items: &'a [crate::markdown::ListItem],
    theme: &'a Theme,
) -> BlockRender {
    let mut h = 0u16;
    for (i, item) in items.iter().enumerate() {
        let line = spans_to_line(&item.spans, theme);
        let prefix = format!("  {}. ", i + 1);
        let mut spans_vec: Vec<RSpan<'_>> = Vec::with_capacity(line.spans.len() + 1);
        spans_vec.push(RSpan::styled(prefix, theme.bullet_style()));
        spans_vec.extend(line.spans);
        let combined = Line::from(spans_vec);
        let lh = estimate_line_height(&combined, area.width);
        frame.render_widget(
            Paragraph::new(combined).wrap(Wrap { trim: false }),
            Rect::new(area.x, area.y.saturating_add(h), area.width, lh),
        );
        h = h.saturating_add(lh);
    }
    BlockRender::self_entrance(h)
}

fn render_code(
    frame: &mut Frame,
    area: Rect,
    lang: Option<&str>,
    code: &str,
    theme: &Theme,
    ctx: &mut RenderCtx,
    block_idx: usize,
) -> BlockRender {
    let title = lang.unwrap_or("");
    let highlighted = ctx
        .highlighter
        .highlight(code, if title.is_empty() { "txt" } else { title });
    let total_lines = highlighted.len();
    let inner_height = total_lines.max(1) as u16;
    let total_height = inner_height + 2; // borders

    let typewriter_dur = std::time::Duration::from_millis(
        (total_lines as u64).saturating_mul(50).saturating_add(200),
    );
    let entrance = ctx.entrances.get_or_start(
        ctx.slide_index,
        block_idx,
        0,
        EntranceKind::Typewriter,
        typewriter_dur,
    );

    // Build visible lines for the code block. After the typewriter is done,
    // reuse the cached `Arc<Vec<Line>>` directly instead of rebuilding the vec
    // frame after frame.
    let lines: Vec<Line<'static>> = match entrance {
        None => (*highlighted).clone(),
        Some(state) => {
            let (visible_lines, char_frac) =
                entrance::typewriter_visible(state.progress(), total_lines);
            let cursor_style = theme.status_accent();
            let mut out: Vec<Line<'static>> = Vec::with_capacity(inner_height as usize);
            for (i, hl) in highlighted.iter().enumerate() {
                if i < visible_lines {
                    out.push(hl.clone());
                } else if i == visible_lines && visible_lines < total_lines {
                    let total_chars: usize =
                        hl.spans.iter().map(|s| s.content.chars().count()).sum();
                    let chars_to_show = (char_frac * total_chars as f64) as usize;
                    out.push(typewriter_partial_line(
                        &hl.spans,
                        chars_to_show,
                        cursor_style,
                    ));
                } else {
                    out.push(Line::from(""));
                }
            }
            out
        }
    };

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
    BlockRender::self_entrance(total_height.saturating_add(1)) // code handles its own entrance via typewriter
}

fn render_horizontal_rule(frame: &mut Frame, area: Rect, theme: &Theme) -> BlockRender {
    let style = theme.rule_style();
    let buf = frame.buffer_mut();
    for x in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x.saturating_add(x), area.y)) {
            cell.set_char('─');
            cell.set_style(style);
        }
    }
    BlockRender::self_entrance(2)
}

fn render_image(
    frame: &mut Frame,
    area: Rect,
    path: &str,
    alt: &str,
    theme: &Theme,
    ctx: &mut RenderCtx,
) -> BlockRender {
    let resized = match ctx
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
            return BlockRender::self_entrance(2);
        }
    };

    let (px_w, px_h) = resized.dimensions();
    let consumed_rows = u16::try_from(px_h.div_ceil(2)).unwrap_or(u16::MAX);
    let consumed_cols = u16::try_from(px_w).unwrap_or(u16::MAX);

    let x_offset = area.width.saturating_sub(consumed_cols) / 2;
    let img_area = Rect::new(
        area.x.saturating_add(x_offset),
        area.y,
        consumed_cols,
        consumed_rows.min(area.height),
    );

    let buf = frame.buffer_mut();
    image_renderer::render_halfblocks(buf, img_area, &resized);

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

    BlockRender::with_entrance(consumed_rows.saturating_add(1), img_area)
}

fn spans_to_line<'a>(spans: &'a [Span], theme: &Theme) -> Line<'a> {
    let mut rspans: Vec<RSpan<'a>> = Vec::with_capacity(spans.len());
    for s in spans {
        let span = match s {
            Span::Plain(t) => RSpan::styled(t.as_str(), theme.body_style()),
            Span::Bold(t) => RSpan::styled(t.as_str(), theme.bold_style()),
            Span::Italic(t) => RSpan::styled(t.as_str(), theme.italic_style()),
            // `Code` still allocates for the `\`...\`` wrapping; pre-baking the
            // backticks at parse time would remove this last per-frame alloc.
            Span::Code(t) => RSpan::styled(format!("`{t}`"), theme.code_style()),
        };
        rspans.push(span);
    }
    Line::from(rspans)
}

/// Build the partially-typed line for the code-block typewriter entrance.
///
/// Truncates `spans` to the first `chars_to_show` characters (preserving
/// per-span styling), then appends a blinking cursor span. Slicing is
/// byte-accurate via `char_indices` so multi-byte UTF-8 spans don't panic at a
/// boundary.
fn typewriter_partial_line(
    spans: &[RSpan<'static>],
    chars_to_show: usize,
    cursor_style: Style,
) -> Line<'static> {
    if chars_to_show == 0 {
        return Line::from(RSpan::styled("▌", cursor_style));
    }
    let mut partial: Vec<RSpan<'static>> = Vec::with_capacity(spans.len() + 1);
    let mut chars_left = chars_to_show;
    for span in spans {
        let span_chars = span.content.chars().count();
        if chars_left >= span_chars {
            partial.push(span.clone());
            chars_left -= span_chars;
            if chars_left == 0 {
                break;
            }
        } else {
            let byte_idx = span
                .content
                .char_indices()
                .nth(chars_left)
                .map(|(i, _)| i)
                .unwrap_or(span.content.len());
            partial.push(RSpan::styled(
                span.content[..byte_idx].to_string(),
                span.style,
            ));
            break;
        }
    }
    partial.push(RSpan::styled("▌", cursor_style));
    Line::from(partial)
}

fn estimate_line_height(line: &Line, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    // ASCII fast path: byte-length equals display width, skip the East-Asian-Width walk.
    let ascii_only = line.spans.iter().all(|s| s.content.is_ascii());
    let text_width: usize = if ascii_only {
        line.spans.iter().map(|s| s.content.len()).sum()
    } else {
        line.spans.iter().map(|s| s.content.width()).sum()
    };
    let w = width as usize;
    text_width.div_ceil(w).max(1).min(u16::MAX as usize) as u16
}

fn estimate_height(blocks: &[Block], width: u16) -> u16 {
    blocks
        .iter()
        .map(|b| match b {
            Block::Heading { level: 1, .. } => 8, // max(Block=6, Large=8) for centering
            Block::Heading { .. } => 2,
            Block::Paragraph { spans } => {
                let text_len: usize = spans
                    .iter()
                    .map(|s| match s {
                        Span::Plain(t) | Span::Bold(t) | Span::Italic(t) | Span::Code(t) => {
                            t.width()
                        }
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

    #[test]
    fn estimate_height_bullet_list() {
        let blocks = vec![Block::BulletList {
            items: vec![
                crate::markdown::ListItem {
                    spans: vec![Span::Plain("a".into())],
                },
                crate::markdown::ListItem {
                    spans: vec![Span::Plain("b".into())],
                },
                crate::markdown::ListItem {
                    spans: vec![Span::Plain("c".into())],
                },
            ],
        }];
        assert_eq!(estimate_height(&blocks, 80), 3);
    }

    #[test]
    fn estimate_height_code_block() {
        let blocks = vec![Block::Code {
            lang: Some("rust".into()),
            code: "fn a() {}\nfn b() {}\nfn c() {}".into(),
        }];
        // 3 lines + 3 (borders + spacing) = 6
        assert_eq!(estimate_height(&blocks, 80), 6);
    }

    #[test]
    fn estimate_height_mixed_blocks() {
        let blocks = vec![
            Block::Heading {
                level: 1,
                text: "Title".into(),
            },
            Block::Paragraph {
                spans: vec![Span::Plain("hello".into())],
            },
            Block::HorizontalRule,
        ];
        // H1=8, paragraph=2 (ceil(5/80)+1), rule=2 = 12
        assert_eq!(estimate_height(&blocks, 80), 12);
    }
}
