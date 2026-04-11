//! symbol.rs — SymbolDict (Token Substitution)
//!
//! 자주 등장하는 전문 용어를 Unicode Private Use Area(PUA) 문자로
//! 치환하여 토큰 수를 절감한다.
//!
//! # 설계 원칙
//! - 치환 기호: U+E000–U+F8FF (PUA) 사용
//!   → 가시적 `$1`, `$2` 방식 대비 역치환 충돌 제로
//! - `intern()` / `decode_str()` 쌍으로 encode ↔ decode 완전 대칭
//! - `<D>` 전역 사전 블록을 문서 상단에 1회만 출력

use std::cell::RefCell;
use std::collections::HashMap;

/// Unicode PUA 시작 코드포인트.
const PUA_START: u32 = 0xE000;
/// Unicode PUA 종료 코드포인트 (포함).
const PUA_END: u32 = 0xF8FF;
/// 최대 심볼 수.
pub const MAX_SYMBOLS: usize = (PUA_END - PUA_START + 1) as usize;

// ────────────────────────────────────────────────
// SymbolDict
// ────────────────────────────────────────────────

type AcCache = (Vec<String>, Vec<String>, aho_corasick::AhoCorasick);

/// 전문 용어 ↔ PUA 기호 양방향 매핑 테이블.
///
/// # 스레드 안전성
/// `RefCell` 내부 가변성을 사용하므로 `!Send`입니다.
/// `tokio::spawn` 등에 직접 전달할 수 없으며, 필요 시 `Arc<Mutex<SymbolDict>>` 래핑이 필요합니다.
pub struct SymbolDict {
    /// term → PUA 문자
    encode: HashMap<String, char>,
    /// PUA 문자 → term
    decode: HashMap<char, String>,
    /// 다음에 할당할 PUA 코드포인트
    next_code: u32,
    /// `encode_str` lazy build 캐시 (내부 가변성).
    /// `intern()` 호출 시 무효화, `encode_str()` 첫 호출 시 lazy build.
    ac_cache: RefCell<Option<AcCache>>,
}

impl Default for SymbolDict {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolDict {
    /// 빈 사전을 생성한다.
    pub fn new() -> Self {
        Self {
            encode: HashMap::new(),
            decode: HashMap::new(),
            next_code: PUA_START,
            ac_cache: RefCell::new(None),
        }
    }

    /// 등록된 심볼 수를 반환한다.
    pub fn len(&self) -> usize {
        self.encode.len()
    }

    /// 사전이 비어 있으면 `true`를 반환한다.
    pub fn is_empty(&self) -> bool {
        self.encode.is_empty()
    }

    /// 용어를 사전에 등록하고 대응하는 PUA 기호를 반환한다.
    ///
    /// 이미 등록된 용어라면 기존 기호를 반환한다 (멱등성 보장).
    ///
    /// # Errors
    /// PUA 할당 한도 초과 시 `Err(SymbolOverflowError)` 반환.
    pub fn intern(&mut self, term: &str) -> Result<char, SymbolOverflowError> {
        if let Some(&sym) = self.encode.get(term) {
            return Ok(sym);
        }
        if self.next_code > PUA_END {
            return Err(SymbolOverflowError { max: MAX_SYMBOLS });
        }
        let sym = char::from_u32(self.next_code)
            .expect("PUA 범위 내 코드포인트는 항상 유효하다");
        self.encode.insert(term.to_string(), sym);
        self.decode.insert(sym, term.to_string());
        self.next_code += 1;
        *self.ac_cache.borrow_mut() = None; // 사전 변경 시 캐시 무효화
        Ok(sym)
    }

    /// 입력 문자열에서 PUA 기호를 원래 용어로 복원한다.
    ///
    /// 알 수 없는 PUA 문자는 그대로 통과시킨다.
    #[cfg(test)]
    pub(crate) fn decode_str(&self, input: &str) -> String {
        input
            .chars()
            .flat_map(|c| {
                if let Some(term) = self.decode.get(&c) {
                    term.chars().collect::<Vec<_>>()
                } else {
                    vec![c]
                }
            })
            .collect()
    }

    /// 입력 문자열에서 사전에 등록된 용어를 PUA 기호로 치환한다.
    ///
    /// aho-corasick LeftmostLongest 단일 패스로 O(n+T) 복잡도.
    /// automaton은 첫 호출 시 lazy build되고 `intern()` 호출 전까지 캐시된다.
    pub fn encode_str(&self, input: &str) -> String {
        if self.encode.is_empty() {
            return input.to_string();
        }

        // 캐시 히트 경로: 단일 borrow() (공유 참조)
        {
            let cache = self.ac_cache.borrow();
            if let Some((_, replacements, ac)) = cache.as_ref() {
                return ac.replace_all(input, replacements);
            }
        }

        // 캐시 미스: automaton 빌드 후 borrow_mut()으로 저장
        {
            let mut pairs: Vec<(String, String)> = self
                .encode
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect();
            // LeftmostLongest가 길이 기준으로 선택하지만, 동일 길이 충돌 시
            // 등록 순서(ID)가 낮은 쪽이 선택되므로 긴 것부터 정렬하여 ID를 부여한다.
            pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

            let patterns: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
            let replacements: Vec<String> = pairs.iter().map(|(_, v)| v.clone()).collect();

            let ac = aho_corasick::AhoCorasick::builder()
                .match_kind(aho_corasick::MatchKind::LeftmostLongest)
                .build(&patterns)
                .expect("유효한 패턴으로 AhoCorasick 빌드 실패 불가");

            let pattern_strs: Vec<String> = pairs.into_iter().map(|(k, _)| k).collect();
            *self.ac_cache.borrow_mut() = Some((pattern_strs, replacements, ac));
        }

        let cache = self.ac_cache.borrow();
        let (_, replacements, ac) = cache.as_ref().unwrap();
        ac.replace_all(input, replacements)
    }

    /// `<D>` 전역 사전 블록을 생성한다.
    ///
    /// 사전이 비어 있으면 빈 문자열을 반환한다.
    pub fn render_dict_header(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        // 코드포인트 순서로 정렬하여 결정론적 출력 보장
        let mut entries: Vec<(char, &str)> =
            self.decode.iter().map(|(c, s)| (*c, s.as_str())).collect();
        entries.sort_by_key(|(c, _)| *c as u32);

        let body: String = entries
            .iter()
            .map(|(sym, term)| format!("{}={}", sym, term))
            .collect::<Vec<_>>()
            .join("\n");

        format!("<D>\n{}\n</D>\n", body)
    }
}

// ────────────────────────────────────────────────
// 에러 타입
// ────────────────────────────────────────────────

/// PUA 기호 할당 한도 초과 에러.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolOverflowError {
    pub max: usize,
}

impl std::fmt::Display for SymbolOverflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "심볼 테이블 초과: 최대 {} 기호", self.max)
    }
}

impl std::error::Error for SymbolOverflowError {}

// ────────────────────────────────────────────────
// 단위 테스트
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_idempotent() {
        let mut dict = SymbolDict::new();
        let sym1 = dict.intern("법률용어").unwrap();
        let sym2 = dict.intern("법률용어").unwrap();
        assert_eq!(sym1, sym2, "동일 용어 재 intern은 동일 기호를 반환해야 한다");
    }

    #[test]
    fn encode_decode_roundtrip() {
        let mut dict = SymbolDict::new();
        dict.intern("손해배상").unwrap();
        dict.intern("계약해제").unwrap();

        let original = "손해배상 청구와 계약해제 요건";
        let encoded = dict.encode_str(original);
        let decoded = dict.decode_str(&encoded);

        assert_eq!(decoded, original, "encode → decode 라운드트립이 원문을 복원해야 한다");
    }

    #[test]
    fn no_collision_with_dollar_sign() {
        let mut dict = SymbolDict::new();
        let sym = dict.intern("테스트용어").unwrap();
        // PUA 문자는 가시적 '$1' 패턴과 겹치지 않는다
        assert!(sym as u32 >= PUA_START);
        assert!(sym as u32 <= PUA_END);
    }

    #[test]
    fn decode_passes_through_unknown_pua() {
        let dict = SymbolDict::new(); // 빈 사전
        let unknown = "\u{E100}hello";
        // 등록되지 않은 PUA 문자는 그대로 통과
        assert_eq!(dict.decode_str(unknown), unknown);
    }

    #[test]
    fn render_dict_header_empty() {
        let dict = SymbolDict::new();
        assert!(dict.render_dict_header().is_empty());
    }

    #[test]
    fn render_dict_header_format() {
        let mut dict = SymbolDict::new();
        dict.intern("Alpha").unwrap();
        let header = dict.render_dict_header();
        assert!(header.starts_with("<D>\n"));
        assert!(header.contains("Alpha"));
        assert!(header.ends_with("</D>\n"));
    }

    #[test]
    fn overflow_returns_error() {
        // 한도 초과 시뮬레이션: next_code를 강제로 PUA_END + 1로 밀기
        let mut dict = SymbolDict::new();
        dict.next_code = PUA_END + 1;
        let result = dict.intern("overflow_term");
        assert!(result.is_err());
    }

    #[test]
    fn encode_str_aho_corasick_no_partial_match() {
        let mut dict = SymbolDict::new();
        dict.intern("ab").unwrap();
        dict.intern("abc").unwrap();
        let sym_ab  = *dict.encode.get("ab").unwrap();
        let sym_abc = *dict.encode.get("abc").unwrap();
        // "abc"는 "ab"로 부분 매칭되지 않고 "abc" 전체로 치환되어야 한다
        let encoded = dict.encode_str("abc");
        assert_eq!(encoded, sym_abc.to_string(),
            "LeftmostLongest: 'abc' 전체가 치환되어야 함, sym_ab={:?}", sym_ab);
    }
}
