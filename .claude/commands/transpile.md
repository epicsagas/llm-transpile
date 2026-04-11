---
description: Transpile a document to LLM-optimized bridge format using llm-transpiler CLI
tags: [llm-transpiler, transpile, token, document]
---

# Transpile Document

Convert a document to LLM-optimized bridge format (`<D>?<H><B>`) using the `transpile` CLI.

## Task

1. **Check if the CLI is installed**
   ```bash
   which transpile || cargo install --git https://github.com/epicsagas/llm-transpiler --bin transpile
   ```

2. **Ask the user for**
   - Target file path (or use stdin)
   - Fidelity level: `lossless` / `semantic` (default) / `compressed`
   - Token budget (optional — leave empty for unlimited)

3. **Run the transpile command**
   ```bash
   # File input
   transpile --input <FILE> --fidelity <LEVEL> [--budget <N>]

   # stdin
   cat <FILE> | transpile --format <FORMAT> --fidelity <LEVEL> [--budget <N>]
   ```

4. **Show the result**
   - Print the bridge-format output
   - Report: input tokens → output tokens, reduction %

5. **If the user wants JSON output** (for piping into another tool)
   ```bash
   transpile --input <FILE> --json | jq '.content'
   ```

## Requirements

- `transpile` binary must be installed (step 1 handles this)
- Format is auto-detected from file extension (`.md`, `.html`, `.txt`)
- Stats are written to stderr — stdout is clean for piping

## Notes

- Fidelity guide:
  - `lossless` — legal/audit docs, no information loss
  - `semantic` — general RAG pipelines, 15–30% token reduction
  - `compressed` — summarization, tight budgets, maximum compression
- Token budget is a hard upper limit on output tokens
- Use `--count` flag to check input token count before choosing a budget:
  ```bash
  transpile --input doc.md --count
  ```
