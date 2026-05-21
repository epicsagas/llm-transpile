<div align="center">
<h1>llm-transpile</h1> 

<p align="center">
  <a href="https://github.com/epicsagas/llm-transpile/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=ffd700&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/network/members"><img alt="Forks" src="https://img.shields.io/github/forks/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=2ecc71&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/issues"><img alt="Issues" src="https://img.shields.io/github/issues/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=ff6b6b&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/commits/main"><img alt="Last commit" src="https://img.shields.io/github/last-commit/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=58a6ff&logo=git&logoColor=white" /></a>
</p>
<p align="center">
  <a href="https://crates.io/crates/llm-transpile"><img alt="Crates.io" src="https://img.shields.io/crates/v/llm-transpile?style=for-the-badge&labelColor=0d1117&color=fc8d62&logo=rust&logoColor=white" /></a>
  <a href="https://docs.rs/llm-transpile"><img alt="docs.rs" src="https://img.shields.io/docsrs/llm-transpile?style=for-the-badge&labelColor=0d1117&color=8e44ad&logo=docsdotrs&logoColor=white" /></a>
  <a href="../../LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-3fb950?style=for-the-badge&labelColor=0d1117" /></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.92+-d73a49?style=for-the-badge&labelColor=0d1117&logo=rust&logoColor=white" />
  <a href="https://buymeacoffee.com/epicsaga"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/buy_me_a_coffee-FFDD00?style=for-the-badge&labelColor=0d1117&logo=buymeacoffee&logoColor=black" /></a>
</p>

**LLM 파이프라인을 위한 토큰 최적화 문서 트랜스파일러**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

원본 문서(Markdown, HTML, 일반 텍스트) → 구조화된 브리지 포맷 `<D>?<H><B>` — 토큰 예산 내에 맞추는 적응형 압축 지원.

---

<details>
<summary>목차</summary>

- [왜 사용하는가](#왜-사용하는가)
- [설치](#설치)
- [업데이트](#업데이트)
- [CLI 사용법](#cli-사용법)
- [사용 통계](#사용-통계)
- [벤치마킹](#벤치마킹)
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

| | 기능 | 중요한 이유 |
|--|------|------------|
| 🏗️ | **구조적 파싱** | Markdown/HTML/일반 텍스트 → 타입이 지정된 IR 노드(제목, 단락, 표, 목록, 코드 블록) |
| 📉 | **적응형 압축** | 토큰 예산이 소진될수록 4단계를 자동으로 에스컬레이션 |
| 🔣 | **심볼 치환** | 반복되는 도메인 용어 → 유니코드 PUA 문자, `<D>` 사전 헤더로 복원 |
| 📊 | **표 선형화** | Markdown 표 → 간결한 `Key:Val`(≤5행) 또는 파이프 구분 행으로 변환 |
| 🌊 | **스트리밍 출력** | Tokio 스트림이 첫 번째 청크를 즉시 전달해 TTFT 최소화 |

---

## 설치

### 라이브러리 (Rust 크레이트)

```toml
[dependencies]
llm-transpile = "0.1"
```

**Rust 1.92+** 필요.

### CLI 바이너리 + 도구 연동

**macOS / Linux**

```bash
brew install epicsagas/tap/llm-transpile
```

Homebrew가 없다면 설치 스크립트를 사용하세요:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

**Windows**

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

**Rust 도구 체인**

```bash
cargo binstall llm-transpile   # 사전 빌드 바이너리 (빠름)
cargo install llm-transpile    # 소스에서 빌드
```

도구 연동 설정:

```bash
transpile install
```

`transpile install`은 설치된 도구를 감지해 자동으로 설정하는 대화형 마법사를 실행합니다:

| 도구 | 연동 방식 | 동작 |
|------|-----------|------|
| **Claude Code** | PostToolUse 훅 | Read 시 `.md/.html/.txt` 파일 자동 압축 |
| **Antigravity** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |
| **Codex CLI** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |
| **Cursor** | `.mdc` 규칙 (`alwaysApply`) | 문서 파일 읽기 전 `transpile` 실행 |
| **OpenCode** | `SKILL.md` | LLM이 문서 파일 확장자에 `transpile` 자동 실행 |

Claude Code가 아닌 모든 도구는 LLM이 `TRANSPILE_AGENT=<agent> transpile --input <file>`을 자동으로 실행하도록 가이드하는 스킬 파일을 사용합니다. 크기 검사가 필요 없으며, 확장자만으로 트리거됩니다.

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

다음 세션 시작 시 바이너리를 자동 설치하고 PostToolUse 훅을 구성합니다 — 추가 설정이 필요 없습니다.

소스에서 설치:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## 업데이트

| 방법 | 명령어 |
|--------|---------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShell 설치 | 위의 설치 명령어 재실행 |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## CLI 사용법

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       입력 파일 경로 (생략 시 stdin에서 읽음)
  -f, --format <FORMAT>    입력 포맷: markdown | html | plaintext  [기본값: markdown]
                           --input 사용 시 파일 확장자로 자동 감지
  -l, --fidelity <LEVEL>   압축 수준: lossless | semantic | compressed  [기본값: semantic]
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

> 통계(`[273 → 150 tok  45.1% reduction]`)는 기본적으로 **stderr**로 출력되어 stdout은 파이프용으로 깔끔하게 유지됩니다. `--quiet`로 숨기거나 `--stats`로 stdout에 출력할 수 있습니다.

---

## 사용 통계

`transpile`을 실행할 때마다 `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`에 레코드가 자동으로 추가됩니다. `transpile stats` 서브커맨드로 해당 파일을 읽어 요약 테이블을 출력합니다.

```
transpile stats                # 오늘
transpile stats --days 7       # 최근 N일
transpile stats --agent claude # 에이전트별 필터
```

출력 예시:

```
transpile stats — 최근 7일

  날짜       에이전트    호출   입력 tok   출력 tok   절약    축소율
  ──────────────────────────────────────────────────────────────────
  2026-04-13  claude       5     14 965      10 872   4 093     27.3%
  2026-04-13  gemini       2      4 800       3 500   1 300     27.1%
  ──────────────────────────────────────────────────────────────────
  Total                    7     19 765       14 372   5 393     27.3%
```

**대화형 HTML 대시보드**

```bash
transpile stats report                 # 브라우저에서 열기 (기본값: 최근 7일)
transpile stats report --days 30       # 최근 30일
transpile stats report --no-open       # 열지 않고 생성만 하기
transpile stats report --out /tmp/custom.html
```

> 리포트는 기본적으로 `~/.agents/transpile/reports/`에 생성됩니다. `--out`으로 덮어쓸 수 있습니다.

대시보드 포함 내용:

- **KPI 카드** — 총 호출, 절약된 토큰, 평균 축소율, 고유 파일, 에이전트, 활성 일수
- **6개 차트** — 일별 토큰 사용량, 충실도 비율, 입출력 추세, 에이전트 분포, 시간대별 패턴, 축소율 분포
- **날짜 범위 프리셋** — 원클릭 필터링: `오늘` · `1주` · `2주` · `1개월` · `90일` (기본값: 1주)
- **필터** — 프로젝트, 에이전트, 파일 텍스트 필터 및 CSV 내보내기
- **테마 토글** — 지속성 있는 다크 / 라이트 모드 설정
- **이중 언어** — 한국어 로캘 자동 감지; 수동 한/EN 토글

**JSONL 레코드 필드**

| 필드 | 타입 | 설명 |
|------|------|------|
| `ts` | ISO 8601 | 실행 타임스탬프 |
| `agent` | 문자열 | 호출을 트리거한 도구 (`claude`, `gemini`, `codex`, `opencode`) |
| `file` | 문자열 | 입력 파일 경로 (stdin 읽기 시 빈 값) |
| `format` | 문자열 | `markdown`, `html`, 또는 `plaintext` |
| `fidelity` | 문자열 | `lossless`, `semantic`, 또는 `compressed` |
| `input_tok` | 정수 | 트랜스파일 전 토큰 수 |
| `output_tok` | 정수 | 트랜스파일 후 토큰 수 |
| `reduction_pct` | 실수 | 절약된 토큰 비율 |
| `saved` | 정수 | 절약된 절대 토큰 수 (`input_tok − output_tok`) |

**`TRANSPILE_AGENT` 환경변수**

`agent` 필드는 `TRANSPILE_AGENT` 환경변수에서 가져옵니다. 각 연동 도구가 자동으로 설정합니다(`claude`, `gemini`, `codex`, `opencode`, `cursor`). 수동으로 설정할 수도 있습니다:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

### 벤치마킹

```bash
# 테스트 파일 디렉토리에 대해 벤치마크 실행
transpile bench run --dataset ./eval                    # JSONL 로그 생성
transpile bench run --dataset ./eval --report           # 실행 + HTML 리포트 열기
transpile bench report                                  # 로그에서 리포트 재생성
```

HTML 벤치마크 리포트 포함 내용:

- **KPI 카드** — semantic 축소율, compressed 축소율, 처리량 (tok/ms), 단어 커버리지, 총 입력 토큰, 실행 횟수
- **7개 차트** — 시간에 따른 축소율 추세, 실행별 처리량, semantic 대비 처리량 산점도, 포맷별 박스 플롯, 포맷 분포, 토큰 크기 히스토그램, 단어 커버리지 도넛
- **실행 테이블** — 집계 지표가 포함된 실행별 요약
- **레코드 테이블** — 포맷, 실행, 파일명 필터가 있는 파일별 상세 정보
- **테마 토글** — 지속성 있는 다크 / 라이트 모드 설정
- **이중 언어** — 한국어 로캘 자동 감지; 수동 한/EN 토글

---

## 라이브러리 사용법

### 동기식

```rust
use llm_transpiler::{transpile, FidelityLevel, InputFormat};

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
use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};
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
let n = llm_transpiler::token_count("Hello, world!");
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
use llm_transpiler::TranspileError;

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

자세한 가이드라인은 [CONTRIBUTING.md](../../CONTRIBUTING.md)를 참조하세요. PR 환영 — `good first issue` 라벨이 있는 오픈 이슈를 확인해 보세요.

---

## 라이선스

Apache-2.0 — [LICENSE](../../LICENSE) 참조.
