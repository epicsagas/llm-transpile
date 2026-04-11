---
name: transpile
description: >
  Transpile documents to LLM-optimized bridge format using llm-transpiler.
  Triggers when user asks to convert, compress, or optimize a document for LLM consumption.
  Handles CLI usage, token budget decisions, and fidelity level selection.
---

# transpile

## Trigger

Invoke when user wants to convert a document to bridge format for LLM use.

Keywords: transpile, convert document, compress for LLM, optimize tokens, bridge format, token budget, llm-transpiler

## Behavior

1. **Check CLI is installed**
   ```bash
   which transpile 2>/dev/null || cargo install --git https://github.com/epicsagas/llm-transpiler --bin transpile
   ```

2. **Determine inputs** from user context:
   - File path or stdin content
   - Fidelity level (default: `semantic`)
     - `lossless` — legal/audit docs, zero information loss
     - `semantic` — general RAG, 15–30% reduction
     - `compressed` — summarization, maximum compression
   - Token budget (ask user or infer from their LLM context window)

3. **Check token count first** if budget is unclear:
   ```bash
   transpile --input <FILE> --count
   ```

4. **Run transpile**:
   ```bash
   transpile --input <FILE> --fidelity <LEVEL> [--budget <N>]
   ```

5. **Report result**: input tokens → output tokens, reduction %, show bridge output

6. **If user needs JSON** (for piping to another tool):
   ```bash
   transpile --input <FILE> --json
   ```

## Rules

- Default fidelity is `semantic` unless user specifies otherwise
- Always show token reduction stats after transpiling
- If file extension is `.md`/`.html`/`.txt`, format is auto-detected — no `--format` flag needed
- Stderr stats do not pollute stdout — piping is safe
- Never modify the source document
