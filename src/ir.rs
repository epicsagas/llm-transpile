//! ir.rs — Intermediate Representation
//!
//! A language-neutral internal representation (IR) that holds raw documents
//! before they are converted into the LLM bridge format.
//! Semantic preservation level is controlled explicitly.

// ────────────────────────────────────────────────
// 1. Semantic preservation level
// ────────────────────────────────────────────────

/// The degree of information loss permitted during document conversion.
///
/// Decided once at the top of the pipeline; every subsequent transformation
/// stage consistently respects this constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityLevel {
    /// Audit/legal documents — 100% source preservation, compression forbidden.
    Lossless,
    /// General RAG pipeline — minimal compression at the semantic unit level.
    Semantic,
    /// Summarization pipeline — maximum compression, only key information kept.
    Compressed,
}

impl FidelityLevel {
    /// Returns whether lossy compression is permitted.
    pub fn allows_compression(self) -> bool {
        matches!(self, FidelityLevel::Semantic | FidelityLevel::Compressed)
    }
}

// ────────────────────────────────────────────────
// 2. Document node (DocNode)
// ────────────────────────────────────────────────

/// A semantic unit that makes up a document.
///
/// Produced by the parser and consumed by the renderer, compressor, and symbolizer.
#[derive(Debug, Clone)]
pub enum DocNode {
    /// Heading (H1–H6).
    Header {
        /// Heading level (1–6).
        level: u8,
        text: String,
    },

    /// Regular paragraph.
    Para {
        text: String,
        /// Importance score (0.0 = lowest, 1.0 = highest).
        /// Used by the compressor for priority-based trimming.
        importance: f32,
    },

    /// Table.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },

    /// Code block.
    Code { lang: Option<String>, body: String },

    /// List (ordered or unordered).
    List { ordered: bool, items: Vec<String> },

    /// Key-value metadata (title, summary, keywords, etc.).
    Metadata { key: String, value: String },
}

impl DocNode {
    /// Returns the importance score of the node.
    ///
    /// Nodes other than `Para` have a default importance of 1.0.
    pub fn importance(&self) -> f32 {
        match self {
            DocNode::Para { importance, .. } => *importance,
            _ => 1.0,
        }
    }

    /// Returns the approximate character count of the text held by the node.
    /// Used for pre-filtering against the token budget.
    pub fn char_len(&self) -> usize {
        match self {
            DocNode::Header { text, .. } => text.len(),
            DocNode::Para { text, .. } => text.len(),
            DocNode::Table { headers, rows } => {
                headers.iter().map(|h| h.len()).sum::<usize>()
                    + rows
                        .iter()
                        .flat_map(|r| r.iter())
                        .map(|c| c.len())
                        .sum::<usize>()
            }
            DocNode::Code { body, .. } => body.len(),
            DocNode::List { items, .. } => items.iter().map(|i| i.len()).sum(),
            DocNode::Metadata { key, value } => key.len() + value.len(),
        }
    }
}

// ────────────────────────────────────────────────
// 3. IR document
// ────────────────────────────────────────────────

/// The complete IR representation of a parsed document.
///
/// `fidelity` and `token_budget` act as constraints for every subsequent transformation stage.
#[derive(Debug, Clone)]
pub struct IRDocument {
    /// Semantic preservation level.
    pub fidelity: FidelityLevel,
    /// Sequence of document nodes.
    pub nodes: Vec<DocNode>,
    /// Maximum allowed token count. `None` means unlimited.
    pub token_budget: Option<usize>,
}

impl IRDocument {
    /// Creates a new IR document.
    pub fn new(fidelity: FidelityLevel, token_budget: Option<usize>) -> Self {
        Self {
            fidelity,
            nodes: Vec::new(),
            token_budget,
        }
    }

    /// Appends a node.
    pub fn push(&mut self, node: DocNode) {
        self.nodes.push(node);
    }

    /// Total character count of the document (for pre-validation against the token budget).
    pub fn total_char_len(&self) -> usize {
        self.nodes.iter().map(|n| n.char_len()).sum()
    }

    /// Looks up the value for a specific key from metadata nodes.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.nodes.iter().find_map(|n| {
            if let DocNode::Metadata { key: k, value } = n
                && k == key
            {
                return Some(value.as_str());
            }
            None
        })
    }
}

// ────────────────────────────────────────────────
// 4. Unit tests
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
        let header = DocNode::Header {
            level: 1,
            text: "제목".into(),
        };
        assert_eq!(header.importance(), 1.0);

        let para = DocNode::Para {
            text: "내용".into(),
            importance: 0.3,
        };
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
        // "이름"(6 bytes) + "나이"(6 bytes) + "홍길동"(9 bytes) + "30"(2 bytes) = 23 bytes
        assert_eq!(node.char_len(), 23);
    }
}
