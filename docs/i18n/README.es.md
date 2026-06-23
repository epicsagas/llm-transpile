<div align="center">
<h1>llm-transpile</h1> 

<p align="center">
  <a href="https://github.com/epicsagas/llm-transpile/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=ffd700&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/network/members"><img alt="Forks" src="https://img.shields.io/github/forks/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=2ecc71&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/issues"><img alt="Issues" src="https://img.shields.io/github/issues/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=ff6b6b&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/llm-transpile/commits/main"><img alt="Last commit" src="https://img.shields.io/github/last-commit/epicsagas/llm-transpile?style=for-the-badge&labelColor=0d1117&color=58a6ff&logo=git&logoColor=white" /></a>
</p>
<p align="center">
  <a href="https://crates.io/crates/llm-transpile"><img alt="Crates.io" src="https://img.shields.io/crates/v/llm-transpile?style=for-the-badge&labelColor=0d1117&color=fc8d62&logo=rust&logoColor=white" /></a>
  <a href="https://docs.rs/llm-transpile"><img alt="docs.rs" src="https://img.shields.io/docsrs/llm-transpile?style=for-the-badge&labelColor=0d1117&color=8e44ad&logo=docsdotrs&logoColor=white" /></a>
  <a href="../../LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-3fb950?style=for-the-badge&labelColor=0d1117" /></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.92+-d73a49?style=for-the-badge&labelColor=0d1117&logo=rust&logoColor=white" />
  <a href="https://buymeacoffee.com/epicsaga"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/buy_me_a_coffee-FFDD00?style=for-the-badge&labelColor=0d1117&logo=buymeacoffee&logoColor=black" /></a>
</p>

**Transpilador de documentos optimizado para tokens en pipelines de LLM**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

Documentos en bruto (Markdown, HTML, texto plano) → formato puente estructurado `<D>?<H><B>` — con compresión adaptativa que mantiene el presupuesto de tokens.

---

<details>
<summary>Tabla de contenidos</summary>
- [Por qué](#por-qué)
- [Instalación](#instalación)
- [Actualización](#actualización)
- [Uso de CLI](#uso-de-cli)
- [Estadísticas de uso](#estadísticas-de-uso)
- [Uso de la biblioteca](#uso-de-la-biblioteca)
- [Formato de salida](#formato-de-salida)
- [Niveles de fidelidad](#niveles-de-fidelidad)
- [Compresión adaptativa](#compresión-adaptativa)
- [Formatos de entrada](#formatos-de-entrada)
- [Manejo de errores](#manejo-de-errores)
- [Rendimiento](#rendimiento)
- [Contribuir](#contribuir)
- [Licencia](#licencia)- [Pruebas de rendimiento](#pruebas-de-rendimiento-benchmarking)

</details>

---

## Por qué

Los LLM funcionan mejor cuando el contexto es limpio y denso. Esta biblioteca maneja el trabajo mecánico:

| | Característica | Por qué importa |
|--|----------------|-----------------|
| 🏗️ | **Análisis estructural** | Markdown/HTML/texto plano → nodos IR tipados (encabezados, párrafos, tablas, listas, bloques de código) |
| 📉 | **Compresión adaptativa** | Escala automáticamente por 4 etapas a medida que el presupuesto de tokens se agota |
| 🔣 | **Sustitución de símbolos** | Términos de dominio repetidos → caracteres Unicode PUA, decodificados por el encabezado de diccionario `<D>` |
| 📊 | **Linearización de tablas** | Tablas Markdown → secuencias compactas `Key:Val` (≤5 filas) o filas separadas por pipes para tablas más grandes |
| 🌊 | **Salida en streaming** | El stream de Tokio entrega el primer bloque inmediatamente, minimizando el TTFT |

### Benchmarks

48 documentos, 3 formatos, 15 idiomas — Apple M-series, build `--release`. Las cifras a continuación se miden con el **tokenizador BPE `cl100k` real** (no la heurística auto-referencial — ver el análisis). Metodología completa y desglose de honestidad de tokens: [`docs/EVALUATION.md`](../EVALUATION.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown | 27.4% | 69.4% | 99.0% | — |
| HTML | 98.7% | 99.3% | 99.0% | — |
| PlainText | -3.5% | 30.4% | 99.0% | — |
| **Overall (BPE)** | **81.5%** | **91.8%** | **99.0%** | **~1,070 tok/ms** |

> ⚠️ La cifra global está dominada por la eliminación del marcado HTML. **Markdown 27.4% es la tasa de compresión genuina.** PlainText es neto-negativo en modo Semantic debido al overhead estructural. Ver [`docs/EVALUATION.md`](../EVALUATION.md) para la realidad por formato.

> La reducción de HTML refleja la eliminación del overhead de marcado (nav, scripts, estilos), no solo la compresión del texto.

---

## Instalación

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Auto-instala el binario y configura el hook PostToolUse en el próximo inicio de sesión — sin configuración adicional necesaria.

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

El hook PostToolUse se registra automáticamente — no se necesitan pasos adicionales.

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

¿Sin Homebrew? Usa el script de instalación:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### Vía toolchain de Rust

```bash
cargo binstall llm-transpile   # binario precompilado (rápido)
cargo install llm-transpile    # compilar desde fuente
```

### Después de instalar

Configurar integraciones de herramientas:

```bash
transpile install
```

`transpile install` lanza un asistente interactivo que detecta y configura las herramientas instaladas:

| Herramienta | Método de integración | Función |
|-------------|----------------------|---------|
| **Antigravity** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |
| **Cursor** | Regla `.mdc` (`alwaysApply`) | Activa `transpile` antes de leer archivos de documento |
| **OpenCode** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |
| **Cline** | `SKILL.md` | LLM invoca automáticamente `transpile` en extensiones de archivo |

Todas las herramientas usan un archivo skill que enseña al LLM a ejecutar `TRANSPILE_AGENT=<agent> transpile --input <file>` automáticamente — no se necesita verificación de tamaño, la extensión por sí sola lo activa.

**Instalación / desinstalación selectiva**

```bash
transpile install antigravity cursor    # herramientas específicas
transpile install --all            # todo a la vez
transpile install --dry-run        # previsualizar cambios
transpile install --list           # ver estado de integraciones

transpile uninstall cursor         # eliminar una
transpile uninstall --all          # eliminar todo
transpile uninstall --dry-run      # previsualizar eliminaciones
```

### Biblioteca (crate de Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requiere **Rust 1.92+**.

### Antigravity (Gemini CLI)

```bash
agy plugins install https://github.com/epicsagas/llm-transpile
```

Instala automáticamente el plugin (hooks) y lo registra al iniciar la próxima sesión.


### Pruebas de rendimiento (Benchmarking)


```bash
# Ejecutar pruebas contra un directorio de archivos
transpile bench run --dataset ./eval                    # genera un registro JSONL
transpile bench run --dataset ./eval --report           # ejecutar + abrir informe HTML
transpile bench report                                  # regenerar informe desde los registros
```

El informe HTML incluye:

- **Tarjetas KPI** — reducción semántica, reducción comprimida, rendimiento (tok/ms), cobertura de palabras, total de tokens de entrada, número de ejecuciones
- **7 gráficos** — tendencia de reducción, rendimiento por ejecución, dispersión semántica vs rendimiento, diagrama de caja por formato, distribución de formatos, histograma de tamaño de token, anillo de cobertura de palabras
- **Tabla de ejecuciones** — resumen con métricas agregadas
- **Tabla de registros** — detalle por archivo con filtro de formato, ejecución y nombre
- **Tema** — modo oscuro/claro con preferencia persistente
- **Bilingüe** — autodetecta configuración regional coreana; interruptor manual KO/EN


---

---

## Actualización

| Método | Comando |
|--------|---------|
| Homebrew | `brew upgrade llm-transpile` |
| Instalador curl / PowerShell | Re-ejecutar el comando de instalación anterior |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
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

## Estadísticas de uso

Cada invocación de `transpile` agrega automáticamente un registro a `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`. El subcomando `transpile stats` lee esos archivos e imprime una tabla resumen.

```
transpile stats show                # hoy
transpile stats show --days 7       # últimos N días
transpile stats show --agent claude # filtrar por agente
```

Ejemplo de salida:

```
transpile stats — últimos 7 días

  Fecha       Agente     Llamadas  Tokens entrada  Tokens salida  Ahorrados  Reducción
  ──────────────────────────────────────────────────────────────────────────────────
  2026-04-13  claude          5      14 965           10 872       4 093      27.3%
  2026-04-13  antigravity          2       4 800            3 500       1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────────────
  Total                       7      19 765           14 372       5 393      27.3%
```

**Campos del registro JSONL**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `ts` | ISO 8601 | Marca de tiempo de la invocación |
| `agent` | string | Herramienta que activó la llamada (`claude`, `antigravity`, `codex`, `opencode`) |
| `file` | string | Ruta del archivo de entrada (vacío al leer desde stdin) |
| `format` | string | `markdown`, `html`, o `plaintext` |
| `fidelity` | string | `lossless`, `semantic`, o `compressed` |
| `input_tok` | integer | Recuento de tokens antes de la transpilación |
| `output_tok` | integer | Recuento de tokens después de la transpilación |
| `reduction_pct` | float | Porcentaje de tokens ahorrados |
| `saved` | integer | Tokens ahorrados absolutos (`input_tok − output_tok`) |

**Variable de entorno `TRANSPILE_AGENT`**

El campo `agent` se completa desde la variable de entorno `TRANSPILE_AGENT`. Cada integración la configura automáticamente (`claude`, `antigravity`, `codex`, `opencode`, `cursor`). También puedes configurarla manualmente:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## Uso de la biblioteca

### Síncrono

```rust
use llm_transpiler::{transpile, FidelityLevel, InputFormat};

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
use llm_transpiler::{transpile_stream, FidelityLevel, InputFormat};
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
let n = llm_transpiler::token_count("Hello, world!");
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
use llm_transpiler::TranspileError;

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

Medido con build de release (`cargo build --release`), Apple M-series, 48 documentos entre Markdown/HTML/PlainText. Todas las cifras de reducción se miden con el **tokenizador BPE `cl100k` real** (no la heurística auto-referencial). Ver [`docs/EVALUATION.md`](../EVALUATION.md) para la metodología completa y el desglose por formato.

| Métrica | Medido | Notas |
|---------|--------|-------|
| Rendimiento (pico solo Markdown) | **10,975 tok/ms** | ≈75× más rápido que la línea base de Python; pico de formato único |
| Rendimiento (agregado del dataset) | **~1,070 tok/ms** | Ponderado en los 48 docs / 3 formatos (BPE) — ver tabla de Benchmarks |
| Reducción Semantic | **27.4%** (Markdown) | Tasa de compresión genuina; dentro de la banda objetivo 15–30% |
| Reducción Compressed | **69.4%** (Markdown) | Adaptativo al presupuesto, ≥ PruneLowImportance garantizado |
| Cobertura de palabras Lossless | **99.0% promedio** | En todos los formatos e idiomas |
| Reducción HTML | **98.7%** | Eliminación de overhead de marcado nav/scripts/estilos |
| Soporte multilingüe | 15 idiomas probados | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.0% cobertura promedio |

Ejecuta la suite de evaluación por tu cuenta:

```bash
make eval          # JSON estructurado (BPE + heurística; consumido por `epic eval`)
make eval-report   # tabla por archivo legible + resumen
```

Desglose por archivo, metodología y limitaciones conocidas: [`docs/EVALUATION.md`](../EVALUATION.md)

---

## Contribuir

Consulta [CONTRIBUTING.md](../../CONTRIBUTING.md) para las directrices completas. Se aceptan PRs — revisa los issues abiertos etiquetados `good first issue`.

---

## Licencia

Apache-2.0 — ver [LICENSE](../../LICENSE).
