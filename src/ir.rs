//! ir.rs — Intermediate Representation
//!
//! Raw 문서를 LLM 브릿지 포맷으로 변환하기 전에 보관하는
//! 언어 중립적 내부 표현(IR). 의미 보존 레벨을 명시적으로 제어한다.

// ────────────────────────────────────────────────
// 1. 의미 보존 레벨
// ────────────────────────────────────────────────

/// 문서 변환 시 허용되는 정보 손실 수준.
///
/// 파이프라인 최상단에서 결정하면 이후 모든 변환 단계가
/// 해당 제약을 일관되게 따른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityLevel {
    /// 감사·법률 문서 — 원문 100% 보존, 압축 금지.
    Lossless,
    /// 일반 RAG 파이프라인 — 의미 단위로 최소 압축 허용.
    Semantic,
    /// 요약 파이프라인 — 최대 압축, 핵심 정보만 유지.
    Compressed,
}

impl FidelityLevel {
    /// 압축(손실) 변환이 허용되는지 여부 반환.
    pub fn allows_compression(self) -> bool {
        matches!(self, FidelityLevel::Semantic | FidelityLevel::Compressed)
    }
}

// ────────────────────────────────────────────────
// 2. 문서 노드 (DocNode)
// ────────────────────────────────────────────────

/// 문서를 구성하는 의미 단위.
///
/// 파서가 생성하고, 렌더러·압축기·기호화기가 소비한다.
#[derive(Debug, Clone)]
pub enum DocNode {
    /// 제목 (H1–H6).
    Header {
        /// 제목 레벨 (1–6).
        level: u8,
        text: String,
    },

    /// 일반 단락.
    Para {
        text: String,
        /// 중요도 스코어 (0.0 = 가장 낮음, 1.0 = 가장 높음).
        /// 압축기가 우선순위 기반 트리밍에 활용한다.
        importance: f32,
    },

    /// 표.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },

    /// 코드 블록.
    Code {
        lang: Option<String>,
        body: String,
    },

    /// 목록 (순서 있음 / 없음).
    List {
        ordered: bool,
        items: Vec<String>,
    },

    /// 키-값 메타데이터 (제목, 요약, 키워드 등).
    Metadata {
        key: String,
        value: String,
    },
}

impl DocNode {
    /// 노드의 중요도를 반환한다.
    ///
    /// `Para` 이외의 노드는 기본 중요도 1.0을 가진다.
    pub fn importance(&self) -> f32 {
        match self {
            DocNode::Para { importance, .. } => *importance,
            _ => 1.0,
        }
    }

    /// 노드가 보유한 텍스트의 대략적인 문자 수를 반환한다.
    /// 토큰 예산 사전 필터링에 활용된다.
    pub fn char_len(&self) -> usize {
        match self {
            DocNode::Header { text, .. } => text.len(),
            DocNode::Para { text, .. } => text.len(),
            DocNode::Table { headers, rows } => {
                headers.iter().map(|h| h.len()).sum::<usize>()
                    + rows.iter().flat_map(|r| r.iter()).map(|c| c.len()).sum::<usize>()
            }
            DocNode::Code { body, .. } => body.len(),
            DocNode::List { items, .. } => items.iter().map(|i| i.len()).sum(),
            DocNode::Metadata { key, value } => key.len() + value.len(),
        }
    }
}

// ────────────────────────────────────────────────
// 3. IR 문서
// ────────────────────────────────────────────────

/// 파싱된 문서의 전체 IR 표현.
///
/// `fidelity`와 `token_budget`은 이후 모든 변환 단계의 제약으로 작용한다.
#[derive(Debug, Clone)]
pub struct IRDocument {
    /// 의미 보존 레벨.
    pub fidelity: FidelityLevel,
    /// 문서 노드 시퀀스.
    pub nodes: Vec<DocNode>,
    /// 최대 허용 토큰 수. `None`이면 무제한.
    pub token_budget: Option<usize>,
}

impl IRDocument {
    /// 새 IR 문서를 생성한다.
    pub fn new(fidelity: FidelityLevel, token_budget: Option<usize>) -> Self {
        Self {
            fidelity,
            nodes: Vec::new(),
            token_budget,
        }
    }

    /// 노드를 추가한다.
    pub fn push(&mut self, node: DocNode) {
        self.nodes.push(node);
    }

    /// 문서의 전체 문자 수 (토큰 예산 사전 검증용).
    pub fn total_char_len(&self) -> usize {
        self.nodes.iter().map(|n| n.char_len()).sum()
    }

    /// 메타데이터 노드에서 특정 키의 값을 조회한다.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.nodes.iter().find_map(|n| {
            if let DocNode::Metadata { key: k, value } = n {
                if k == key {
                    return Some(value.as_str());
                }
            }
            None
        })
    }
}

// ────────────────────────────────────────────────
// 4. 단위 테스트
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fidelity_compression_flag() {
        assert!(!FidelityLevel::Lossless.allows_compression());
        assert!(FidelityLevel::Semantic.allows_compression());
        assert!(FidelityLevel::Compressed.allows_compression());
    }

    #[test]
    fn doc_node_importance_defaults() {
        let header = DocNode::Header { level: 1, text: "제목".into() };
        assert_eq!(header.importance(), 1.0);

        let para = DocNode::Para { text: "내용".into(), importance: 0.3 };
        assert_eq!(para.importance(), 0.3);
    }

    #[test]
    fn ir_document_metadata_lookup() {
        let mut doc = IRDocument::new(FidelityLevel::Semantic, Some(4096));
        doc.push(DocNode::Metadata {
            key: "title".into(),
            value: "테스트 문서".into(),
        });
        assert_eq!(doc.get_metadata("title"), Some("테스트 문서"));
        assert_eq!(doc.get_metadata("missing"), None);
    }

    #[test]
    fn table_char_len() {
        let node = DocNode::Table {
            headers: vec!["이름".into(), "나이".into()],
            rows: vec![vec!["홍길동".into(), "30".into()]],
        };
        // "이름"(6) + "나이"(6) + "홍길동"(9) + "30"(2) = 23
        assert_eq!(node.char_len(), 23);
    }
}
