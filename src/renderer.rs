//! renderer.rs — DocNode → bridge format renderer
//!
//! Final output format:
//! ```text
//! <D>                   ← SymbolDict global dictionary (omitted if empty)
//! SymA=TermA
//! </D>
//! <H>                   ← YAML header (title, summary, keywords)
//! t: document title
//! s: one-line summary
//! k: [kw1, kw2]
//! </H>
//! <B>                   ← body (compression + substitution applied)
//! ...
//! </B>
//! ```

use crate::ir::{DocNode, IRDocument};
use crate::symbol::SymbolDict;

// ────────────────────────────────────────────────
// 1. Individual node renderer
// ────────────────────────────────────────────────

/// Renders a single `DocNode` as bridge text.
///
/// If `dict` is provided, registered terms in the body are replaced with PUA symbols.
pub fn render_node(node: &DocNode, dict: &SymbolDict) -> String {
    match node {
        DocNode::Header { level, text } => {
            let prefix = "#".repeat(*level as usize);
            let encoded = dict.encode_str(text);
            format!("{} {}", prefix, encoded.trim())
        }

        DocNode::Para { text, .. } => {
            // Minimize consecutive whitespace and newlines
            let normalized = normalize_whitespace(text);
            dict.encode_str(&normalized)
        }

        DocNode::Table { headers, rows } => linearize_table(headers, rows),

        DocNode::Code { lang, body } => {
            let lang_tag = lang.as_deref().unwrap_or("");
            format!("```{}\n{}\n```", lang_tag, body.trim())
        }

        DocNode::List { ordered, items } => items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let encoded = dict.encode_str(item);
                if *ordered {
                    format!("{}. {}", i + 1, encoded.trim())
                } else {
                    format!("- {}", encoded.trim())
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),

        DocNode::Metadata { key, value } => {
            // Metadata is not emitted directly at the renderer level.
            // The YAML header builder (`build_yaml_header`) handles it separately.
            let _ = (key, value);
            String::new()
        }
    }
}

// ────────────────────────────────────────────────
// 2. Table linearization
// ────────────────────────────────────────────────

/// Converts a table into token-efficient text.
///
/// | Row count | Output format                       |
/// |-----------|-------------------------------------|
/// | ≤ 5       | `Key:Val, Key:Val` sequence         |
/// | > 5       | JSON Lines (1 row = 1 JSON object)  |
pub fn linearize_table(headers: &[String], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    if rows.len() <= 5 {
        rows.iter()
            .enumerate()
            .map(|(i, row)| {
                let pairs: Vec<String> = headers
                    .iter()
                    .zip(row.iter())
                    .map(|(h, v)| format!("{}:{}", h.trim(), v.trim()))
                    .collect();
                format!("[{}] {}", i + 1, pairs.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Compact pipe-separated format — significantly fewer tokens than JSON Lines.
        // Format: header row first, then one data row per line.
        // Example: `Name|Age\nAlice|30\nBob|25`
        let header_row = headers
            .iter()
            .map(|h| h.trim())
            .collect::<Vec<_>>()
            .join("|");
        let data_rows = rows
            .iter()
            .map(|row| row.iter().map(|v| v.trim()).collect::<Vec<_>>().join("|"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{}", header_row, data_rows)
    }
}

// ────────────────────────────────────────────────
// 3. YAML header builder
// ────────────────────────────────────────────────

/// Builds a YAML header block from the IRDocument's metadata.
///
/// Example output:
/// ```yaml
/// t: Contract Analysis Report
/// s: Summary of key clauses in the software license agreement signed in 2024
/// k: [license, contract, software]
/// ```
pub fn build_yaml_header(doc: &IRDocument) -> String {
    let title = doc.get_metadata("title").unwrap_or("");
    let summary = doc.get_metadata("summary").unwrap_or("");
    let keywords = doc.get_metadata("keywords").unwrap_or("");

    let mut lines = Vec::new();
    if !title.is_empty() {
        lines.push(format!("t: {}", title.trim()));
    }
    if !summary.is_empty() {
        lines.push(format!("s: {}", summary.trim()));
    }
    if !keywords.is_empty() {
        // "kw1, kw2, kw3" → "[kw1, kw2, kw3]"  (wrap in brackets)
        let kws: Vec<&str> = keywords.split(',').map(str::trim).collect();
        lines.push(format!("k: [{}]", kws.join(", ")));
    }
    lines.join("\n")
}

// ────────────────────────────────────────────────
// 4. Full document renderer
// ────────────────────────────────────────────────

/// Renders an entire IRDocument as a bridge-format string.
///
/// Output structure: `<D>?` + `<H>` + `<B>`
pub fn render_full(doc: &IRDocument, dict: &mut SymbolDict) -> String {
    let allows_compression = doc.fidelity.allows_compression();
    let mut last_header_text: Option<String> = None;

    // ① Render body first (the dictionary is populated during substitution)
    let body_lines: Vec<String> = doc
        .nodes
        .iter()
        .filter_map(|node| {
            // Metadata is handled by the header builder
            if matches!(node, crate::ir::DocNode::Metadata { .. }) {
                return None;
            }

            // Track header text for R5 dedup
            if let DocNode::Header { text, .. } = node {
                last_header_text = Some(text.clone());
            }

            // R6: Compress code blocks when fidelity allows it
            let rendered = match node {
                DocNode::Code { lang, body } if allows_compression => {
                    let compressed = compress_code_block(body, lang.as_deref());
                    let lang_tag = lang.as_deref().unwrap_or("");
                    format!("```{}\n{}\n```", lang_tag, compressed.trim())
                }
                _ => render_node(node, dict),
            };

            // R5: Strip duplicate header text from immediately following paragraph
            let rendered = if allows_compression {
                if let DocNode::Para { .. } = node {
                    if let Some(ref header_text) = last_header_text.take() {
                        strip_leading_header_duplicate(&rendered, header_text)
                    } else {
                        rendered
                    }
                } else {
                    rendered
                }
            } else {
                rendered
            };

            // Reset header tracking for non-header, non-para nodes
            if !matches!(node, DocNode::Header { .. } | DocNode::Para { .. }) {
                last_header_text = None;
            }

            if rendered.is_empty() {
                None
            } else {
                Some(rendered)
            }
        })
        .collect();
    let body = body_lines.join("\n");

    // ② Global dictionary block
    let dict_block = dict.render_dict_header();

    // ③ YAML header
    let yaml_header = build_yaml_header(doc);

    // ④ Assemble output
    let mut output = String::new();
    if !dict_block.is_empty() {
        output.push_str(&dict_block);
    }
    if !yaml_header.is_empty() {
        output.push_str("<H>\n");
        output.push_str(yaml_header.trim());
        output.push_str("\n</H>\n");
    }
    output.push_str("<B>\n");
    output.push_str(body.trim());
    output.push_str("\n</B>");

    output
}

// ────────────────────────────────────────────────
// 5. Internal utilities
// ────────────────────────────────────────────────

/// Normalizes consecutive whitespace and newlines to a single space.
fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

/// Strips leading duplicate of `header_text` from `para_text`.
///
/// Only strips if the paragraph starts with the header text followed by a space
/// (word boundary), or is exactly equal to the header text.
fn strip_leading_header_duplicate(para_text: &str, header_text: &str) -> String {
    let header = header_text.trim();
    let para = para_text.trim();

    if header.is_empty() || para.is_empty() {
        return para.to_string();
    }

    // Check if paragraph starts with the header text (using char-based comparison)
    if let Some(rest) = para.strip_prefix(header) {
        // Paragraph is longer -- only strip if followed by whitespace (word boundary)
        if !rest.is_empty() && rest.starts_with(char::is_whitespace) {
            return rest.trim().to_string();
        }
        // Exact match -- paragraph was just the header text repeated
        if rest.is_empty() {
            return String::new();
        }
    }

    para.to_string()
}

/// Strips comments and collapses consecutive blank lines in code blocks.
///
/// Only called when fidelity allows compression.
fn compress_code_block(body: &str, lang: Option<&str>) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut result = Vec::with_capacity(lines.len());
    let mut blank_count = 0usize;

    for line in lines {
        let trimmed = line.trim();

        // Skip single-line comments based on language
        let is_comment = match lang {
            Some(l)
                if [
                    "rust",
                    "go",
                    "java",
                    "javascript",
                    "js",
                    "typescript",
                    "ts",
                    "c",
                    "cpp",
                    "csharp",
                    "cs",
                    "swift",
                    "kotlin",
                    "scala",
                ]
                .contains(&l) =>
            {
                trimmed.starts_with("//")
            }
            Some(l)
                if [
                    "python", "py", "ruby", "rb", "perl", "pl", "r", "shell", "bash", "sh", "yaml",
                    "yml", "toml",
                ]
                .contains(&l) =>
            {
                trimmed.starts_with('#') && !trimmed.starts_with("#!")
            }
            Some(l) if ["sql"].contains(&l) => trimmed.starts_with("--"),
            _ => false, // Unknown language: don't strip comments
        };

        if is_comment {
            continue;
        }

        // Collapse consecutive blank lines to max 1
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                result.push(line);
            }
            continue;
        }

        blank_count = 0;
        result.push(line);
    }

    result.join("\n")
}

// ────────────────────────────────────────────────
// 6. Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DocNode, FidelityLevel, IRDocument};

    fn empty_dict() -> SymbolDict {
        SymbolDict::new()
    }

    #[test]
    fn header_renders_with_hashes() {
        let node = DocNode::Header {
            level: 2,
            text: "제목".into(),
        };
        let out = render_node(&node, &empty_dict());
        assert_eq!(out, "## 제목");
    }

    #[test]
    fn para_whitespace_normalized() {
        let node = DocNode::Para {
            text: "  공백이   많은   문장  ".into(),
            importance: 1.0,
        };
        let out = render_node(&node, &empty_dict());
        assert_eq!(out, "공백이 많은 문장");
    }

    #[test]
    fn table_small_key_val_format() {
        let headers = vec!["이름".into(), "나이".into()];
        let rows = vec![
            vec!["홍길동".into(), "30".into()],
            vec!["이순신".into(), "45".into()],
        ];
        let out = linearize_table(&headers, &rows);
        assert!(out.contains("이름:홍길동"));
        assert!(out.contains("나이:30"));
        assert!(out.contains("[1]"));
        assert!(out.contains("[2]"));
    }

    #[test]
    fn table_large_pipe_format() {
        let headers = vec!["id".into(), "val".into()];
        let rows: Vec<Vec<String>> = (0..6)
            .map(|i| vec![i.to_string(), format!("v{}", i)])
            .collect();
        let out = linearize_table(&headers, &rows);
        // Compact pipe format: header row first, then one data row per line
        let mut lines = out.lines();
        let header_line = lines.next().expect("header row");
        assert_eq!(header_line, "id|val");
        for (i, line) in lines.enumerate() {
            assert_eq!(line, format!("{}|v{}", i, i));
        }
    }

    #[test]
    fn ordered_list_renders_numbers() {
        let node = DocNode::List {
            ordered: true,
            items: vec!["첫째".into(), "둘째".into()],
        };
        let out = render_node(&node, &empty_dict());
        assert!(out.contains("1. 첫째"));
        assert!(out.contains("2. 둘째"));
    }

    #[test]
    fn render_full_structure() {
        let mut doc = IRDocument::new(FidelityLevel::Semantic, None);
        doc.push(DocNode::Metadata {
            key: "title".into(),
            value: "테스트".into(),
        });
        doc.push(DocNode::Metadata {
            key: "summary".into(),
            value: "요약".into(),
        });
        doc.push(DocNode::Para {
            text: "본문 내용".into(),
            importance: 1.0,
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        assert!(output.contains("<H>"));
        assert!(output.contains("t: 테스트"));
        assert!(output.contains("<B>"));
        assert!(output.contains("본문 내용"));
    }

    // ── R5: Header-body duplicate text removal ──

    #[test]
    fn header_body_dedup_removes_duplicate() {
        let mut doc = IRDocument::new(FidelityLevel::Semantic, None);
        doc.push(DocNode::Header {
            level: 2,
            text: "API Endpoints".into(),
        });
        doc.push(DocNode::Para {
            text: "API Endpoints are used for communication.".into(),
            importance: 1.0,
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        let body_section = output
            .split("<B>")
            .nth(1)
            .unwrap()
            .split("</B>")
            .next()
            .unwrap();
        // The paragraph line (after the header line) should not start with "API Endpoints"
        // because it duplicates the header text.
        // Body lines: ["", "## API Endpoints", "API Endpoints are used for communication.", ""]
        let para_line = body_section.lines().nth(2).unwrap_or("");
        assert!(
            !para_line.starts_with("API Endpoints"),
            "paragraph should not repeat header text: {para_line} (body: {body_section})"
        );
        assert!(
            body_section.contains("are used for communication"),
            "rest of paragraph must be preserved: {body_section}"
        );
    }

    #[test]
    fn header_body_dedup_lossless_preserves() {
        let mut doc = IRDocument::new(FidelityLevel::Lossless, None);
        doc.push(DocNode::Header {
            level: 2,
            text: "API Endpoints".into(),
        });
        doc.push(DocNode::Para {
            text: "API Endpoints are used for communication.".into(),
            importance: 1.0,
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        let body_section = output
            .split("<B>")
            .nth(1)
            .unwrap()
            .split("</B>")
            .next()
            .unwrap();
        assert!(
            body_section.contains("API Endpoints"),
            "Lossless mode must preserve all text: {body_section}"
        );
    }

    // ── R6: Code block compression ──

    #[test]
    fn code_block_strips_comments_in_semantic() {
        let mut doc = IRDocument::new(FidelityLevel::Semantic, None);
        doc.push(DocNode::Code {
            lang: Some("rust".into()),
            body: "// This is a comment\nfn main() {}\n// Another comment\n".into(),
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        assert!(
            !output.contains("This is a comment"),
            "comments should be stripped in Semantic mode: {output}"
        );
        assert!(
            output.contains("fn main()"),
            "code must be preserved: {output}"
        );
    }

    #[test]
    fn code_block_preserves_comments_in_lossless() {
        let mut doc = IRDocument::new(FidelityLevel::Lossless, None);
        doc.push(DocNode::Code {
            lang: Some("rust".into()),
            body: "// This is a comment\nfn main() {}".into(),
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        assert!(
            output.contains("This is a comment"),
            "Lossless mode must preserve comments: {output}"
        );
    }

    #[test]
    fn code_block_collapse_blank_lines() {
        let mut doc = IRDocument::new(FidelityLevel::Semantic, None);
        doc.push(DocNode::Code {
            lang: Some("python".into()),
            body: "def foo():\n\n\n    pass\n\n\ndef bar():\n    pass".into(),
        });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        assert!(
            !output.contains("\n\n\n"),
            "consecutive blank lines should be collapsed: {output}"
        );
    }
}
