# deck

[![CI](https://github.com/thijsvos/deck/actions/workflows/ci.yml/badge.svg)](https://github.com/thijsvos/deck/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/thijsvos/deck)](https://github.com/thijsvos/deck/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Terminal presentations with style.

A tiny, single-binary presentation tool written in Rust. Render Markdown slides in your terminal with animated mathematical backgrounds, a hacker aesthetic, progressive bullet reveal, column layouts, and a full presenter mode.

**~3 MB binary. No server. No dependencies. Just `deck talk.md`.**

## Features

- **Markdown slides** with TOML frontmatter, separated by `---`
- **Syntax highlighting** - ~50 languages via syntect (`base16-ocean.dark` theme)
- **Cinematic block entrances** - every element animates onto the slide:
  - H1 headings **decrypt** from glitch characters (`░▒▓█@#$` → text)
  - H2+ headings **slide in** left-to-right
  - Code blocks **typewrite** line-by-line with a blinking `▌` cursor
  - Bullet points **cascade** with staggered timing
  - Paragraphs and images **dissolve** in
- **Big ASCII text** for `# H1` headings
- **Images** - PNG, JPEG, GIF with auto-detection of Kitty, Sixel, or half-block rendering
- **Progressive reveal** - bullet points appear one at a time
- **Inline formatting** - **bold**, *italic*, `inline code`, numbered lists
- **Column layouts** - side-by-side content with `::: columns` syntax
- **Centered slides** - `<!-- layout: center -->` directive
- **Speaker notes** - `<!-- note: your note -->`, visible in presenter mode
- **Presenter mode** (`p`) - current slide + next preview + notes + timer
- **Dual-screen mode** - `--present` on your laptop, `--follow` on the projector
- **10 animated backgrounds** - mathematical screensavers for your title slide
- **Slide transitions** - glitch, fade, wipe, and dissolve effects
- **4 themes** - `hacker` (default), `corporate`, `catppuccin`, and `minimal`
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
| `author` | Any string (shown in status bar) | - |
| `theme` | `"hacker"`, `"corporate"`, `"catppuccin"`, `"minimal"` | `"hacker"` |
| `transition` | `"none"`, `"glitch"`, `"fade"`, `"wipe"`, `"dissolve"` | Theme default |
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

## Block Entrances

Every block on a slide gets a cinematic entrance animation when the slide first appears. No configuration needed — effects are assigned automatically by block type.

| Block | Effect | Duration |
|-------|--------|----------|
| `# H1` heading | **Decrypt** — glitch characters resolve into ASCII art | 600ms |
| `## H2+` heading | **Slide-in** — text reveals left-to-right | 300ms |
| Code block | **Typewriter** — syntax-highlighted lines type one-by-one with `▌` cursor | ~50ms per line |
| Bullet item | **Cascade** — staggered fade-in, 100ms delay between items | 200ms each |
| Paragraph | **Fade-in** — smoothstep dissolve | 250ms |
| Image | **Fade-in** — smoothstep dissolve | 300ms |

Entrances replay each time you navigate to a slide. The frame rate automatically elevates to 60fps while animations are active, then drops back to save CPU.

## Syntax Highlighting

Code blocks are syntax-highlighted for ~50 languages using [syntect](https://github.com/trishume/syntect) with the `base16-ocean.dark` theme. The language is detected from the fenced code block tag:

````markdown
```rust
fn main() {
    println!("highlighted!");
}
```
````

Combined with the typewriter entrance, code blocks type themselves out in full color — great for walking an audience through code step by step.

## Images

Add images with standard Markdown syntax:

```markdown
![alt text](photo.png)
```

Supported formats: **PNG**, **JPEG**, **GIF** (first frame). Images are auto-resized to fit the slide area and centered horizontally.

Deck auto-detects the best rendering protocol for your terminal:

| Protocol | Terminals | Quality |
|----------|-----------|---------|
| **Kitty** | Kitty, Ghostty, WezTerm, Konsole | Full resolution, GPU-rendered |
| **Sixel** | iTerm2, foot | High quality ANSI fallback |
| **Half-blocks** | Everything else | Unicode `▀` characters, works everywhere |

Image paths are relative to the markdown file's directory. Absolute paths and paths that escape the presentation directory are blocked for security.

## Markdown Reference

Deck supports standard Markdown within slides:

```markdown
# H1 Heading          Large ASCII art
## H2 Heading         Bold colored text
### H3+ Heading       Bold colored text

**bold text**          Bold styling
*italic text*          Italic styling
`inline code`          Code styling

- Bullet point         Progressive reveal
- Another point

1. Numbered item       Always fully visible
2. Another item

![alt](image.png)      Inline image

---                    Slide separator (between slides)
```

Code blocks use fenced syntax with a language tag for highlighting:

````markdown
```python
def hello():
    print("highlighted!")
```
````

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

| Theme | Look | Font | Transition |
|-------|------|------|------------|
| **hacker** (default) | Matrix green, neon pink, cyan headings | Block (`█`) | Glitch |
| **corporate** | Navy, blue headings, gold accents | Thin (`┌─┐`) | Wipe |
| **catppuccin** | Mocha palette, pink accents, peach bold | Block (`█`) | Dissolve |
| **minimal** | Terminal defaults, white, yellow accents | Thin (`┌─┐`) | Fade |

Each theme carries its own default transition and font style. Override with `transition = "glitch"` in frontmatter.

## License

[MIT](LICENSE)
