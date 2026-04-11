---
name: transpile-batch
description: >
  Batch-transpile all documents in a directory using llm-transpiler CLI.
  Triggers when user wants to process multiple files at once.
  Generates and optionally runs a shell script for bulk conversion.
---

# transpile-batch

## Trigger

Invoke when user wants to transpile multiple documents at once.

Keywords: batch transpile, bulk convert, process all documents, transpile directory, all files, multiple documents

## Behavior

1. **Gather inputs** from user:
   - Input directory
   - Output directory (default: `<input>_transpiled`)
   - Extensions to process (default: `.md`, `.html`, `.txt`)
   - Fidelity level (default: `semantic`)
   - Token budget per document (optional)

2. **Check CLI is installed**:
   ```bash
   which transpile 2>/dev/null || cargo install --git https://github.com/epicsagas/llm-transpiler --bin transpile
   ```

3. **Generate `transpile_batch.sh`**:
   ```bash
   #!/usr/bin/env bash
   set -euo pipefail

   INPUT_DIR="<INPUT_DIR>"
   OUTPUT_DIR="<OUTPUT_DIR>"
   FIDELITY="semantic"
   BUDGET_FLAG=""   # e.g. "--budget 4096"

   mkdir -p "$OUTPUT_DIR"
   total=0; success=0; failed=0

   for f in "$INPUT_DIR"/**/*.md "$INPUT_DIR"/**/*.html "$INPUT_DIR"/**/*.txt; do
     [[ -f "$f" ]] || continue
     rel="${f#$INPUT_DIR/}"
     out="$OUTPUT_DIR/${rel%.*}.bridge.txt"
     mkdir -p "$(dirname "$out")"
     if transpile --input "$f" --fidelity "$FIDELITY" $BUDGET_FLAG > "$out" 2>/dev/null; then
       echo "✓ $rel"; ((success++))
     else
       echo "✗ $rel" >&2; ((failed++))
     fi
     ((total++))
   done

   echo "Done: $success/$total succeeded, $failed failed — output: $OUTPUT_DIR"
   ```

4. **Write the script** and make it executable:
   ```bash
   chmod +x transpile_batch.sh
   ```

5. **Ask user to confirm** before running, then execute:
   ```bash
   ./transpile_batch.sh
   ```

6. **Report**: files processed, success/fail count, output directory path

## Rules

- Always confirm before executing the generated script
- Output files use `.bridge.txt` extension — never overwrite originals
- The script is idempotent — safe to re-run
- Stderr stats from `transpile` are suppressed per file; only failures are surfaced
