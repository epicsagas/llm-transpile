# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 프로젝트 개요

`llm-transpiler` — Raw 문서(Markdown, HTML, PlainText)를 LLM이 최소 토큰으로 소비할 수 있는 구조화된 브릿지 포맷(`<D>?<H><B>`)으로 변환하는 고성능 Rust 라이브러리.

- MSRV: Rust 1.75+
- 목표: Python 대비 ≥10× 파싱 속도, 원문 대비 15–30% 토큰 절감

## 명령어

```bash
# 빌드
cargo build
cargo build --release

# 전체 테스트
cargo test

# 특정 모듈 테스트
cargo test --lib ir::tests
cargo test --lib symbol::tests
cargo test --lib compressor::tests
cargo test --lib renderer::tests

# 단일 테스트 함수
cargo test intern_idempotent

# 벤치마크 (HTML 리포트: target/criterion/)
cargo bench

# 린트
cargo clippy -- -D warnings

# 포맷
cargo fmt
```

## 아키텍처

파이프라인: `parser.rs` → `ir.rs` → `compressor.rs` + `symbol.rs` → `renderer.rs` → `stream.rs`

### 모듈 역할

| 파일 | 역할 |
|------|------|
| `lib.rs` | 공개 API (`transpile`, `transpile_stream`, `token_count`) |
| `ir.rs` | 언어 중립적 IR — `DocNode`, `IRDocument`, `FidelityLevel` |
| `parser.rs` | Markdown/HTML/PlainText → `IRDocument` (내부 모듈) |
| `compressor.rs` | 토큰 예산 소모율 기반 4단계 적응형 압축 |
| `symbol.rs` | 전문 용어를 Unicode PUA(`U+E000–U+F8FF`)로 치환하는 `SymbolDict` |
| `renderer.rs` | `DocNode` → 브릿지 텍스트, 테이블 선형화, YAML 헤더 조립 |
| `stream.rs` | Tokio 기반 스트리밍 `TranspileChunk` 생성 |

### 핵심 불변 조건

- `FidelityLevel::Lossless`에서는 압축 완전 금지 (`AdaptiveCompressor::compress` 즉시 반환)
- `SymbolDict`는 문서당 독립 인스턴스 — 스레드 간 공유 금지
- PUA 기호 할당 한도(`U+F8FF`) 초과 시 `SymbolOverflowError` 반환
- `importance` 범위: `0.0..=1.0`

### 출력 포맷

```
<D>          ← SymbolDict 사전 (치환 없으면 생략)
SymA=용어A
</D>
<H>          ← YAML 헤더 (t/s/k 키만 사용)
t: 제목
s: 요약
k: [kw1, kw2]
</H>
<B>          ← 본문 (압축·치환 적용)
...
</B>
```

### 압축 단계 (예산 소모율 기준)

| 소모율 | `CompressionStage` | 동작 |
|--------|-------------------|------|
| 0–60% | `StopwordOnly` | 불용어 제거 |
| 60–80% | `PruneLowImportance` | + 중요도 하위 20% 단락 제거 |
| 80–95% | `DeduplicateAndLinearize` | + 중복 문장 제거 |
| 95%+ | `MaxCompression` | + 단락 → 첫 문장만 유지 (Semantic 이상) |
