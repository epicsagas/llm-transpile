#!/usr/bin/env bash
# install.sh — llm-transpile setup / update / uninstall
#
# Usage:
#   bash install.sh              # install or update
#   bash install.sh install      # same
#   bash install.sh uninstall    # remove everything this script added
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/epicsagas/llm-transpile/main/install.sh | bash

set -euo pipefail

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[llm-transpile]${RESET} $*"; }
success() { echo -e "${GREEN}✓${RESET} $*"; }
warn()    { echo -e "${YELLOW}~${RESET} $*"; }
error()   { echo -e "${RED}✗${RESET} $*" >&2; }

MARKER_BEGIN="# >>> llm-transpile"
MARKER_END="# <<< llm-transpile"

# ── Shell profile detection ────────────────────────────────────────────────────
detect_profile() {
  if [[ -f "${HOME}/.zshrc" ]];   then echo "${HOME}/.zshrc"
  elif [[ -f "${HOME}/.bashrc" ]]; then echo "${HOME}/.bashrc"
  else echo "${HOME}/.profile"
  fi
}

# ── Shell profile block management ────────────────────────────────────────────
# Replace (or insert) the BEGIN…END block in the given profile file.
# If the block doesn't exist yet, it is appended.
upsert_block() {
  local profile="$1"
  local content="$2"          # what goes between BEGIN and END

  local full_block
  printf -v full_block '\n%s\n%s\n%s\n' "$MARKER_BEGIN" "$content" "$MARKER_END"

  if grep -q "$MARKER_BEGIN" "$profile" 2>/dev/null; then
    # Replace the existing block using Python (portable, no temp-file race)
    python3 - "$profile" "$MARKER_BEGIN" "$MARKER_END" "$full_block" << 'PYEOF'
import sys
path, begin, end, replacement = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
with open(path) as f:
    text = f.read()
import re
pattern = re.escape(begin) + r'.*?' + re.escape(end)
new_text = re.sub(pattern, replacement.strip(), text, flags=re.DOTALL)
with open(path, 'w') as f:
    f.write(new_text)
PYEOF
    success "Updated shell block in $profile"
  else
    printf '%s' "$full_block" >> "$profile"
    success "Added shell block to $profile"
  fi
}

# Remove the BEGIN…END block from the profile.
remove_block() {
  local profile="$1"
  if ! grep -q "$MARKER_BEGIN" "$profile" 2>/dev/null; then
    return 0
  fi
  python3 - "$profile" "$MARKER_BEGIN" "$MARKER_END" << 'PYEOF'
import sys, re
path, begin, end = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path) as f:
    text = f.read()
pattern = r'\n?' + re.escape(begin) + r'.*?' + re.escape(end) + r'\n?'
new_text = re.sub(pattern, '', text, flags=re.DOTALL)
with open(path, 'w') as f:
    f.write(new_text)
PYEOF
  success "Removed shell block from $profile"
}

# ── Claude Code settings.json ──────────────────────────────────────────────────
claude_settings="${HOME}/.claude/settings.json"

upsert_claude_hook() {
  [[ ! -d "${HOME}/.claude" ]] && ! command -v claude &>/dev/null && return 0
  info "Configuring Claude Code..."
  mkdir -p "${HOME}/.claude"
  [[ ! -f "$claude_settings" ]] && echo '{}' > "$claude_settings"

  python3 - "$claude_settings" << 'PYEOF'
import json, sys

path = sys.argv[1]
with open(path) as f:
    cfg = json.load(f)

hooks = cfg.setdefault("hooks", {})
post  = hooks.setdefault("PostToolUse", [])

MARKER = "llm-transpile"
NEW_HOOK = {
    "_id": MARKER,
    "matcher": "Read",
    "hooks": [{
        "type": "command",
        "command": (
            "bash -c '"
            "bytes=$(wc -c < \"$CLAUDE_TOOL_RESULT\" 2>/dev/null || echo 0); "
            "if [ \"$bytes\" -gt 8192 ]; then "
            "  echo \"[transpile] $(basename \\\"$CLAUDE_TOOL_INPUT_FILE_PATH\\\" 2>/dev/null) "
            "is ${bytes}B — consider: transpile --input <file> --quiet\" >&2; "
            "fi'"
        )
    }]
}

# Remove existing entry (update), then re-insert
hooks["PostToolUse"] = [h for h in post if h.get("_id") != MARKER]
hooks["PostToolUse"].append(NEW_HOOK)

with open(path, "w") as f:
    json.dump(cfg, f, indent=2)

print(f"  Claude Code hook upserted in {path}")
PYEOF

  # /tctx slash command
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
EOF
  success "Claude Code: hook + /tctx command configured"
}

remove_claude_hook() {
  [[ ! -f "$claude_settings" ]] && return 0
  info "Removing Claude Code hook..."
  python3 - "$claude_settings" << 'PYEOF'
import json, sys
path = sys.argv[1]
with open(path) as f:
    cfg = json.load(f)
MARKER = "llm-transpile"
if "hooks" in cfg and "PostToolUse" in cfg["hooks"]:
    cfg["hooks"]["PostToolUse"] = [
        h for h in cfg["hooks"]["PostToolUse"] if h.get("_id") != MARKER
    ]
    # Clean up empty arrays/objects
    if not cfg["hooks"]["PostToolUse"]:
        del cfg["hooks"]["PostToolUse"]
    if not cfg["hooks"]:
        del cfg["hooks"]
with open(path, "w") as f:
    json.dump(cfg, f, indent=2)
print(f"  Claude Code hook removed from {path}")
PYEOF
  rm -f "${HOME}/.claude/commands/tctx.md"
  success "Claude Code: hook + /tctx command removed"
}

# ── Shell wrappers block ───────────────────────────────────────────────────────
build_shell_block() {
  # Core helpers — always added
  local block
  block=$(cat << 'EOF'
# Core helpers
tctx()   { transpile --input "$1" --fidelity "${2:-semantic}" --quiet; }
talias() { transpile --format "${1:-markdown}" --fidelity "${2:-semantic}" --quiet; }
trun()   { local f="$1"; shift; transpile --input "$f" --quiet | "$@"; }
EOF
)

  # Per-tool wrappers — only when the tool is present
  if command -v gemini &>/dev/null; then
    block+=$'\n\n# Gemini CLI\n'
    block+='tgemini() { local f="$1"; shift; transpile --input "$f" --fidelity compressed --quiet | gemini "$@"; }'
  fi

  if command -v codex &>/dev/null; then
    block+=$'\n\n# Codex CLI\n'
    block+='tcodex() {
  local f="$1"; shift
  local tmp; tmp=$(mktemp /tmp/transpile.XXXXXX)
  transpile --input "$f" --fidelity compressed --quiet > "$tmp"
  codex --context "$tmp" "$@"; rm -f "$tmp"
}'
  fi

  if command -v opencode &>/dev/null; then
    block+=$'\n\n# OpenCode\n'
    block+='topencode() {
  local ctx=""
  for f in "$@"; do ctx+=$(transpile --input "$f" --fidelity compressed --quiet); ctx+=$'"'"'\n---\n'"'"'; done
  OPENCODE_SYSTEM_PROMPT="$ctx" opencode
}'
  fi

  echo "$block"
}

# ── Cursor helper script ───────────────────────────────────────────────────────
cursor_script=".cursor/transpile-ctx.sh"

upsert_cursor() {
  [[ ! -d ".cursor" ]] && ! command -v cursor &>/dev/null && return 0
  info "Configuring Cursor..."
  mkdir -p ".cursor"
  cat > "$cursor_script" << 'EOF'
#!/usr/bin/env bash
# Auto-generated by llm-transpile install.sh — safe to re-run.
# Regenerates .cursor/context.md by compressing project docs.
# Run whenever docs change, or wire into a git pre-commit hook.
set -euo pipefail
OUT=".cursor/context.md"
: > "$OUT"
for f in README.md ARCHITECTURE.md CLAUDE.md SPEC.md; do
  [[ -f "$f" ]] || continue
  printf '### %s\n' "$f" >> "$OUT"
  transpile --input "$f" --fidelity semantic --quiet >> "$OUT"
  printf '\n' >> "$OUT"
done
echo "[transpile] $OUT regenerated ($(wc -c < "$OUT") bytes)"
EOF
  chmod +x "$cursor_script"
  success "Cursor: $cursor_script written (add '@.cursor/context.md' to .cursorrules)"
}

remove_cursor() {
  [[ -f "$cursor_script" ]] && rm -f "$cursor_script" && success "Cursor: $cursor_script removed"
}

# ── Install / Update ───────────────────────────────────────────────────────────
cmd_install() {
  echo -e "${BOLD}llm-transpile — install / update${RESET}"
  echo "──────────────────────────────────────────"

  # Binary
  if command -v transpile &>/dev/null; then
    warn "transpile already installed ($(transpile --version 2>/dev/null || echo '?')) — skipping binary."
    warn "To upgrade: cargo install llm-transpile --force"
  else
    info "Installing transpile binary..."
    if command -v cargo &>/dev/null; then
      cargo install llm-transpile
      success "Binary installed."
    else
      error "cargo not found. Install Rust: https://rustup.rs"; exit 1
    fi
  fi

  local profile; profile=$(detect_profile)
  upsert_block "$profile" "$(build_shell_block)"

  upsert_claude_hook
  upsert_cursor

  echo ""
  success "Done. Restart your shell or:"
  echo -e "  ${CYAN}source $profile${RESET}"
  echo ""
  echo -e "Quick start:"
  echo -e "  ${CYAN}tctx README.md${RESET}                      # compress to stdout"
  echo -e "  ${CYAN}trun README.md gemini 'summarise'${RESET}   # compress → gemini"
  echo -e "  ${CYAN}trun README.md codex  'implement'${RESET}   # compress → codex"
  echo -e "  ${CYAN}bash install.sh uninstall${RESET}            # remove everything"
}

# ── Uninstall ──────────────────────────────────────────────────────────────────
cmd_uninstall() {
  echo -e "${BOLD}llm-transpile — uninstall${RESET}"
  echo "──────────────────────────────────────────"

  # Binary
  if command -v transpile &>/dev/null; then
    info "Removing transpile binary..."
    cargo uninstall llm-transpile 2>/dev/null && success "Binary removed." || warn "Could not remove binary (not installed via cargo?)."
  else
    warn "transpile binary not found — skipping."
  fi

  local profile; profile=$(detect_profile)
  remove_block "$profile"

  remove_claude_hook
  remove_cursor

  echo ""
  success "Uninstall complete."
  echo -e "  ${CYAN}source $profile${RESET}  (or open a new shell)"
}

# ── Entry point ────────────────────────────────────────────────────────────────
case "${1:-install}" in
  install|update) cmd_install ;;
  uninstall)      cmd_uninstall ;;
  *)
    echo "Usage: bash install.sh [install|uninstall]"
    exit 1
    ;;
esac
