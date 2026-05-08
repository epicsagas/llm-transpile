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
//!
//! ## Stopword matching strategy
//!
//! - **ASCII stopwords**: indexed into a single [`AhoCorasick`] automaton (case-insensitive).
//!   Word-boundary semantics are enforced by checking the characters immediately before and
//!   after each match — the same contract as the previous `\b word \b` regex approach, but
//!   in a single O(N + M) pass instead of O(N × S) repeated regex sweeps.
//! - **Non-ASCII stopwords** (Korean, Japanese, CJK, Arabic, etc.): matched as exact
//!   whitespace-delimited tokens. This is necessary because `\b` does not recognise
//!   Unicode word boundaries for scripts without ASCII-style spacing.

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use crate::ir::{DocNode, FidelityLevel};

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
            _ => CompressionStage::MaxCompression,
        }
    }

    /// Returns the minimum compression stage enforced by the fidelity level,
    /// regardless of budget usage ratio.
    ///
    /// - `Compressed`: always applies at least `PruneLowImportance`
    /// - Others: no minimum (budget ratio decides)
    pub fn min_stage(&self) -> CompressionStage {
        match self.fidelity {
            FidelityLevel::Compressed => CompressionStage::PruneLowImportance,
            _ => CompressionStage::StopwordOnly,
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
    /// Single Aho-Corasick automaton built from all ASCII stopwords (case-insensitive).
    /// Replaces the previous per-stopword regex list — one O(N+M) pass instead of O(N×S).
    ascii_ac: Option<AhoCorasick>,
    /// Non-ASCII stopword list for exact whitespace-token matching.
    /// Applied with a whitespace-split-filter pass to handle CJK / Korean / Arabic etc.
    nonascii_stopwords: Vec<String>,
}

impl Default for AdaptiveCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveCompressor {
    /// Creates a compressor with the default stopword list.
    ///
    /// The default list includes common English function words (ASCII) and
    /// standalone Korean connective words (non-ASCII). For domain-specific
    /// stopwords use [`Self::with_stopwords`].
    pub fn new() -> Self {
        Self::with_stopwords(default_stopwords())
    }

    /// Creates a compressor with a fully custom stopword list.
    ///
    /// Stopwords are partitioned at construction time:
    /// - ASCII words → indexed into a single Aho-Corasick automaton (case-insensitive).
    /// - Non-ASCII words → stored as plain strings for token-level matching.
    pub fn with_stopwords(stopwords: Vec<String>) -> Self {
        let mut ascii_stopwords: Vec<String> = Vec::new();
        let mut nonascii_stopwords = Vec::new();

        for sw in &stopwords {
            if sw.is_ascii() {
                ascii_stopwords.push(sw.to_ascii_lowercase());
            } else {
                // Non-ASCII (Korean, CJK, Arabic, Devanagari, …):
                // stored as plain strings for whitespace-token matching.
                nonascii_stopwords.push(sw.clone());
            }
        }

        let ascii_ac = if ascii_stopwords.is_empty() {
            None
        } else {
            AhoCorasickBuilder::new()
                .ascii_case_insensitive(true)
                .match_kind(MatchKind::LeftmostFirst)
                .build(&ascii_stopwords)
                .ok()
        };

        Self {
            ascii_ac,
            nonascii_stopwords,
        }
    }

    /// Returns true when no stopwords are configured (both lists empty).
    pub fn has_stopwords(&self) -> bool {
        self.ascii_ac.is_some() || !self.nonascii_stopwords.is_empty()
    }

    /// Applies compression to the node list and returns the result.
    ///
    /// Stopword removal is also skipped at `FidelityLevel::Lossless`.
    pub fn compress(&self, mut nodes: Vec<DocNode>, cfg: &CompressionConfig) -> Vec<DocNode> {
        if cfg.fidelity == FidelityLevel::Lossless {
            return nodes; // Lossless: compression entirely forbidden
        }

        let stage = cfg.stage().max(cfg.min_stage());

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
        // Lossless early-returns at the top, so fidelity != Lossless is guaranteed here.
        if stage >= CompressionStage::MaxCompression {
            nodes = truncate_to_first_sentence(nodes);
        }

        nodes
    }

    // ── Internal helpers ─────────────────────────

    fn remove_stopwords(&self, nodes: Vec<DocNode>) -> Vec<DocNode> {
        if !self.has_stopwords() {
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

    /// Removes stopwords from a single text string.
    ///
    /// Two passes:
    /// 1. ASCII Aho-Corasick pass — single O(N+M) scan with word-boundary validation.
    ///    Each match is accepted only when the character immediately before the match
    ///    start and the character immediately after the match end are both non-word
    ///    characters (i.e. not `[A-Za-z0-9_]`). Trailing whitespace after an accepted
    ///    match is also consumed to avoid double-spaces.
    /// 2. Non-ASCII whitespace-token pass — splits on whitespace, filters exact matches,
    ///    then rejoins. O(N) per token.
    ///
    /// A final `split_whitespace` + rejoin collapses any residual consecutive spaces.
    fn strip_stopwords(&self, text: &str) -> String {
        // ── Pass 1: ASCII Aho-Corasick with word-boundary check ──────────────
        let result: String = if let Some(ac) = &self.ascii_ac {
            let bytes = text.as_bytes();
            let mut out = String::with_capacity(text.len());
            let mut last = 0usize;

            for mat in ac.find_iter(text) {
                let start = mat.start();
                let end = mat.end();

                // Word-boundary check: char before must be a non-word char (or start of string).
                let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
                // Word-boundary check: char after must be a non-word char (or end of string).
                let after_ok = end == bytes.len() || !is_word_byte(bytes[end]);

                if before_ok && after_ok {
                    // Emit the text before this match.
                    out.push_str(&text[last..start]);
                    // Consume any trailing whitespace that immediately follows the stopword.
                    let skip_end = skip_trailing_space(bytes, end);
                    last = skip_end;
                }
                // If boundary check fails, we do nothing — the match is skipped and
                // `last` stays where it was so the text is emitted unchanged.
            }

            out.push_str(&text[last..]);
            out
        } else {
            text.to_string()
        };

        // ── Pass 2: Non-ASCII token stopwords (whitespace-delimited exact match) ──
        let mut out2 = String::with_capacity(result.len());
        if !self.nonascii_stopwords.is_empty() {
            for token in result.split_whitespace().filter(|token| {
                !self
                    .nonascii_stopwords
                    .iter()
                    .any(|sw| sw.as_str() == *token)
            }) {
                if !out2.is_empty() {
                    out2.push(' ');
                }
                out2.push_str(token);
            }
        } else {
            // Collapse consecutive whitespace even when no non-ASCII stopwords exist.
            for token in result.split_whitespace() {
                if !out2.is_empty() {
                    out2.push(' ');
                }
                out2.push_str(token);
            }
        }

        out2
    }
}

// ── Word-boundary helpers ────────────────────────────────────────────────────

/// Returns `true` when `b` is an ASCII word character (`[A-Za-z0-9_]`).
///
/// The AC automaton operates on the UTF-8 byte slice.  Because all stopwords
/// are ASCII, every match start/end lands on an ASCII byte boundary, so a
/// simple byte-level check is safe and avoids a `char`-decode round-trip.
#[inline]
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Returns the index just past any ASCII horizontal whitespace (` `, `\t`)
/// immediately following position `pos` in `bytes`.
///
/// Only a single run of whitespace tokens immediately after the stopword is
/// consumed; sentence-level whitespace collapse is handled by the
/// `split_whitespace` pass that follows.
#[inline]
fn skip_trailing_space(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }
    pos
}

// ────────────────────────────────────────────────
// 3. Internal compression functions
// ────────────────────────────────────────────────

/// Maximum fraction of paragraphs that may be removed by pruning.
const MAX_PRUNE_RATIO: f32 = 0.40;

/// Removes `Para` nodes in the bottom `threshold` fraction by importance.
/// Never removes more than `MAX_PRUNE_RATIO` of total paragraphs.
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

    // If cutoff equals the maximum importance, all paragraphs share the same value
    // and there is nothing meaningfully "low importance" to prune.
    if cutoff >= sorted[sorted.len() - 1] {
        return nodes;
    }

    // Calculate max allowed removals
    let total_paras = para_importances.len();
    let max_removals = ((total_paras as f32 * MAX_PRUNE_RATIO) as usize).max(1);
    let mut removed = 0usize;

    let filtered: Vec<DocNode> = nodes
        .iter()
        .filter(|n| {
            if let DocNode::Para { importance, .. } = n {
                if *importance > cutoff {
                    true
                } else {
                    if removed < max_removals {
                        removed += 1;
                        false
                    } else {
                        true // Keep -- we've hit the cap
                    }
                }
            } else {
                true
            }
        })
        .cloned()
        .collect();

    // Safety net: if all paragraphs would be removed, return original
    let filtered_has_para = filtered.iter().any(|n| matches!(n, DocNode::Para { .. }));
    let input_had_para = nodes.iter().any(|n| matches!(n, DocNode::Para { .. }));

    if input_had_para && !filtered_has_para {
        nodes
    } else {
        filtered
    }
}

/// Removes `Para` nodes with near-identical content using Jaccard similarity.
/// Paragraphs with similarity >= 0.85 are considered duplicates; only the first is kept.
fn deduplicate_paras(nodes: Vec<DocNode>) -> Vec<DocNode> {
    use std::collections::HashSet;

    const SIMILARITY_THRESHOLD: f64 = 0.85;

    let mut signatures: Vec<HashSet<String>> = Vec::new();
    nodes
        .into_iter()
        .filter(|n| {
            if let DocNode::Para { text, .. } = n {
                let tokens: HashSet<String> =
                    text.split_whitespace().map(|t| t.to_lowercase()).collect();

                if tokens.is_empty() {
                    return true;
                }

                for existing in &signatures {
                    let sim = jaccard_similarity(&tokens, existing);
                    if sim >= SIMILARITY_THRESHOLD {
                        return false;
                    }
                }
                signatures.push(tokens);
            }
            true
        })
        .collect()
}

/// Computes Jaccard similarity between two sets: |intersection| / |union|.
fn jaccard_similarity(
    a: &std::collections::HashSet<String>,
    b: &std::collections::HashSet<String>,
) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    intersection as f64 / union as f64
}

/// Truncates each `Para` to its first sentence.
fn truncate_to_first_sentence(nodes: Vec<DocNode>) -> Vec<DocNode> {
    nodes
        .into_iter()
        .map(|node| match node {
            DocNode::Para { text, importance } => {
                let first = first_sentence(&text);
                DocNode::Para {
                    text: first,
                    importance,
                }
            }
            other => other,
        })
        .collect()
}

/// Extracts the first sentence from text (delimited by `.`, `!`, or `?`).
///
/// Periods that are part of common abbreviations (e.g. "Dr.", "U.S.", "Fig.")
/// or decimal numbers (e.g. "3.50") are skipped and do not terminate a sentence.
fn first_sentence(text: &str) -> String {
    for (i, c) in text.char_indices() {
        if c == '.' {
            // Check if this period is part of an abbreviation or a decimal number.
            if is_abbreviation_or_decimal(text, i) {
                continue;
            }
            return text[..i + c.len_utf8()].trim().to_string();
        }
        if matches!(
            c,
            '!' | '?'
            | '。' | '！' | '？'      // CJK fullwidth (U+3002, U+FF01, U+FF1F)
            | '।' | '॥'              // Devanagari Danda / Double Danda (U+0964, U+0965)
            | '۔'                    // Arabic Full Stop (U+06D4)
            | '።'                    // Ethiopic Full Stop (U+1362)
            | '᙮'                    // Canadian Syllabics Full Stop (U+166E)
            | '꓿'                    // Lisu Punctuation Full Stop (U+A4FF)
            | '︒'                    // Presentation Form Vertical Ideographic Full Stop (U+FE12)
            | '﹒'                    // Small Full Stop (U+FE52)
            | '．' // Fullwidth Full Stop (U+FF0E)
        ) {
            return text[..i + c.len_utf8()].trim().to_string();
        }
    }
    text.trim().to_string() // No sentence terminator found — return the full text
}

/// Common abbreviations whose trailing period is NOT a sentence terminator.
const ABBREVIATIONS: &[&str] = &[
    "dr", "mr", "mrs", "ms", "prof", "sr", "jr", "vs", "etc", "eg", "ie", "fig", "eq", "no", "vol",
    "us", "am", "pm", "gen", "sgt", "capt", "lt", "col", "inc", "ltd", "corp", "dept", "est",
    "govt", "assn", "al", "approx", "appt", "apt", "ave", "blvd", "cot", "def", "div", "fx", "min",
    "max", "misc", "mon", "tue", "wed", "thu", "fri", "sat", "sun", "jan", "feb", "mar", "apr",
    "jun", "jul", "aug", "sep", "sept", "oct", "nov", "dec", "st", "nd", "rd", "th",
];

/// Returns true if the period at byte position `pos` in `text` is part of
/// an abbreviation (e.g., "Dr.", "U.S.") or a decimal number (e.g., "3.50").
fn is_abbreviation_or_decimal(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();

    // Check if this is a decimal point: digit on both sides (e.g., "3.50") or
    // digit on the right side only (e.g., ".5").
    // A period preceded by a digit but followed by a non-digit is a sentence end
    // (e.g., "1234." at end of sentence), NOT a decimal.
    let preceded_by_digit = pos > 0 && bytes[pos - 1].is_ascii_digit();
    let followed_by_digit = pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit();
    if followed_by_digit {
        return true;
    }
    if preceded_by_digit && !followed_by_digit {
        // "1234." is a sentence-ending period, not a decimal.
        return false;
    }

    // Extract the word token before the period: go backwards to find a word boundary.
    // Periods are included so that multi-period abbreviations like "U.S." stay intact.
    let before = &text[..pos];
    let word_start = before
        .rfind(|c: char| !c.is_alphanumeric() && c != '.')
        .map(|i| i + 1)
        .unwrap_or(0);
    let word_before = &before[word_start..];

    // Also collect the continuation after the period (letter + period pairs like "g." in "e.g.").
    // This handles multi-period abbreviations when the first period is encountered.
    let after = &text[pos + 1..]; // skip the current period (ASCII '.')
    let mut extended = word_before.to_string();
    let mut rest = after.chars().peekable();
    while let Some(c) = rest.peek() {
        if c.is_ascii_alphabetic() {
            extended.push(*c);
            rest.next();
            // If followed by a period, include it and continue
            if let Some('.') = rest.peek() {
                extended.push('.');
                rest.next();
            } else {
                // Letter not followed by a period — not part of a multi-period abbreviation.
                // Remove the last letter we added (it belongs to the next word).
                extended.pop();
                break;
            }
        } else {
            break;
        }
    }

    // Collect alphabetic characters only (strips internal periods for "U.S." -> "us"),
    // then lowercase for comparison.
    let cleaned: String = extended
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();

    if cleaned.is_empty() {
        return false;
    }

    ABBREVIATIONS.contains(&cleaned.as_str())
}

// ────────────────────────────────────────────────
// 4. Default stopword list
// ────────────────────────────────────────────────

/// Default stopword list — English function words + Korean standalone connectives.
///
/// **English (ASCII)**: common articles, prepositions, auxiliaries, and pronouns
/// that carry little semantic weight in most technical / business documents.
///
/// **Korean (non-ASCII)**: standalone connective words that appear as discrete
/// whitespace-delimited tokens (그리고, 하지만, …). Grammatical particles
/// (은/는/이/가/을/를/…) are *not* included because they are fused to the preceding
/// noun in Korean text and cannot be stripped by whitespace-token matching without
/// morphological analysis.
///
/// For domain-specific stopwords use [`AdaptiveCompressor::with_stopwords`].
fn default_stopwords() -> Vec<String> {
    // ── English function words ────────────────────────────────────────────
    // Articles
    let articles = ["a", "an", "the"];
    // Coordinating conjunctions
    let conjunctions = ["and", "or", "but", "nor", "yet", "so", "for"];
    // Common prepositions
    let prepositions = [
        "in", "on", "at", "to", "of", "by", "as", "up", "via", "into", "from", "with", "than",
        "about", "over", "after", "before", "between", "through", "during", "within", "without",
    ];
    // Auxiliary / modal verbs
    let auxiliaries = [
        "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do", "does",
        "did", "will", "would", "shall", "should", "may", "might", "must", "can", "could",
    ];
    // Common pronouns / determiners
    let pronouns = [
        "it", "its", "this", "that", "these", "those", "not", "no", "also", "too", "very", "just",
        "such",
    ];

    // ── Korean standalone connectives (non-ASCII) ─────────────────────────
    // These are whole whitespace-delimited words in Korean prose.
    // Particles (은/는/이/가/…) are excluded — they require morphological analysis.
    let korean_connectives = [
        "그리고",
        "하지만",
        "그러나",
        "따라서",
        "또한",
        "즉",
        "및",
        "또는",
        "그래서",
        "그런데",
        "게다가",
        "다만",
        "단지",
        "특히",
        "주로",
        "왜냐하면",
        "그러므로",
        "한편",
        "반면",
        "이처럼",
        "이렇게",
        "이에",
        "이후",
        "이전",
    ];

    articles
        .iter()
        .chain(conjunctions.iter())
        .chain(prepositions.iter())
        .chain(auxiliaries.iter())
        .chain(pronouns.iter())
        .map(|s| s.to_string())
        .chain(korean_connectives.iter().map(|s| s.to_string()))
        .collect()
}

// ────────────────────────────────────────────────
// 5. Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_para(text: &str, importance: f32) -> DocNode {
        DocNode::Para {
            text: text.into(),
            importance,
        }
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
    fn new_compressor_has_stopwords() {
        let compressor = AdaptiveCompressor::new();
        // Default constructor must load the built-in stopword list.
        assert!(
            compressor.has_stopwords(),
            "default compressor must have a non-empty stopword list"
        );
    }

    #[test]
    fn empty_compressor_has_no_stopwords() {
        let compressor = AdaptiveCompressor::with_stopwords(vec![]);
        assert!(
            !compressor.has_stopwords(),
            "compressor built with empty list must report no stopwords"
        );
    }

    #[test]
    fn stopword_removal_ascii_works() {
        // "the" is in the default list → should be removed
        let compressor = AdaptiveCompressor::new();
        let nodes = vec![make_para("the quick brown fox", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100, // ~10% — StopwordOnly stage
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(
                !text.to_lowercase().starts_with("the "),
                "stopword 'the' must be removed: got '{}'",
                text
            );
        }
    }

    #[test]
    fn with_stopwords_removes_specified_ascii_words() {
        let compressor = AdaptiveCompressor::with_stopwords(vec!["hello".into(), "world".into()]);
        let nodes = vec![make_para("hello world foo", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(
                !text.to_lowercase().contains("hello"),
                "'hello' must be removed: got '{}'",
                text
            );
            assert!(
                !text.to_lowercase().contains("world"),
                "'world' must be removed: got '{}'",
                text
            );
            assert!(text.contains("foo"), "'foo' must remain: got '{}'", text);
        }
    }

    #[test]
    fn nonascii_stopword_removal_works() {
        // Korean connective "그리고" is in the default list and should be removed
        // when it appears as a standalone whitespace-delimited token.
        let compressor = AdaptiveCompressor::new();
        let nodes = vec![make_para("사과 그리고 바나나", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(
                !text.contains("그리고"),
                "Korean connective '그리고' must be removed: got '{}'",
                text
            );
            assert!(text.contains("사과"), "'사과' must remain: got '{}'", text);
            assert!(
                text.contains("바나나"),
                "'바나나' must remain: got '{}'",
                text
            );
        }
    }

    #[test]
    fn nonascii_stopword_partial_match_not_removed() {
        // "그리고" should NOT be removed when it is a substring of another word,
        // e.g. "그리고나서" is a different word and must be preserved.
        let compressor = AdaptiveCompressor::with_stopwords(vec!["그리고".into()]);
        let nodes = vec![make_para("그리고나서 확인", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(
                text.contains("그리고나서"),
                "'그리고나서' must NOT be removed (not an exact token): got '{}'",
                text
            );
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
        assert_eq!(
            first_sentence("문장 부호 없는 텍스트"),
            "문장 부호 없는 텍스트"
        );
        assert_eq!(first_sentence("Hello world! Bye."), "Hello world!");
    }

    #[test]
    fn first_sentence_multilingual() {
        // Hindi Devanagari Danda (U+0964)
        assert_eq!(
            first_sentence("यह पहला वाक्य है। यह दूसरा है।"),
            "यह पहला वाक्य है।"
        );
        // Arabic Full Stop (U+06D4)
        assert_eq!(
            first_sentence("هذه الجملة الأولى۔ هذه الثانية۔"),
            "هذه الجملة الأولى۔"
        );
        // Amharic Ethiopic Full Stop (U+1362)
        assert_eq!(
            first_sentence("ይህ የመጀመሪያ ዓረፍተ ነገር ነው። ሁለተኛ።"),
            "ይህ የመጀመሪያ ዓረፍተ ነገር ነው።"
        );
        // Fullwidth Small Full Stop (U+FE52)
        assert_eq!(
            first_sentence("これが最初の文です．これが二番目です．"),
            "これが最初の文です．"
        );
    }

    #[test]
    fn prune_keeps_single_paragraph() {
        let compressor = AdaptiveCompressor::with_stopwords(vec![]);
        let nodes = vec![make_para("only paragraph", 0.1)]; // low importance
        let cfg = CompressionConfig {
            budget: 100,
            current_tokens: 65,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        assert_eq!(
            result.len(),
            1,
            "the sole paragraph in a single-paragraph document must not be removed"
        );
    }

    #[test]
    fn prune_keeps_all_equal_importance_paragraphs() {
        let compressor = AdaptiveCompressor::with_stopwords(vec![]);
        // 3 paragraphs, all same importance — none should be removed
        let nodes = vec![
            make_para("first", 0.5),
            make_para("second", 0.5),
            make_para("third", 0.5),
        ];
        let cfg = CompressionConfig {
            budget: 100,
            current_tokens: 65,
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        assert_eq!(
            result.len(),
            3,
            "paragraphs with equal importance must not all be removed"
        );
    }

    /// Word-boundary regression: stopword "the" must be removed as a whole word but
    /// must NOT be stripped from inside "theory", "there", or "gather".
    #[test]
    fn ascii_stopword_respects_word_boundaries() {
        let compressor = AdaptiveCompressor::with_stopwords(vec!["the".into()]);
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100,
            fidelity: FidelityLevel::Semantic,
        };

        // "the" at start-of-string followed by space → must be removed
        let nodes = vec![make_para("the cat sat", 1.0)];
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(
                !text.to_lowercase().starts_with("the "),
                "standalone 'the' at start must be removed: got '{}'",
                text
            );
            assert!(
                text.contains("cat") && text.contains("sat"),
                "non-stopword tokens must remain: got '{}'",
                text
            );
        }

        // "theory" contains "the" as a prefix → must NOT be altered
        let nodes2 = vec![make_para("theory is important", 1.0)];
        let result2 = compressor.compress(nodes2, &cfg);
        if let DocNode::Para { text, .. } = &result2[0] {
            assert!(
                text.contains("theory"),
                "'theory' must not be modified by stopword 'the': got '{}'",
                text
            );
        }

        // "there" starts with "the" → must NOT be altered
        let nodes3 = vec![make_para("there are cats", 1.0)];
        let result3 = compressor.compress(nodes3, &cfg);
        if let DocNode::Para { text, .. } = &result3[0] {
            assert!(
                text.contains("there"),
                "'there' must not be modified by stopword 'the': got '{}'",
                text
            );
        }

        // "gather" contains "the" inside → must NOT be altered
        let nodes4 = vec![make_para("we gather here", 1.0)];
        let result4 = compressor.compress(nodes4, &cfg);
        if let DocNode::Para { text, .. } = &result4[0] {
            assert!(
                text.contains("gather"),
                "'gather' must not be modified by stopword 'the': got '{}'",
                text
            );
        }
    }

    #[test]
    fn default_stopwords_no_duplicates() {
        let stopwords = default_stopwords();
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        for sw in &stopwords {
            if !seen.insert(sw.as_str()) {
                duplicates.push(sw.clone());
            }
        }
        assert!(
            duplicates.is_empty(),
            "duplicate stopwords found: {:?}",
            duplicates
        );
    }

    #[test]
    fn first_sentence_skips_abbreviation_dr() {
        assert_eq!(
            first_sentence("Dr. Smith confirmed the results. The experiment was successful."),
            "Dr. Smith confirmed the results."
        );
    }

    #[test]
    fn first_sentence_skips_abbreviation_us() {
        assert_eq!(
            first_sentence("U.S. patent no. 1234. Filed in 2024."),
            "U.S. patent no. 1234."
        );
    }

    #[test]
    fn first_sentence_skips_abbreviation_eg() {
        assert_eq!(
            first_sentence("e.g. machine learning. Used widely."),
            "e.g. machine learning."
        );
    }

    #[test]
    fn first_sentence_skips_abbreviation_ie() {
        assert_eq!(
            first_sentence("i.e. the model output. This is correct."),
            "i.e. the model output."
        );
    }

    #[test]
    fn first_sentence_skips_abbreviation_fig() {
        assert_eq!(
            first_sentence("See Fig. 3 for details. The chart shows growth."),
            "See Fig. 3 for details."
        );
    }

    #[test]
    fn first_sentence_preserves_normal_period() {
        assert_eq!(
            first_sentence("This is a normal sentence. And another one."),
            "This is a normal sentence."
        );
    }

    #[test]
    fn first_sentence_handles_price() {
        // "$3.50" should not be treated as end of sentence
        assert_eq!(
            first_sentence("This costs $3.50 per unit. Total is $350."),
            "This costs $3.50 per unit."
        );
    }

    #[test]
    fn stage_thresholds() {
        let base = CompressionConfig {
            budget: 100,
            current_tokens: 0,
            fidelity: FidelityLevel::Semantic,
        };
        let at = |tokens| CompressionConfig {
            current_tokens: tokens,
            ..base.clone()
        };

        assert_eq!(at(50).stage(), CompressionStage::StopwordOnly);
        assert_eq!(at(70).stage(), CompressionStage::PruneLowImportance);
        assert_eq!(at(85).stage(), CompressionStage::DeduplicateAndLinearize);
        assert_eq!(at(96).stage(), CompressionStage::MaxCompression);
    }

    // ── R7: Fuzzy deduplication tests ──────────────────────────────────────

    #[test]
    fn fuzzy_dedup_removes_near_duplicates() {
        // "The system processes requests" vs "The system processes the requests"
        // Jaccard similarity should be high enough to trigger dedup
        let nodes = vec![
            make_para("The system processes requests", 1.0),
            make_para("The system processes the requests", 0.9),
            make_para("Completely different content here", 1.0),
        ];
        let result = deduplicate_paras(nodes);
        assert_eq!(
            result.len(),
            2,
            "near-duplicate paragraphs should be deduped: {:?}",
            result
                .iter()
                .filter_map(|n| if let DocNode::Para { text, .. } = n {
                    Some(text.clone())
                } else {
                    None
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn fuzzy_dedup_keeps_dissimilar() {
        let nodes = vec![
            make_para("The quick brown fox jumps", 1.0),
            make_para("Completely different content here", 1.0),
        ];
        let result = deduplicate_paras(nodes);
        assert_eq!(result.len(), 2, "dissimilar paragraphs must be kept");
    }

    // ── R8: Prune cap tests ───────────────────────────────────────────────

    #[test]
    fn prune_caps_at_40_percent() {
        // 10 paragraphs all with importance 0.5 except 2 at 0.6
        // Without cap, 80% would be removed (all 0.5 ones)
        // With cap, at most 4 should be removed
        let mut nodes = vec![];
        for _ in 0..8 {
            nodes.push(make_para("medium importance", 0.5));
        }
        for _ in 0..2 {
            nodes.push(make_para("high importance", 0.6));
        }
        let result = prune_low_importance(nodes, 0.20);
        let remaining = result.len();
        assert!(
            remaining >= 6,
            "at least 60% of paragraphs must survive the cap, got {remaining}/10"
        );
    }
}
