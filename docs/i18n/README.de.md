# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Token-optimierter Dokument-Transpiler für LLM-Pipelines**

Rohdokumente (Markdown, HTML, Klartext) → strukturiertes Brückenformat `<D>?<H><B>` — mit adaptiver Komprimierung, die das Token-Budget einhält.

```
<H>
t: Softwarelizenzvertrag
s: Jährliche Lizenzbedingungen zwischen Lizenzgeber und Lizenznehmer
k: [Lizenz, Vertrag, Software]
</H>
<B>
# Vertragsparteien
Dieser Vertrag wird zwischen dem Lizenzgeber und dem Lizenznehmer geschlossen.
...
</B>
```

---

<details>
<summary>Inhaltsverzeichnis</summary>

- [Warum](#warum)
- [Installation](#installation)
- [Aktualisierung](#aktualisierung)
- [CLI-Nutzung](#cli-nutzung)
- [Nutzungsstatistiken](#nutzungsstatistiken)
- [Bibliotheksnutzung](#bibliotheksnutzung)
- [Ausgabeformat](#ausgabeformat)
- [Treuestufen](#treuestufen)
- [Adaptive Komprimierung](#adaptive-komprimierung)
- [Eingabeformate](#eingabeformate)
- [Fehlerbehandlung](#fehlerbehandlung)
- [Leistung](#leistung)
- [Mitwirken](#mitwirken)
- [Lizenz](#lizenz)

</details>

---

## Warum

LLMs arbeiten besser, wenn der Kontext sauber und kompakt ist. Diese Bibliothek übernimmt die mechanische Arbeit:

| | Funktion | Warum es wichtig ist |
|--|---------|---------------------|
| 🏗️ | **Strukturelles Parsing** | Markdown/HTML/Klartext → typisierte IR-Knoten (Überschriften, Absätze, Tabellen, Listen, Codeblöcke) |
| 📉 | **Adaptive Komprimierung** | Eskaliert automatisch durch 4 Stufen, wenn das Token-Budget aufgebraucht wird |
| 🔣 | **Symbolersetzung** | Wiederholte Fachbegriffe → Unicode-PUA-Zeichen, dekodiert durch den `<D>`-Wörterbuch-Header |
| 📊 | **Tabellenlinearisierung** | Markdown-Tabellen → kompakte `Key:Val`-Sequenzen (≤5 Zeilen) oder pipe-getrennte Zeilen für größere Tabellen |
| 🌊 | **Streaming-Ausgabe** | Tokio-Stream liefert den ersten Block sofort und minimiert die TTFT |

---

## Installation

### Bibliothek (Rust-Crate)

```toml
[dependencies]
llm-transpile = "0.1"
```

Erfordert **Rust 1.92+**.

### CLI-Binärdatei + Tool-Integration

**macOS / Linux**

```bash
brew install epicsagas/tap/llm-transpile
```

Kein Homebrew? Installer-Skript verwenden:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

**Windows**

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

**Über Rust-Toolchain**

```bash
cargo binstall llm-transpile   # vorgefertigte Binärdatei (schnell)
cargo install llm-transpile    # aus dem Quellcode kompilieren
```

Tool-Integrationen konfigurieren:

```bash
transpile install
```

`transpile install` startet einen interaktiven Assistenten, der installierte Tools erkennt und konfiguriert:

| Tool | Integrationsmethode | Funktion |
|------|---------------------|---------|
| **Claude Code** | PostToolUse-Hook | Komprimiert `.md/.html/.txt`-Dateien beim Lesen automatisch |
| **Gemini CLI** | `SKILL.md` | LLM ruft `transpile` bei Dokumentdateiendungen automatisch auf |
| **Codex CLI** | `SKILL.md` | LLM ruft `transpile` bei Dokumentdateiendungen automatisch auf |
| **Cursor** | `.mdc`-Regel (`alwaysApply`) | Löst `transpile` vor dem Lesen von Dokumentdateien aus |
| **OpenCode** | `SKILL.md` | LLM ruft `transpile` bei Dokumentdateiendungen automatisch auf |

Alle Nicht-Claude-Tools verwenden eine Skill-Datei, die den LLM anweist, `TRANSPILE_AGENT=<agent> transpile --input <file>` automatisch auszuführen — keine Größenprüfung erforderlich, allein die Dateiendung löst es aus.

**Selektive Installation / Deinstallation**

```bash
transpile install claude gemini    # nur bestimmte Tools
transpile install --all            # alles auf einmal
transpile install --dry-run        # Vorschau der Änderungen
transpile install --list           # Integrationsstatus anzeigen

transpile uninstall cursor         # eines entfernen
transpile uninstall --all          # alles entfernen
transpile uninstall --dry-run      # Vorschau der Entfernungen
```

**Claude Code-Plugin**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Installiert die Binärdatei automatisch und richtet den PostToolUse-Hook beim nächsten Sitzungsstart ein — keine zusätzliche Einrichtung erforderlich.

Aus dem Quellcode:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## Aktualisierung

| Methode | Befehl |
|---------|--------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShell-Installer | Installationsbefehl erneut ausführen |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## CLI-Nutzung

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Eingabedateipfad (liest von stdin, wenn weggelassen)
  -f, --format <FORMAT>    Eingabeformat: markdown | html | plaintext  [Standard: markdown]
                           Wird bei --input automatisch von der Dateiendung erkannt
  -l, --fidelity <LEVEL>  Kompressionsstufe: lossless | semantic | compressed  [Standard: semantic]
  -b, --budget <N>         Oberes Limit für Token-Budget (unbegrenzt wenn weggelassen)
  -c, --count              Gibt nur die Token-Anzahl aus und beendet
  -j, --json               Ausgabe als JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Unterdrückt die Statistikzeile auf stderr
      --stats              Gibt Statistikzeile nach Inhalt auf stdout aus
  -h, --help               Hilfe anzeigen
  -V, --version            Version anzeigen
```

**Beispiele**

```bash
# Markdown-Datei konvertieren (Format automatisch per .md-Endung erkannt)
transpile --input doc.md

# Von stdin lesen — sauberes stdout, Statistiken auf stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Saubere Pipe — Statistiken vollständig unterdrücken
transpile --input doc.md --quiet | send_to_llm_api

# Token-Anzahl ohne Konvertierung prüfen
transpile --input doc.md --count

# JSON-Ausgabe für Skripte und Pipelines
transpile --input doc.md --json | jq '.reduction_pct'

# Inhalt + Statistiken in einem Stream erfassen
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — keine Komprimierung, vollständiger Inhalt erhalten (Rechts-/Prüfdokumente)
transpile --input contract.md --fidelity lossless

# Aggressive Komprimierung auf 512 Tokens
transpile --input article.md --fidelity compressed --budget 512
```

> Statistiken (`[273 → 150 tok  45.1% reduction]`) werden standardmäßig auf **stderr** geschrieben, damit stdout für Pipes sauber bleibt. Mit `--quiet` unterdrücken oder mit `--stats` auf stdout umleiten.

---

## Nutzungsstatistiken

Jeder `transpile`-Aufruf hängt automatisch einen Datensatz an `~/.agents/transpile/stats/YYYY-MM-DD.jsonl` an. Der Unterbefehl `transpile stats` liest diese Dateien und gibt eine Zusammenfassungstabelle aus.

```
transpile stats                # heute
transpile stats --days 7       # letzte N Tage
transpile stats --agent claude # nach Agent filtern
```

Beispielausgabe:

```
transpile stats — last 7 days

  Date        Agent       Calls   Input tok   Output tok   Saved    Reduction
  ──────────────────────────────────────────────────────────────────────────
  2026-04-13  claude          5      14 965       10 872   4 093      27.3%
  2026-04-13  gemini          2       4 800        3 500   1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────
  Total                       7      19 765       14 372   5 393      27.3%
```

**JSONL-Datensatzfelder**

| Feld | Typ | Beschreibung |
|------|-----|-------------|
| `ts` | ISO 8601 | Zeitstempel des Aufrufs |
| `agent` | String | Tool, das den Aufruf ausgelöst hat (`claude`, `gemini`, `codex`, `opencode`) |
| `file` | String | Pfad der Eingabedatei (leer bei stdin) |
| `format` | String | `markdown`, `html` oder `plaintext` |
| `fidelity` | String | `lossless`, `semantic` oder `compressed` |
| `input_tok` | Integer | Token-Anzahl vor dem Transpilieren |
| `output_tok` | Integer | Token-Anzahl nach dem Transpilieren |
| `reduction_pct` | Float | Prozentsatz der eingesparten Tokens |
| `saved` | Integer | Absolute eingesparte Tokens (`input_tok − output_tok`) |

**Umgebungsvariable `TRANSPILE_AGENT`**

Das Feld `agent` wird aus der Umgebungsvariable `TRANSPILE_AGENT` befüllt. Jede Integration setzt diese automatisch (`claude`, `gemini`, `codex`, `opencode`, `cursor`). Kann auch manuell gesetzt werden:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## Bibliotheksnutzung

### Synchron

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

### Token-Anzahl schätzen

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## Ausgabeformat

```
<D>                  ← Symbolwörterbuch (weggelassen ohne Ersetzungen)
{sym}=wiederholter-Begriff
</D>
<H>                  ← YAML-ähnlicher Metadaten-Header
t: Dokumenttitel
s: einzeilige Zusammenfassung
k: [Schlüsselwort1, Schlüsselwort2]
</H>
<B>                  ← Dokumentkörper (komprimiert + ersetzt)
...Inhalt...
</B>
```

Der `<D>`-Block verwendet Unicode-Private-Use-Area-Zeichen (`U+E000–U+F8FF`) als kompakte Symbolkenner und vermeidet Kollisionen mit sichtbaren Textmustern. Das Wörterbuch unterstützt bis zu **6.400 eindeutige Begriffe** pro Dokument.

---

## Treuestufen

| Stufe | Typischer Anwendungsfall | Angewendete Komprimierung |
|-------|------------------------|--------------------------|
| `Lossless` | Rechts-/Prüfdokumente | Keine — Originalinhalt garantiert |
| `Semantic` | Allgemeine RAG-Pipelines | Stoppwortentfernung + Beschneidung nach Wichtigkeit |
| `Compressed` | Zusammenfassung, knappe Budgets | Maximale Komprimierung, Extraktion des ersten Satzes |

---

## Adaptive Komprimierung

Der Kompressor überwacht die Budgetnutzung in Echtzeit und eskaliert automatisch:

| Budgetnutzung | Stufe | Was passiert |
|--------------|-------|-------------|
| 0–60% | `StopwordOnly` | Englische/koreanische Stoppwörter entfernt |
| 60–80% | `PruneLowImportance` | Untere 20% der Absätze nach Wichtigkeit entfernt |
| 80–95% | `DeduplicateAndLinearize` | Doppelte Sätze entfernt; Tabellen linearisiert |
| 95%+ | `MaxCompression` | Jeder Absatz auf den ersten Satz gekürzt |

> Der `Lossless`-Modus umgeht alle Kompressionsstufen bedingungslos.

Beim Streaming werden verbleibende Knoten automatisch in den `Compressed`-Modus umgeschaltet, wenn die Budgetnutzung 80% überschreitet.

---

## Eingabeformate

| `InputFormat` | Parser |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM-Tabellen |
| `Html` | ammonia-Bereinigung → Tag-Entfernung → Klartext-Pipeline |
| `PlainText` | Absatztrennung durch Leerzeilen |

---

## Fehlerbehandlung

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* Ausgabe verwenden */ }
    Err(TranspileError::Parse(msg))            => eprintln!("Parse-Fehler: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("zu viele eindeutige Begriffe: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("Komprimierung im Lossless-Modus"),
    Err(e)                                     => eprintln!("Fehler: {e}"),
}
```

---

## Leistung

Gemessen im Release-Build (`cargo build --release`), Apple M-Series, 48 Dokumente über Markdown/HTML/PlainText:

| Kennzahl | Gemessen | Hinweise |
|----------|----------|---------|
| Durchsatz | **10.975 tok/ms** | ≈75× schneller als Python-Parsing-Baseline |
| Semantic-Reduktion | **33,9%** (Markdown) | Ziel 15–30% erreicht |
| Compressed-Reduktion | **39,7%** (Markdown) | Budgetadaptiv, ≥ PruneLowImportance garantiert |
| Lossless-Wortabdeckung | **98,8% Durchschnitt** | Über alle Formate und Sprachen |
| HTML-Reduktion | **97,6%** | Entfernung von Nav-/Skript-/Style-Markup-Overhead |
| Mehrsprachige Unterstützung | 15 Sprachen getestet | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99,4% Wortabdeckung im Durchschnitt |

Die Evaluierungs-Suite selbst ausführen:

```bash
cargo run --release --example eval
```

---

## Mitwirken

Siehe [CONTRIBUTING.md](../../CONTRIBUTING.md) für vollständige Richtlinien. PRs willkommen — offene Issues mit dem Label `good first issue` beachten.

---

## Lizenz

Apache-2.0 — siehe [LICENSE](LICENSE).
