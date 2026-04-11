//! parser.rs — 입력 포맷 → IRDocument 파서
//!
//! 현재 지원 포맷:
//! - `InputFormat::Markdown`  — pulldown-cmark 기반
//! - `InputFormat::PlainText` — 문단 분리 파서
//! - `InputFormat::Html`      — HTML 태그 제거 후 PlainText 처리

use crate::ir::{DocNode, FidelityLevel, IRDocument};
use crate::InputFormat;

/// 입력 텍스트를 `IRDocument`로 변환한다.
pub fn parse(
    input: &str,
    format: InputFormat,
    fidelity: FidelityLevel,
    budget: Option<usize>,
) -> Result<IRDocument, String> {
    let mut doc = IRDocument::new(fidelity, budget);

    match format {
        InputFormat::Markdown => parse_markdown(input, &mut doc),
        InputFormat::PlainText => parse_plaintext(input, &mut doc),
        InputFormat::Html => {
            // HTML → PlainText 후 단락 파서 위임
            let plain = strip_html_tags(input);
            parse_plaintext(&plain, &mut doc);
        }
    }

    Ok(doc)
}

// ────────────────────────────────────────────────
// Markdown 파서 (pulldown-cmark)
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
    // 테이블 상태
    let mut in_table = false;
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_table_head = false;

    for event in parser {
        match event {
            // ── 제목 ────────────────────────────
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
                }
            }

            // ── 단락 ────────────────────────────
            Event::Start(Tag::Paragraph) => {
                current_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                let text = current_text.trim().to_string();
                if !text.is_empty() {
                    doc.push(DocNode::Para { text, importance: 1.0 });
                }
                current_text.clear();
            }

            // ── 코드 블록 ────────────────────────
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

            // ── 목록 ────────────────────────────
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

            // ── 테이블 ────────────────────────────
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
            Event::Start(Tag::TableHead) => { in_table_head = true; }
            Event::End(TagEnd::TableHead) => { in_table_head = false; }
            Event::Start(Tag::TableRow) => { current_row.clear(); }
            Event::End(TagEnd::TableRow) => {
                if !in_table_head {
                    table_rows.push(std::mem::take(&mut current_row));
                }
            }
            Event::Start(Tag::TableCell) => { current_cell.clear(); }
            Event::End(TagEnd::TableCell) => {
                let cell = current_cell.trim().to_string();
                if in_table_head {
                    table_headers.push(cell);
                } else {
                    current_row.push(cell);
                }
                current_cell.clear();
            }

            // ── 텍스트 ────────────────────────────
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

            Event::SoftBreak | Event::HardBreak => {
                if !in_code_block {
                    current_text.push(' ');
                }
            }

            _ => {}
        }
    }
}

// ────────────────────────────────────────────────
// PlainText 파서
// ────────────────────────────────────────────────

fn parse_plaintext(input: &str, doc: &mut IRDocument) {
    // 빈 줄로 문단을 분리
    for para in input.split("\n\n") {
        let text = para.trim();
        if text.is_empty() {
            continue;
        }
        // '#'으로 시작하면 제목으로 처리
        if let Some(stripped) = text.strip_prefix("# ") {
            doc.push(DocNode::Header { level: 1, text: stripped.to_string() });
        } else if let Some(stripped) = text.strip_prefix("## ") {
            doc.push(DocNode::Header { level: 2, text: stripped.to_string() });
        } else {
            doc.push(DocNode::Para {
                text: text.replace('\n', " "),
                importance: 1.0,
            });
        }
    }
}

// ────────────────────────────────────────────────
// HTML 태그 제거 (ammonia 기반 안전한 파싱)
// ────────────────────────────────────────────────

fn strip_html_tags(input: &str) -> String {
    // ammonia에 빈 허용 태그 집합을 전달하면 모든 태그가 제거되고 엔티티가 디코딩된다.
    // 정규식 방식과 달리 중첩 태그·주석·악성 HTML도 안전하게 처리한다.
    ammonia::Builder::new()
        .tags(std::collections::HashSet::new())
        .clean(input)
        .to_string()
}

// ────────────────────────────────────────────────
// 내부 유틸리티
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
// 단위 테스트
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_heading_parsed() {
        let md = "# 제목1\n\n## 제목2";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let headers: Vec<_> = doc.nodes.iter().filter(|n| matches!(n, DocNode::Header { .. })).collect();
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
        let paras: Vec<_> = doc.nodes.iter().filter(|n| matches!(n, DocNode::Para { .. })).collect();
        assert_eq!(paras.len(), 2);
    }

    #[test]
    fn markdown_table_parsed() {
        let md = "| 이름 | 나이 |\n|------|------|\n| 홍길동 | 30 |";
        let doc = parse(md, InputFormat::Markdown, FidelityLevel::Semantic, None).unwrap();
        let tables: Vec<_> = doc.nodes.iter().filter(|n| matches!(n, DocNode::Table { .. })).collect();
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
        let lists: Vec<_> = doc.nodes.iter().filter(|n| matches!(n, DocNode::List { .. })).collect();
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
        let all_text: String = doc.nodes.iter().filter_map(|n| {
            if let DocNode::Para { text, .. } = n { Some(text.clone()) } else { None }
        }).collect();
        assert!(!all_text.contains('<'), "HTML 태그가 제거되어야 한다");
        assert!(all_text.contains("제목") || all_text.contains("본문"));
    }
}
