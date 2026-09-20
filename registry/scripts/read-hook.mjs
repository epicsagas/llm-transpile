#!/usr/bin/env node
// llm-transpile cross-host PreToolUse hook
// Works on Claude Code, Codex, and grok: each sends the tool call as JSON on
// stdin (Claude/Codex snake_case `tool_input`, grok camelCase `toolInput` with
// `path` instead of `file_path`). For compressible docs the raw Read is denied
// and the transpiled bridge format is delivered as the denial reason, so the
// model never sees the uncompressed file.
//
// Output uses the canonical Claude JSON shape
// (hookSpecificOutput.permissionDecision). grok documents compatibility with
// exactly this shape; Codex mirrors Claude's hook protocol. Anything that goes
// wrong is a silent allow (exit 0, no output) — a broken hook must never block
// or corrupt a read.
//
// No env vars are referenced from hooks.json beyond the plugin-root ones with
// `:-` defaults: grok statically validates `${VAR}` refs in hook commands and
// refuses to spawn the hook when one is unset.

import { spawnSync } from "node:child_process";
import { extname } from "node:path";

const COMPRESSIBLE = new Set([".md", ".markdown", ".html", ".htm", ".txt"]);

function readStdin() {
  return new Promise((resolve) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => { data += chunk; });
    process.stdin.on("end", () => resolve(data));
    // Some hosts may keep stdin open longer than the payload needs.
    setTimeout(() => resolve(data), 5000).unref();
  });
}

function extractFilePath(payload) {
  try {
    const data = JSON.parse(payload);
    const input = data.tool_input || data.toolInput || data.input;
    if (typeof input !== "object" || input === null) return "";
    return String(input.file_path || input.filePath || input.path || "");
  } catch {
    return "";
  }
}

const filePath = extractFilePath(await readStdin());
if (!filePath || !COMPRESSIBLE.has(extname(filePath).toLowerCase())) {
  process.exit(0);
}

const result = spawnSync(
  "transpile",
  ["--input", filePath, "--fidelity", "semantic", "--quiet"],
  { stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 },
);
const transpiled = result.status === 0 && result.stdout
  ? result.stdout.toString("utf8").trim()
  : "";
if (!transpiled) process.exit(0);

process.stdout.write(JSON.stringify({
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "deny",
    permissionDecisionReason: transpiled,
  },
}));
process.exit(0);
