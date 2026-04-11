---
description: Add llm-transpiler to a Rust project and generate integration code
tags: [llm-transpiler, rust, integration, crate]
---

# Integrate llm-transpiler

Add the `llm-transpiler` crate to the current Rust project and generate working integration code.

## Task

1. **Add the dependency to `Cargo.toml`**
   ```toml
   [dependencies]
   llm-transpiler = "0.1"
   ```
   Run `cargo add llm-transpiler` or edit `Cargo.toml` directly.

2. **Ask the user what they need**
   - (A) Synchronous one-shot conversion
   - (B) Async streaming (Tokio)
   - (C) Both

3. **Generate integration code based on selection**

   **Option A — Synchronous**
   ```rust
   use llm_transpiler::{transpile, FidelityLevel, InputFormat};

   fn transpile_doc(content: &str) -> anyhow::Result<String> {
       let output = transpile(
           content,
           InputFormat::Markdown,
           FidelityLevel::Semantic,
           Some(4096),
       )?;
       Ok(output)
   }
   ```

   **Option B — Streaming (Tokio)**
   ```rust
   use futures::StreamExt;
   use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};

   async fn stream_doc(content: &str) -> anyhow::Result<String> {
       let mut stream = transpile_stream(
           content,
           InputFormat::Markdown,
           FidelityLevel::Semantic,
           4096,
       )
       .await;

       let mut output = String::new();
       while let Some(chunk) = stream.next().await {
           let chunk = chunk?;
           output.push_str(&chunk.content);
           if chunk.is_final {
               break;
           }
       }
       Ok(output)
   }
   ```

4. **Insert the generated code** into the appropriate file in the user's project

5. **Verify it compiles**
   ```bash
   cargo build
   ```

6. **Show token count utility** if the user needs it
   ```rust
   let n = llm_transpiler::token_count(content);
   println!("Input: {n} tokens");
   ```

## Requirements

- Detect whether the project already uses Tokio before suggesting async code
- Use the user's existing error handling style (`anyhow`, `thiserror`, plain `Result`, etc.)
- Do not add unnecessary dependencies beyond `llm-transpiler`

## Notes

- `InputFormat` options: `Markdown`, `Html`, `PlainText`
- `FidelityLevel` options: `Lossless` (no compression), `Semantic` (default), `Compressed` (maximum)
- `token_count()` uses a character-based heuristic — not a real tokenizer
- Full API docs: https://docs.rs/llm-transpiler
