# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**LLM 파이프라인을 위한 토큰 최적화 문서 트랜스파일러**

원본 문서(Markdown, HTML, 일반 텍스트) → 구조화된 브리지 포맷 `<D>?<H><B>` — 토큰 예산 내에 맞추는 적응형 압축 지원.

```
<H>
t: 소프트웨어 라이선스 계약
s: 라이선서와 라이선시 간의 연간 라이선스 조건
k: [라이선스, 계약, 소프트웨어]
</H>
<B>
# 계약 당사자
본 계약은 갑(라이선서)과 을(라이선시) 사이에 체결됩니다.
...
</B>
```

---

<details>
<summary>목차</summary>
- [왜 사용하는가](#왜-사용하는가)
- [설치](#설치)
- [CLI 사용법](#cli-사용법)
- [라이브러리 사용법](#라이브러리-사용법)
- [출력 포맷](#출력-포맷)
- [충실도 수준](#충실도-수준)
- [적응형 압축](#적응형-압축)
- [입력 포맷](#입력-포맷)
- [에러 처리](#에러-처리)
- [성능](#성능)
- [기여](#기여)
- [라이선스](#라이선스)
</details>

---

## 왜 사용하는가

LLM은 컨텍스트가 깔끔하고 밀도 높을 때 더 잘 작동합니다. 이 라이브러리가 기계적인 작업을 대신 처리합니다:

- **구조적 파싱** — Markdown/HTML/일반 텍스트 → 타입이 지정된 IR 노드(제목, 단락, 표, 목록, 코드 블록)
- **적응형 압축** — 토큰 예산이 소진될수록 4단계를 자동으로 에스컬레이션
- **심볼 치환** — 반복되는 도메인 용어 → 유니코드 PUA 문자, `<D>` 사전 헤더로 복원
- **표 선형화** — Markdown 표 → 간결한 `Key:Val` 시퀀스(≤5행) 또는 파이프 구분 행(`h1|h2\nv1|v2`)
- **스트리밍 출력** — Tokio 스트림이 첫 번째 청크를 즉시 전달해 TTFT 최소화

---

## 설치

### 라이브러리 (Rust 크레이트)

```toml
[dependencies]
llm-transpile = "0.1"
```

**Rust 1.75+** 필요.

### CLI 바이너리 + 도구 연동

```bash
# Homebrew (macOS)
brew tap epicsagas/tap
brew install llm-transpile

# 사전 빌드 바이너리 (컴파일 없이 빠르게)
cargo binstall llm-transpile

# crates.io에서 설치
cargo install llm-transpile
```

도구 연동 설정:

```bash
transpile install
```

`transpile install`은 설치된 도구를 감지해 자동으로 설정하는 대화형 마법사를 실행합니다:

| 도구 | 연동 방식 | 동작 |
|------|-----------|------|
| **Claude Code** | PostToolUse 훅 | Read 시 `.md/.html/.txt` 파일 자동 압축 |
| **Gemini CLI** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |
| **Codex CLI** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |
| **Cursor** | `.mdc` 규칙 (`alwaysApply`) | 문서 파일 읽기 전 `transpile` 실행 |
| **OpenCode** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |

**선택적 설치 / 제거**

```bash
transpile install claude gemini    # 특정 도구만
transpile install --all            # 전체 설치
transpile install --dry-run        # 미리보기
transpile install --list           # 연동 상태 확인

transpile uninstall cursor         # 하나 제거
transpile uninstall --all          # 전체 제거
transpile uninstall --dry-run      # 제거 미리보기
```

**Claude Code 플러그인**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

소스에서 설치:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## CLI 사용법

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       입력 파일 경로 (생략 시 stdin에서 읽음)
  -f, --format <FORMAT>    입력 포맷: markdown | html | plaintext  [기본값: markdown]
                           --input 사용 시 파일 확장자로 자동 감지
  -l, --fidelity <LEVEL>  압축 수준: lossless | semantic | compressed  [기본값: semantic]
  -b, --budget <N>         토큰 예산 상한 (생략 시 무제한)
  -c, --count              입력 토큰 수만 출력하고 종료
  -j, --json               JSON 형식으로 출력 {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              stderr 통계 줄 숨김
      --stats              내용 다음에 통계를 stdout으로 출력 (단일 스트림 캡처용)
  -h, --help               도움말 출력
  -V, --version            버전 출력
```

**예시**

```bash
# Markdown 파일 변환 (.md 확장자로 포맷 자동 감지)
transpile --input doc.md

# stdin에서 읽기 — stdout은 깔끔하게, 통계는 stderr로
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# 파이프 연결 — 통계 완전히 숨김
transpile --input doc.md --quiet | send_to_llm_api

# 변환 없이 토큰 수 확인
transpile --input doc.md --count

# 스크립트/파이프라인용 JSON 출력
transpile --input doc.md --json | jq '.reduction_pct'

# 내용 + 통계를 한 스트림으로 캡처
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — 압축 없음, 전체 내용 보존 (법률/감사 문서)
transpile --input contract.md --fidelity lossless

# 512 토큰 예산으로 공격적 압축
transpile --input article.md --fidelity compressed --budget 512
```

> 통계(`[273 → 150 tok  45.1% reduction]`)는 기본적으로 **stderr**로 출력되어 stdout은 파이프용으로 깨끗하게 유지됩니다. `--quiet`로 숨기거나 `--stats`로 stdout에 출력할 수 있습니다.

---

## 라이브러리 사용법

### 동기식

```rust
use llm_transpile::{transpile, FidelityLevel, InputFormat};

let md = r#"
# Software License Agreement

This agreement is made between Licensor and Licensee.

| Item     | Cost  |
|----------|-------|
| Base fee | $800  |
| Support  | $200  |
"#;

let output = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))?;
println!("{}", output);
```

### 스트리밍 (Tokio)

```rust
use llm_transpile::{transpile_stream, FidelityLevel, InputFormat};
use futures::StreamExt;

let mut stream = transpile_stream(input, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
    if chunk.is_final { break; }
}
```

### 토큰 수 추정

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## 출력 포맷

```
<D>                  ← 심볼 사전 (치환 없을 때 생략)
{sym}=반복-용어
</D>
<H>                  ← YAML형 메타데이터 헤더
t: 문서 제목
s: 한 줄 요약
k: [키워드1, 키워드2]
</H>
<B>                  ← 문서 본문 (압축 + 치환 적용)
...내용...
</B>
```

`<D>` 블록은 유니코드 사용자 정의 영역 문자(`U+E000–U+F8FF`)를 심볼 핸들로 사용해 일반 텍스트와 충돌을 방지합니다. 사전은 문서당 최대 **6,400개** 고유 용어를 지원합니다.

---

## 충실도 수준

| 수준 | 일반적인 사용 사례 | 적용되는 압축 |
|------|-------------------|---------------|
| `Lossless` | 법률/감사 문서 | 없음 — 원본 내용 보장 |
| `Semantic` | 일반 RAG 파이프라인 | 불용어 제거 + 중요도 낮은 항목 제거 |
| `Compressed` | 요약, 엄격한 예산 | 최대 압축, 첫 문장 추출 |

---

## 적응형 압축

압축기는 예산 사용량을 실시간으로 모니터링하고 자동으로 단계를 에스컬레이션합니다:

| 예산 사용량 | 단계 | 동작 |
|------------|------|------|
| 0–60% | `StopwordOnly` | 영어/한국어 불용어 제거 |
| 60–80% | `PruneLowImportance` | 중요도 하위 20% 단락 제거 |
| 80–95% | `DeduplicateAndLinearize` | 중복 문장 제거; 표 선형화 |
| 95%+ | `MaxCompression` | 각 단락을 첫 문장으로 단축 |

> `Lossless` 모드는 모든 압축 단계를 무조건 건너뜁니다.

스트리밍 중 예산 사용량이 80%를 넘으면 나머지 노드는 자동으로 `Compressed` 모드로 전환됩니다.

---

## 입력 포맷

| `InputFormat` | 파서 |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM 표 |
| `Html` | ammonia 정제 → 태그 제거 → 일반 텍스트 파이프라인 |
| `PlainText` | 빈 줄 기준 단락 분리 |

---

## 에러 처리

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* 출력 사용 */ }
    Err(TranspileError::Parse(msg))            => eprintln!("파싱 실패: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("고유 용어 초과: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("Lossless 모드에서 압축 시도"),
    Err(e)                                     => eprintln!("에러: {e}"),
}
```

---

## 성능

릴리스 빌드(`cargo build --release`), Apple M 시리즈, Markdown/HTML/PlainText 48개 문서 기준:

| 지표 | 측정값 | 비고 |
|------|--------|------|
| 처리량 | **10,975 tok/ms** | Python 파싱 기준 대비 ≈75배 빠름 |
| Semantic 축소율 | **33.9%** (Markdown) | 15–30% 목표 달성 |
| Compressed 축소율 | **39.7%** (Markdown) | 예산 적응형, PruneLowImportance 이상 보장 |
| Lossless 단어 커버리지 | **98.8% 평균** | 모든 포맷 및 언어 기준 |
| HTML 축소율 | **97.6%** | 네비게이션/스크립트/스타일 마크업 오버헤드 제거 |
| 다국어 지원 | 15개 언어 테스트 | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 평균 99.4% 단어 커버리지 |

직접 평가 스위트 실행:

```bash
cargo run --release --example eval
```

---

## 기여

버그 리포트, 기능 요청, 풀 리퀘스트 모두 환영합니다.

```bash
# 클론 및 빌드
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# 테스트 실행
cargo test

# 벤치마크 실행 (HTML 리포트 → target/criterion/)
cargo bench

# 린트 및 포맷
cargo clippy -- -D warnings
cargo fmt
```

**가이드라인**

- MSRV를 Rust 1.75로 유지 — 이후에 도입된 기능 사용 금지.
- 새로운 압축 동작이 `Lossless` 모드에 영향을 주어서는 안 됩니다.
- 각 PR에는 관련 모듈(`ir`, `compressor`, `symbol`, `renderer`)의 새 로직에 대한 테스트를 포함해야 합니다.
- 제출 전 `cargo clippy -- -D warnings`와 `cargo fmt`를 실행하세요.

---

## 라이선스

Apache-2.0 — [LICENSE](LICENSE) 참조.
