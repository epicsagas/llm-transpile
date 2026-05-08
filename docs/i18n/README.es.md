# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Transpilador de documentos optimizado para tokens en pipelines LLM**

Documentos en bruto (Markdown, HTML, texto plano) → formato puente estructurado `<D>?<H><B>` — con compresión adaptativa que mantiene el presupuesto de tokens.

```
<H>
t: Contrato de Licencia de Software
s: Términos de licencia anuales entre licenciante y licenciatario
k: [licencia, contrato, software]
</H>
<B>
# Partes Contratantes
Este acuerdo se celebra entre el Licenciante y el Licenciatario.
...
</B>
```

---

<details>
<summary>Tabla de contenidos</summary>
- [Por qué](#por-qué)
- [Instalación](#instalación)
- [Uso de CLI](#uso-de-cli)
- [Uso de la biblioteca](#uso-de-la-biblioteca)
- [Formato de salida](#formato-de-salida)
- [Niveles de fidelidad](#niveles-de-fidelidad)
- [Compresión adaptativa](#compresión-adaptativa)
- [Formatos de entrada](#formatos-de-entrada)
- [Manejo de errores](#manejo-de-errores)
- [Rendimiento](#rendimiento)
- [Contribuir](#contribuir)
- [Licencia](#licencia)
</details>

---

## Por qué

Los LLM funcionan mejor cuando el contexto es limpio y denso. Esta biblioteca maneja el trabajo mecánico:

- **Análisis estructural** — Markdown/HTML/texto plano → nodos IR tipados (encabezados, párrafos, tablas, listas, bloques de código)
- **Compresión adaptativa** — escala automáticamente por 4 etapas a medida que el presupuesto de tokens se agota
- **Sustitución de símbolos** — términos de dominio repetidos → caracteres Unicode PUA, decodificados por el encabezado de diccionario `<D>`
- **Linearización de tablas** — tablas Markdown → secuencias compactas `Key:Val` (≤5 filas) o filas separadas por pipes para tablas más grandes
- **Salida en streaming** — el stream de Tokio entrega el primer bloque inmediatamente, minimizando el TTFT

---

## Instalación

### Biblioteca (crate de Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requiere **Rust 1.75+**.

### Binario CLI + integración de herramientas

```bash
# Homebrew (macOS)
brew tap epicsagas/tap
brew install llm-transpile

# Binario precompilado (más rápido, sin compilar)
cargo binstall llm-transpile

# Desde crates.io
cargo install llm-transpile
```

Configurar integraciones de herramientas:

```bash
transpile install
```

`transpile install` lanza un asistente interactivo que detecta y configura las herramientas instaladas:

| Herramienta | Método de integración | Función |
|-------------|----------------------|---------|
| **Claude Code** | Hook PostToolUse | Auto-comprime archivos `.md/.html/.txt` al leer |
| **Gemini CLI** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |
| **Codex CLI** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |
| **Cursor** | Regla `.mdc` (`alwaysApply`) | Activa `transpile` antes de leer archivos de documento |
| **OpenCode** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |

**Instalación / desinstalación selectiva**

```bash
transpile install claude gemini    # herramientas específicas
transpile install --all            # todo a la vez
transpile install --dry-run        # previsualizar cambios
transpile install --list           # ver estado de integraciones

transpile uninstall cursor         # eliminar una
transpile uninstall --all          # eliminar todo
transpile uninstall --dry-run      # previsualizar eliminaciones
```

**Plugin de Claude Code**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Desde el código fuente:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## Uso de CLI

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Ruta del archivo de entrada (lee desde stdin si se omite)
  -f, --format <FORMAT>    Formato de entrada: markdown | html | plaintext  [predeterminado: markdown]
                           Se detecta automáticamente desde la extensión del archivo con --input
  -l, --fidelity <LEVEL>  Nivel de compresión: lossless | semantic | compressed  [predeterminado: semantic]
  -b, --budget <N>         Límite superior del presupuesto de tokens (ilimitado si se omite)
  -c, --count              Imprime solo el recuento de tokens de entrada y sale
  -j, --json               Salida en JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Suprime la línea de estadísticas en stderr
      --stats              Imprime la línea de estadísticas en stdout tras el contenido
  -h, --help               Mostrar ayuda
  -V, --version            Mostrar versión
```

**Ejemplos**

```bash
# Convertir un archivo Markdown (formato detectado automáticamente por extensión .md)
transpile --input doc.md

# Leer desde stdin — stdout limpio, estadísticas en stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Pipe limpio — suprimir estadísticas completamente
transpile --input doc.md --quiet | send_to_llm_api

# Verificar recuento de tokens sin convertir
transpile --input doc.md --count

# Salida JSON para scripts y pipelines
transpile --input doc.md --json | jq '.reduction_pct'

# Capturar contenido + estadísticas en un stream
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — sin compresión, contenido completo preservado (documentos legales/auditoría)
transpile --input contract.md --fidelity lossless

# Compresión agresiva en un presupuesto de 512 tokens
transpile --input article.md --fidelity compressed --budget 512
```

> Las estadísticas (`[273 → 150 tok  45.1% reduction]`) se escriben en **stderr** por defecto, manteniendo stdout limpio para pipes. Use `--quiet` para suprimirlas, o `--stats` para redirigirlas a stdout.

---

## Uso de la biblioteca

### Síncrono

```rust
use llm_transpile::{transpile, FidelityLevel, InputFormat};

let md = r#"
# Software License Agreement

This agreement is made between Licensor and Licensee.

| Item     | Cost  |
|----------|-------|
| Base fee | $800  |
| Support  | $200  |
"#;

let output = transpile(md, InputFormat::Markdown, FidelityLevel::Semantic, Some(4096))?;
println!("{}", output);
```

### Streaming (Tokio)

```rust
use llm_transpile::{transpile_stream, FidelityLevel, InputFormat};
use futures::StreamExt;

let mut stream = transpile_stream(input, InputFormat::Markdown, FidelityLevel::Semantic, 4096).await;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
    if chunk.is_final { break; }
}
```

### Estimación de recuento de tokens

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## Formato de salida

```
<D>                  ← Diccionario de símbolos (omitido sin sustituciones)
{sym}=término-repetido
</D>
<H>                  ← Encabezado de metadatos tipo YAML
t: título del documento
s: resumen en una línea
k: [keyword1, keyword2]
</H>
<B>                  ← Cuerpo del documento (comprimido + sustituido)
...contenido...
</B>
```

El bloque `<D>` usa caracteres del Área de Uso Privado Unicode (`U+E000–U+F8FF`) como identificadores de símbolo compactos, evitando colisiones con patrones de texto visible. El diccionario soporta hasta **6,400 términos únicos** por documento.

---

## Niveles de fidelidad

| Nivel | Caso de uso típico | Compresión aplicada |
|-------|-------------------|---------------------|
| `Lossless` | Documentos legales/auditoría | Ninguna — contenido original garantizado |
| `Semantic` | Pipelines RAG generales | Eliminación de palabras vacías + poda de baja importancia |
| `Compressed` | Resumen, presupuestos ajustados | Compresión máxima, extracción de primera oración |

---

## Compresión adaptativa

El compresor monitorea el uso del presupuesto en tiempo real y escala automáticamente:

| Uso del presupuesto | Etapa | Qué ocurre |
|--------------------|-------|------------|
| 0–60% | `StopwordOnly` | Se eliminan palabras vacías inglés/coreano |
| 60–80% | `PruneLowImportance` | Se eliminan el 20% inferior de párrafos por importancia |
| 80–95% | `DeduplicateAndLinearize` | Se eliminan oraciones duplicadas; tablas linealizadas |
| 95%+ | `MaxCompression` | Cada párrafo truncado a la primera oración |

> El modo `Lossless` omite todas las etapas de compresión incondicionalmente.

Durante el streaming, cuando el uso del presupuesto supera el 80%, los nodos restantes cambian automáticamente al modo `Compressed`.

---

## Formatos de entrada

| `InputFormat` | Analizador |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + tablas GFM |
| `Html` | saneamiento ammonia → eliminación de etiquetas → pipeline de texto plano |
| `PlainText` | División de párrafos por línea en blanco |

---

## Manejo de errores

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* usar output */ }
    Err(TranspileError::Parse(msg))            => eprintln!("error de análisis: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("demasiados términos únicos: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("compresión en modo lossless"),
    Err(e)                                     => eprintln!("error: {e}"),
}
```

---

## Rendimiento

Medido con build de release (`cargo build --release`), Apple M-series, 48 documentos entre Markdown/HTML/PlainText:

| Métrica | Medido | Notas |
|---------|--------|-------|
| Rendimiento | **10,975 tok/ms** | ≈75× más rápido que la línea base de Python |
| Reducción Semantic | **33.9%** (Markdown) | Objetivo 15–30% alcanzado |
| Reducción Compressed | **39.7%** (Markdown) | Adaptativo al presupuesto, ≥ PruneLowImportance garantizado |
| Cobertura de palabras Lossless | **98.8% promedio** | En todos los formatos e idiomas |
| Reducción HTML | **97.6%** | Eliminación de overhead de marcado nav/scripts/estilos |
| Soporte multilingüe | 15 idiomas probados | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.4% cobertura promedio |

Ejecuta la suite de evaluación por tu cuenta:

```bash
cargo run --release --example eval
```

---

## Contribuir

Se aceptan reportes de errores, solicitudes de funciones y pull requests.

```bash
# Clonar y compilar
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# Ejecutar pruebas
cargo test

# Ejecutar benchmarks (informe HTML → target/criterion/)
cargo bench

# Lint y formato
cargo clippy -- -D warnings
cargo fmt
```

**Directrices**

- Mantener MSRV en Rust 1.75 — evitar características introducidas después.
- Los nuevos comportamientos de compresión no deben afectar el modo `Lossless`.
- Cada PR debe incluir pruebas para la nueva lógica en el módulo relevante (`ir`, `compressor`, `symbol`, `renderer`).
- Ejecutar `cargo clippy -- -D warnings` y `cargo fmt` antes de enviar.

---

## Licencia

Apache-2.0 — ver [LICENSE](LICENSE).
