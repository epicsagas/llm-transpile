//! install.rs — `transpile install` / `transpile uninstall` subcommands
//!
//! Configures shell wrappers and per-tool integrations.
//! All state written to the shell profile is bracketed by:
//!   # >>> llm-transpile
//!   ...
//!   # <<< llm-transpile
//! so it can be cleanly updated or removed on re-run.

use std::io::{self, IsTerminal, Write as IoWrite};
use std::path::PathBuf;

// ── Tool registry ─────────────────────────────────────────────────────────────

struct Tool {
    id: &'static str,
    label: &'static str,
    /// Returns true when the tool appears to be installed on this machine.
    detect: fn() -> bool,
}

const TOOLS: &[Tool] = &[
    Tool { id: "claude",   label: "Claude Code",  detect: || cmd_exists("claude") || dir_exists("~/.claude") },
    Tool { id: "gemini",   label: "Gemini CLI",   detect: || cmd_exists("gemini") },
    Tool { id: "codex",    label: "Codex CLI",    detect: || cmd_exists("codex") },
    Tool { id: "cursor",   label: "Cursor",       detect: || cmd_exists("cursor") || dir_exists("~/.cursor") },
    Tool { id: "opencode", label: "OpenCode",     detect: || cmd_exists("opencode") },
];

fn cmd_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn dir_exists(path: &str) -> bool {
    let expanded = path.replacen("~", &home(), 1);
    std::path::Path::new(&expanded).exists()
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}

// ── Public entry points ────────────────────────────────────────────────────────

pub fn run_install(tools: Vec<String>, all: bool) -> i32 {
    let selected = if all {
        TOOLS.iter().map(|t| t.id).collect::<Vec<_>>()
    } else if !tools.is_empty() {
        // Validate names
        let mut out = Vec::new();
        for name in &tools {
            if TOOLS.iter().any(|t| t.id == name.as_str()) {
                out.push(name.as_str());
            } else {
                eprintln!("error: unknown tool '{name}'. Valid: {}", tool_names());
                return 1;
            }
        }
        out
    } else {
        wizard_select()
    };

    if selected.is_empty() {
        eprintln!("No tools selected.");
        return 0;
    }

    let profile = detect_profile();
    upsert_shell_block(&profile, &selected);

    for id in &selected {
        match *id {
            "claude"   => setup_claude(),
            "gemini"   => setup_gemini(),
            "codex"    => setup_codex(),
            "opencode" => setup_opencode(),
            "cursor"   => setup_cursor(),
            _ => {}
        }
    }

    eprintln!();
    eprintln!("Done. Restart your shell or:  source {profile}");
    0
}

pub fn run_uninstall(tools: Vec<String>) -> i32 {
    let selected: Vec<&str> = if !tools.is_empty() {
        // Validate each name
        let mut out = Vec::new();
        for name in &tools {
            if TOOLS.iter().any(|t| t.id == name.as_str()) {
                out.push(name.as_str());
            } else {
                eprintln!("error: unknown tool '{name}'. Valid: {}", tool_names());
                return 1;
            }
        }
        out
    } else {
        wizard_uninstall_select()
    };

    if selected.is_empty() {
        eprintln!("No tools selected.");
        return 0;
    }

    // Determine which tools remain installed after removal
    let profile = detect_profile();
    let currently_installed = installed_tools_from_profile(&profile);
    let remaining: Vec<&str> = currently_installed
        .iter()
        .filter(|id| !selected.contains(id))
        .copied()
        .collect();

    // Update or remove the shell block
    if remaining.is_empty() {
        remove_shell_block(&profile);
        eprintln!("  removed shell wrappers from {profile}");
    } else {
        upsert_shell_block(&profile, &remaining);
    }

    // Remove per-tool config for each selected tool
    for id in &selected {
        match *id {
            "claude"   => remove_claude(),
            "gemini"   => remove_gemini(),
            "codex"    => remove_codex(),
            "opencode" => remove_opencode(),
            "cursor"   => remove_cursor(),
            _ => {}
        }
        eprintln!("  {id}: removed");
    }

    eprintln!();
    eprintln!("Done.");
    0
}

/// Returns tools currently present in the shell profile block.
fn installed_tools_from_profile(profile: &str) -> Vec<&'static str> {
    let content = std::fs::read_to_string(profile).unwrap_or_default();
    if !content.contains(MARKER_BEGIN) {
        return vec![];
    }
    // Extract block content
    let start = content.find(MARKER_BEGIN).unwrap_or(0);
    let end = content[start..].find(MARKER_END).map(|i| start + i).unwrap_or(content.len());
    let block = &content[start..end];

    // Detect which tools are referenced by their section comments
    let mut found = vec!["claude"]; // tctx/talias/trun always imply base install
    if block.contains("# Gemini CLI")  { found.push("gemini"); }
    if block.contains("# Codex CLI")   { found.push("codex"); }
    if block.contains("# OpenCode")    { found.push("opencode"); }
    // cursor doesn't add shell block entries, detect via file
    if std::path::Path::new(".cursor/transpile-ctx.sh").exists() {
        found.push("cursor");
    }
    found
}

/// Uninstall wizard: same TTY/pipe UI but labeled "remove".
fn wizard_uninstall_select() -> Vec<&'static str> {
    let tty = io::stdin().is_terminal() && io::stderr().is_terminal();
    if tty {
        wizard_uninstall_tty()
    } else {
        wizard_uninstall_pipe()
    }
}

fn wizard_uninstall_tty() -> Vec<&'static str> {
    let profile = detect_profile();
    let installed = installed_tools_from_profile(&profile);
    // Pre-check everything that's installed
    let mut checked: Vec<bool> = TOOLS.iter().map(|t| installed.contains(&t.id)).collect();
    let mut cursor = 0usize;

    let _ = std::process::Command::new("stty").args(["-echo", "raw"]).status();

    loop {
        eprint!("\x1b[2J\x1b[H");
        eprintln!("transpile uninstall — select integrations to remove\r");
        eprintln!("  Space: toggle  ·  A: all  ·  N: none  ·  Enter: confirm  ·  Q: quit\r");
        eprintln!("\r");

        for (i, tool) in TOOLS.iter().enumerate() {
            let is_installed = installed.contains(&tool.id);
            let check  = if checked[i]   { "◉" } else { "○" };
            let arrow  = if i == cursor  { "▶" } else { " " };
            let status = if is_installed { " (installed)" } else { " (not installed)" };
            eprintln!("  {} {} {}  {}{}\r", arrow, check, tool.id, tool.label, status);
        }

        let _ = io::stderr().flush();

        let key = read_key();
        match key {
            b' ' => { checked[cursor] = !checked[cursor]; }
            b'A' | b'a' => { checked.iter_mut().for_each(|c| *c = true); }
            b'N' | b'n' => { checked.iter_mut().for_each(|c| *c = false); }
            b'Q' | b'q' => {
                let _ = std::process::Command::new("stty").args(["echo", "-raw"]).status();
                return vec![];
            }
            b'\r' | b'\n' => break,
            27 => {
                let b2 = read_key();
                if b2 == b'[' {
                    match read_key() {
                        b'A' => { if cursor > 0 { cursor -= 1; } }
                        b'B' => { if cursor < TOOLS.len() - 1 { cursor += 1; } }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let _ = std::process::Command::new("stty").args(["echo", "-raw"]).status();
    eprint!("\x1b[2J\x1b[H");

    TOOLS.iter().enumerate()
        .filter_map(|(i, t)| if checked[i] { Some(t.id) } else { None })
        .collect()
}

fn wizard_uninstall_pipe() -> Vec<&'static str> {
    let profile = detect_profile();
    let installed = installed_tools_from_profile(&profile);

    eprintln!("transpile uninstall — select integrations to remove");
    eprintln!("────────────────────────────────────────────────────");
    for (i, t) in TOOLS.iter().enumerate() {
        let status = if installed.contains(&t.id) { " [installed]" } else { "" };
        eprintln!("  [{}] {:<10} {}{}", i + 1, t.id, t.label, status);
    }
    eprintln!("  [a] All of the above");
    eprintln!();
    eprint!("Selection (e.g. 1,3 or a): ");
    let _ = io::stderr().flush();

    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    let line = line.trim().to_lowercase();

    if line == "a" || line == "all" {
        return TOOLS.iter().map(|t| t.id).collect();
    }

    let mut out = Vec::new();
    for token in line.split(',') {
        if let Ok(n) = token.trim().parse::<usize>() {
            if n >= 1 && n <= TOOLS.len() {
                out.push(TOOLS[n - 1].id);
            }
        }
    }
    out
}

// ── Interactive wizard ─────────────────────────────────────────────────────────

fn wizard_select() -> Vec<&'static str> {
    let tty = io::stdin().is_terminal() && io::stderr().is_terminal();

    if tty {
        wizard_tty()
    } else {
        wizard_pipe()
    }
}

/// TTY: space-toggle checkbox list, Enter to confirm.
fn wizard_tty() -> Vec<&'static str> {
    let detected: Vec<bool> = TOOLS.iter().map(|t| (t.detect)()).collect();
    // Pre-select detected tools
    let mut checked: Vec<bool> = detected.clone();
    let mut cursor = 0usize;

    // Switch terminal to raw mode via stty
    let _ = std::process::Command::new("stty").args(["-echo", "raw"]).status();

    loop {
        // Render
        eprint!("\x1b[2J\x1b[H"); // clear screen
        eprintln!("transpile install — select integrations\r");
        eprintln!("  Space: toggle  ·  A: all  ·  N: none  ·  Enter: confirm  ·  Q: quit\r");
        eprintln!("\r");

        for (i, tool) in TOOLS.iter().enumerate() {
            let check = if checked[i] { "◉" } else { "○" };
            let arrow = if i == cursor { "▶" } else { " " };
            let det   = if detected[i] { " (detected)" } else { "" };
            eprintln!("  {} {} {}  {}{}\r", arrow, check, tool.id, tool.label, det);
        }

        let _ = io::stderr().flush();

        // Read one keypress
        let key = read_key();
        match key {
            b' ' => { checked[cursor] = !checked[cursor]; }
            b'A' | b'a' => { checked.iter_mut().for_each(|c| *c = true); }
            b'N' | b'n' => { checked.iter_mut().for_each(|c| *c = false); }
            b'Q' | b'q' => {
                let _ = std::process::Command::new("stty").args(["echo", "-raw"]).status();
                return vec![];
            }
            b'\r' | b'\n' => break,
            27 => { // ESC sequences: [A = up, [B = down
                let b2 = read_key();
                if b2 == b'[' {
                    match read_key() {
                        b'A' => { if cursor > 0 { cursor -= 1; } }
                        b'B' => { if cursor < TOOLS.len() - 1 { cursor += 1; } }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let _ = std::process::Command::new("stty").args(["echo", "-raw"]).status();
    eprint!("\x1b[2J\x1b[H"); // clear screen

    TOOLS.iter().enumerate()
        .filter_map(|(i, t)| if checked[i] { Some(t.id) } else { None })
        .collect()
}

fn read_key() -> u8 {
    use std::io::Read;
    let mut buf = [0u8; 1];
    io::stdin().read_exact(&mut buf).ok();
    buf[0]
}

/// Non-TTY fallback: numbered list, comma-separated input.
fn wizard_pipe() -> Vec<&'static str> {
    eprintln!("transpile install — select integrations");
    eprintln!("────────────────────────────────────────");
    for (i, t) in TOOLS.iter().enumerate() {
        let det = if (t.detect)() { " *" } else { "" };
        eprintln!("  [{}] {:<10} {}{}", i + 1, t.id, t.label, det);
    }
    eprintln!("  [a] All of the above");
    eprintln!();
    eprint!("Selection (e.g. 1,3 or a): ");
    let _ = io::stderr().flush();

    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    let line = line.trim().to_lowercase();

    if line == "a" || line == "all" {
        return TOOLS.iter().map(|t| t.id).collect();
    }

    let mut out = Vec::new();
    for token in line.split(',') {
        if let Ok(n) = token.trim().parse::<usize>() {
            if n >= 1 && n <= TOOLS.len() {
                out.push(TOOLS[n - 1].id);
            }
        }
    }
    out
}

fn tool_names() -> String {
    TOOLS.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
}

// ── Shell profile block ────────────────────────────────────────────────────────

const MARKER_BEGIN: &str = "# >>> llm-transpile";
const MARKER_END:   &str = "# <<< llm-transpile";

fn detect_profile() -> String {
    let home = home();
    for name in &[".zshrc", ".bashrc", ".profile"] {
        let p = format!("{home}/{name}");
        if std::path::Path::new(&p).exists() {
            return p;
        }
    }
    format!("{home}/.profile")
}

fn build_shell_block(_tools: &[&str]) -> String {
    // Single unified helper — all tools use `transpile` directly.
    // Per-tool wrappers are replaced by /transpile skills installed in each tool.
    vec![
        r#"tctx() { transpile --input "$1" --fidelity "${2:-semantic}" --quiet; }"#.to_string(),
    ].join("\n")
}

fn upsert_shell_block(profile: &str, tools: &[&str]) {
    let block_body = build_shell_block(tools);
    let full_block = format!("\n{MARKER_BEGIN}\n{block_body}\n{MARKER_END}\n");

    let existing = std::fs::read_to_string(profile).unwrap_or_default();

    let new_content = if existing.contains(MARKER_BEGIN) {
        // Replace existing block
        // Manual replace (no regex dep): find begin..end and splice
        splice_block(&existing, MARKER_BEGIN, MARKER_END, &full_block.trim())
    } else {
        format!("{existing}{full_block}")
    };

    std::fs::write(profile, new_content).ok();
    eprintln!("  {} shell wrappers in {profile}", if existing.contains(MARKER_BEGIN) { "updated" } else { "added" });
}

fn splice_block(text: &str, begin: &str, end: &str, replacement: &str) -> String {
    let start = match text.find(begin) {
        Some(i) => i,
        None => return format!("{text}\n{replacement}\n"),
    };
    let after_end = match text[start..].find(end) {
        Some(i) => start + i + end.len(),
        None => text.len(),
    };
    // Trim any leading newline before the marker
    let prefix_end = if start > 0 && text.as_bytes()[start - 1] == b'\n' { start - 1 } else { start };
    format!("{}\n{}\n{}", &text[..prefix_end], replacement, &text[after_end..])
}

fn remove_shell_block(profile: &str) {
    let existing = std::fs::read_to_string(profile).unwrap_or_default();
    if !existing.contains(MARKER_BEGIN) { return; }
    let cleaned = splice_block(&existing, MARKER_BEGIN, MARKER_END, "");
    // Remove double-blank lines left behind
    let cleaned = cleaned.trim_end().to_string() + "\n";
    std::fs::write(profile, cleaned).ok();
}

// ── Claude Code ────────────────────────────────────────────────────────────────

fn claude_settings_path() -> PathBuf {
    PathBuf::from(home()).join(".claude").join("settings.json")
}

/// The PostToolUse hook script written to ~/.claude/transpile-hook.sh.
///
/// Reads the hook JSON payload from stdin, extracts the file path, and if the
/// file exceeds the byte threshold, runs `transpile` and returns a JSON object
/// with `additionalContext` so Claude receives the compressed version alongside
/// the raw read result — no manual intervention required.
const CLAUDE_HOOK_SCRIPT: &str = r#"#!/usr/bin/env bash
# Auto-generated by `transpile install`. Re-run to update.
# PostToolUse hook: auto-compress large files read by Claude Code.
# Outputs {"additionalContext": "..."} so Claude prefers the token-efficient version.
set -euo pipefail

THRESHOLD=${TRANSPILE_THRESHOLD:-8192}   # bytes; override via env var

# Parse file_path from hook JSON on stdin
INPUT=$(cat)
FILE=$(printf '%s' "$INPUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('tool_input', {}).get('file_path', ''))
" 2>/dev/null) || exit 0

[ -z "$FILE" ] && exit 0
[ -f "$FILE" ] || exit 0

BYTES=$(wc -c < "$FILE" 2>/dev/null || echo 0)
[ "$BYTES" -lt "$THRESHOLD" ] && exit 0

# Run transpile; silently skip if binary not on PATH or transpile fails
COMPRESSED=$(transpile --input "$FILE" --fidelity semantic --quiet 2>/dev/null) || exit 0
[ -z "$COMPRESSED" ] && exit 0

FNAME=$(basename "$FILE")
python3 -c "
import json, sys
compressed = sys.argv[1]
fname      = sys.argv[2]
bytes_val  = sys.argv[3]
msg = (
    f'[llm-transpile] {fname} is {bytes_val}B — token-compressed version below '
    f'(prefer this over the raw content above):\n\n{compressed}'
)
print(json.dumps({'additionalContext': msg}))
" "$COMPRESSED" "$FNAME" "$BYTES"
"#;

fn setup_claude() {
    let claude_dir = PathBuf::from(home()).join(".claude");
    std::fs::create_dir_all(&claude_dir).ok();

    // Write the hook script
    let hook_script = claude_dir.join("transpile-hook.sh");
    std::fs::write(&hook_script, CLAUDE_HOOK_SCRIPT).ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(&hook_script) {
            let mut p = m.permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(&hook_script, p).ok();
        }
    }
    eprintln!("  Claude Code: hook script written to {}", hook_script.display());

    // Upsert settings.json with the PostToolUse hook
    let settings_path = claude_dir.join("settings.json");
    if !settings_path.exists() {
        std::fs::write(&settings_path, "{}").ok();
    }
    let raw = std::fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".into());
    let mut cfg: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    let hook_cmd = format!(
        "bash \"{}\"",
        hook_script.display()
    );
    let hook = serde_json::json!({
        "_id": "llm-transpile",
        "matcher": "Read",
        "hooks": [{ "type": "command", "command": hook_cmd }]
    });

    let arr = cfg["hooks"]["PostToolUse"]
        .as_array_mut();
    if let Some(arr) = arr {
        arr.retain(|h| h.get("_id").and_then(|v| v.as_str()) != Some("llm-transpile"));
        arr.push(hook);
    } else {
        cfg["hooks"]["PostToolUse"] = serde_json::json!([hook]);
    }

    std::fs::write(&settings_path, serde_json::to_string_pretty(&cfg).unwrap()).ok();
    eprintln!("  Claude Code: PostToolUse hook registered in {}", settings_path.display());

    // /transpile slash command
    let cmd_dir = claude_dir.join("commands");
    std::fs::create_dir_all(&cmd_dir).ok();
    std::fs::write(
        cmd_dir.join("transpile.md"),
        "---\nname: transpile\ndescription: Compress a file with llm-transpile and insert into context\n---\n\nRun and insert the token-compressed version:\n\n```bash\ntranspile --input $ARGUMENTS --fidelity semantic --quiet\n```\n",
    ).ok();
    eprintln!("  Claude Code: /transpile command written");
}

fn remove_claude() {
    let path = claude_settings_path();
    if !path.exists() { return; }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".into());
    let mut cfg: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    if let Some(arr) = cfg["hooks"]["PostToolUse"].as_array_mut() {
        arr.retain(|h| h.get("_id").and_then(|v| v.as_str()) != Some("llm-transpile"));
    }
    // Clean up empty containers
    if cfg["hooks"]["PostToolUse"].as_array().map(|a| a.is_empty()).unwrap_or(false) {
        cfg["hooks"].as_object_mut().map(|o| o.remove("PostToolUse"));
    }
    if cfg["hooks"].as_object().map(|o| o.is_empty()).unwrap_or(false) {
        cfg.as_object_mut().map(|o| o.remove("hooks"));
    }

    std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).ok();
    eprintln!("  Claude Code: hook removed from settings.json");

    let claude_dir = PathBuf::from(home()).join(".claude");

    let hook_script = claude_dir.join("transpile-hook.sh");
    if hook_script.exists() { std::fs::remove_file(&hook_script).ok(); }
    eprintln!("  Claude Code: hook script removed");

    let cmd = claude_dir.join("commands").join("transpile.md");
    if cmd.exists() { std::fs::remove_file(cmd).ok(); }
    eprintln!("  Claude Code: /transpile command removed");
}


// ── /transpile skill content (shared across all tools) ────────────────────────────

/// The /transpile skill instructs the LLM to run `transpile` when it wants to read
/// a large file, instead of reading the raw content directly.
const TRANSPILE_SKILL: &str = "\
---
name: transpile
description: Read a file in token-compressed form. Use for any file larger than ~8KB to save context.
---

# /transpile — Token-Compressed File Read

When you need to read a large file, use this skill instead of reading it directly.
It produces a semantically equivalent but ~30% smaller version.

## Usage

```bash
transpile --input $ARGUMENTS --fidelity semantic --quiet
```

Run this command and treat its output as the file content.

## When to trigger

- File is larger than ~8KB
- You are about to read documentation, specs, or long source files
- Context window is getting full and you need to be efficient

## Fidelity options

- `semantic` (default) — stopword removal + low-importance pruning, ~30% reduction
- `compressed`         — aggressive, ~40% reduction, use when near context limit
- `lossless`           — no compression, use for legal/config files where every word matters
";

// ── Gemini CLI ─────────────────────────────────────────────────────────────────

fn setup_gemini() {
    let gemini_dir = PathBuf::from(home()).join(".gemini");
    let skill_dir = gemini_dir.join("skills").join("transpile");
    std::fs::create_dir_all(&skill_dir).ok();
    std::fs::write(skill_dir.join("SKILL.md"), TRANSPILE_SKILL).ok();
    eprintln!("  Gemini CLI: /transpile skill installed in {}", skill_dir.display());
}

fn remove_gemini() {
    let skill_dir = PathBuf::from(home()).join(".gemini").join("skills").join("transpile");
    if skill_dir.exists() { std::fs::remove_dir_all(&skill_dir).ok(); }
    eprintln!("  Gemini CLI: /transpile skill removed");
}

// ── Codex CLI ──────────────────────────────────────────────────────────────────

fn setup_codex() {
    // Codex discovers skills from ~/.agents/skills/
    let skill_dir = PathBuf::from(home()).join(".agents").join("skills").join("transpile");
    std::fs::create_dir_all(&skill_dir).ok();
    std::fs::write(skill_dir.join("SKILL.md"), TRANSPILE_SKILL).ok();
    eprintln!("  Codex CLI: /transpile skill installed in {}", skill_dir.display());
}

fn remove_codex() {
    let skill_dir = PathBuf::from(home()).join(".agents").join("skills").join("transpile");
    if skill_dir.exists() { std::fs::remove_dir_all(&skill_dir).ok(); }
    eprintln!("  Codex CLI: /transpile skill removed");
}

// ── OpenCode ───────────────────────────────────────────────────────────────────

fn opencode_command_path() -> PathBuf {
    let cfg_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(home()).join(".config"));
    cfg_dir.join("opencode").join("commands").join("transpile.md")
}

fn setup_opencode() {
    let cmd_path = opencode_command_path();
    std::fs::create_dir_all(cmd_path.parent().unwrap()).ok();
    std::fs::write(&cmd_path, TRANSPILE_SKILL).ok();
    eprintln!("  OpenCode: /transpile command installed in {}", cmd_path.display());
}

fn remove_opencode() {
    let p = opencode_command_path();
    if p.exists() { std::fs::remove_file(&p).ok(); }
    eprintln!("  OpenCode: /transpile command removed");
}

// ── Cursor ─────────────────────────────────────────────────────────────────────

fn setup_cursor() {
    // Cursor commands live in .cursor/commands/ (project-local)
    let cmd_dir = std::path::Path::new(".cursor").join("commands");
    std::fs::create_dir_all(&cmd_dir).ok();
    std::fs::write(cmd_dir.join("transpile.md"), TRANSPILE_SKILL).ok();
    eprintln!("  Cursor: /transpile command installed in .cursor/commands/transpile.md");
}

fn remove_cursor() {
    let p = std::path::Path::new(".cursor/commands/transpile.md");
    if p.exists() { std::fs::remove_file(p).ok(); }
    eprintln!("  Cursor: /transpile command removed");
}
