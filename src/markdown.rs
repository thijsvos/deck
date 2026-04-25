use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

/// One block of slide content.
///
/// The intermediate representation produced by [`parse_blocks`] and consumed
/// by the render layer.
#[derive(Debug, Clone)]
pub enum Block {
    Heading { level: u8, text: String },
    Paragraph { spans: Vec<Span> },
    BulletList { items: Vec<ListItem> },
    NumberedList { items: Vec<ListItem> },
    Code { lang: Option<String>, code: String },
    HorizontalRule,
    Image { path: String, alt: String },
}

/// One item in a bullet or numbered list, holding inline spans.
#[derive(Debug, Clone)]
pub struct ListItem {
    pub spans: Vec<Span>,
}

/// Inline text fragment with style. Used inside paragraphs, headings, and list
/// items to mix plain text with bold, italic, and inline-code runs.
#[derive(Debug, Clone)]
pub enum Span {
    Plain(String),
    Bold(String),
    Italic(String),
    Code(String),
}

/// Parse a markdown fragment into a flat list of blocks.
///
/// Wraps `pulldown-cmark`'s event stream into the deck IR the renderer
/// expects. Nested lists are flattened: only the innermost items are kept.
pub fn parse_blocks(markdown: &str) -> Vec<Block> {
    let parser = Parser::new(markdown);
    let mut blocks = Vec::new();
    let mut state = ParseState::default();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                state.in_heading = true;
                state.heading_level = heading_to_u8(level);
                state.text_buf.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                state.in_heading = false;
                let text: String = state.text_buf.drain(..).collect();
                blocks.push(Block::Heading {
                    level: state.heading_level,
                    text,
                });
            }
            Event::Start(Tag::Paragraph) => {
                if !state.in_list {
                    state.spans.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !state.in_list && !state.spans.is_empty() {
                    blocks.push(Block::Paragraph {
                        spans: state.spans.drain(..).collect(),
                    });
                }
            }
            Event::Start(Tag::List(ordered)) => {
                // Save parent list state for nested lists
                state
                    .list_stack
                    .push((state.ordered, std::mem::take(&mut state.list_items)));
                state.in_list = true;
                state.ordered = ordered.is_some();
                state.list_items.clear();
            }
            Event::End(TagEnd::List(_)) => {
                let items: Vec<ListItem> = state.list_items.drain(..).collect();
                if state.ordered {
                    blocks.push(Block::NumberedList { items });
                } else {
                    blocks.push(Block::BulletList { items });
                }
                // Restore parent list state
                if let Some((parent_ordered, parent_items)) = state.list_stack.pop() {
                    state.ordered = parent_ordered;
                    state.list_items = parent_items;
                    state.in_list = true;
                } else {
                    state.in_list = false;
                }
            }
            Event::Start(Tag::Item) => {
                state.spans.clear();
            }
            Event::End(TagEnd::Item) => {
                state.list_items.push(ListItem {
                    spans: state.spans.drain(..).collect(),
                });
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                state.in_code = true;
                state.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string();
                        if l.is_empty() {
                            None
                        } else {
                            Some(l)
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                state.text_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                state.in_code = false;
                let code: String = state.text_buf.drain(..).collect();
                blocks.push(Block::Code {
                    lang: state.code_lang.take(),
                    code: code.trim_end().to_string(),
                });
            }
            Event::Start(Tag::Emphasis) => {
                state.in_italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                state.in_italic = false;
            }
            Event::Start(Tag::Strong) => {
                state.in_bold = true;
            }
            Event::End(TagEnd::Strong) => {
                state.in_bold = false;
            }
            Event::Text(text) => {
                if state.in_image || state.in_code || state.in_heading {
                    state.text_buf.push(text.to_string());
                } else if state.in_bold {
                    state.spans.push(Span::Bold(text.to_string()));
                } else if state.in_italic {
                    state.spans.push(Span::Italic(text.to_string()));
                } else {
                    state.spans.push(Span::Plain(text.to_string()));
                }
            }
            Event::Code(text) => {
                state.spans.push(Span::Code(text.to_string()));
            }
            Event::SoftBreak | Event::HardBreak => {
                if state.in_code {
                    state.text_buf.push("\n".to_string());
                } else if state.in_heading {
                    state.text_buf.push(" ".to_string());
                } else {
                    state.spans.push(Span::Plain(" ".to_string()));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                state.in_image = true;
                state.image_dest = Some(dest_url.to_string());
                state.text_buf.clear();
            }
            Event::End(TagEnd::Image) => {
                state.in_image = false;
                if let Some(dest) = state.image_dest.take() {
                    let alt: String = state.text_buf.drain(..).collect();
                    blocks.push(Block::Image { path: dest, alt });
                }
            }
            Event::Rule => {
                blocks.push(Block::HorizontalRule);
            }
            _ => {}
        }
    }

    blocks
}

fn heading_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[derive(Default)]
struct ParseState {
    in_heading: bool,
    heading_level: u8,
    in_list: bool,
    ordered: bool,
    in_code: bool,
    in_bold: bool,
    in_italic: bool,
    code_lang: Option<String>,
    text_buf: Vec<String>,
    spans: Vec<Span>,
    list_items: Vec<ListItem>,
    list_stack: Vec<(bool, Vec<ListItem>)>,
    in_image: bool,
    image_dest: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_heading() {
        let blocks = parse_blocks("# Hello");
        assert!(matches!(&blocks[0], Block::Heading { level: 1, text } if text == "Hello"));
    }

    #[test]
    fn parse_h2_heading() {
        let blocks = parse_blocks("## Sub");
        assert!(matches!(&blocks[0], Block::Heading { level: 2, text } if text == "Sub"));
    }

    #[test]
    fn parse_paragraph() {
        let blocks = parse_blocks("Just some text");
        assert!(matches!(&blocks[0], Block::Paragraph { spans } if !spans.is_empty()));
    }

    #[test]
    fn parse_bold_span() {
        let blocks = parse_blocks("**bold**");
        if let Block::Paragraph { spans } = &blocks[0] {
            assert!(matches!(&spans[0], Span::Bold(t) if t == "bold"));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_italic_span() {
        let blocks = parse_blocks("*italic*");
        if let Block::Paragraph { spans } = &blocks[0] {
            assert!(matches!(&spans[0], Span::Italic(t) if t == "italic"));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_code() {
        let blocks = parse_blocks("`code`");
        if let Block::Paragraph { spans } = &blocks[0] {
            assert!(matches!(&spans[0], Span::Code(t) if t == "code"));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_bullet_list() {
        let blocks = parse_blocks("- One\n- Two\n- Three");
        if let Block::BulletList { items } = &blocks[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected bullet list");
        }
    }

    #[test]
    fn parse_numbered_list() {
        let blocks = parse_blocks("1. First\n2. Second");
        if let Block::NumberedList { items } = &blocks[0] {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected numbered list");
        }
    }

    #[test]
    fn parse_code_block() {
        let blocks = parse_blocks("```rust\nfn main() {}\n```");
        if let Block::Code { lang, code } = &blocks[0] {
            assert_eq!(lang.as_deref(), Some("rust"));
            assert!(code.contains("fn main"));
        } else {
            panic!("expected code block");
        }
    }

    #[test]
    fn parse_horizontal_rule() {
        let blocks = parse_blocks("---");
        assert!(matches!(&blocks[0], Block::HorizontalRule));
    }

    #[test]
    fn parse_image() {
        let blocks = parse_blocks("![alt text](./photo.png)");
        if let Block::Image { path, alt } = &blocks[0] {
            assert_eq!(path, "./photo.png");
            assert_eq!(alt, "alt text");
        } else {
            panic!("expected image, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn parse_empty_input() {
        let blocks = parse_blocks("");
        assert!(blocks.is_empty());
    }

    #[test]
    fn parse_code_block_no_language() {
        let blocks = parse_blocks("```\nplain code\n```");
        if let Block::Code { lang, code } = &blocks[0] {
            assert!(lang.is_none());
            assert!(code.contains("plain code"));
        } else {
            panic!("expected code block");
        }
    }

    #[test]
    fn parse_h3_through_h6() {
        for level in 3..=6u8 {
            let hashes = "#".repeat(level as usize);
            let input = format!("{hashes} Level {level}");
            let blocks = parse_blocks(&input);
            assert!(
                matches!(&blocks[0], Block::Heading { level: l, .. } if *l == level),
                "H{level} not parsed correctly"
            );
        }
    }
}
