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

        DocNode::List { ordered, items } => {
            items
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
                .join("\n")
        }

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
        rows.iter()
            .map(|row| {
                let obj: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .zip(row.iter())
                    .map(|(h, v)| {
                        (h.trim().to_string(), serde_json::Value::String(v.trim().to_string()))
                    })
                    .collect();
                serde_json::to_string(&obj).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\n")
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
    let title   = doc.get_metadata("title").unwrap_or("");
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
    // ① Render body first (the dictionary is populated during substitution)
    let body_lines: Vec<String> = doc
        .nodes
        .iter()
        .filter_map(|node| {
            // Metadata is handled by the header builder
            if matches!(node, crate::ir::DocNode::Metadata { .. }) {
                return None;
            }
            let rendered = render_node(node, dict);
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
        let node = DocNode::Header { level: 2, text: "제목".into() };
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
    fn table_large_jsonl_format() {
        let headers = vec!["id".into(), "val".into()];
        let rows: Vec<Vec<String>> = (0..6)
            .map(|i| vec![i.to_string(), format!("v{}", i)])
            .collect();
        let out = linearize_table(&headers, &rows);
        // JSON Lines: each line is a JSON object
        for line in out.lines() {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            assert!(parsed.get("id").is_some());
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
        doc.push(DocNode::Metadata { key: "title".into(),   value: "테스트".into() });
        doc.push(DocNode::Metadata { key: "summary".into(), value: "요약".into() });
        doc.push(DocNode::Para { text: "본문 내용".into(), importance: 1.0 });

        let mut dict = SymbolDict::new();
        let output = render_full(&doc, &mut dict);

        assert!(output.contains("<H>"));
        assert!(output.contains("t: 테스트"));
        assert!(output.contains("<B>"));
        assert!(output.contains("본문 내용"));
    }
}
