---
name: transpile-integrate
description: >
  Add llm-transpiler crate to a Rust project and generate integration code.
  Triggers when user wants to use llm-transpiler as a library in their own Rust project.
  Detects project style (sync/async, error handling) and generates idiomatic code.
---

# transpile-integrate

## Trigger

Invoke when user wants to integrate llm-transpiler into their Rust codebase.

Keywords: add llm-transpiler, integrate transpiler, use transpiler crate, llm-transpiler dependency, transpile in rust

## Behavior

1. **Add the dependency**:
   ```bash
   cargo add llm-transpiler
   ```
   Or add manually to `Cargo.toml`:
   ```toml
   [dependencies]
   llm-transpiler = "0.1"
   ```

2. **Detect project style** by reading existing source files:
   - Uses Tokio? → generate async streaming code
   - Uses `anyhow`? → match error handling style
   - Sync-only? → generate synchronous code

3. **Generate integration code** based on detected style:

   **Synchronous**:
   ```rust
   use llm_transpiler::{transpile, FidelityLevel, InputFormat};

   fn prepare_for_llm(content: &str) -> anyhow::Result<String> {
       Ok(transpile(content, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))?)
   }
   ```

   **Async streaming**:
   ```rust
   use futures::StreamExt;
   use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};

   async fn prepare_for_llm(content: &str) -> anyhow::Result<String> {
       let mut stream = transpile_stream(content, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;
       let mut out = String::new();
       while let Some(chunk) = stream.next().await {
           let chunk = chunk?;
           out.push_str(&chunk.content);
           if chunk.is_final { break; }
       }
       Ok(out)
   }
   ```

4. **Insert the code** into the appropriate file in the user's project

5. **Verify it compiles**:
   ```bash
   cargo build
   ```

## Rules

- Match the user's existing error handling (`anyhow`, `thiserror`, plain `Result`)
- Do not add Tokio if the project is sync-only
- Do not add unnecessary dependencies
- `InputFormat` options: `Markdown`, `Html`, `PlainText`
- `FidelityLevel` options: `Lossless`, `Semantic`, `Compressed`
- Full docs: https://docs.rs/llm-transpiler
