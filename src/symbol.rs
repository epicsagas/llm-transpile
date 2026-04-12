//! symbol.rs — SymbolDict (Token Substitution)
//!
//! Reduces token count by replacing frequently occurring technical terms
//! with Unicode Private Use Area (PUA) characters.
//!
//! # Design principles
//! - Substitution characters: U+E000–U+F8FF (PUA)
//!   → Zero reverse-substitution collisions compared to visible `$1`, `$2` patterns
//! - `intern()` / `decode_str()` pair provides fully symmetric encode ↔ decode
//! - The `<D>` global dictionary block is emitted only once at the top of the document
//!
//! # Thread safety
//! `SymbolDict` uses `std::sync::RwLock` for the internal Aho-Corasick automaton cache,
//! making the type `Send + Sync`. It can therefore be moved into `tokio::spawn` tasks or
//! shared across threads via `Arc<SymbolDict>` (with `Arc<Mutex<SymbolDict>>` for mutation).

use std::collections::HashMap;
use std::sync::RwLock;

/// Unicode PUA start codepoint.
const PUA_START: u32 = 0xE000;
/// Unicode PUA end codepoint (inclusive).
const PUA_END: u32 = 0xF8FF;
/// Maximum number of symbols.
pub const MAX_SYMBOLS: usize = (PUA_END - PUA_START + 1) as usize;

// ────────────────────────────────────────────────
// SymbolDict
// ────────────────────────────────────────────────

type AcCache = (Vec<String>, Vec<String>, aho_corasick::AhoCorasick);

/// Bidirectional mapping table between technical terms and PUA symbols.
///
/// # Thread safety
/// Uses `std::sync::RwLock` for the lazy Aho-Corasick automaton cache.
/// The type is `Send + Sync` and can be safely moved into async tasks.
/// For concurrent mutation, wrap in `Arc<Mutex<SymbolDict>>`.
pub struct SymbolDict {
    /// term → PUA character
    encode: HashMap<String, char>,
    /// PUA character → term
    decode: HashMap<char, String>,
    /// Next PUA codepoint to assign
    next_code: u32,
    /// Lazy-build cache for `encode_str` (interior mutability via RwLock).
    /// Invalidated on `intern()` calls; lazily rebuilt on the first `encode_str()` call.
    ac_cache: RwLock<Option<AcCache>>,
}

impl Default for SymbolDict {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolDict {
    /// Creates an empty dictionary.
    pub fn new() -> Self {
        Self {
            encode: HashMap::new(),
            decode: HashMap::new(),
            next_code: PUA_START,
            ac_cache: RwLock::new(None),
        }
    }

    /// Returns the number of registered symbols.
    pub fn len(&self) -> usize {
        self.encode.len()
    }

    /// Returns `true` if the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.encode.is_empty()
    }

    /// Registers a term in the dictionary and returns its corresponding PUA symbol.
    ///
    /// If the term is already registered, the existing symbol is returned (idempotent).
    ///
    /// # Errors
    /// Returns `Err(SymbolOverflowError)` when the PUA allocation limit is exceeded.
    pub fn intern(&mut self, term: &str) -> Result<char, SymbolOverflowError> {
        if let Some(&sym) = self.encode.get(term) {
            return Ok(sym);
        }
        if self.next_code > PUA_END {
            return Err(SymbolOverflowError { max: MAX_SYMBOLS });
        }
        let sym = char::from_u32(self.next_code)
            .expect("codepoints within the PUA range are always valid");
        self.encode.insert(term.to_string(), sym);
        self.decode.insert(sym, term.to_string());
        self.next_code += 1;
        // Invalidate the Aho-Corasick cache — it will be rebuilt on the next encode_str() call.
        *self.ac_cache.write().unwrap() = None;
        Ok(sym)
    }

    /// Restores PUA symbols in the input string back to their original terms.
    ///
    /// Unknown PUA characters are passed through unchanged.
    ///
    /// # Why test-only?
    /// Decoding PUA back to original terms is only needed in tests for round-trip
    /// verification. Production output is consumed by LLMs which treat PUA symbols
    /// as opaque tokens, so no decoding path is needed at runtime.
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

    /// Replaces dictionary-registered terms in the input string with PUA symbols.
    ///
    /// Single Aho-Corasick `LeftmostLongest` pass, O(n+T) complexity.
    /// The automaton is lazily built on the first call and cached until the next `intern()` call.
    pub fn encode_str(&self, input: &str) -> String {
        if self.encode.is_empty() {
            return input.to_string();
        }

        // ── Cache-hit path: shared read lock ─────────────────────────────
        {
            let cache = self.ac_cache.read().unwrap();
            if let Some((_, replacements, ac)) = cache.as_ref() {
                return ac.replace_all(input, replacements);
            }
        } // read lock released here

        // ── Cache-miss path: build automaton, then store ──────────────────
        {
            let mut pairs: Vec<(String, String)> = self
                .encode
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect();
            // LeftmostLongest selects by length; sort longest-first to assign lower IDs
            // to longer patterns so they are preferred on equal-length conflicts.
            pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

            let patterns: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
            let replacements: Vec<String> = pairs.iter().map(|(_, v)| v.clone()).collect();

            let ac = aho_corasick::AhoCorasick::builder()
                .match_kind(aho_corasick::MatchKind::LeftmostLongest)
                .build(&patterns)
                .expect("AhoCorasick build cannot fail with valid patterns");

            let pattern_strs: Vec<String> = pairs.into_iter().map(|(k, _)| k).collect();
            *self.ac_cache.write().unwrap() = Some((pattern_strs, replacements, ac));
        }

        let cache = self.ac_cache.read().unwrap();
        let (_, replacements, ac) = cache.as_ref().unwrap();
        ac.replace_all(input, replacements)
    }

    /// Generates the `<D>` global dictionary block.
    ///
    /// Returns an empty string if the dictionary is empty.
    pub fn render_dict_header(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        // Sort by codepoint order for deterministic output
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
// Error type
// ────────────────────────────────────────────────

/// Error returned when the PUA symbol allocation limit is exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolOverflowError {
    pub max: usize,
}

impl std::fmt::Display for SymbolOverflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "symbol table overflow: maximum {} symbols", self.max)
    }
}

impl std::error::Error for SymbolOverflowError {}

// ────────────────────────────────────────────────
// Unit tests
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_idempotent() {
        let mut dict = SymbolDict::new();
        let sym1 = dict.intern("법률용어").unwrap();
        let sym2 = dict.intern("법률용어").unwrap();
        assert_eq!(
            sym1, sym2,
            "re-interning the same term must return the same symbol"
        );
    }

    #[test]
    fn encode_decode_roundtrip() {
        let mut dict = SymbolDict::new();
        dict.intern("손해배상").unwrap();
        dict.intern("계약해제").unwrap();

        let original = "손해배상 청구와 계약해제 요건";
        let encoded = dict.encode_str(original);
        let decoded = dict.decode_str(&encoded);

        assert_eq!(
            decoded, original,
            "encode → decode round-trip must restore the original text"
        );
    }

    #[test]
    fn no_collision_with_dollar_sign() {
        let mut dict = SymbolDict::new();
        let sym = dict.intern("테스트용어").unwrap();
        // PUA characters do not overlap with visible '$1' patterns
        assert!(sym as u32 >= PUA_START);
        assert!(sym as u32 <= PUA_END);
    }

    #[test]
    fn decode_passes_through_unknown_pua() {
        let dict = SymbolDict::new(); // empty dictionary
        let unknown = "\u{E100}hello";
        // Unregistered PUA characters are passed through unchanged
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
        // Simulate overflow: force next_code past PUA_END + 1
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
        let sym_ab = *dict.encode.get("ab").unwrap();
        let sym_abc = *dict.encode.get("abc").unwrap();
        // "abc" must not be partially matched as "ab" — the entire "abc" should be substituted
        let encoded = dict.encode_str("abc");
        assert_eq!(
            encoded,
            sym_abc.to_string(),
            "LeftmostLongest: full 'abc' must be substituted, sym_ab={:?}",
            sym_ab
        );
    }

    /// Compile-time check: SymbolDict must be Send + Sync after the RwLock migration.
    #[test]
    fn symbol_dict_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SymbolDict>();
    }
}
