use serde::Deserialize;

use crate::background::BackgroundKind;
use crate::markdown::{self, Block};
use crate::theme::ThemeName;
use crate::transition::TransitionKind;

#[derive(Debug)]
pub struct Deck {
    pub meta: DeckMeta,
    pub slides: Vec<Slide>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeckMeta {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub theme: ThemeName,
    #[serde(default)]
    pub transition: TransitionKind,
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
            date: None,
            theme: ThemeName::default(),
            transition: TransitionKind::default(),
            background: None,
        }
    }
}

#[derive(Debug)]
pub struct Slide {
    pub blocks: Vec<Block>,
    pub layout: Layout,
    pub columns: Option<Columns>,
    pub notes: Vec<String>,
    pub background: Option<BackgroundKind>,
}

#[derive(Debug)]
pub struct Columns {
    pub left: Vec<Block>,
    pub right: Vec<Block>,
}

#[derive(Debug, Default)]
pub enum Layout {
    #[default]
    Default,
    Center,
    Columns,
}

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
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return (DeckMeta::default(), input.to_string());
    }

    let after_first = &trimmed[3..];
    let after_first = after_first.trim_start_matches(['\n', '\r']);

    if let Some(end) = after_first.find("\n---") {
        let frontmatter_str = &after_first[..end];
        let rest = &after_first[end + 4..];

        if let Ok(meta) = toml::from_str::<DeckMeta>(frontmatter_str) {
            return (meta, rest.to_string());
        }
    }

    (DeckMeta::default(), input.to_string())
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
        layout: if is_center { Layout::Center } else { Layout::Default },
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
    let mut depth = 0;

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
            if depth == 0 {
                break;
            }
            depth -= 1;
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
                bg = match name.trim() {
                    "matrix" => Some(BackgroundKind::Matrix),
                    "plasma" => Some(BackgroundKind::Plasma),
                    "lissajous" => Some(BackgroundKind::Lissajous),
                    "spiral" => Some(BackgroundKind::Spiral),
                    "wave" => Some(BackgroundKind::Wave),
                    "aurora" => Some(BackgroundKind::Aurora),
                    "rain" => Some(BackgroundKind::Rain),
                    "noise" => Some(BackgroundKind::Noise),
                    "lattice" => Some(BackgroundKind::Lattice),
                    "orbit" => Some(BackgroundKind::Orbit),
                    _ => None,
                };
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
