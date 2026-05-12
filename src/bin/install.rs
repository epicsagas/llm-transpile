//! install.rs — `transpile install` / `transpile uninstall` subcommands
//!
//! Marketplace-style plugin installer. Each integration is declared in the
//! static `INTEGRATIONS` registry with its own install/uninstall logic.
//! All writes are idempotent — re-running install is always safe.
//!
//! Features:
//!   transpile install              # interactive wizard
//!   transpile install claude       # specific integration(s)
//!   transpile install --all        # everything
//!   transpile install --list       # show available integrations + status
//!   transpile install --dry-run    # preview without writing
//!   transpile uninstall [tool]     # remove integration(s)
//!   transpile uninstall --all      # remove everything
//!   transpile uninstall --dry-run  # preview without removing

use std::io::{self, IsTerminal, Write as IoWrite};
use std::path::PathBuf;

// ═══════════════════════════════════════════════════════════════════════════════
// Sync result — returned by every install/uninstall fn for uniform output
// ═══════════════════════════════════════════════════════════════════════════════

enum SyncResult {
    Added(PathBuf),
    Updated(PathBuf),
    Unchanged(PathBuf),
    Removed(PathBuf),
    Skipped(PathBuf),     // would remove but file absent
    Configured(String),   // non-file change (e.g. settings.json merge)
    Deconfigured(String), // non-file removal
    DryRun(String),       // what would happen
}

impl SyncResult {
    fn symbol(&self) -> &'static str {
        match self {
            Self::Added(_) | Self::Configured(_) => "✓",
            Self::Updated(_) => "~",
            Self::Unchanged(_) | Self::Skipped(_) => "·",
            Self::Removed(_) | Self::Deconfigured(_) => "✗",
            Self::DryRun(_) => "?",
        }
    }
    fn tag(&self) -> &'static str {
        match self {
            Self::Added(_) => "added     ",
            Self::Updated(_) => "updated   ",
            Self::Unchanged(_) => "unchanged ",
            Self::Removed(_) => "removed   ",
            Self::Skipped(_) => "skipped   ",
            Self::Configured(_) => "configured",
            Self::Deconfigured(_) => "removed   ",
            Self::DryRun(_) => "dry-run   ",
        }
    }
    fn detail(&self) -> String {
        match self {
            Self::Added(p)
            | Self::Updated(p)
            | Self::Unchanged(p)
            | Self::Removed(p)
            | Self::Skipped(p) => shorten_path(p),
            Self::Configured(s) | Self::Deconfigured(s) | Self::DryRun(s) => s.clone(),
        }
    }
}

fn print_results(id: &str, results: &[SyncResult]) {
    for r in results {
        eprintln!("  {id:<10} {} {}  {}", r.symbol(), r.tag(), r.detail());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Integration registry
// ═══════════════════════════════════════════════════════════════════════════════

struct Integration {
    id: &'static str,
    label: &'static str,
    detect: fn() -> bool,
    /// Primary artifact — used for installed-status detection and --list display.
    artifact: fn() -> PathBuf,
    install: fn(dry_run: bool) -> Vec<SyncResult>,
    uninstall: fn(dry_run: bool) -> Vec<SyncResult>,
}

static INTEGRATIONS: &[Integration] = &[
    Integration {
        id: "claude",
        label: "Claude Code",
        detect: detect_claude,
        artifact: claude_artifact,
        install: install_claude,
        uninstall: uninstall_claude,
    },
    Integration {
        id: "gemini",
        label: "Gemini CLI",
        detect: detect_gemini,
        artifact: gemini_artifact,
        install: install_gemini,
        uninstall: uninstall_gemini,
    },
    Integration {
        id: "codex",
        label: "Codex CLI",
        detect: detect_codex,
        artifact: codex_artifact,
        install: install_codex,
        uninstall: uninstall_codex,
    },
    Integration {
        id: "opencode",
        label: "OpenCode",
        detect: detect_opencode,
        artifact: opencode_artifact,
        install: install_opencode,
        uninstall: uninstall_opencode,
    },
    Integration {
        id: "cursor",
        label: "Cursor",
        detect: detect_cursor,
        artifact: cursor_artifact,
        install: install_cursor,
        uninstall: uninstall_cursor,
    },
];

fn integration_names() -> String {
    INTEGRATIONS
        .iter()
        .map(|i| i.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_integration(id: &str) -> Option<&'static Integration> {
    INTEGRATIONS.iter().find(|i| i.id == id)
}

// ── Detectors ──────────────────────────────────────────────────────────────────
fn detect_claude() -> bool {
    cmd_exists("claude") || dir_exists("~/.claude")
}
fn detect_gemini() -> bool {
    cmd_exists("gemini")
}
fn detect_codex() -> bool {
    cmd_exists("codex")
}
fn detect_opencode() -> bool {
    cmd_exists("opencode")
}
fn detect_cursor() -> bool {
    cmd_exists("cursor") || dir_exists("~/.cursor")
}

// ── Artifact paths (canonical installed-status marker) ─────────────────────────
fn claude_artifact() -> PathBuf {
    PathBuf::from(home())
        .join(".claude")
        .join("transpile-hook.js")
}
fn gemini_artifact() -> PathBuf {
    skill_path("gemini")
}
fn codex_artifact() -> PathBuf {
    skill_path("codex")
}
fn opencode_artifact() -> PathBuf {
    skill_path("opencode")
}
fn cursor_artifact() -> PathBuf {
    skill_path("cursor")
}

// ── Per-integration install fns ────────────────────────────────────────────────
fn install_claude(dry_run: bool) -> Vec<SyncResult> {
    let mut out = Vec::new();

    // 1) Hook script
    let hook = claude_artifact();
    out.push(sync_file(&hook, CLAUDE_HOOK_SCRIPT, true, dry_run));

    // 2) settings.json PostToolUse entry
    let settings = PathBuf::from(home()).join(".claude").join("settings.json");
    if dry_run {
        out.push(SyncResult::DryRun(format!(
            "merge PostToolUse hook → {}",
            shorten_path(&settings)
        )));
    } else {
        out.push(merge_claude_hook(&hook, &settings));
    }

    out
}

fn uninstall_claude(dry_run: bool) -> Vec<SyncResult> {
    let mut out = Vec::new();

    // 1) Hook script
    let hook = claude_artifact();
    out.push(remove_file(&hook, dry_run));

    // 2) settings.json entry
    let settings = PathBuf::from(home()).join(".claude").join("settings.json");
    if dry_run {
        out.push(SyncResult::DryRun(format!(
            "remove PostToolUse hook from {}",
            shorten_path(&settings)
        )));
    } else {
        out.push(strip_claude_hook(&settings));
    }

    // 3) Legacy command file (silent)
    if !dry_run {
        let legacy = PathBuf::from(home())
            .join(".claude")
            .join("commands")
            .join("transpile.md");
        if legacy.exists() {
            std::fs::remove_file(legacy).ok();
        }
    }

    out
}

fn install_gemini(dry_run: bool) -> Vec<SyncResult> {
    vec![sync_file(
        &gemini_artifact(),
        &transpile_skill("gemini"),
        false,
        dry_run,
    )]
}
fn install_codex(dry_run: bool) -> Vec<SyncResult> {
    vec![sync_file(
        &codex_artifact(),
        &transpile_skill("codex"),
        false,
        dry_run,
    )]
}
fn install_opencode(dry_run: bool) -> Vec<SyncResult> {
    vec![sync_file(
        &opencode_artifact(),
        &transpile_skill("opencode"),
        false,
        dry_run,
    )]
}
fn install_cursor(dry_run: bool) -> Vec<SyncResult> {
    vec![sync_file(
        &cursor_artifact(),
        &transpile_skill("cursor"),
        false,
        dry_run,
    )]
}

fn uninstall_gemini(dry_run: bool) -> Vec<SyncResult> {
    vec![remove_file(&gemini_artifact(), dry_run)]
}
fn uninstall_codex(dry_run: bool) -> Vec<SyncResult> {
    vec![remove_file(&codex_artifact(), dry_run)]
}
fn uninstall_opencode(dry_run: bool) -> Vec<SyncResult> {
    vec![remove_file(&opencode_artifact(), dry_run)]
}
fn uninstall_cursor(dry_run: bool) -> Vec<SyncResult> {
    vec![remove_file(&cursor_artifact(), dry_run)]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Public entry points
// ═══════════════════════════════════════════════════════════════════════════════

pub fn run_install(tools: Vec<String>, all: bool, dry_run: bool, list: bool) -> i32 {
    if list {
        return run_list();
    }

    let selected: Vec<&str> = if all {
        INTEGRATIONS.iter().map(|i| i.id).collect()
    } else if !tools.is_empty() {
        let mut out = Vec::new();
        for name in &tools {
            match find_integration(name) {
                Some(ig) => out.push(ig.id),
                None => {
                    eprintln!(
                        "error: unknown integration '{name}'. Available: {}",
                        integration_names()
                    );
                    return 1;
                }
            }
        }
        out
    } else {
        wizard_select()
    };

    if selected.is_empty() {
        eprintln!("No integrations selected.");
        return 0;
    }

    if dry_run {
        eprintln!("Dry run — no files will be written.\n");
    }

    let profile = detect_profile();
    if !dry_run {
        ensure_shell_block(&profile);
        eprintln!();
    }

    for id in &selected {
        let ig = find_integration(id).unwrap();
        let results = (ig.install)(dry_run);
        print_results(id, &results);
    }

    if !dry_run {
        eprintln!();
        eprintln!("Done. Restart your shell or:  source {profile}");
    }
    0
}

pub fn run_uninstall(tools: Vec<String>, all: bool, dry_run: bool) -> i32 {
    let selected: Vec<&str> = if all {
        INTEGRATIONS.iter().map(|i| i.id).collect()
    } else if !tools.is_empty() {
        let mut out = Vec::new();
        for name in &tools {
            match find_integration(name) {
                Some(ig) => out.push(ig.id),
                None => {
                    eprintln!(
                        "error: unknown integration '{name}'. Available: {}",
                        integration_names()
                    );
                    return 1;
                }
            }
        }
        out
    } else {
        wizard_uninstall_select()
    };

    if selected.is_empty() {
        eprintln!("No integrations selected.");
        return 0;
    }

    if dry_run {
        eprintln!("Dry run — no files will be removed.\n");
    }

    for id in &selected {
        let ig = find_integration(id).unwrap();
        let results = (ig.uninstall)(dry_run);
        print_results(id, &results);
    }

    // Remove shell block if nothing is installed anymore
    if !dry_run {
        let profile = detect_profile();
        let any_remain = INTEGRATIONS.iter().any(|ig| (ig.artifact)().exists());
        if !any_remain {
            remove_shell_block(&profile);
            eprintln!("  shell block removed from {profile}");
        }
        eprintln!("\nDone.");
    }
    0
}

// ── --list ─────────────────────────────────────────────────────────────────────

pub fn run_list() -> i32 {
    eprintln!("Available integrations:\n");
    eprintln!(
        "  {:<10} {:<14} {:<12} {:<14} ARTIFACT",
        "ID", "LABEL", "DETECTED", "STATUS"
    );
    eprintln!("  {}", "─".repeat(72));
    for ig in INTEGRATIONS {
        let artifact = (ig.artifact)();
        let detected = if (ig.detect)() {
            "✓ detected  "
        } else {
            "            "
        };
        let status = if artifact.exists() {
            "✓ installed "
        } else {
            "  not installed"
        };
        eprintln!(
            "  {:<10} {:<14} {:<12} {:<15} {}",
            ig.id,
            ig.label,
            detected,
            status,
            shorten_path(&artifact)
        );
    }
    eprintln!();
    0
}

// ═══════════════════════════════════════════════════════════════════════════════
// File sync helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Write `content` to `path`. Returns Added / Updated / Unchanged.
/// Creates parent dirs as needed. Optionally makes file executable.
fn sync_file(path: &PathBuf, content: &str, executable: bool, dry_run: bool) -> SyncResult {
    if dry_run {
        let action = if path.exists() { "update" } else { "create" };
        return SyncResult::DryRun(format!("{action} {}", shorten_path(path)));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let exists = path.exists();
    let same = exists
        && std::fs::read_to_string(path)
            .map(|s| s == content)
            .unwrap_or(false);

    if same {
        return SyncResult::Unchanged(path.clone());
    }

    std::fs::write(path, content).ok();

    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(path) {
            let mut p = m.permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(path, p).ok();
        }
    }

    if exists {
        SyncResult::Updated(path.clone())
    } else {
        SyncResult::Added(path.clone())
    }
}

/// Remove `path`. Returns Removed / Skipped.
/// Also prunes the parent directory if it becomes empty.
fn remove_file(path: &PathBuf, dry_run: bool) -> SyncResult {
    if !path.exists() {
        return SyncResult::Skipped(path.clone());
    }
    if dry_run {
        return SyncResult::DryRun(format!("remove {}", shorten_path(path)));
    }
    std::fs::remove_file(path).ok();
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent); // no-op if non-empty
    }
    SyncResult::Removed(path.clone())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Claude Code — hook script + settings.json merge
// ═══════════════════════════════════════════════════════════════════════════════

const CLAUDE_HOOK_SCRIPT: &str = r#"#!/usr/bin/env node
// Auto-generated by `transpile install`. Re-run to update.
// PostToolUse hook: auto-compress large files read by Claude Code.
// Outputs {"additionalContext": "..."} so Claude prefers the token-efficient version.
'use strict';
const {execSync} = require('child_process');
const fs = require('fs');
const path = require('path');

const THRESHOLD = parseInt(process.env.TRANSPILE_THRESHOLD || '8192', 10);

let input;
try { input = JSON.parse(require('fs').readFileSync(0, 'utf8')); } catch { process.exit(0); }

const file = (input.tool_input || {}).file_path || '';
if (!file) process.exit(0);

let bytes;
try { bytes = fs.statSync(file).size; } catch { process.exit(0); }
if (bytes < THRESHOLD) process.exit(0);

const project = input.cwd ? path.basename(input.cwd.replace(/[/\\]+$/, '')) : '';
if (project) process.env.TRANSPILE_PROJECT = project;

let compressed;
try { compressed = execSync(`transpile --input "${file}" --fidelity semantic --quiet`, {encoding: 'utf8', stdio: ['pipe','pipe','pipe']}); } catch { process.exit(0); }
if (!compressed) process.exit(0);

const fname = path.basename(file);
const msg = `[llm-transpile] ${fname} is ${bytes}B — token-compressed version below (prefer this over the raw content above):\n\n${compressed}`;
process.stdout.write(JSON.stringify({additionalContext: msg}));
"#;

fn merge_claude_hook(hook_script: &std::path::Path, settings_path: &std::path::Path) -> SyncResult {
    if !settings_path.exists() {
        std::fs::write(settings_path, "{}").ok();
    }
    let raw = std::fs::read_to_string(settings_path).unwrap_or_else(|_| "{}".into());
    let mut cfg: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    let hook = serde_json::json!({
        "_id": "llm-transpile",
        "matcher": "Read",
        "hooks": [{ "type": "command", "command": format!("node \"{}\"", hook_script.display()) }]
    });

    if let Some(arr) = cfg["hooks"]["PostToolUse"].as_array_mut() {
        arr.retain(|h| h.get("_id").and_then(|v| v.as_str()) != Some("llm-transpile"));
        arr.push(hook);
    } else {
        cfg["hooks"]["PostToolUse"] = serde_json::json!([hook]);
    }

    std::fs::write(settings_path, serde_json::to_string_pretty(&cfg).unwrap()).ok();

    let label = format!("PostToolUse hook → {}", shorten_path(settings_path));
    SyncResult::Configured(label)
}

fn strip_claude_hook(settings_path: &std::path::Path) -> SyncResult {
    if !settings_path.exists() {
        return SyncResult::Skipped(settings_path.to_path_buf());
    }
    let raw = std::fs::read_to_string(settings_path).unwrap_or_else(|_| "{}".into());
    let mut cfg: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    if let Some(arr) = cfg["hooks"]["PostToolUse"].as_array_mut() {
        arr.retain(|h| h.get("_id").and_then(|v| v.as_str()) != Some("llm-transpile"));
    }
    // Prune empty containers
    if cfg["hooks"]["PostToolUse"]
        .as_array()
        .map(|a| a.is_empty())
        .unwrap_or(false)
    {
        cfg["hooks"]
            .as_object_mut()
            .map(|o| o.remove("PostToolUse"));
    }
    if cfg["hooks"]
        .as_object()
        .map(|o| o.is_empty())
        .unwrap_or(false)
    {
        cfg.as_object_mut().map(|o| o.remove("hooks"));
    }
    std::fs::write(settings_path, serde_json::to_string_pretty(&cfg).unwrap()).ok();
    SyncResult::Deconfigured(format!(
        "PostToolUse hook removed from {}",
        shorten_path(settings_path)
    ))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared skill content + per-tool paths
// ═══════════════════════════════════════════════════════════════════════════════

fn transpile_skill(agent: &str) -> String {
    format!(
        "\
---
name: transpile
description: >
  ALWAYS run this instead of directly reading .md, .html, or .txt files.
  Trigger on file extension alone — no size check needed.
alwaysApply: true
---

# transpile — Read document files in token-compressed form

## Activation rule

**Whenever you are about to read a `.md`, `.html`, or `.txt` file, run this
instead. Use transpile's output as the file content — do not read the raw file.**

The file extension alone triggers this. No size check needed.

## Usage

```bash
TRANSPILE_AGENT={agent} transpile --input <file> --fidelity semantic --quiet
```

## Fidelity options

- `semantic` (default) — ~30% reduction, preserves all meaning
- `compressed`         — ~40% reduction, use when near context limit
- `lossless`           — no compression, for files where every word matters
"
    )
}

fn skill_path(tool: &str) -> PathBuf {
    let h = home();
    match tool {
        "gemini" => PathBuf::from(&h)
            .join(".gemini")
            .join("skills")
            .join("transpile")
            .join("SKILL.md"),
        "codex" => PathBuf::from(&h)
            .join(".agents")
            .join("skills")
            .join("transpile")
            .join("SKILL.md"),
        "opencode" => {
            let cfg = std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(&h).join(".config"));
            cfg.join("opencode")
                .join("skills")
                .join("transpile")
                .join("SKILL.md")
        }
        "cursor" => std::path::Path::new(".cursor")
            .join("rules")
            .join("transpile.mdc"),
        _ => unreachable!(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shell profile block  (tctx helper — optional convenience alias)
// ═══════════════════════════════════════════════════════════════════════════════

const MARKER_BEGIN: &str = "# >>> llm-transpile";
const MARKER_END: &str = "# <<< llm-transpile";

const SHELL_BLOCK: &str =
    r#"tctx() { transpile --input "$1" --fidelity "${2:-semantic}" --quiet; }"#;

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

fn ensure_shell_block(profile: &str) {
    let existing = std::fs::read_to_string(profile).unwrap_or_default();
    if existing.contains(MARKER_BEGIN) {
        return;
    }
    let block = format!("\n{MARKER_BEGIN}\n{SHELL_BLOCK}\n{MARKER_END}\n");
    let new = format!("{existing}{block}");
    std::fs::write(profile, new).ok();
    eprintln!("  shell    ✓ added      tctx() helper → {profile}");
}

fn remove_shell_block(profile: &str) {
    let existing = std::fs::read_to_string(profile).unwrap_or_default();
    if !existing.contains(MARKER_BEGIN) {
        return;
    }
    let cleaned = splice_block(&existing, MARKER_BEGIN, MARKER_END, "");
    let cleaned = cleaned.trim_end().to_string() + "\n";
    std::fs::write(profile, cleaned).ok();
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
    let prefix_end = if start > 0 && text.as_bytes()[start - 1] == b'\n' {
        start - 1
    } else {
        start
    };
    if replacement.is_empty() {
        format!("{}{}", &text[..prefix_end], &text[after_end..])
    } else {
        format!(
            "{}\n{}\n{}",
            &text[..prefix_end],
            replacement,
            &text[after_end..]
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Interactive wizard — install
// ═══════════════════════════════════════════════════════════════════════════════

fn wizard_select() -> Vec<&'static str> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        wizard_tty()
    } else {
        wizard_pipe()
    }
}

fn wizard_tty() -> Vec<&'static str> {
    let detected: Vec<bool> = INTEGRATIONS.iter().map(|i| (i.detect)()).collect();
    let mut checked: Vec<bool> = detected.clone();
    let mut cursor = 0usize;

    let _ = std::process::Command::new("stty")
        .args(["-echo", "raw"])
        .status();

    loop {
        eprint!("\x1b[2J\x1b[H");
        eprintln!("transpile install — select integrations\r");
        eprintln!("  Space: toggle  ·  A: all  ·  N: none  ·  Enter: confirm  ·  Q: quit\r");
        eprintln!("\r");
        for (i, ig) in INTEGRATIONS.iter().enumerate() {
            let artifact = (ig.artifact)();
            let status = if artifact.exists() {
                " [installed]"
            } else if detected[i] {
                " [detected] "
            } else {
                "            "
            };
            let check = if checked[i] { "◉" } else { "○" };
            let arrow = if i == cursor { "▶" } else { " " };
            eprintln!(
                "  {} {} {:<10}  {}{}\r",
                arrow, check, ig.id, ig.label, status
            );
        }
        let _ = io::stderr().flush();

        match read_key() {
            b' ' => {
                checked[cursor] = !checked[cursor];
            }
            b'A' | b'a' => {
                checked.iter_mut().for_each(|c| *c = true);
            }
            b'N' | b'n' => {
                checked.iter_mut().for_each(|c| *c = false);
            }
            b'Q' | b'q' => {
                let _ = std::process::Command::new("stty")
                    .args(["echo", "-raw"])
                    .status();
                return vec![];
            }
            b'\r' | b'\n' => break,
            27 if read_key() == b'[' => match read_key() {
                b'A' => {
                    cursor = cursor.saturating_sub(1);
                }
                b'B' if cursor < INTEGRATIONS.len() - 1 => {
                    cursor += 1;
                }
                _ => {}
            },
            _ => {}
        }
    }

    let _ = std::process::Command::new("stty")
        .args(["echo", "-raw"])
        .status();
    eprint!("\x1b[2J\x1b[H");
    INTEGRATIONS
        .iter()
        .enumerate()
        .filter_map(|(i, t)| if checked[i] { Some(t.id) } else { None })
        .collect()
}

fn wizard_pipe() -> Vec<&'static str> {
    eprintln!("transpile install — select integrations");
    eprintln!("────────────────────────────────────────");
    for (i, ig) in INTEGRATIONS.iter().enumerate() {
        let flag = if (ig.detect)() { " *" } else { "" };
        eprintln!("  [{}] {:<10} {}{}", i + 1, ig.id, ig.label, flag);
    }
    eprintln!("  [a] All of the above");
    eprintln!();
    eprint!("Selection (e.g. 1,3 or a): ");
    let _ = io::stderr().flush();
    read_selection()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Interactive wizard — uninstall
// ═══════════════════════════════════════════════════════════════════════════════

fn wizard_uninstall_select() -> Vec<&'static str> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        wizard_uninstall_tty()
    } else {
        wizard_uninstall_pipe()
    }
}

fn wizard_uninstall_tty() -> Vec<&'static str> {
    let installed: Vec<bool> = INTEGRATIONS
        .iter()
        .map(|ig| (ig.artifact)().exists())
        .collect();
    let mut checked: Vec<bool> = installed.clone();
    let mut cursor = 0usize;

    let _ = std::process::Command::new("stty")
        .args(["-echo", "raw"])
        .status();

    loop {
        eprint!("\x1b[2J\x1b[H");
        eprintln!("transpile uninstall — select integrations to remove\r");
        eprintln!("  Space: toggle  ·  A: all  ·  N: none  ·  Enter: confirm  ·  Q: quit\r");
        eprintln!("\r");
        for (i, ig) in INTEGRATIONS.iter().enumerate() {
            let status = if installed[i] {
                " [installed]    "
            } else {
                " [not installed]"
            };
            let check = if checked[i] { "◉" } else { "○" };
            let arrow = if i == cursor { "▶" } else { " " };
            eprintln!(
                "  {} {} {:<10}  {}{}\r",
                arrow, check, ig.id, ig.label, status
            );
        }
        let _ = io::stderr().flush();

        match read_key() {
            b' ' => {
                checked[cursor] = !checked[cursor];
            }
            b'A' | b'a' => {
                checked.iter_mut().for_each(|c| *c = true);
            }
            b'N' | b'n' => {
                checked.iter_mut().for_each(|c| *c = false);
            }
            b'Q' | b'q' => {
                let _ = std::process::Command::new("stty")
                    .args(["echo", "-raw"])
                    .status();
                return vec![];
            }
            b'\r' | b'\n' => break,
            27 if read_key() == b'[' => match read_key() {
                b'A' => {
                    cursor = cursor.saturating_sub(1);
                }
                b'B' if cursor < INTEGRATIONS.len() - 1 => {
                    cursor += 1;
                }
                _ => {}
            },
            _ => {}
        }
    }

    let _ = std::process::Command::new("stty")
        .args(["echo", "-raw"])
        .status();
    eprint!("\x1b[2J\x1b[H");
    INTEGRATIONS
        .iter()
        .enumerate()
        .filter_map(|(i, t)| if checked[i] { Some(t.id) } else { None })
        .collect()
}

fn wizard_uninstall_pipe() -> Vec<&'static str> {
    eprintln!("transpile uninstall — select integrations to remove");
    eprintln!("─────────────────────────────────────────────────────");
    for (i, ig) in INTEGRATIONS.iter().enumerate() {
        let status = if (ig.artifact)().exists() {
            " [installed]"
        } else {
            ""
        };
        eprintln!("  [{}] {:<10} {}{}", i + 1, ig.id, ig.label, status);
    }
    eprintln!("  [a] All of the above");
    eprintln!();
    eprint!("Selection (e.g. 1,3 or a): ");
    let _ = io::stderr().flush();
    read_selection()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared I/O helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn read_key() -> u8 {
    use std::io::Read;
    let mut buf = [0u8; 1];
    io::stdin().read_exact(&mut buf).ok();
    buf[0]
}

fn read_selection() -> Vec<&'static str> {
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    let line = line.trim().to_lowercase();
    if line == "a" || line == "all" {
        return INTEGRATIONS.iter().map(|i| i.id).collect();
    }
    let mut out = Vec::new();
    for token in line.split(',') {
        if let Ok(n) = token.trim().parse::<usize>()
            && n >= 1
            && n <= INTEGRATIONS.len()
        {
            out.push(INTEGRATIONS[n - 1].id);
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════════
// System helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}

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

fn shorten_path(p: &std::path::Path) -> String {
    let h = home();
    let s = p.to_string_lossy();
    if s.starts_with(&h) {
        format!("~{}", &s[h.len()..])
    } else {
        s.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::transpile_skill;

    #[test]
    fn skill_gemini_contains_agent_env() {
        let content = transpile_skill("gemini");
        assert!(
            content.contains("TRANSPILE_AGENT=gemini transpile"),
            "gemini skill must include TRANSPILE_AGENT=gemini in usage command"
        );
    }

    #[test]
    fn skill_codex_contains_agent_env() {
        let content = transpile_skill("codex");
        assert!(
            content.contains("TRANSPILE_AGENT=codex transpile"),
            "codex skill must include TRANSPILE_AGENT=codex in usage command"
        );
    }

    #[test]
    fn skill_opencode_contains_agent_env() {
        let content = transpile_skill("opencode");
        assert!(
            content.contains("TRANSPILE_AGENT=opencode transpile"),
            "opencode skill must include TRANSPILE_AGENT=opencode in usage command"
        );
    }

    #[test]
    fn skill_cursor_contains_agent_env() {
        let content = transpile_skill("cursor");
        assert!(
            content.contains("TRANSPILE_AGENT=cursor transpile"),
            "cursor skill must include TRANSPILE_AGENT=cursor in usage command"
        );
    }

    #[test]
    fn skill_preserves_rest_of_content() {
        for agent in &["gemini", "codex", "opencode", "cursor"] {
            let content = transpile_skill(agent);
            assert!(
                content.contains("name: transpile"),
                "agent={agent}: missing frontmatter"
            );
            assert!(
                content.contains("--fidelity semantic --quiet"),
                "agent={agent}: missing fidelity flag"
            );
            assert!(
                content.contains("alwaysApply: true"),
                "agent={agent}: missing alwaysApply"
            );
        }
    }
}
