//! parser.rs — input format → IRDocument parser
//!
//! Currently supported formats:
//! - `InputFormat::Markdown`  — pulldown-cmark based
//! - `InputFormat::PlainText` — paragraph-splitting parser
//! - `InputFormat::Html`      — strips HTML tags then delegates to PlainText
//!
//! ## Paragraph importance scoring
//!
//! Every `DocNode::Para` is assigned an `importance` value in `0.1..=1.0`
//! using a lightweight heuristic that combines three signals:
//!
//! | Signal | Weight | Rationale |
//! |--------|--------|-----------|
//! | Position | 50% | Inverted-pyramid principle: earlier paragraphs introduce the topic |
//! | Length   | 40% | Very short paragraphs (captions, footnotes) carry less information |
//! | Heading proximity | 10% | Paragraphs immediately after a heading introduce a new section |
//!
//! This ensures that `AdaptiveCompressor::prune_low_importance` (Stage 2) has
//! genuinely differentiated scores to work with rather than a flat `1.0` for all nodes.

use crate::InputFormat;
use crate::ir::{DocNode, FidelityLevel, IRDocument};

/// Converts input text into an `IRDocument`.
pub fn parse(
    input: &str,
    format: InputFormat,
    fidelity: FidelityLevel,
    budget: Option<usize>,
) -> Result<IRDocument, String> {
    let mut doc = IRDocument::new(fidelity, budget);

    match format {
        InputFormat::Markdown => {
            // Extract YAML front matter (---...---) before handing off to the
            // Markdown parser.  The front matter block is consumed here so that
            // pulldown-cmark does not see it as raw paragraph text.
            let (fm, body) = split_yaml_front_matter(input);
            if let Some(fm) = fm {
                push_yaml_front_matter_metadata(&fm, &mut doc);
            }
            parse_markdown(body, &mut doc);
        }
        InputFormat::PlainText => parse_plaintext(input, &mut doc),
        InputFormat::Html => {
            // Extract <title> and <meta name="description"> from raw HTML before
            // stripping tags, so the YAML header can be populated.
            extract_html_metadata(input, &mut doc);

            // Strip HTML → delegate to PlainText paragraph parser.
            // Re-apply PUA stripping after ammonia tag removal: ammonia decodes HTML
            // entities (e.g. &#xE000;) into actual PUA codepoints which would otherwise
            // collide with the internal symbol substitution scheme.
            let plain = strip_html_tags(input);
            let plain = crate::strip_pua(&plain);
            parse_plaintext(&plain, &mut doc);
        }
    }

    Ok(doc)
}

// ────────────────────────────────────────────────
// Metadata extraction helpers
// ────────────────────────────────────────────────

/// Splits a Markdown string into an optional YAML front matter block and the
/// remaining body.
///
/// Front matter is recognised when the document starts with a line containing
/// only `---`, followed by one or more lines, followed by a closing `---` or
/// `...` line.  Returns `(Some(fm_text), body)` when found, or `(None, input)`
/// when there is no front matter.
fn split_yaml_front_matter(input: &str) -> (Option<&str>, &str) {
    // Must start with "---" (optionally followed by whitespace on the same line).
    let Some(after_open) = input.strip_prefix("---") else {
        return (None, input);
    };
    // The opening delimiter must be immediately followed by a newline (or be the
    // whole string, though that would be an empty front matter).
    let after_open = match after_open.strip_prefix('\n') {
        Some(s) => s,
        None => match after_open.strip_prefix("\r\n") {
            Some(s) => s,
            None => return (None, input), // "---" not followed by newline — not FM
        },
    };

    // Scan for the closing delimiter (--- or ...) on its own line.
    for (i, line) in after_open.match_indices('\n') {
        let _ = line; // we want the byte offset, not the matched char
        let up_to = &after_open[..i];
        let closing_start = i + 1; // byte offset of the line after '\n'
        let rest = &after_open[closing_start..];
        let is_close = rest.starts_with("---") || rest.starts_with("...");
        if is_close {
            // Consume the closing delimiter line.
            let after_close = rest
                .find('\n')
                .map(|j| &rest[j + 1..])
                .unwrap_or(""); // closing delimiter was the last line
            return (Some(up_to), after_close);
        }
    }

    // No closing delimiter found — treat entire document as body.
    (None, input)
}

/// Parses key: value pairs from a YAML front matter string and pushes them as
/// `DocNode::Metadata` nodes.  Only simple scalar mappings are supported
/// (lists and nested maps are ignored).  Recognised keys: `title`, `summary`,
/// `description` (alias for summary), `keywords`, `tags` (alias for keywords).
fn push_yaml_front_matter_metadata(fm: &str, doc: &mut IRDocument) {
    for line in fm.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        let canonical_key = match key.as_str() {
            "title" => "title",
            "summary" | "description" => "summary",
            "keywords" | "tags" => "keywords",
            _ => continue, // ignore unrecognised keys
        };
        doc.push(DocNode::Metadata {
            key: canonical_key.to_string(),
            value: value.to_string(),
        });
    }
}

/// Extracts `<title>` and `<meta name="description" content="…">` from raw
/// HTML and pushes them as `DocNode::Metadata` nodes.
///
/// Uses simple byte-level scanning — not a full HTML parser — which is
/// sufficient for the well-formed subset produced by real-world pages.
fn extract_html_metadata(html: &str, doc: &mut IRDocument) {
    let lower = html.to_ascii_lowercase();

    // ── <title>…</title> ────────────────────────────────────────────────────
    if let Some(start) = lower.find("<title>") {
        let after = start + "<title>".len();
        if let Some(end) = lower[after..].find("</title>") {
            let title = html[after..after + end].trim();
            if !title.is_empty() {
                doc.push(DocNode::Metadata {
                    key: "title".to_string(),
                    value: title.to_string(),
                });
            }
        }
    }

    // ── <meta name="description" content="…"> ───────────────────────────────
    // Scan all <meta …> tags and look for name="description".
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find("<meta") {
        let tag_start = search_from + rel;
        let tag_end = lower[tag_start..]
            .find('>')
            .map(|e| tag_start + e + 1)
            .unwrap_or(lower.len());
        let tag_lower = &lower[tag_start..tag_end];
        let tag_raw = &html[tag_start..tag_end];

        if tag_lower.contains("name=\"description\"") || tag_lower.contains("name='description'") {
            if let Some(content) = extract_attr_value(tag_raw, "content") {
                let content = content.trim();
                if !content.is_empty() {
                    doc.push(DocNode::Metadata {
                        key: "summary".to_string(),
                        value: content.to_string(),
                    });
                }
            }
        }

        search_from = tag_end;
        if search_from >= lower.len() {
            break;
        }
    }
}

/// Extracts the value of `attr="…"` or `attr='…'` from a tag string.
fn extract_attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let needle_dq = format!("{}=\"", attr);
    let needle_sq = format!("{}='", attr);

    if let Some(pos) = lower.find(&needle_dq) {
        let value_start = pos + needle_dq.len();
        let value_end = tag[value_start..].find('"').map(|e| value_start + e)?;
        return Some(&tag[value_start..value_end]);
    }
    if let Some(pos) = lower.find(&needle_sq) {
        let value_start = pos + needle_sq.len();
        let value_end = tag[value_start..].find('\'').map(|e| value_start + e)?;
        return Some(&tag[value_start..value_end]);
    }
    None
}

// ────────────────────────────────────────────────
// Importance scoring
// ────────────────────────────────────────────────

/// Computes the importance score for a paragraph.
///
/// # Parameters
/// - `para_idx`          — 0-based index among all paragraphs seen so far
/// - `char_count`        — length of the paragraph text in Unicode scalar values
/// - `just_after_heading`— `true` when the preceding node was a heading
///
/// # Scoring
/// Score = position_score × 0.5 + length_score × 0.4 + heading_bonus × 0.1,
/// clamped to `[0.1, 1.0]`.
fn calc_importance(para_idx: usize, char_count: usize, just_after_heading: bool) -> f32 {
    // ── Position score: first paragraphs are more important ──────────────
    // Inspired by the "inverted pyramid" principle in journalism.
    let position_score: f32 = match para_idx {
        0 => 1.00,
        1 => 0.95,
        2 => 0.90,
        3..=5 => 0.80,
        6..=10 => 0.65,
        _ => 0.50,
    };

    // ── Length score: very short paragraphs are often captions/footnotes ─
    let length_score: f32 = match char_count {
        0..=15 => 0.30,
        16..=40 => 0.55,
        41..=80 => 0.75,
        81..=200 => 0.90,
        _ => 1.00,
    };

    // ── Heading proximity bonus ───────────────────────────────────────────
    // A paragraph immediately after a heading introduces the section topic.
    let heading_bonus: f32 = if just_after_heading { 1.0 } else { 0.0 };

    // Weighted blend (weights sum to 1.0 when heading_bonus is active)
    let score = position_score * 0.5 + length_score * 0.4 + heading_bonus * 0.1;
    score.clamp(0.1, 1.0)
}

// ────────────────────────────────────────────────
// Markdown parser (pulldown-cmark)
// ────────────────────────────────────────────────

fn parse_markdown(input: &str, doc: &mut IRDocument) {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(input, opts);

    let mut current_text = String::new();
    let mut current_heading: Option<u8> = None;
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;
    let mut code_body = String::new();
    let mut in_list = false;
    let mut list_ordered = false;
    let mut list_items: Vec<String> = Vec::new();
    let mut current_list_item = String::new();
    // Table state
    let mut in_table = false;
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_table_head = false;
    // Importance tracking
    let mut para_idx: usize = 0;
    let mut just_after_heading: bool = true; // treat the very first paragraph as post-heading

    for event in parser {
        match event {
            // ── Heading ─────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some(heading_level_to_u8(level));
                current_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = current_heading.take() {
                    doc.push(DocNode::Header {
                        level,
                        text: current_text.trim().to_string(),
                    });
                    current_text.clear();
                    just_after_heading = true; // next paragraph is a section intro
                }
            }

            // ── Paragraph ───────────────────────
            Event::Start(Tag::Paragraph) => {
                current_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                let text = current_text.trim().to_string();
                if !text.is_empty() {
                    let importance =
                        calc_importance(para_idx, text.chars().count(), just_after_heading);
                    doc.push(DocNode::Para { text, importance });
                    para_idx += 1;
                    just_after_heading = false;
                }
                current_text.clear();
            }

            // ── Code block ──────────────────────
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string();
                        if l.is_empty() { None } else { Some(l) }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                code_body.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                doc.push(DocNode::Code {
                    lang: code_lang.take(),
                    body: code_body.trim().to_string(),
                });
                code_body.clear();
            }

            // ── List ────────────────────────────
            Event::Start(Tag::List(num)) => {
                in_list = true;
                list_ordered = num.is_some();
                list_items.clear();
            }
            Event::End(TagEnd::List(_)) => {
                in_list = false;
                if !list_items.is_empty() {
                    doc.push(DocNode::List {
                        ordered: list_ordered,
                        items: std::mem::take(&mut list_items),
                    });
                }
            }
            Event::Start(Tag::Item) => {
                current_list_item.clear();
            }
            Event::End(TagEnd::Item) => {
                let item = current_list_item.trim().to_string();
                if !item.is_empty() {
                    list_items.push(item);
                }
                current_list_item.clear();
            }

            // ── Table ───────────────────────────
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_headers.clear();
                table_rows.clear();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                if !table_headers.is_empty() {
                    doc.push(DocNode::Table {
                        headers: std::mem::take(&mut table_headers),
                        rows: std::mem::take(&mut table_rows),
                    });
                }
            }
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
            }
            Event::End(TagEnd::TableHead) => {
                in_table_head = false;
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) if !in_table_head => {
                table_rows.push(std::mem::take(&mut current_row));
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                let cell = current_cell.trim().to_string();
                if in_table_head {
                    table_headers.push(cell);
                } else {
                    current_row.push(cell);
                }
                current_cell.clear();
            }

            // ── Text ────────────────────────────
            Event::Text(text) | Event::Code(text) => {
                let s = text.as_ref();
                if in_code_block {
                    code_body.push_str(s);
                } else if in_table {
                    current_cell.push_str(s);
                } else if in_list {
                    current_list_item.push_str(s);
                } else {
                    current_text.push_str(s);
                }
            }

            Event::SoftBreak | Event::HardBreak if !in_code_block => {
                current_text.push(' ');
            }

            _ => {}
        }
    }
}

// ────────────────────────────────────────────────
// PlainText parser
// ────────────────────────────────────────────────

fn parse_plaintext(input: &str, doc: &mut IRDocument) {
    let mut para_idx: usize = 0;
    let mut just_after_heading: bool = true; // treat the very first paragraph as post-heading

    // Split paragraphs on blank lines
    for para in input.split("\n\n") {
        let text = para.trim();
        if text.is_empty() {
            continue;
        }
        // Only H1 and H2 are recognized in plain-text mode. H3+ markers are treated as
        // paragraph text — plain-text inputs rarely use deep heading hierarchies and this
        // keeps the parser simple.
        if let Some(stripped) = text.strip_prefix("# ") {
            doc.push(DocNode::Header {
                level: 1,
                text: stripped.to_string(),
            });
            just_after_heading = true;
        } else if let Some(stripped) = text.strip_prefix("## ") {
            doc.push(DocNode::Header {
                level: 2,
                text: stripped.to_string(),
            });
            just_after_heading = true;
        } else {
            let body = text.replace('\n', " ");
            let importance = calc_importance(para_idx, body.chars().count(), just_after_heading);
            doc.push(DocNode::Para {
                text: body,
                importance,
            });
            para_idx += 1;
            just_after_heading = false;
        }
    }
}

// ────────────────────────────────────────────────
// HTML tag stripping (safe parsing via ammonia)
// ────────────────────────────────────────────────

/// HTML block-level and line-break elements whose removal must insert a space
/// so that adjacent text nodes are not merged without a separator.
///
/// Without this, `<p>foo</p><p>bar</p>` would become `"foobar"` after tag
/// removal, losing the word boundary.  We replace each matching opening or
/// closing tag with a single space *before* handing the sanitised string to
/// ammonia, which then strips the remaining inline tags cleanly.
const BLOCK_ELEMENTS: &[&str] = &[
    "p", "div", "section", "article", "aside", "header", "footer", "main", "nav",
    "h1", "h2", "h3", "h4", "h5", "h6",
    "li", "dt", "dd", "blockquote", "pre", "figure", "figcaption",
    "table", "thead", "tbody", "tfoot", "tr", "th", "td",
    "br", "hr",
];

fn strip_html_tags(input: &str) -> String {
    // Phase 1: replace block/line-break element tags with a single space so
    // that adjacent text content is not concatenated without a separator.
    // This is a lightweight, regex-free approach: scan for `<` and check the
    // tag name against the block list.
    let spaced = insert_block_element_spaces(input);

    // Phase 2: use ammonia with an empty allowed-tag set to safely remove all
    // remaining tags and decode HTML entities.
    // Unlike a regex approach, this correctly handles nested tags, comments,
    // and potentially malicious HTML.
    ammonia::Builder::new()
        .tags(std::collections::HashSet::new())
        .clean(&spaced)
        .to_string()
}

/// Replaces opening and closing tags of known block-level / line-break elements
/// with a single space character.  All other tags are left intact for ammonia
/// to remove in the second phase.
fn insert_block_element_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'<' {
            // Find the end of this tag.
            if let Some(close) = memchr_naive(bytes, i + 1, b'>') {
                let tag_content = &input[i + 1..close]; // content between < and >
                let tag_content = tag_content.trim();

                // Strip leading '/' for closing tags.
                let name_part = tag_content.strip_prefix('/').unwrap_or(tag_content);
                // Tag name ends at the first whitespace or '/'.
                let tag_name = name_part
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();

                if BLOCK_ELEMENTS.contains(&tag_name.as_str()) {
                    // Replace the entire tag with a single space.
                    out.push(' ');
                    i = close + 1;
                    continue;
                }
            }
            // Not a recognised block element — emit as-is for ammonia.
            out.push('<');
            i += 1;
        } else {
            out.push(input[i..].chars().next().unwrap_or_default());
            // Advance by the byte length of the char, not just 1.
            let ch = input[i..].chars().next().unwrap_or_default();
            i += ch.len_utf8();
        }
    }

    out
}

/// Minimal forward scan for a target byte — avoids pulling in `memchr` crate.
#[inline]
fn memchr_naive(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    bytes[start..].iter().position(|&b| b == target).map(|p| start + p)
}

// ────────────────────────────────────────────────
// Internal utilities
// ────────────────────────────────────────────────

fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel as HL;
    match level {
        HL::H1 => 1,
        HL::H2 => 2,
        HL::H3 => 3,
        HL::H4 => 4,
        HL::H5 => 5,
        HL::H6 => 6,
    }
}

// ────────────────────────────────────────────────
// Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_heading_parsed() {
        let md = "# 제목1\n\n## 제목2";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let headers: Vec<_> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Header { .. }))
            .collect();
        assert_eq!(headers.len(), 2);
        if let DocNode::Header { level, text } = &headers[0] {
            assert_eq!(*level, 1);
            assert_eq!(text, "제목1");
        }
    }

    #[test]
    fn markdown_para_parsed() {
        let md = "첫 번째 단락입니다.\n\n두 번째 단락입니다.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let paras: Vec<_> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Para { .. }))
            .collect();
        assert_eq!(paras.len(), 2);
    }

    #[test]
    fn markdown_table_parsed() {
        let md = "| 이름 | 나이 |\n|------|------|\n| 홍길동 | 30 |";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let tables: Vec<_> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Table { .. }))
            .collect();
        assert_eq!(tables.len(), 1);
        if let DocNode::Table { headers, rows } = &tables[0] {
            assert_eq!(headers[0].trim(), "이름");
            assert_eq!(rows[0][0].trim(), "홍길동");
        }
    }

    #[test]
    fn markdown_list_parsed() {
        let md = "- 항목1\n- 항목2\n- 항목3";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let lists: Vec<_> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::List { .. }))
            .collect();
        assert_eq!(lists.len(), 1);
        if let DocNode::List { ordered, items } = &lists[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 3);
        }
    }

    #[test]
    fn plaintext_para_split() {
        let text = "첫 문단\n\n두 번째 문단";
        let doc = parse(text, InputFormat::PlainText, FidelityLevel::Semantic, None).unwrap();
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn strip_html_tags_removes_tags() {
        let result1 = strip_html_tags("<b>hello</b> world");
        let result2 = strip_html_tags("<p>foo</p><br/>bar");
        assert!(result1.contains("hello") && result1.contains("world"));
        assert!(result2.contains("foo") && result2.contains("bar"));
    }

    #[test]
    fn html_tags_stripped() {
        let html = "<h1>제목</h1><p>본문 내용</p>";
        let doc = parse(html, InputFormat::Html, FidelityLevel::Semantic, None).unwrap();
        let all_text: String = doc
            .nodes
            .iter()
            .filter_map(|n| {
                if let DocNode::Para { text, .. } = n {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(!all_text.contains('<'), "HTML tags must be stripped");
        assert!(all_text.contains("제목") || all_text.contains("본문"));
    }

    /// First paragraph (idx=0) must have higher importance than a later one (idx=12).
    #[test]
    fn importance_position_decay() {
        let first = calc_importance(0, 120, false);
        let later = calc_importance(12, 120, false);
        assert!(
            first > later,
            "first paragraph ({first}) must be more important than a later one ({later})"
        );
    }

    /// A short paragraph must have lower importance than a long one at the same position.
    #[test]
    fn importance_length_effect() {
        let short = calc_importance(3, 10, false); // 10 chars — caption-length
        let long = calc_importance(3, 300, false); // 300 chars — full paragraph
        assert!(
            long > short,
            "long paragraph ({long}) must be more important than a short one ({short})"
        );
    }

    /// A paragraph just after a heading must score higher than the same paragraph elsewhere.
    #[test]
    fn importance_heading_bonus() {
        let after_heading = calc_importance(5, 80, true);
        let no_heading = calc_importance(5, 80, false);
        assert!(
            after_heading > no_heading,
            "paragraph after heading ({after_heading}) must score higher than \
             the same paragraph without heading context ({no_heading})"
        );
    }

    /// Importance scores must always be within the defined range.
    #[test]
    fn importance_range_invariant() {
        for idx in [0usize, 1, 5, 10, 50] {
            for chars in [5usize, 20, 100, 500] {
                for after in [true, false] {
                    let score = calc_importance(idx, chars, after);
                    assert!(
                        (0.1..=1.0).contains(&score),
                        "importance out of range [{score}] for idx={idx}, chars={chars}, after_heading={after}"
                    );
                }
            }
        }
    }

    /// Paragraph importance values in a multi-paragraph document must be differentiated
    /// (not all equal to 1.0).
    #[test]
    fn markdown_para_importances_are_differentiated() {
        let md = "# Intro\n\n\
                  This is the first paragraph after the heading.\n\n\
                  Second paragraph with moderate content here.\n\n\
                  Third paragraph.\n\n\
                  Fourth paragraph.\n\n\
                  Fifth paragraph with some text.\n\n\
                  Sixth paragraph.\n\n\
                  Seventh paragraph.\n\n\
                  Eighth.\n\n\
                  Ninth paragraph with a few words.\n\n\
                  Tenth paragraph ends the document.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let importances: Vec<f32> = doc
            .nodes
            .iter()
            .filter_map(|n| {
                if let DocNode::Para { importance, .. } = n {
                    Some(*importance)
                } else {
                    None
                }
            })
            .collect();
        assert!(importances.len() >= 3, "expected at least 3 paragraphs");
        let all_same = importances
            .windows(2)
            .all(|w| (w[0] - w[1]).abs() < f32::EPSILON);
        assert!(
            !all_same,
            "paragraph importance scores must be differentiated, got: {importances:?}"
        );
    }

    // ── HTML block-element space insertion ───────────────────────────────────

    #[test]
    fn html_block_elements_produce_word_boundary() {
        // Without the fix, <p>foo</p><p>bar</p> → "foobar" (merged).
        // With the fix it must become "foo bar" (space-separated) after stripping.
        let result = strip_html_tags("<p>foo</p><p>bar</p>");
        assert!(
            result.contains("foo") && result.contains("bar"),
            "both words must survive: {result:?}"
        );
        // Must NOT be "foobar" — there should be a space between them.
        assert!(
            !result.replace(' ', "").starts_with("foobar") || result.contains(' '),
            "block elements must produce a word boundary: {result:?}"
        );
    }

    #[test]
    fn html_br_inserts_space() {
        let result = strip_html_tags("hello<br/>world");
        assert!(
            result.contains("hello") && result.contains("world"),
            "text around <br/> must be preserved: {result:?}"
        );
        // "hello" and "world" must not be directly adjacent.
        assert!(
            !result.contains("helloworld"),
            "<br/> must produce a separator: {result:?}"
        );
    }

    // ── YAML front matter extraction ─────────────────────────────────────────

    #[test]
    fn markdown_yaml_front_matter_title_extracted() {
        let md = "---\ntitle: My Document\n---\n\n# Body heading\n\nContent here.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let title = doc.get_metadata("title");
        assert_eq!(
            title,
            Some("My Document"),
            "YAML front matter title must be extracted as Metadata node"
        );
    }

    #[test]
    fn markdown_yaml_front_matter_summary_extracted() {
        let md = "---\ntitle: Doc\ndescription: A short summary.\n---\n\nBody.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        assert_eq!(doc.get_metadata("summary"), Some("A short summary."));
    }

    #[test]
    fn markdown_yaml_front_matter_body_still_parsed() {
        let md = "---\ntitle: Doc\n---\n\n# Heading\n\nParagraph content.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let has_para = doc
            .nodes
            .iter()
            .any(|n| matches!(n, DocNode::Para { text, .. } if text.contains("Paragraph")));
        assert!(has_para, "body content after front matter must be parsed");
    }

    #[test]
    fn markdown_no_front_matter_unaffected() {
        let md = "# Normal heading\n\nNormal paragraph.";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        // No Metadata nodes expected
        let meta_count = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Metadata { .. }))
            .count();
        assert_eq!(meta_count, 0, "no Metadata nodes without front matter");
    }

    // ── HTML metadata extraction ─────────────────────────────────────────────

    #[test]
    fn html_title_extracted_as_metadata() {
        let html = "<html><head><title>페이지 제목</title></head><body><p>본문</p></body></html>";
        let doc = parse(html, InputFormat::Html, FidelityLevel::Semantic, None).unwrap();
        assert_eq!(doc.get_metadata("title"), Some("페이지 제목"));
    }

    #[test]
    fn html_meta_description_extracted_as_summary() {
        let html = r#"<html><head><meta name="description" content="페이지 요약입니다."></head><body><p>본문</p></body></html>"#;
        let doc = parse(html, InputFormat::Html, FidelityLevel::Semantic, None).unwrap();
        assert_eq!(doc.get_metadata("summary"), Some("페이지 요약입니다."));
    }

    // ── transpile() end-to-end: <H> block appears when front matter present ──

    #[test]
    fn transpile_emits_h_block_with_yaml_front_matter() {
        let md = "---\ntitle: 계약서\ndescription: 소프트웨어 라이선스 계약\n---\n\n본문 내용입니다.";
        let output = crate::transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))
            .expect("transpile must succeed");
        assert!(
            output.contains("<H>"),
            "output must contain <H> when front matter is present: {output}"
        );
        assert!(
            output.contains("t: 계약서"),
            "title must appear in <H> block: {output}"
        );
    }
}
