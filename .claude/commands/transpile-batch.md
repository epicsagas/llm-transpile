---
description: Generate a batch script to transpile all documents in a directory
tags: [llm-transpiler, batch, script, automation]
---

# Transpile Batch

Generate a shell script that batch-transpiles all documents in a directory using the `transpile` CLI.

## Task

1. **Ask the user for**
   - Input directory path
   - Output directory path (default: `<input_dir>_transpiled`)
   - File extensions to process (default: `.md`, `.html`, `.txt`)
   - Fidelity level: `lossless` / `semantic` / `compressed` (default: `semantic`)
   - Token budget per document (optional)

2. **Check if the CLI is installed**
   ```bash
   which transpile || cargo install --git https://github.com/epicsagas/llm-transpiler --bin transpile
   ```

3. **Generate the batch script**
   ```bash
   #!/usr/bin/env bash
   set -euo pipefail

   INPUT_DIR="<INPUT_DIR>"
   OUTPUT_DIR="<OUTPUT_DIR>"
   FIDELITY="semantic"
   BUDGET_FLAG=""   # set to "--budget 4096" if needed

   mkdir -p "$OUTPUT_DIR"

   total=0; success=0; failed=0

   for f in "$INPUT_DIR"/**/*.md "$INPUT_DIR"/**/*.html "$INPUT_DIR"/**/*.txt; do
     [[ -f "$f" ]] || continue
     rel="${f#$INPUT_DIR/}"
     out="$OUTPUT_DIR/${rel%.*}.bridge.txt"
     mkdir -p "$(dirname "$out")"

     if transpile --input "$f" --fidelity "$FIDELITY" $BUDGET_FLAG > "$out" 2>/dev/null; then
       echo "✓ $rel"
       ((success++))
     else
       echo "✗ $rel" >&2
       ((failed++))
     fi
     ((total++))
   done

   echo ""
   echo "Done: $success/$total succeeded, $failed failed"
   echo "Output: $OUTPUT_DIR"
   ```

4. **Write the script** to `transpile_batch.sh` in the current directory and make it executable
   ```bash
   chmod +x transpile_batch.sh
   ```

5. **Run it** if the user confirms
   ```bash
   ./transpile_batch.sh
   ```

6. **Report summary** — files processed, success rate, output directory

## Requirements

- `transpile` binary must be installed before running the script
- Output files use `.bridge.txt` extension to avoid overwriting originals
- The script must be idempotent — re-running overwrites existing output files

## Notes

- For JSON output (machine-readable), add `--json` flag and pipe through `jq`
- For very large directories, consider adding `parallel` or `xargs -P` for concurrency
- Stderr stats from `transpile` are suppressed in the script — redirect if needed
