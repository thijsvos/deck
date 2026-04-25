use serde::Deserialize;

use crate::background::BackgroundKind;
use crate::markdown::{self, Block};
use crate::theme::ThemeName;
use crate::transition::TransitionKind;

/// A fully parsed presentation: TOML frontmatter (`meta`) plus ordered slides.
///
/// Produced by [`parse_deck`].
#[derive(Debug)]
pub struct Deck {
    pub meta: DeckMeta,
    pub slides: Vec<Slide>,
}

/// Deck-wide metadata read from a TOML frontmatter block at the top of the
/// markdown file. All fields are optional and default when absent.
#[derive(Debug, Deserialize)]
pub struct DeckMeta {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub theme: ThemeName,
    #[serde(default)]
    pub transition: TransitionKind,
    /// Default animated background for the first slide. Per-slide overrides
    /// (via `<!-- background: ... -->`) take precedence.
    #[serde(default)]
    pub background: Option<BackgroundKind>,
}

fn default_title() -> String {
    "Untitled".to_string()
}

impl Default for DeckMeta {
    fn default() -> Self {
        Self {
            title: default_title(),
            author: None,
            theme: ThemeName::default(),
            transition: TransitionKind::default(),
            background: None,
        }
    }
}

/// One slide: ordered blocks plus optional layout/columns/notes/background.
#[derive(Debug)]
pub struct Slide {
    pub blocks: Vec<Block>,
    pub layout: Layout,
    /// Side-by-side body when `layout == Columns`. Any leading blocks above
    /// the `::: columns` marker live in `blocks` and render above the split.
    pub columns: Option<Columns>,
    /// Speaker notes extracted from `<!-- note: ... -->` comments. Hidden in
    /// normal mode; shown in presenter mode.
    pub notes: Vec<String>,
    /// Per-slide animated background that overrides the deck-level default.
    pub background: Option<BackgroundKind>,
}

impl Slide {
    /// Total bullets across all top-level `BulletList` blocks. Numbered lists
    /// and bullets nested inside other blocks (e.g. columns) are excluded —
    /// only the top-level reveal cursor consumes these.
    pub fn bullet_count(&self) -> usize {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::BulletList { items } => items.len(),
                _ => 0,
            })
            .sum()
    }

    /// Initial value of `App::reveal_count` for this slide: `0` when there are
    /// bullets to progressively reveal, `usize::MAX` otherwise so `advance`
    /// moves straight to the next slide.
    pub fn initial_reveal(&self) -> usize {
        if self.bullet_count() > 0 {
            0
        } else {
            usize::MAX
        }
    }
}

/// Two-column slide body: left and right block sequences rendered side-by-side.
#[derive(Debug)]
pub struct Columns {
    pub left: Vec<Block>,
    pub right: Vec<Block>,
}

/// How a slide's content is laid out. Selected by HTML-comment directives in
/// markdown: `<!-- layout: center -->` for `Center`, a `::: columns` block for
/// `Columns`. Drives renderer dispatch in [`crate::render::render_slide`].
#[derive(Debug, Default)]
pub enum Layout {
    #[default]
    Default,
    Center,
    Columns,
}

/// Parse a markdown deck.
///
/// Extracts an optional TOML frontmatter (`---`-fenced), splits the body on
/// `---` separators, parses each slide's blocks, and applies the deck-level
/// background to the first slide if it doesn't already have one.
///
/// Always returns a `Deck`; malformed frontmatter falls back to defaults
/// rather than erroring.
pub fn parse_deck(input: &str) -> Deck {
    let (meta, body) = extract_frontmatter(input);
    let raw_slides = split_slides(&body);

    let mut slides: Vec<Slide> = raw_slides
        .into_iter()
        .map(|raw| parse_slide(&raw))
        .filter(|s| !s.blocks.is_empty() || s.columns.is_some())
        .collect();

    // Apply frontmatter background to first slide if not already set
    if let Some(ref bg) = meta.background {
        if let Some(first) = slides.first_mut() {
            if first.background.is_none() {
                first.background = Some(bg.clone());
            }
        }
    }

    Deck { meta, slides }
}

fn extract_frontmatter(input: &str) -> (DeckMeta, String) {
    // Normalize CRLF so Windows-authored markdown (or files checked out with
    // `core.autocrlf=true`) parses identically to LF.
    let normalized = if input.contains("\r\n") {
        input.replace("\r\n", "\n")
    } else {
        input.to_string()
    };
    let trimmed = normalized.trim_start();
    let after_first = match trimmed.strip_prefix("---") {
        Some(rest) => rest,
        None => return (DeckMeta::default(), normalized.clone()),
    };
    let after_first = after_first.trim_start_matches(['\n', '\r']);

    // Match end-of-frontmatter only when `---` is line-anchored: followed by
    // EOF, `\n`, or `\r\n`. Avoids prematurely terminating on a TOML triple-
    // quoted string that happens to contain `\n---`.
    let mut search_from = 0;
    let end = loop {
        let Some(pos) = after_first[search_from..].find("\n---") else {
            break None;
        };
        let abs = search_from + pos;
        let after = &after_first[abs + 4..];
        if after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n") {
            break Some(abs);
        }
        search_from = abs + 4;
    };
    if let Some(end) = end {
        let frontmatter_str = &after_first[..end];
        let rest = after_first.get(end + 4..).unwrap_or("");

        if let Ok(meta) = toml::from_str::<DeckMeta>(frontmatter_str) {
            return (meta, rest.to_string());
        }
    }

    (DeckMeta::default(), normalized)
}

fn split_slides(body: &str) -> Vec<String> {
    let mut slides = Vec::new();
    let mut current = String::new();

    for line in body.lines() {
        if line.trim() == "---" {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                slides.push(trimmed);
            }
            current.clear();
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        slides.push(trimmed);
    }

    slides
}

fn parse_slide(raw: &str) -> Slide {
    let (cleaned, notes) = extract_notes(raw);
    let (cleaned, background) = extract_background(&cleaned);

    // Check for column layout
    if cleaned.contains("::: columns") {
        if let Some((before, left_md, right_md)) = extract_columns(&cleaned) {
            let blocks = if before.trim().is_empty() {
                Vec::new()
            } else {
                markdown::parse_blocks(before.trim())
            };

            return Slide {
                blocks,
                layout: Layout::Columns,
                columns: Some(Columns {
                    left: markdown::parse_blocks(&left_md),
                    right: markdown::parse_blocks(&right_md),
                }),
                notes,
                background,
            };
        }
    }

    let is_center = cleaned.contains("<!-- layout: center -->");
    let cleaned = cleaned.replace("<!-- layout: center -->", "");

    let blocks = markdown::parse_blocks(cleaned.trim());

    Slide {
        blocks,
        layout: if is_center {
            Layout::Center
        } else {
            Layout::Default
        },
        columns: None,
        notes,
        background,
    }
}

fn extract_notes(raw: &str) -> (String, Vec<String>) {
    let mut notes = Vec::new();
    let mut cleaned = String::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<!-- note:") {
            if let Some(note) = rest.strip_suffix("-->") {
                notes.push(note.trim().to_string());
                continue;
            }
        }
        if !cleaned.is_empty() {
            cleaned.push('\n');
        }
        cleaned.push_str(line);
    }

    (cleaned, notes)
}

fn extract_columns(raw: &str) -> Option<(String, String, String)> {
    let lines: Vec<&str> = raw.lines().collect();

    let col_start = lines.iter().position(|l| l.trim() == "::: columns")?;
    let before: String = lines[..col_start].join("\n");

    let mut left = String::new();
    let mut right = String::new();
    let mut in_left = false;
    let mut in_right = false;

    for line in &lines[col_start + 1..] {
        let trimmed = line.trim();

        if trimmed == "::: left" {
            in_left = true;
            in_right = false;
            continue;
        }
        if trimmed == "::: right" {
            in_right = true;
            in_left = false;
            continue;
        }
        if trimmed == ":::" {
            if in_left {
                in_left = false;
                continue;
            }
            if in_right {
                in_right = false;
                continue;
            }
            break;
        }

        if in_left {
            if !left.is_empty() {
                left.push('\n');
            }
            left.push_str(line);
        } else if in_right {
            if !right.is_empty() {
                right.push('\n');
            }
            right.push_str(line);
        }
    }

    if left.is_empty() && right.is_empty() {
        None
    } else {
        Some((before, left, right))
    }
}

fn extract_background(raw: &str) -> (String, Option<BackgroundKind>) {
    let mut bg = None;
    let mut cleaned = String::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<!-- background:") {
            if let Some(name) = rest.strip_suffix("-->") {
                bg = name.trim().parse().ok();
                continue;
            }
        }
        if !cleaned.is_empty() {
            cleaned.push('\n');
        }
        cleaned.push_str(line);
    }

    (cleaned, bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frontmatter_toml() {
        let input = "---\ntitle = \"Hello\"\ntheme = \"minimal\"\n---\n# Slide 1";
        let deck = parse_deck(input);
        assert_eq!(deck.meta.title, "Hello");
        assert!(matches!(deck.meta.theme, ThemeName::Minimal));
        assert_eq!(deck.slides.len(), 1);
    }

    #[test]
    fn parse_frontmatter_defaults() {
        let input = "# Just a slide";
        let deck = parse_deck(input);
        assert_eq!(deck.meta.title, "Untitled");
        assert!(matches!(deck.meta.theme, ThemeName::Hacker));
    }

    #[test]
    fn split_slides_on_separator() {
        let input = "# One\n\n---\n\n# Two\n\n---\n\n# Three";
        let deck = parse_deck(input);
        assert_eq!(deck.slides.len(), 3);
    }

    #[test]
    fn empty_slides_filtered() {
        let input = "---\ntitle = \"T\"\n---\n\n---\n\n# Real slide";
        let deck = parse_deck(input);
        assert_eq!(deck.slides.len(), 1);
    }

    #[test]
    fn extract_speaker_notes() {
        let input = "# Slide\n\n- Point\n\n<!-- note: Remember this -->";
        let deck = parse_deck(input);
        assert_eq!(deck.slides[0].notes, vec!["Remember this"]);
    }

    #[test]
    fn extract_background_directive() {
        let input = "<!-- background: matrix -->\n\n# Title";
        let deck = parse_deck(input);
        assert!(deck.slides[0].background.is_some());
    }

    #[test]
    fn center_layout_detected() {
        let input = "<!-- layout: center -->\n\n# Centered";
        let deck = parse_deck(input);
        assert!(matches!(deck.slides[0].layout, Layout::Center));
    }

    #[test]
    fn column_layout_parsed() {
        let input = "## Title\n\n::: columns\n::: left\nLeft\n:::\n::: right\nRight\n:::\n:::";
        let deck = parse_deck(input);
        assert!(matches!(deck.slides[0].layout, Layout::Columns));
        assert!(deck.slides[0].columns.is_some());
    }

    #[test]
    fn frontmatter_background_applies_to_first_slide() {
        let input = "---\nbackground = \"plasma\"\n---\n# First\n\n---\n\n# Second";
        let deck = parse_deck(input);
        assert!(deck.slides[0].background.is_some());
        assert!(deck.slides[1].background.is_none());
    }

    #[test]
    fn empty_input_produces_no_slides() {
        let deck = parse_deck("");
        assert!(deck.slides.is_empty());
    }

    #[test]
    fn malformed_frontmatter_falls_back_to_defaults() {
        let input = "---\nbad = [\n---\n# Slide";
        let deck = parse_deck(input);
        assert_eq!(deck.meta.title, "Untitled");
    }

    #[test]
    fn multiple_notes_extracted() {
        let input = "# Slide\n<!-- note: First -->\n<!-- note: Second -->";
        let deck = parse_deck(input);
        assert_eq!(deck.slides[0].notes, vec!["First", "Second"]);
    }

    #[test]
    fn unknown_background_name_ignored() {
        let input = "<!-- background: fire -->\n# Title";
        let deck = parse_deck(input);
        assert!(deck.slides[0].background.is_none());
    }

    #[test]
    fn frontmatter_with_crlf_line_endings() {
        // Simulates a file authored on Windows or checked out with autocrlf.
        let input = "---\r\ntitle = \"Hello\"\r\ntheme = \"minimal\"\r\n---\r\n# Slide 1";
        let deck = parse_deck(input);
        assert_eq!(deck.meta.title, "Hello");
        assert!(matches!(deck.meta.theme, ThemeName::Minimal));
        assert_eq!(deck.slides.len(), 1);
    }
}
