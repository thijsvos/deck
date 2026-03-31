# deck

Terminal presentations with style.

A tiny, single-binary presentation tool written in Rust. Render Markdown slides in your terminal with animated mathematical backgrounds, a hacker aesthetic, progressive bullet reveal, column layouts, and a full presenter mode.

**1.1 MB binary. No server. No dependencies. Just `deck talk.md`.**

## Features

- **Markdown slides** with TOML frontmatter, separated by `---`
- **Big ASCII text** for `# H1` headings
- **Progressive reveal** - bullet points appear one at a time
- **Column layouts** - side-by-side content with `::: columns` syntax
- **Centered slides** - `<!-- layout: center -->` directive
- **Speaker notes** - `<!-- note: your note -->`, visible in presenter mode
- **Presenter mode** (`p`) - current slide + next preview + notes + timer
- **Dual-screen mode** - `--present` on your laptop, `--follow` on the projector
- **10 animated backgrounds** - mathematical screensavers for your title slide
- **Slide transitions** - glitch and fade effects
- **3 themes** - `hacker` (default), `corporate`, and `minimal`
- **Vim-style navigation** - arrow keys, hjkl, go-to-slide

## Installation

### Build from source

```bash
git clone https://github.com/thijsvos/deck.git
cd deck
cargo build --release
# Binary is at ./target/release/deck
```

### Download binary

Grab a prebuilt binary from [Releases](https://github.com/thijsvos/deck/releases).

## Quick Start

```bash
deck examples/demo.md
```

## Dual-Screen Presenting

Use two terminals for real presentations with a projector:

```bash
# Terminal 1 — drag to projector/external display
deck talk.md --follow

# Terminal 2 — your laptop screen
deck talk.md --present
```

The `--present` terminal shows the presenter view (notes, timer, next slide preview) and controls navigation. The `--follow` terminal shows full-screen slides and follows along automatically. Synced via a tiny temp file — no server, no networking.

## Writing Slides

Slides are Markdown files with TOML frontmatter:

```markdown
---
title = "My Talk"
author = "Your Name"
theme = "hacker"
transition = "glitch"
background = "aurora"
---

# Welcome

<!-- layout: center -->

Your opening slide with animated aurora background.

---

## Key Points

- First point appears on keypress
- Then second
- Then third

<!-- note: Remember to mention the demo -->

---

## Side by Side

::: columns
::: left
Left content here
:::
::: right
Right content here
:::
:::
```

### Frontmatter Options

| Field | Values | Default |
|-------|--------|---------|
| `title` | Any string | `"Untitled"` |
| `author` | Any string | - |
| `theme` | `"hacker"`, `"corporate"`, `"minimal"` | `"hacker"` |
| `transition` | `"none"`, `"glitch"`, `"fade"` | `"none"` |
| `background` | See backgrounds below | - |

### Per-Slide Directives

Use HTML comments to configure individual slides:

```markdown
<!-- layout: center -->       Center content vertically
<!-- background: matrix -->   Set animated background
<!-- note: Speaker note -->   Add speaker note
```

## Backgrounds

10 animated mathematical backgrounds, perfect for title slides while your audience walks in.

| Name | Math | Visual |
|------|------|--------|
| `aurora` | Horizontal sin curtains + Gaussian bell | Flowing northern lights |
| `matrix` | Hash-seeded falling columns with trails | Classic green rain |
| `plasma` | 4 overlapping sin/cos interference | Smooth flowing density |
| `lissajous` | Parametric sin curve, morphing a/b/delta | Tracing figure-eight trail |
| `spiral` | sin(angle * arms + dist - t) in polar coords | Rotating 3-arm spiral |
| `wave` | Concentric ripple + horizontal sine | Expanding ring interference |
| `rain` | Vertical drops with short fading trails | Gentle drizzle |
| `noise` | 2-octave value noise with smoothstep | Drifting cloudscape |
| `lattice` | sin product in rotating coordinate frame | Morphing grid intersections |
| `orbit` | Particles on tilted elliptical orbits | Circling dots with trails |

Set in frontmatter (applies to first slide) or per-slide with `<!-- background: name -->`.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Right` / `Space` / `Enter` / `l` / `j` | Next (reveal bullet or next slide) |
| `Left` / `Backspace` / `h` / `k` | Previous |
| `g` | First slide |
| `G` | Last slide |
| `:` then number then `Enter` | Go to slide N |
| `p` | Toggle presenter mode |
| `r` | Reset timer |
| `?` | Toggle help overlay |
| `q` / `Esc` | Quit |

## Themes

Use `--theme corporate` or set `theme = "corporate"` in frontmatter.

- **hacker** (default) - Dark background, matrix green, neon pink accents, cyan headings
- **corporate** - Deep navy background, blue headings, gold accents — for business meetings
- **minimal** - Terminal default background, white text, yellow accents

## License

[MIT](LICENSE)
