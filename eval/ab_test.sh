#!/usr/bin/env bash
# ab_test.sh — Raw vs Transpiled A/B 품질 비교
#
# Usage:
#   ab_test.sh @doc.md "질문"
#   ab_test.sh @doc.md "질문" --llm gemini
#   ab_test.sh @doc.md "질문" --llm gemini,opencode
#   ab_test.sh @doc.md "질문" --fidelity compressed
#   ab_test.sh @doc.md "질문" --all --judge
#
# 지원 LLM: gemini, opencode  (설치된 것 자동 감지)
# 각 LLM은 "@<tmpfile> 질문" 형태로 직접 호출됨

set -euo pipefail

BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'
C_RED='\033[0;31m'; C_GREEN='\033[0;32m'; C_YELLOW='\033[1;33m'
C_CYAN='\033[0;36m'; C_BLUE='\033[0;34m'; C_MAG='\033[0;35m'

# ── 기본값 ──────────────────────────────────────────────────────────────────
DOC=""
QUESTION=""
FIDELITY="semantic"
ALL_MODE=false
JUDGE_MODE=false
BUDGET=""
LLM_LIST=""
TRANSPILE_BIN="$(cd "$(dirname "$0")/.." && pwd)/target/debug/transpile"

# ── 인수 파싱 ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case $1 in
    @*)            DOC="${1#@}"; shift ;;
    --doc|-d)      DOC="$2"; shift 2 ;;
    --llm)         LLM_LIST="$2"; shift 2 ;;
    --fidelity|-l) FIDELITY="$2"; shift 2 ;;
    --budget|-b)   BUDGET="$2"; shift 2 ;;
    --all|-a)      ALL_MODE=true; shift ;;
    --judge|-j)    JUDGE_MODE=true; shift ;;
    --transpile)   TRANSPILE_BIN="$2"; shift 2 ;;
    --help|-h)     sed -n '2,12p' "$0" | sed 's/^# \?//'; exit 0 ;;
    -*)            echo "Unknown option: $1"; exit 1 ;;
    *)
      if [[ -z "$DOC" && -f "$1" ]]; then DOC="$1"
      elif [[ -z "$QUESTION" ]]; then QUESTION="$1"
      fi
      shift ;;
  esac
done

# ── 검증 ────────────────────────────────────────────────────────────────────
[[ -z "$DOC" ]]      && { echo -e "${C_RED}error:${RESET} @파일 경로 필요"; exit 1; }
[[ -z "$QUESTION" ]] && { echo -e "${C_RED}error:${RESET} 질문 필요"; exit 1; }
[[ ! -f "$DOC" ]]    && { echo -e "${C_RED}error:${RESET} 파일 없음: $DOC"; exit 1; }
[[ ! -x "$TRANSPILE_BIN" ]] && {
  echo -e "${C_RED}error:${RESET} transpile 바이너리 없음 — cargo build 먼저"
  exit 1
}

# ── LLM 자동 감지 ────────────────────────────────────────────────────────────
if [[ -z "$LLM_LIST" ]]; then
  LLMS=()
  command -v gemini   &>/dev/null && LLMS+=("gemini")
  command -v opencode &>/dev/null && LLMS+=("opencode")
  [[ ${#LLMS[@]} -eq 0 ]] && { echo -e "${C_RED}error:${RESET} gemini / opencode 중 하나 이상 필요"; exit 1; }
else
  IFS=',' read -ra LLMS <<< "$LLM_LIST"
fi

# ── LLM 호출 ─────────────────────────────────────────────────────────────────
# raw=true  → "@파일 질문" (원본 파일 직접 참조)
# raw=false → "Context:\n$내용\n\nQuestion: $질문" (transpiled 내용 embed)
call_llm() {
  local llm="$1"
  local file="$2"
  local question="$3"
  local is_raw="${4:-false}"   # raw 일 때만 @file 사용

  case "$llm" in
    gemini)
      if [[ "$is_raw" == "true" ]]; then
        # 원본: @파일 경로를 프롬프트에 직접
        local abs_file
        abs_file="$(cd "$(dirname "$file")" && pwd)/$(basename "$file")"
        gemini -p "@${abs_file} ${question}" --output-format text 2>/dev/null
      else
        # Transpiled: 내용을 프롬프트에 embed (bridge format 직접 주입)
        local content
        content=$(cat "$file")
        gemini -p "Answer using ONLY the context below. Do not use any tools.\n\n${content}\n\nQuestion: ${question}" \
          --output-format text 2>/dev/null
      fi
      ;;
    opencode)
      local abs_file
      abs_file="$(cd "$(dirname "$file")" && pwd)/$(basename "$file")"
      opencode run --file "$abs_file" "$question" 2>/dev/null \
        | sed 's/\x1B\[[0-9;]*[mKJH]//g; s/\r//g'
      ;;
    *)
      echo -e "${C_RED}[unsupported: $llm]${RESET}"
      ;;
  esac
}

# ── 헬퍼 ────────────────────────────────────────────────────────────────────
separator() { echo -e "${DIM}$(printf '─%.0s' {1..70})${RESET}"; }
print_header() { echo ""; echo -e "${BOLD}${C_CYAN}$1${RESET}"; separator; }
tok_approx() { echo $(( ${#1} / 4 )); }

# ── 임시 파일 관리 ────────────────────────────────────────────────────────────
# Gemini @file 은 프로젝트 디렉토리 내 파일만 인식하므로 eval/ 아래에 생성
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TMP_DIR="${SCRIPT_DIR}/.tmp"
mkdir -p "$TMP_DIR"

TMPFILES=()
cleanup() { for f in "${TMPFILES[@]}"; do rm -f "$f"; done; }
trap cleanup EXIT

make_tmpfile() {
  local ext="${1:-md}"
  local tmp
  tmp=$(mktemp "${TMP_DIR}/ab_XXXX.${ext}")
  TMPFILES+=("$tmp")
  echo "$tmp"
}

# ── 변환 ────────────────────────────────────────────────────────────────────
transpile_to_file() {
  local fid="$1" dst="$2"
  local budget_args=()
  [[ -n "$BUDGET" ]] && budget_args=(--budget "$BUDGET")
  "$TRANSPILE_BIN" --input "$DOC" --fidelity "$fid" "${budget_args[@]}" 2>/dev/null > "$dst"
}

# ── 준비 ────────────────────────────────────────────────────────────────────
RAW_TEXT=$(cat "$DOC")
RAW_TOK=$(tok_approx "$RAW_TEXT")

echo ""
echo -e "${BOLD}📄 ${C_CYAN}$(basename "$DOC")${RESET}  ${DIM}~${RAW_TOK} tok${RESET}"
echo -e "${BOLD}❓ ${RESET}$QUESTION"
echo -e "${DIM}🤖 $(IFS=','; echo "${LLMS[*]}")${RESET}"

$ALL_MODE && FIDELITIES=("semantic" "compressed") || FIDELITIES=("$FIDELITY")

# Transpiled 파일 미리 생성
declare -A TRANS_FILES TRANS_TOKS
for fid in "${FIDELITIES[@]}"; do
  dst=$(make_tmpfile "md")
  transpile_to_file "$fid" "$dst"
  TRANS_FILES["$fid"]="$dst"
  text=$(cat "$dst")
  TRANS_TOKS["$fid"]=$(tok_approx "$text")
done

# ── A/B 실행 ─────────────────────────────────────────────────────────────────
declare -A RAW_ANS TRANS_ANS  # key: llm  /  llm:fid

for llm in "${LLMS[@]}"; do
  [[ "$llm" == "gemini" ]] && LC="$C_BLUE" || LC="$C_MAG"
  echo ""; echo -e "${BOLD}${LC}══ ${llm^^} ══${RESET}"

  # RAW — @파일 직접 참조
  print_header "🔵 RAW  ~${RAW_TOK} tok  (@파일)"
  echo ""
  ans=$(call_llm "$llm" "$DOC" "$QUESTION" "true")
  echo "$ans"
  RAW_ANS["$llm"]="$ans"

  # Transpiled — bridge format 내용 embed
  for fid in "${FIDELITIES[@]}"; do
    ttok="${TRANS_TOKS[$fid]}"
    pct=$(awk "BEGIN{printf \"%.1f\",(1-$ttok/$RAW_TOK)*100}")
    case $fid in
      lossless)   fl="🟡 LOSSLESS"  ; fc="$C_YELLOW" ;;
      semantic)   fl="🟢 SEMANTIC"  ; fc="$C_GREEN"  ;;
      compressed) fl="🔴 COMPRESSED"; fc="$C_RED"    ;;
    esac
    print_header "${fl}  ~${RAW_TOK} → ~${ttok} tok  (${fc}-${pct}%${RESET}${C_CYAN})  (embed)"
    echo ""
    ans=$(call_llm "$llm" "${TRANS_FILES[$fid]}" "$QUESTION" "false")
    echo "$ans"
    TRANS_ANS["${llm}:${fid}"]="$ans"
  done

  # Judge
  if $JUDGE_MODE; then
    cmp_fid="${FIDELITIES[-1]}"
    ttok="${TRANS_TOKS[$cmp_fid]}"
    pct=$(awk "BEGIN{printf \"%.1f\",(1-$ttok/$RAW_TOK)*100}")
    print_header "⚖️  JUDGE  Raw(A) vs ${cmp_fid^^}(B)  tok -${pct}%"
    echo ""

    judge_tmp=$(make_tmpfile "txt")
    cat > "$judge_tmp" <<EOF
You are an expert evaluator. Score these two answers.

Question: ${QUESTION}

=== Answer A (Raw document) ===
${RAW_ANS[$llm]}

=== Answer B (${cmp_fid^^} compressed) ===
${TRANS_ANS[${llm}:${cmp_fid}]}

Rate accuracy and completeness 0-10. Output ONLY:
A: <score>/10
B: <score>/10
Winner: <A or B or Tie>
Reason: <one sentence>
EOF
    call_llm "$llm" "$judge_tmp" "Follow the evaluation instructions in this file exactly."
    echo -e "\n${DIM}A=Raw  B=${cmp_fid^^}${RESET}"
  fi
done

separator
echo -e "\n${DIM}완료.  Raw: ~${RAW_TOK} tok  |  $(basename "$DOC")${RESET}\n"
