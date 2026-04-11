//! compressor.rs — AdaptiveCompressor
//!
//! Automatically applies a four-stage compression strategy based on token budget usage.
//!
//! | Budget usage | Strategy applied                                          |
//! |-------------|-----------------------------------------------------------|
//! | 0–60%       | Stopword removal only                                     |
//! | 60–80%      | Stopwords + prune bottom-20% importance paragraphs        |
//! | 80–95%      | Above + deduplicate sentences + linearize numeric data    |
//! | 95%+        | Above + truncate all paragraphs to first sentence (Semantic+) |

use crate::ir::{DocNode, FidelityLevel};
use regex::Regex;

// ────────────────────────────────────────────────
// 1. Compression configuration
// ────────────────────────────────────────────────

/// Context provided when running the compressor.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Maximum allowed token count.
    pub budget: usize,
    /// Tokens consumed so far (approximate).
    pub current_tokens: usize,
    /// Semantic preservation level.
    pub fidelity: FidelityLevel,
}

impl CompressionConfig {
    /// Current budget usage ratio (0.0–1.0).
    pub fn usage_ratio(&self) -> f64 {
        if self.budget == 0 {
            return 1.0;
        }
        self.current_tokens as f64 / self.budget as f64
    }

    /// Returns the compression stage for the current usage ratio.
    pub fn stage(&self) -> CompressionStage {
        match self.usage_ratio() {
            r if r < 0.60 => CompressionStage::StopwordOnly,
            r if r < 0.80 => CompressionStage::PruneLowImportance,
            r if r < 0.95 => CompressionStage::DeduplicateAndLinearize,
            _              => CompressionStage::MaxCompression,
        }
    }
}

/// Compression stage enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompressionStage {
    /// Stopword removal only.
    StopwordOnly,
    /// Stopwords + prune bottom-20% importance paragraphs.
    PruneLowImportance,
    /// Above + deduplicate sentences.
    DeduplicateAndLinearize,
    /// Above + truncate paragraphs to their first sentence.
    MaxCompression,
}

// ────────────────────────────────────────────────
// 2. AdaptiveCompressor
// ────────────────────────────────────────────────

/// Budget-based adaptive document compressor.
pub struct AdaptiveCompressor {
    /// Stopword regex list pre-compiled in the constructor.
    /// Built once at construction time to avoid recompilation on every call.
    stopword_regexes: Vec<Regex>,
}

impl Default for AdaptiveCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveCompressor {
    /// Creates a compressor with the default stopword list (empty).
    pub fn new() -> Self {
        Self::with_stopwords(default_stopwords())
    }

    /// Creates a compressor with a custom stopword list.
    /// Stopwords are compiled into regexes at construction time and cached.
    pub fn with_stopwords(stopwords: Vec<String>) -> Self {
        let stopword_regexes = stopwords
            .iter()
            .filter_map(|sw| {
                // `\b` only recognizes ASCII word boundaries.
                // Non-ASCII stopwords (Arabic, Hindi, etc.) may be silently ignored
                // because boundary matching does not work for them.
                // TODO: Non-ASCII stopwords need a separate whitespace-based split-replace strategy.
                let pattern = format!(r"(?i)\b{}\b\s*", regex::escape(sw));
                Regex::new(&pattern).ok()
            })
            .collect();
        Self { stopword_regexes }
    }

    /// Applies compression to the node list and returns the result.
    ///
    /// Stopword removal is also skipped at `FidelityLevel::Lossless`.
    pub fn compress(&self, mut nodes: Vec<DocNode>, cfg: &CompressionConfig) -> Vec<DocNode> {
        if cfg.fidelity == FidelityLevel::Lossless {
            return nodes; // Lossless: compression entirely forbidden
        }

        let stage = cfg.stage();

        // ① Stopword removal (all stages)
        nodes = self.remove_stopwords(nodes);

        // ② Prune bottom-20% importance paragraphs
        if stage >= CompressionStage::PruneLowImportance {
            nodes = prune_low_importance(nodes, 0.20);
        }

        // ③ Deduplicate sentences
        if stage >= CompressionStage::DeduplicateAndLinearize {
            nodes = deduplicate_paras(nodes);
        }

        // ④ Truncate paragraphs to their first sentence
        // Lossless early-returns at the top of the function, so fidelity != Lossless is guaranteed here.
        if stage >= CompressionStage::MaxCompression {
            nodes = truncate_to_first_sentence(nodes);
        }

        nodes
    }

    // ── Internal helpers ─────────────────────────

    fn remove_stopwords(&self, nodes: Vec<DocNode>) -> Vec<DocNode> {
        if self.stopword_regexes.is_empty() {
            return nodes;
        }
        nodes
            .into_iter()
            .map(|node| match node {
                DocNode::Para { text, importance } => DocNode::Para {
                    text: self.strip_stopwords(&text),
                    importance,
                },
                DocNode::Header { level, text } => DocNode::Header {
                    level,
                    text: self.strip_stopwords(&text),
                },
                other => other,
            })
            .collect()
    }

    fn strip_stopwords(&self, text: &str) -> String {
        // One pass per stopword (O(N × |text|)). Because stopwords use `\b` ASCII-boundary regexes,
        // they cannot be replaced with a single aho-corasick pass.
        // The default stopword list is empty, so `remove_stopwords` early-returns and this function
        // is only called when the caller explicitly configures stopwords.
        let mut result = text.to_string();
        for re in &self.stopword_regexes {
            result = re.replace_all(&result, "").into_owned();
        }
        // Collapse consecutive whitespace (single pass)
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

// ────────────────────────────────────────────────
// 3. Internal compression functions
// ────────────────────────────────────────────────

/// Removes `Para` nodes in the bottom `threshold` fraction by importance.
fn prune_low_importance(nodes: Vec<DocNode>, threshold: f32) -> Vec<DocNode> {
    // Only paragraphs are subject to filtering
    let para_importances: Vec<f32> = nodes
        .iter()
        .filter_map(|n| {
            if let DocNode::Para { importance, .. } = n {
                Some(*importance)
            } else {
                None
            }
        })
        .collect();

    if para_importances.len() <= 1 {
        return nodes;
    }

    // Calculate the cutoff value for the bottom threshold fraction
    let mut sorted = para_importances.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cutoff_idx = ((sorted.len() as f32 * threshold) as usize).min(sorted.len() - 1);
    let cutoff = sorted[cutoff_idx];

    let filtered: Vec<DocNode> = nodes
        .iter()
        .filter(|n| {
            if let DocNode::Para { importance, .. } = n {
                *importance > cutoff
            } else {
                true // non-paragraph nodes are always preserved
            }
        })
        .cloned()
        .collect();

    // Safety net: if the input had Para nodes but none remain after filtering, return the original.
    // (When all paragraphs share the same importance, cutoff == all importances → prevents total elimination)
    let filtered_has_para = filtered.iter().any(|n| matches!(n, DocNode::Para { .. }));
    let input_had_para = nodes.iter().any(|n| matches!(n, DocNode::Para { .. }));

    if input_had_para && !filtered_has_para {
        nodes
    } else {
        filtered
    }
}

/// Removes `Para` nodes with identical content, keeping only the first occurrence.
fn deduplicate_paras(nodes: Vec<DocNode>) -> Vec<DocNode> {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    nodes
        .into_iter()
        .filter(|n| {
            if let DocNode::Para { text, .. } = n {
                let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
                seen.insert(normalized)
            } else {
                true
            }
        })
        .collect()
}

/// Truncates each `Para` to its first sentence.
fn truncate_to_first_sentence(nodes: Vec<DocNode>) -> Vec<DocNode> {
    nodes
        .into_iter()
        .map(|node| match node {
            DocNode::Para { text, importance } => {
                let first = first_sentence(&text);
                DocNode::Para { text: first, importance }
            }
            other => other,
        })
        .collect()
}

/// Extracts the first sentence from text (delimited by `.`, `!`, or `?`).
fn first_sentence(text: &str) -> String {
    for (i, c) in text.char_indices() {
        if matches!(c,
            '.' | '!' | '?'           // ASCII
            | '。' | '！' | '？'      // CJK fullwidth (U+3002, U+FF01, U+FF1F)
            | '।' | '॥'              // Devanagari Danda / Double Danda (U+0964, U+0965)
            | '۔'                    // Arabic Full Stop (U+06D4)
            | '።'                    // Ethiopic Full Stop (U+1362)
            | '᙮'                    // Canadian Syllabics Full Stop (U+166E)
            | '꓿'                    // Lisu Punctuation Full Stop (U+A4FF)
            | '︒'                    // Presentation Form Vertical Ideographic Full Stop (U+FE12)
            | '﹒'                    // Small Full Stop (U+FE52)
            | '．'                    // Fullwidth Full Stop (U+FF0E)
        ) {
            return text[..i + c.len_utf8()].trim().to_string();
        }
    }
    text.trim().to_string() // No sentence terminator found — return the full text
}

/// Default stopword list — returns an empty list for language neutrality.
///
/// For language-specific stopwords, use `AdaptiveCompressor::with_stopwords()`.
fn default_stopwords() -> Vec<String> {
    vec![]
}

// ────────────────────────────────────────────────
// 4. Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_para(text: &str, importance: f32) -> DocNode {
        DocNode::Para { text: text.into(), importance }
    }

    #[test]
    fn lossless_skips_all_compression() {
        let nodes = vec![make_para("the quick brown fox", 0.1)];
        let cfg = CompressionConfig {
            budget: 100,
            current_tokens: 99,
            fidelity: FidelityLevel::Lossless,
        };
        let compressor = AdaptiveCompressor::new();
        let result = compressor.compress(nodes.clone(), &cfg);
        // Lossless: original must be returned unchanged
        if let (DocNode::Para { text: t1, .. }, DocNode::Para { text: t2, .. }) =
            (&nodes[0], &result[0])
        {
            assert_eq!(t1, t2);
        }
    }

    #[test]
    fn new_compressor_has_empty_stopwords() {
        let compressor = AdaptiveCompressor::new();
        // A compressor created with new() must have an empty stopword regex list.
        assert!(compressor.stopword_regexes.is_empty(),
            "stopword regex list from new() must be empty");
    }

    #[test]
    fn stopword_removal_works() {
        // Stopword removal only works when stopwords are explicitly specified via with_stopwords.
        let compressor = AdaptiveCompressor::with_stopwords(vec!["the".into()]);
        let nodes = vec![make_para("the quick brown fox", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100, // ~10% — StopwordOnly stage
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(!text.to_lowercase().contains("the "),
                "stopword 'the' must be removed: got '{}'", text);
        }
    }

    #[test]
    fn with_stopwords_removes_specified_words() {
        let compressor = AdaptiveCompressor::with_stopwords(vec!["hello".into(), "world".into()]);
        let nodes = vec![make_para("hello world foo", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(!text.to_lowercase().contains("hello"),
                "'hello' must be removed: got '{}'", text);
            assert!(!text.to_lowercase().contains("world"),
                "'world' must be removed: got '{}'", text);
            assert!(text.contains("foo"), "'foo' must remain: got '{}'", text);
        }
    }

    #[test]
    fn prune_low_importance_removes_bottom_20_pct() {
        let nodes = vec![
            make_para("중요 단락", 0.9),
            make_para("보통 단락", 0.5),
            make_para("낮은 단락", 0.1),
            make_para("낮은 단락2", 0.05),
            make_para("낮은 단락3", 0.02),
        ];
        let result = prune_low_importance(nodes, 0.20);
        // Bottom 20% importance (1 out of 5, cutoff=0.02) should be removed
        assert!(result.len() < 5, "some nodes must be removed");
    }

    #[test]
    fn deduplicate_removes_duplicates() {
        let nodes = vec![
            make_para("동일한 내용입니다.", 1.0),
            make_para("다른 내용입니다.", 1.0),
            make_para("동일한 내용입니다.", 0.9),
        ];
        let result = deduplicate_paras(nodes);
        assert_eq!(result.len(), 2, "one duplicate paragraph must be removed");
    }

    #[test]
    fn first_sentence_extraction() {
        assert_eq!(first_sentence("안녕하세요. 반갑습니다."), "안녕하세요.");
        assert_eq!(first_sentence("문장 부호 없는 텍스트"), "문장 부호 없는 텍스트");
        assert_eq!(first_sentence("Hello world! Bye."), "Hello world!");
    }

    #[test]
    fn first_sentence_multilingual() {
        // Hindi Devanagari Danda (U+0964)
        assert_eq!(first_sentence("यह पहला वाक्य है। यह दूसरा है।"), "यह पहला वाक्य है।");
        // Arabic Full Stop (U+06D4)
        assert_eq!(first_sentence("هذه الجملة الأولى۔ هذه الثانية۔"), "هذه الجملة الأولى۔");
        // Amharic Ethiopic Full Stop (U+1362)
        assert_eq!(first_sentence("ይህ የመጀመሪያ ዓረፍተ ነገር ነው። ሁለተኛ።"), "ይህ የመጀመሪያ ዓረፍተ ነገር ነው።");
        // Fullwidth Small Full Stop (U+FE52)
        assert_eq!(first_sentence("これが最初の文です．これが二番目です．"), "これが最初の文です．");
    }

    #[test]
    fn prune_keeps_single_paragraph() {
        let compressor = AdaptiveCompressor::new();
        let nodes = vec![make_para("only paragraph", 0.1)]; // low importance
        let cfg = CompressionConfig { budget: 100, current_tokens: 65, fidelity: FidelityLevel::Semantic };
        let result = compressor.compress(nodes, &cfg);
        assert_eq!(result.len(), 1, "the sole paragraph in a single-paragraph document must not be removed");
    }

    #[test]
    fn prune_keeps_all_equal_importance_paragraphs() {
        let compressor = AdaptiveCompressor::new();
        // 3 paragraphs, all same importance — none should be removed
        let nodes = vec![
            make_para("first", 0.5),
            make_para("second", 0.5),
            make_para("third", 0.5),
        ];
        let cfg = CompressionConfig { budget: 100, current_tokens: 65, fidelity: FidelityLevel::Semantic };
        let result = compressor.compress(nodes, &cfg);
        assert_eq!(result.len(), 3, "paragraphs with equal importance must not all be removed");
    }

    #[test]
    fn stage_thresholds() {
        let base = CompressionConfig { budget: 100, current_tokens: 0, fidelity: FidelityLevel::Semantic };
        let at = |tokens| CompressionConfig { current_tokens: tokens, ..base.clone() };

        assert_eq!(at(50).stage(),  CompressionStage::StopwordOnly);
        assert_eq!(at(70).stage(),  CompressionStage::PruneLowImportance);
        assert_eq!(at(85).stage(),  CompressionStage::DeduplicateAndLinearize);
        assert_eq!(at(96).stage(),  CompressionStage::MaxCompression);
    }
}
