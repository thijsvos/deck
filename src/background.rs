use ratatui::{layout::Rect, style::Style, Frame};
use serde::Deserialize;

use crate::theme::Theme;

/// Animated mathematical backgrounds for title slides.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundKind {
    Matrix,
    Plasma,
    Lissajous,
    Spiral,
    Wave,
    Aurora,
    Rain,
    Noise,
    Lattice,
    Orbit,
}

/// Apply animated background into empty (space) cells of the buffer.
/// Call this AFTER the slide content has been rendered so the background
/// only fills the gaps around text.
pub fn apply_background(
    frame: &mut Frame,
    area: Rect,
    kind: &BackgroundKind,
    time: f64,
    theme: &Theme,
) {
    match kind {
        // Scatter-based: pre-compute points, mark cells – O(points)
        BackgroundKind::Lissajous => apply_lissajous(frame, area, time, theme),
        BackgroundKind::Orbit => apply_orbit(frame, area, time, theme),
        // Per-cell: O(1) per cell
        _ => apply_percell(frame, area, kind, time, theme),
    }
}

fn apply_percell(
    frame: &mut Frame,
    area: Rect,
    kind: &BackgroundKind,
    time: f64,
    theme: &Theme,
) {
    let buf = frame.buffer_mut();

    for y in 0..area.height {
        for x in 0..area.width {
            let pos = (area.x + x, area.y + y);

            if !is_empty(buf, pos) {
                continue;
            }

            let (ch, brightness) = compute_cell(kind, x, y, area.width, area.height, time);
            write_bg_cell(buf, pos, ch, brightness, theme);
        }
    }
}

// ── helpers ────────────────────────────────────────────────────────

fn is_empty(buf: &ratatui::buffer::Buffer, pos: (u16, u16)) -> bool {
    buf.cell(pos)
        .map(|c| {
            let s = c.symbol();
            s == " " || s.is_empty()
        })
        .unwrap_or(false)
}

fn write_bg_cell(
    buf: &mut ratatui::buffer::Buffer,
    pos: (u16, u16),
    ch: char,
    brightness: f64,
    theme: &Theme,
) {
    if brightness < 0.02 {
        if let Some(cell) = buf.cell_mut(pos) {
            cell.set_char(' ');
            cell.set_style(Style::default().bg(theme.bg));
        }
    } else {
        let fg = shade_color(theme, brightness);
        if let Some(cell) = buf.cell_mut(pos) {
            cell.set_char(ch);
            cell.set_style(Style::default().fg(fg).bg(theme.bg));
        }
    }
}

fn shade_color(theme: &Theme, brightness: f64) -> ratatui::style::Color {
    use ratatui::style::Color;
    let b = brightness.clamp(0.0, 1.0);
    match theme.fg {
        Color::Rgb(r, g, bl) => {
            let factor = 0.35 * b;
            Color::Rgb(
                (r as f64 * factor) as u8,
                (g as f64 * factor) as u8,
                (bl as f64 * factor) as u8,
            )
        }
        _ => {
            let v = (b * 50.0) as u8;
            Color::Rgb(v, v, v)
        }
    }
}

// ── cell dispatcher (for O(1)-per-cell backgrounds) ────────────────

fn compute_cell(
    kind: &BackgroundKind,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    t: f64,
) -> (char, f64) {
    match kind {
        BackgroundKind::Matrix => matrix_cell(x, y, w, h, t),
        BackgroundKind::Plasma => plasma_cell(x, y, w, h, t),
        BackgroundKind::Spiral => spiral_cell(x, y, w, h, t),
        BackgroundKind::Wave => wave_cell(x, y, w, h, t),
        BackgroundKind::Aurora => aurora_cell(x, y, w, h, t),
        BackgroundKind::Rain => rain_cell(x, y, w, h, t),
        BackgroundKind::Noise => noise_cell(x, y, w, h, t),
        BackgroundKind::Lattice => lattice_cell(x, y, w, h, t),
        BackgroundKind::Lissajous | BackgroundKind::Orbit => (' ', 0.0), // scatter
    }
}

// ── simple hash (SplitMix64-ish) ───────────────────────────────────

fn hash2(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(6364136223846793005).wrapping_add(b);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h
}

// ════════════════════════════════════════════════════════════════════
// 1. MATRIX RAIN  – falling columns of characters with fading trails
// ════════════════════════════════════════════════════════════════════

const MATRIX_CHARS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', '@', '#',
    '$', '%', '&', '*', '+', '-', '=', '~', ':', ';', '<', '>', '{', '}', '[', ']', '|', '/',
    '\\',
];

fn matrix_cell(x: u16, y: u16, _w: u16, h: u16, t: f64) -> (char, f64) {
    let height = h as f64;
    let row = y as f64;
    let mut brightness = 0.0f64;

    // 3 drops per column at different speeds
    for drop in 0..3u64 {
        let speed = 1.2 + (hash2(x as u64, drop) % 25) as f64 / 10.0;
        let offset = (hash2(x as u64, drop + 50) % (h as u64 + 20)) as f64;
        let trail = 5.0 + (hash2(x as u64, drop + 200) % 10) as f64;
        let head = (t * speed + offset) % (height + trail + 8.0);
        let dist = head - row;

        if dist >= 0.0 && dist < trail {
            let b = 1.0 - (dist / trail);
            brightness = brightness.max(b);
        }
    }

    let ch = MATRIX_CHARS[hash2(x as u64, y as u64) as usize % MATRIX_CHARS.len()];
    (ch, brightness)
}

// ════════════════════════════════════════════════════════════════════
// 2. PLASMA  – classic demoscene sine interference pattern
// ════════════════════════════════════════════════════════════════════

const DENSITY: &[char] = &[' ', '.', '·', ':', ';', '░', '▒', '▓', '█'];

fn plasma_cell(x: u16, y: u16, _w: u16, _h: u16, t: f64) -> (char, f64) {
    let fx = x as f64 * 0.07;
    let fy = y as f64 * 0.14; // aspect ratio compensation (chars are ~2:1)
    let t = t * 0.25; // slow

    let v1 = (fx + t).sin();
    let v2 = (fy + t * 0.7).sin();
    let v3 = ((fx + fy) * 0.5 + t * 0.5).sin();
    let v4 = ((fx * fx + fy * fy).sqrt() * 0.25 - t * 0.4).sin();

    let value = (v1 + v2 + v3 + v4 + 4.0) / 8.0; // 0..1

    let idx = (value * (DENSITY.len() - 1) as f64) as usize;
    let ch = DENSITY[idx.min(DENSITY.len() - 1)];
    (ch, value)
}

// ════════════════════════════════════════════════════════════════════
// 3. LISSAJOUS  – parametric curve tracing with a fading trail
//    Uses scatter approach: compute curve points, mark cells.  O(points)
// ════════════════════════════════════════════════════════════════════

fn apply_lissajous(frame: &mut Frame, area: Rect, time: f64, theme: &Theme) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    // Pre-compute brightness grid by scattering curve points
    let mut grid = vec![0.0f64; w * h];
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;

    // Slowly morphing Lissajous parameters
    let a = 3.0 + (time * 0.08).sin() * 0.5;
    let b = 2.0 + (time * 0.06).cos() * 0.5;
    let delta = time * 0.12;

    let num_points = 800;

    for i in 0..num_points {
        let tp = time * 0.4 - i as f64 * 0.01;
        let px = cx + (cx - 4.0) * (a * tp + delta).sin();
        let py = cy + (cy - 2.0) * (b * tp).sin();

        let ix = px.round() as isize;
        let iy = py.round() as isize;

        let age = i as f64 / num_points as f64;
        let brightness = (1.0 - age) * 0.95;

        // Mark the cell and its immediate horizontal neighbour for thickness
        for dx in 0..=1isize {
            let nx = ix + dx;
            let ny = iy;
            if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                let idx = ny as usize * w + nx as usize;
                if brightness > grid[idx] {
                    grid[idx] = brightness;
                }
            }
        }
    }

    // Apply grid to empty cells
    let buf = frame.buffer_mut();
    for y in 0..area.height {
        for x in 0..area.width {
            let pos = (area.x + x, area.y + y);
            if !is_empty(buf, pos) {
                continue;
            }

            let brightness = grid[y as usize * w + x as usize];
            let ch = if brightness > 0.75 {
                '█'
            } else if brightness > 0.5 {
                '▓'
            } else if brightness > 0.3 {
                '▒'
            } else if brightness > 0.12 {
                '░'
            } else if brightness > 0.04 {
                '·'
            } else {
                ' '
            };
            write_bg_cell(buf, pos, ch, brightness, theme);
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// 4. SPIRAL  – rotating multi-arm spiral using polar coordinates
// ════════════════════════════════════════════════════════════════════

fn spiral_cell(x: u16, y: u16, w: u16, h: u16, t: f64) -> (char, f64) {
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let dx = x as f64 - cx;
    let dy = (y as f64 - cy) * 2.0; // aspect ratio
    let dist = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);

    let arms = 3.0;
    let rotation = t * 0.15; // very slow rotation

    let spiral_val = (angle * arms + dist * 0.12 - rotation * 2.0).sin();
    let fade = 1.0 / (1.0 + dist * 0.04);
    let value = ((spiral_val + 1.0) / 2.0 * fade).clamp(0.0, 1.0);

    let idx = (value * (DENSITY.len() - 1) as f64) as usize;
    let ch = DENSITY[idx.min(DENSITY.len() - 1)];
    (ch, value)
}

// ════════════════════════════════════════════════════════════════════
// 5. WAVE  – sine wave interference / ripple from centre
// ════════════════════════════════════════════════════════════════════

fn wave_cell(x: u16, y: u16, w: u16, h: u16, t: f64) -> (char, f64) {
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let dx = x as f64 - cx;
    let dy = (y as f64 - cy) * 2.0;
    let dist = (dx * dx + dy * dy).sqrt();
    let t = t * 0.35;

    // Two concentric ripples at different frequencies
    let v1 = (dist * 0.18 - t * 2.0).sin();
    let v2 = (dist * 0.12 - t * 1.4 + 1.5).sin();
    // Horizontal wave overlay
    let v3 = (x as f64 * 0.06 + t * 0.8).sin() * 0.4;

    let value = ((v1 + v2 + v3 + 3.0) / 6.0).clamp(0.0, 1.0);

    let idx = (value * (DENSITY.len() - 1) as f64) as usize;
    let ch = DENSITY[idx.min(DENSITY.len() - 1)];
    (ch, value)
}

// ════════════════════════════════════════════════════════════════════
// 6. AURORA  – flowing horizontal light bands like the northern lights
//    ★ Best suited for title pages with centred text ★
// ════════════════════════════════════════════════════════════════════

fn aurora_cell(x: u16, y: u16, w: u16, h: u16, t: f64) -> (char, f64) {
    let fx = x as f64;
    let fy = y as f64;
    let _fw = w as f64;
    let fh = h as f64;
    let t = t * 0.18;

    // Three curtain-like horizontal bands that sway over time
    let sway1 = (fx * 0.015 + t).sin() * 4.0;
    let sway2 = (fx * 0.022 - t * 0.6).sin() * 3.0;
    let sway3 = (fx * 0.01 + t * 0.4).cos() * 2.5;

    let band1 = (fy * 0.2 + sway1 + t * 0.3).sin();
    let band2 = (fy * 0.13 + sway2 + t * 0.5).sin();
    let band3 = (fy * 0.09 + sway3).sin() * 0.6;

    let base = (band1 + band2 + band3 + 3.0) / 6.0;

    // Gentle vertical bell curve – brightest in upper third, still visible elsewhere
    let y_norm = fy / fh;
    let bell = (-((y_norm - 0.3) * 2.5).powi(2)).exp() * 0.6 + 0.4;

    // Horizontal shimmer
    let shimmer = (fx * 0.05 + t * 1.2).sin() * 0.12 + 1.0;

    let value = (base * bell * shimmer).clamp(0.0, 1.0);

    let idx = (value * (DENSITY.len() - 1) as f64) as usize;
    let ch = DENSITY[idx.min(DENSITY.len() - 1)];
    (ch, value)
}

// ════════════════════════════════════════════════════════════════════
// 7. RAIN  – gentle vertical drops with short fading trails
// ════════════════════════════════════════════════════════════════════

fn rain_cell(x: u16, y: u16, _w: u16, h: u16, t: f64) -> (char, f64) {
    let height = h as f64;
    let row = y as f64;
    let mut brightness = 0.0f64;

    // 2 drops per column, staggered
    for drop in 0..2u64 {
        let speed = 0.8 + (hash2(x as u64, drop + 300) % 18) as f64 / 10.0;
        let offset = (hash2(x as u64, drop + 400) % (h as u64 + 10)) as f64;
        let trail = 2.0 + (hash2(x as u64, drop + 500) % 4) as f64;
        let head = (t * speed + offset) % (height + trail + 6.0);
        let dist = head - row;

        if dist >= 0.0 && dist < trail {
            let b = 1.0 - dist / trail;
            brightness = brightness.max(b);
        }
    }

    let ch = if brightness > 0.7 {
        '│'
    } else if brightness > 0.3 {
        ':'
    } else {
        '·'
    };
    (ch, brightness * 0.6)
}

// ════════════════════════════════════════════════════════════════════
// 8. NOISE  – slowly evolving value-noise cloudscape
// ════════════════════════════════════════════════════════════════════

fn noise_cell(x: u16, y: u16, _w: u16, _h: u16, t: f64) -> (char, f64) {
    let t = t * 0.12;

    // Two octaves of value noise at different scales, drifting over time
    let v1 = value_noise(x as f64 * 0.09 + t, y as f64 * 0.18 + t * 0.3);
    let v2 = value_noise(x as f64 * 0.17 + t * 0.5, y as f64 * 0.34 - t * 0.2) * 0.5;
    let value = ((v1 + v2) / 1.5).clamp(0.0, 1.0);

    let idx = (value * (DENSITY.len() - 1) as f64) as usize;
    let ch = DENSITY[idx.min(DENSITY.len() - 1)];
    (ch, value)
}

/// Simple 2D value noise with smoothstep interpolation.
fn value_noise(x: f64, y: f64) -> f64 {
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();

    // Smoothstep
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);

    let v00 = hash_f64(ix, iy);
    let v10 = hash_f64(ix + 1, iy);
    let v01 = hash_f64(ix, iy + 1);
    let v11 = hash_f64(ix + 1, iy + 1);

    let v0 = v00 + sx * (v10 - v00);
    let v1 = v01 + sx * (v11 - v01);
    v0 + sy * (v1 - v0)
}

fn hash_f64(x: i64, y: i64) -> f64 {
    (hash2(x as u64, y as u64) % 10000) as f64 / 10000.0
}

// ════════════════════════════════════════════════════════════════════
// 9. LATTICE  – slowly rotating & morphing grid intersection pattern
// ════════════════════════════════════════════════════════════════════

fn lattice_cell(x: u16, y: u16, w: u16, h: u16, t: f64) -> (char, f64) {
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let t = t * 0.1;

    // Translate to centre, apply aspect ratio
    let dx = x as f64 - cx;
    let dy = (y as f64 - cy) * 2.0;

    // Slowly rotate the coordinate space
    let angle = t * 0.25;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let rx = dx * cos_a - dy * sin_a;
    let ry = dx * sin_a + dy * cos_a;

    // Morphing grid scale
    let scale = 6.0 + (t * 0.4).sin() * 2.0;
    let gx = (rx / scale).sin().abs();
    let gy = (ry / scale).sin().abs();

    // Intersection glow: bright where both sin values are near 1
    let grid_val = (gx * gy).sqrt();
    let fade = 1.0 / (1.0 + (dx * dx + dy * dy).sqrt() * 0.025);
    let value = (grid_val * fade).clamp(0.0, 1.0);

    let ch = if value > 0.7 {
        '+'
    } else {
        let idx = (value * (DENSITY.len() - 1) as f64) as usize;
        DENSITY[idx.min(DENSITY.len() - 1)]
    };
    (ch, value)
}

// ════════════════════════════════════════════════════════════════════
// 10. ORBIT  – particles circling the centre at different speeds/radii
//     Uses scatter approach like Lissajous for efficiency.
// ════════════════════════════════════════════════════════════════════

fn apply_orbit(frame: &mut Frame, area: Rect, time: f64, theme: &Theme) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    let mut grid = vec![0.0f64; w * h];
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;

    let num_particles = 24;
    let trail_len = 40;

    for p in 0..num_particles {
        let radius = 4.0 + (hash2(p, 0) % 35) as f64;
        let speed = 0.15 + (hash2(p, 1) % 20) as f64 / 60.0;
        let phase = (hash2(p, 2) % 628) as f64 / 100.0;
        // Elliptical: different x/y radii
        let rx = radius;
        let ry = radius * (0.3 + (hash2(p, 3) % 7) as f64 / 10.0);
        // Tilt each orbit slightly
        let tilt = (hash2(p, 4) % 314) as f64 / 100.0 - 1.57;

        for ti in 0..trail_len {
            let angle = time * speed + phase - ti as f64 * 0.04;
            let ox = rx * angle.cos();
            let oy = ry * angle.sin();
            // Apply tilt rotation
            let px = cx + ox * tilt.cos() - oy * tilt.sin();
            let py = cy + (ox * tilt.sin() + oy * tilt.cos()) / 2.0; // aspect

            let ix = px.round() as isize;
            let iy = py.round() as isize;

            if ix >= 0 && ix < w as isize && iy >= 0 && iy < h as isize {
                let brightness = (1.0 - ti as f64 / trail_len as f64) * 0.9;
                let idx = iy as usize * w + ix as usize;
                if brightness > grid[idx] {
                    grid[idx] = brightness;
                }
            }
        }
    }

    // Apply to empty cells
    let buf = frame.buffer_mut();
    for y in 0..area.height {
        for x in 0..area.width {
            let pos = (area.x + x, area.y + y);
            if !is_empty(buf, pos) {
                continue;
            }
            let brightness = grid[y as usize * w + x as usize];
            let ch = if brightness > 0.7 {
                '●'
            } else if brightness > 0.4 {
                '◦'
            } else if brightness > 0.15 {
                '·'
            } else {
                ' '
            };
            write_bg_cell(buf, pos, ch, brightness, theme);
        }
    }
}
