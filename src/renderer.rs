//! renderer.rs — DocNode → 브릿지 포맷 렌더러
//!
//! 최종 출력 포맷:
//! ```text
//! <D>                   ← SymbolDict 전역 사전 (없으면 생략)
//! SymA=전문용어A
//! </D>
//! <H>                   ← YAML 헤더 (title, summary, keywords)
//! t: 문서 제목
//! s: 한줄 요약
//! k: [kw1, kw2]
//! </H>
//! <B>                   ← 본문 (압축·치환 적용)
//! ...
//! </B>
//! ```

use crate::ir::{DocNode, IRDocument};
use crate::symbol::SymbolDict;

// ────────────────────────────────────────────────
// 1. 개별 노드 렌더러
// ────────────────────────────────────────────────

/// 단일 `DocNode`를 브릿지 텍스트로 렌더링한다.
///
/// `dict`가 제공되면 본문 내 등록 용어를 PUA 기호로 치환한다.
pub fn render_node(node: &DocNode, dict: &SymbolDict) -> String {
    match node {
        DocNode::Header { level, text } => {
            let prefix = "#".repeat(*level as usize);
            let encoded = dict.encode_str(text);
            format!("{} {}", prefix, encoded.trim())
        }

        DocNode::Para { text, .. } => {
            // 연속 공백·줄바꿈 최소화
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
            // 메타데이터는 렌더러 수준에서 직접 출력하지 않는다.
            // YAML 헤더 빌더(`build_yaml_header`)가 별도로 처리한다.
            let _ = (key, value);
            String::new()
        }
    }
}

// ────────────────────────────────────────────────
// 2. 테이블 선형화
// ────────────────────────────────────────────────

/// 테이블을 토큰 효율적인 텍스트로 변환한다.
///
/// | 행 수    | 출력 형식                        |
/// |---------|----------------------------------|
/// | ≤ 5     | `Key:Val, Key:Val` 시퀀스        |
/// | > 5     | JSON Lines (1행 = 1 JSON 객체)   |
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
// 3. YAML 헤더 빌더
// ────────────────────────────────────────────────

/// IRDocument의 메타데이터에서 YAML 헤더 블록을 생성한다.
///
/// 출력 예:
/// ```yaml
/// t: 계약서 분석 보고서
/// s: 2024년 체결된 소프트웨어 라이선스 계약의 핵심 조항 요약
/// k: [라이선스, 계약, 소프트웨어]
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
        // "kw1, kw2, kw3" → "[kw1, kw2, kw3]"
        let kws: Vec<&str> = keywords.split(',').map(str::trim).collect();
        lines.push(format!("k: [{}]", kws.join(", ")));
    }
    lines.join("\n")
}

// ────────────────────────────────────────────────
// 4. 전체 문서 렌더러
// ────────────────────────────────────────────────

/// 전체 IRDocument를 브릿지 포맷 문자열로 렌더링한다.
///
/// 출력 구조: `<D>?` + `<H>` + `<B>`
pub fn render_full(doc: &IRDocument, dict: &mut SymbolDict) -> String {
    // ① 본문 먼저 렌더링 (치환 과정에서 사전이 채워진다)
    let body_lines: Vec<String> = doc
        .nodes
        .iter()
        .filter_map(|node| {
            // Metadata는 헤더에서 처리
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

    // ② 전역 사전 블록
    let dict_block = dict.render_dict_header();

    // ③ YAML 헤더
    let yaml_header = build_yaml_header(doc);

    // ④ 조립
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
// 5. 내부 유틸리티
// ────────────────────────────────────────────────

/// 연속된 공백과 줄바꿈을 단일 공백으로 정규화한다.
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
// 6. 단위 테스트
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
        // JSON Lines: 각 줄이 JSON 객체
        for line in out.lines() {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("유효한 JSON");
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
