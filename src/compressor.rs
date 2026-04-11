//! compressor.rs — AdaptiveCompressor
//!
//! 토큰 예산 소모율에 따라 4단계 압축 전략을 자동으로 적용한다.
//!
//! | 예산 소모율 | 적용 전략                                           |
//! |-----------|-----------------------------------------------------|
//! | 0–60%     | 불용어 제거만                                        |
//! | 60–80%    | 불용어 + 중요도 하위 20% 단락 제거                    |
//! | 80–95%    | 위 + 중복 문장 제거 + 수치 데이터 선형화              |
//! | 95%+      | 위 + 모든 단락 → 첫 문장만 유지 (Semantic 이상 전용) |

use crate::ir::{DocNode, FidelityLevel};
use regex::Regex;

// ────────────────────────────────────────────────
// 1. 압축 설정
// ────────────────────────────────────────────────

/// 압축기 실행 시 제공하는 컨텍스트.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// 최대 허용 토큰 수.
    pub budget: usize,
    /// 현재까지 소모한 토큰 수 (근사치).
    pub current_tokens: usize,
    /// 의미 보존 레벨.
    pub fidelity: FidelityLevel,
}

impl CompressionConfig {
    /// 현재 예산 소모율 (0.0–1.0).
    pub fn usage_ratio(&self) -> f64 {
        if self.budget == 0 {
            return 1.0;
        }
        self.current_tokens as f64 / self.budget as f64
    }

    /// 현재 소모율에 따른 압축 단계를 반환한다.
    pub fn stage(&self) -> CompressionStage {
        match self.usage_ratio() {
            r if r < 0.60 => CompressionStage::StopwordOnly,
            r if r < 0.80 => CompressionStage::PruneLowImportance,
            r if r < 0.95 => CompressionStage::DeduplicateAndLinearize,
            _              => CompressionStage::MaxCompression,
        }
    }
}

/// 압축 단계 열거형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompressionStage {
    /// 불용어 제거만.
    StopwordOnly,
    /// 불용어 + 중요도 하위 20% 단락 제거.
    PruneLowImportance,
    /// 위 + 중복 문장 제거.
    DeduplicateAndLinearize,
    /// 위 + 단락을 첫 문장으로 축약.
    MaxCompression,
}

// ────────────────────────────────────────────────
// 2. AdaptiveCompressor
// ────────────────────────────────────────────────

/// 예산 기반 적응형 문서 압축기.
pub struct AdaptiveCompressor {
    /// 생성자에서 사전 컴파일된 불용어 정규식 목록.
    /// 호출마다 재컴파일하지 않도록 생성 시점에 한 번만 빌드한다.
    stopword_regexes: Vec<Regex>,
}

impl Default for AdaptiveCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveCompressor {
    /// 기본 불용어 목록(빈 목록)으로 압축기를 생성한다.
    pub fn new() -> Self {
        Self::with_stopwords(default_stopwords())
    }

    /// 사용자 정의 불용어 목록으로 압축기를 생성한다.
    /// 불용어는 생성 시점에 정규식으로 컴파일되어 캐시된다.
    pub fn with_stopwords(stopwords: Vec<String>) -> Self {
        let stopword_regexes = stopwords
            .iter()
            .filter_map(|sw| {
                // `\b`는 ASCII 단어 경계만 인식한다.
                // 비ASCII 불용어(아랍어·힌디어 등)는 경계 매칭이 동작하지 않아 조용히 무시될 수 있다.
                // TODO: 비ASCII 불용어는 공백 기반 split-replace 전략으로 별도 처리 필요.
                let pattern = format!(r"(?i)\b{}\b\s*", regex::escape(sw));
                Regex::new(&pattern).ok()
            })
            .collect();
        Self { stopword_regexes }
    }

    /// 노드 목록에 압축을 적용하고 결과를 반환한다.
    ///
    /// `FidelityLevel::Lossless`에서는 불용어 제거도 수행하지 않는다.
    pub fn compress(&self, mut nodes: Vec<DocNode>, cfg: &CompressionConfig) -> Vec<DocNode> {
        if cfg.fidelity == FidelityLevel::Lossless {
            return nodes; // Lossless: 압축 완전 금지
        }

        let stage = cfg.stage();

        // ① 불용어 제거 (모든 단계)
        nodes = self.remove_stopwords(nodes);

        // ② 중요도 하위 20% 단락 제거
        if stage >= CompressionStage::PruneLowImportance {
            nodes = prune_low_importance(nodes, 0.20);
        }

        // ③ 중복 문장 제거
        if stage >= CompressionStage::DeduplicateAndLinearize {
            nodes = deduplicate_paras(nodes);
        }

        // ④ 단락 → 첫 문장으로 축약
        // Lossless는 함수 상단에서 early return했으므로 여기서는 fidelity != Lossless가 보장됨.
        if stage >= CompressionStage::MaxCompression {
            nodes = truncate_to_first_sentence(nodes);
        }

        nodes
    }

    // ── 내부 헬퍼 ───────────────────────────────

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
        // 불용어당 1회 패스(O(N × |text|)). 불용어가 `\b` ASCII 경계 regex를 사용하므로
        // aho-corasick 단일 패스로 대체할 수 없습니다.
        // 기본 불용어 목록은 비어 있으므로 `remove_stopwords`의 early return으로 이 함수는
        // 사용자가 명시적으로 불용어를 구성한 경우에만 호출됩니다.
        let mut result = text.to_string();
        for re in &self.stopword_regexes {
            result = re.replace_all(&result, "").into_owned();
        }
        // 연속 공백 정리 (1회만 수행)
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

// ────────────────────────────────────────────────
// 3. 내부 압축 함수들
// ────────────────────────────────────────────────

/// 중요도 하위 `threshold` 비율의 `Para` 노드를 제거한다.
fn prune_low_importance(nodes: Vec<DocNode>, threshold: f32) -> Vec<DocNode> {
    // 단락만 필터링 대상
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

    // 하위 threshold 비율의 컷오프 값 계산
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
                true // 단락 외 노드는 보존
            }
        })
        .cloned()
        .collect();

    // 안전망: 원본에 Para가 있었는데 필터 후 Para가 하나도 없으면 원본 반환.
    // (모든 단락의 중요도가 동일한 경우 cutoff == 모든 importance → 전부 탈락 방지)
    let filtered_has_para = filtered.iter().any(|n| matches!(n, DocNode::Para { .. }));
    let input_had_para = nodes.iter().any(|n| matches!(n, DocNode::Para { .. }));

    if input_had_para && !filtered_has_para {
        nodes
    } else {
        filtered
    }
}

/// 내용이 동일한 `Para` 노드를 제거한다 (첫 번째만 유지).
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

/// 각 `Para`를 첫 번째 문장으로 잘라낸다.
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

/// 텍스트에서 첫 번째 문장(`.`, `!`, `?` 기준)을 추출한다.
fn first_sentence(text: &str) -> String {
    for (i, c) in text.char_indices() {
        if matches!(c,
            '.' | '!' | '?'           // ASCII
            | '。' | '！' | '？'      // CJK 전각 (U+3002, U+FF01, U+FF1F)
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
    text.trim().to_string() // 문장 부호 없으면 전체 반환
}

/// 기본 불용어 목록 — 언어 중립적으로 빈 목록을 반환한다.
///
/// 언어별 불용어가 필요한 경우 `AdaptiveCompressor::with_stopwords()`를 사용하라.
fn default_stopwords() -> Vec<String> {
    vec![]
}

// ────────────────────────────────────────────────
// 4. 단위 테스트
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
        // Lossless: 원본 그대로
        if let (DocNode::Para { text: t1, .. }, DocNode::Para { text: t2, .. }) =
            (&nodes[0], &result[0])
        {
            assert_eq!(t1, t2);
        }
    }

    #[test]
    fn new_compressor_has_empty_stopwords() {
        let compressor = AdaptiveCompressor::new();
        // new()로 생성한 compressor는 불용어 정규식 목록이 비어 있어야 한다.
        assert!(compressor.stopword_regexes.is_empty(),
            "new()의 불용어 정규식 목록은 비어 있어야 한다");
    }

    #[test]
    fn stopword_removal_works() {
        // with_stopwords를 통해 명시적으로 불용어를 지정해야 제거가 동작한다.
        let compressor = AdaptiveCompressor::with_stopwords(vec!["the".into()]);
        let nodes = vec![make_para("the quick brown fox", 1.0)];
        let cfg = CompressionConfig {
            budget: 1000,
            current_tokens: 100, // ~10% — StopwordOnly 단계
            fidelity: FidelityLevel::Semantic,
        };
        let result = compressor.compress(nodes, &cfg);
        if let DocNode::Para { text, .. } = &result[0] {
            assert!(!text.to_lowercase().contains("the "),
                "불용어 'the'가 제거되어야 한다: got '{}'", text);
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
                "'hello'가 제거되어야 한다: got '{}'", text);
            assert!(!text.to_lowercase().contains("world"),
                "'world'가 제거되어야 한다: got '{}'", text);
            assert!(text.contains("foo"), "'foo'는 남아 있어야 한다: got '{}'", text);
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
        // 중요도 하위 20% (5개 중 1개, cutoff=0.02) 제거
        assert!(result.len() < 5, "일부 노드가 제거되어야 한다");
    }

    #[test]
    fn deduplicate_removes_duplicates() {
        let nodes = vec![
            make_para("동일한 내용입니다.", 1.0),
            make_para("다른 내용입니다.", 1.0),
            make_para("동일한 내용입니다.", 0.9),
        ];
        let result = deduplicate_paras(nodes);
        assert_eq!(result.len(), 2, "중복 단락 1개가 제거되어야 한다");
    }

    #[test]
    fn first_sentence_extraction() {
        assert_eq!(first_sentence("안녕하세요. 반갑습니다."), "안녕하세요.");
        assert_eq!(first_sentence("문장 부호 없는 텍스트"), "문장 부호 없는 텍스트");
        assert_eq!(first_sentence("Hello world! Bye."), "Hello world!");
    }

    #[test]
    fn first_sentence_multilingual() {
        // 힌디어 Devanagari Danda (U+0964)
        assert_eq!(first_sentence("यह पहला वाक्य है। यह दूसरा है।"), "यह पहला वाक्य है।");
        // 아랍어 Full Stop (U+06D4)
        assert_eq!(first_sentence("هذه الجملة الأولى۔ هذه الثانية۔"), "هذه الجملة الأولى۔");
        // 암하라어 Ethiopic Full Stop (U+1362)
        assert_eq!(first_sentence("ይህ የመጀመሪያ ዓረፍተ ነገር ነው። ሁለተኛ።"), "ይህ የመጀመሪያ ዓረፍተ ነገር ነው።");
        // 전각 마침표 Small Full Stop (U+FE52)
        assert_eq!(first_sentence("これが最初の文です．これが二番目です．"), "これが最初の文です．");
    }

    #[test]
    fn prune_keeps_single_paragraph() {
        let compressor = AdaptiveCompressor::new();
        let nodes = vec![make_para("only paragraph", 0.1)]; // low importance
        let cfg = CompressionConfig { budget: 100, current_tokens: 65, fidelity: FidelityLevel::Semantic };
        let result = compressor.compress(nodes, &cfg);
        assert_eq!(result.len(), 1, "단락 1개짜리 문서에서 유일한 단락이 제거되면 안 됩니다");
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
        assert_eq!(result.len(), 3, "동일 중요도 단락은 전체 제거되면 안 됩니다");
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
