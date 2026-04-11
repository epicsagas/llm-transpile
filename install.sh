#!/usr/bin/env bash
# install.sh — llm-transpile one-command setup
#
# Installs the transpile binary and wires it into whichever AI coding tools
# are present on this machine (Claude Code, Gemini CLI, Codex, Cursor, OpenCode).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/epicsagas/llm-transpile/main/install.sh | bash
#   # or, after cloning:
#   bash install.sh

set -euo pipefail

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[llm-transpile]${RESET} $*"; }
success() { echo -e "${GREEN}[llm-transpile]${RESET} $*"; }
warn()    { echo -e "${YELLOW}[llm-transpile]${RESET} $*"; }
error()   { echo -e "${RED}[llm-transpile]${RESET} $*" >&2; }

# ── 1. Install binary ──────────────────────────────────────────────────────────
install_binary() {
  info "Installing transpile binary..."

  if command -v transpile &>/dev/null; then
    local ver
    ver=$(transpile --version 2>/dev/null || echo "unknown")
    warn "transpile already installed ($ver) — skipping. Run 'cargo install llm-transpile' to upgrade."
    return 0
  fi

  if command -v cargo &>/dev/null; then
    cargo install llm-transpile
    success "transpile installed via cargo."
  else
    error "cargo not found. Install Rust first: https://rustup.rs"
    exit 1
  fi
}

# ── 2. Shell profile ───────────────────────────────────────────────────────────
detect_profile() {
  if [[ -n "${BASH_VERSION:-}" ]]; then
    echo "${HOME}/.bashrc"
  elif [[ -n "${ZSH_VERSION:-}" ]]; then
    echo "${HOME}/.zshrc"
  elif [[ -f "${HOME}/.zshrc" ]]; then
    echo "${HOME}/.zshrc"
  elif [[ -f "${HOME}/.bashrc" ]]; then
    echo "${HOME}/.bashrc"
  else
    echo "${HOME}/.profile"
  fi
}

append_to_profile() {
  local profile="$1"
  local marker="# llm-transpile"
  if grep -q "$marker" "$profile" 2>/dev/null; then
    warn "Shell wrappers already in $profile — skipping."
    return 0
  fi

  cat >> "$profile" << 'EOF'

# llm-transpile — AI tool wrappers (added by install.sh)
# Compress a document and print to stdout
tctx() { transpile --input "$1" --fidelity "${2:-semantic}" --quiet; }

# Pipe stdin through transpile
talias() { transpile --format "${1:-markdown}" --fidelity "${2:-semantic}" --quiet; }

# Wrap any LLM CLI: compress a file, then pass to the CLI
# Usage: trun <file> <cli-command> [cli-args...]
trun() {
  local file="$1"; shift
  transpile --input "$file" --quiet | "$@"
}
EOF

  success "Shell wrappers added to $profile"
  info "  tctx <file>              — compress a file to stdout"
  info "  talias                   — pipe stdin through transpile"
  info "  trun <file> <cmd> [args] — compress then hand off to any CLI"
}

# ── 3. Claude Code ─────────────────────────────────────────────────────────────
setup_claude_code() {
  local settings="${HOME}/.claude/settings.json"

  if ! command -v claude &>/dev/null && [[ ! -f "$settings" ]]; then
    return 0
  fi

  info "Claude Code detected — configuring hooks..."

  mkdir -p "${HOME}/.claude"

  if [[ ! -f "$settings" ]]; then
    echo '{}' > "$settings"
  fi

  # Inject a PostToolUse hook that logs token savings when Read is used on large files.
  # Uses python3 for portable JSON editing without requiring jq.
  python3 - "$settings" << 'PYEOF'
import json, sys

path = sys.argv[1]
with open(path) as f:
    cfg = json.load(f)

hooks = cfg.setdefault("hooks", {})
post = hooks.setdefault("PostToolUse", [])

marker = "llm-transpile"
if any(marker in str(h) for h in post):
    print("[llm-transpile] Claude Code hook already present — skipping.")
    sys.exit(0)

post.append({
    "_comment": marker,
    "matcher": "Read",
    "hooks": [{
        "type": "command",
        # Warns in Claude Code's output when a read file is large enough to benefit from transpile
        "command": (
            "bash -c '"
            "bytes=$(wc -c < \"$CLAUDE_TOOL_RESULT\" 2>/dev/null || echo 0); "
            "if [ \"$bytes\" -gt 8192 ]; then "
            "  echo \"[transpile] hint: $(basename $CLAUDE_TOOL_INPUT_FILE_PATH 2>/dev/null) "
            "is ${bytes}B — run: transpile --input <file> --quiet | pbcopy\" >&2; "
            "fi'"
        )
    }]
})

with open(path, "w") as f:
    json.dump(cfg, f, indent=2)

print("[llm-transpile] Claude Code hook added to " + path)
PYEOF

  # Add a project-level slash command
  mkdir -p "${HOME}/.claude/commands"
  cat > "${HOME}/.claude/commands/tctx.md" << 'EOF'
---
name: tctx
description: Compress a file through llm-transpile and insert into context
---

Run the following shell command and paste the output into the conversation:

```bash
transpile --input $ARGUMENTS --quiet
```

Use this when you want to include a large document in context without consuming excess tokens.
EOF

  success "Claude Code: hook + /tctx command installed."
}

# ── 4. Gemini CLI ──────────────────────────────────────────────────────────────
setup_gemini() {
  if ! command -v gemini &>/dev/null; then return 0; fi

  info "Gemini CLI detected — adding tgemini wrapper..."
  local profile
  profile=$(detect_profile)

  local marker="tgemini()"
  if grep -q "$marker" "$profile" 2>/dev/null; then
    warn "tgemini already in $profile — skipping."; return 0
  fi

  cat >> "$profile" << 'EOF'

# llm-transpile: gemini wrapper — compress file then query
# Usage: tgemini <file> "<prompt>"
tgemini() {
  local file="$1"; shift
  transpile --input "$file" --fidelity compressed --quiet | gemini "$@"
}
EOF
  success "Gemini CLI: tgemini wrapper added to $profile"
}

# ── 5. OpenAI Codex CLI ────────────────────────────────────────────────────────
setup_codex() {
  if ! command -v codex &>/dev/null; then return 0; fi

  info "Codex CLI detected — adding tcodex wrapper..."
  local profile
  profile=$(detect_profile)

  local marker="tcodex()"
  if grep -q "$marker" "$profile" 2>/dev/null; then
    warn "tcodex already in $profile — skipping."; return 0
  fi

  cat >> "$profile" << 'EOF'

# llm-transpile: codex wrapper — compress file then query
# Usage: tcodex <file> "<prompt>"
tcodex() {
  local file="$1"; shift
  local tmp
  tmp=$(mktemp /tmp/transpile.XXXXXX)
  transpile --input "$file" --fidelity compressed --quiet > "$tmp"
  codex --context "$tmp" "$@"
  rm -f "$tmp"
}
EOF
  success "Codex CLI: tcodex wrapper added to $profile"
}

# ── 6. Cursor ──────────────────────────────────────────────────────────────────
setup_cursor() {
  # Cursor doesn't have a CLI we can hook — instead we write a helper script
  # that regenerates .cursor/context.md from project docs.
  if [[ ! -d ".cursor" ]] && ! command -v cursor &>/dev/null; then return 0; fi

  info "Cursor detected — writing .cursor/transpile-ctx.sh..."
  mkdir -p ".cursor"

  cat > ".cursor/transpile-ctx.sh" << 'EOF'
#!/usr/bin/env bash
# Regenerate .cursor/context.md by compressing all project docs through transpile.
# Run this whenever your docs change, or add to a git pre-commit hook.
#
# Usage: bash .cursor/transpile-ctx.sh

set -euo pipefail
OUT=".cursor/context.md"
> "$OUT"

for f in README.md ARCHITECTURE.md CLAUDE.md SPEC.md; do
  [[ -f "$f" ]] || continue
  echo "### $f" >> "$OUT"
  transpile --input "$f" --fidelity semantic --quiet >> "$OUT"
  echo "" >> "$OUT"
done

echo "[transpile] $OUT regenerated ($(wc -c < "$OUT") bytes)"
EOF

  chmod +x ".cursor/transpile-ctx.sh"
  success "Cursor: .cursor/transpile-ctx.sh written. Run it to build your context file."
  info "  Add to .cursorrules:  @.cursor/context.md"
}

# ── 7. OpenCode ────────────────────────────────────────────────────────────────
setup_opencode() {
  if ! command -v opencode &>/dev/null; then return 0; fi

  info "OpenCode detected — adding topencode wrapper..."
  local profile
  profile=$(detect_profile)

  local marker="topencode()"
  if grep -q "$marker" "$profile" 2>/dev/null; then
    warn "topencode already in $profile — skipping."; return 0
  fi

  cat >> "$profile" << 'EOF'

# llm-transpile: opencode wrapper — compress docs into system prompt
# Usage: topencode [file...] (launches opencode with compressed context)
topencode() {
  local ctx=""
  for f in "$@"; do
    ctx+=$(transpile --input "$f" --fidelity compressed --quiet)
    ctx+=$'\n---\n'
  done
  OPENCODE_SYSTEM_PROMPT="$ctx" opencode
}
EOF
  success "OpenCode: topencode wrapper added to $profile"
}

# ── Main ───────────────────────────────────────────────────────────────────────
main() {
  echo -e "${BOLD}llm-transpile installer${RESET}"
  echo "──────────────────────────────────────────"

  install_binary

  local profile
  profile=$(detect_profile)
  append_to_profile "$profile"

  setup_claude_code
  setup_gemini
  setup_codex
  setup_cursor
  setup_opencode

  echo ""
  echo -e "${BOLD}Done.${RESET} Restart your shell or run:"
  echo -e "  ${CYAN}source $profile${RESET}"
  echo ""
  echo -e "Quick start:"
  echo -e "  ${CYAN}tctx README.md${RESET}                     # compress to stdout"
  echo -e "  ${CYAN}trun README.md gemini 'summarize'${RESET}  # compress → gemini"
  echo -e "  ${CYAN}trun README.md codex  'implement'${RESET}  # compress → codex"
}

main "$@"
