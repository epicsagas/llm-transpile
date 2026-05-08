# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Transpilador de documentos otimizado em tokens para pipelines LLM**

Documentos brutos (Markdown, HTML, texto puro) → formato ponte estruturado `<D>?<H><B>` — com compressão adaptativa para manter o orçamento de tokens.

```
<H>
t: Contrato de Licença de Software
s: Termos de licença anuais entre licenciante e licenciado
k: [licença, contrato, software]
</H>
<B>
# Partes Contratantes
Este acordo é celebrado entre o Licenciante e o Licenciado.
...
</B>
```

---

<details>
<summary>Índice</summary>
- [Por quê](#por-quê)
- [Instalação](#instalação)
- [Uso do CLI](#uso-do-cli)
- [Uso da biblioteca](#uso-da-biblioteca)
- [Formato de saída](#formato-de-saída)
- [Níveis de fidelidade](#níveis-de-fidelidade)
- [Compressão adaptativa](#compressão-adaptativa)
- [Formatos de entrada](#formatos-de-entrada)
- [Tratamento de erros](#tratamento-de-erros)
- [Desempenho](#desempenho)
- [Contribuir](#contribuir)
- [Licença](#licença)
</details>

---

## Por quê

LLMs funcionam melhor quando o contexto é limpo e denso. Esta biblioteca cuida do trabalho mecânico:

- **Análise estrutural** — Markdown/HTML/texto puro → nós IR tipados (cabeçalhos, parágrafos, tabelas, listas, blocos de código)
- **Compressão adaptativa** — escala automaticamente por 4 estágios à medida que o orçamento de tokens se esgota
- **Substituição de símbolos** — termos de domínio repetidos → caracteres Unicode PUA, decodificados pelo cabeçalho de dicionário `<D>`
- **Linearização de tabelas** — tabelas Markdown → sequências compactas `Key:Val` (≤5 linhas) ou linhas separadas por pipes para tabelas maiores
- **Saída em streaming** — o stream Tokio entrega o primeiro bloco imediatamente, minimizando o TTFT

---

## Instalação

### Biblioteca (crate Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requer **Rust 1.75+**.

### Binário CLI + integração de ferramentas

```bash
# Homebrew (macOS)
brew tap epicsagas/tap
brew install llm-transpile

# Binário pré-compilado (mais rápido, sem compilar)
cargo binstall llm-transpile

# Do crates.io
cargo install llm-transpile
```

Configurar integrações de ferramentas:

```bash
transpile install
```

`transpile install` inicia um assistente interativo que detecta e configura as ferramentas instaladas:

| Ferramenta | Método de integração | Função |
|------------|---------------------|--------|
| **Claude Code** | Hook PostToolUse | Auto-comprime arquivos `.md/.html/.txt` ao ler |
| **Gemini CLI** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |
| **Codex CLI** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |
| **Cursor** | Regra `.mdc` (`alwaysApply`) | Aciona `transpile` antes de ler arquivos de documento |
| **OpenCode** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |

**Instalação / desinstalação seletiva**

```bash
transpile install claude gemini    # ferramentas específicas apenas
transpile install --all            # tudo de uma vez
transpile install --dry-run        # visualizar o que mudaria
transpile install --list           # ver status das integrações

transpile uninstall cursor         # remover uma
transpile uninstall --all          # remover tudo
transpile uninstall --dry-run      # visualizar remoções
```

**Plugin do Claude Code**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Do código-fonte:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## Uso do CLI

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Caminho do arquivo de entrada (lê do stdin se omitido)
  -f, --format <FORMAT>    Formato de entrada: markdown | html | plaintext  [padrão: markdown]
                           Detectado automaticamente pela extensão com --input
  -l, --fidelity <LEVEL>  Nível de compressão: lossless | semantic | compressed  [padrão: semantic]
  -b, --budget <N>         Limite superior do orçamento de tokens (ilimitado se omitido)
  -c, --count              Imprime apenas a contagem de tokens de entrada e sai
  -j, --json               Saída em JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Suprime a linha de estatísticas no stderr
      --stats              Imprime a linha de estatísticas no stdout após o conteúdo
  -h, --help               Mostrar ajuda
  -V, --version            Mostrar versão
```

**Exemplos**

```bash
# Converter arquivo Markdown (formato detectado automaticamente pela extensão .md)
transpile --input doc.md

# Ler do stdin — stdout limpo, estatísticas no stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Pipe limpo — suprimir estatísticas completamente
transpile --input doc.md --quiet | send_to_llm_api

# Verificar contagem de tokens sem converter
transpile --input doc.md --count

# Saída JSON para scripts e pipelines
transpile --input doc.md --json | jq '.reduction_pct'

# Capturar conteúdo + estatísticas em um stream
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — sem compressão, conteúdo completo preservado (documentos legais/auditoria)
transpile --input contract.md --fidelity lossless

# Compressão agressiva em orçamento de 512 tokens
transpile --input article.md --fidelity compressed --budget 512
```

> Estatísticas (`[273 → 150 tok  45.1% reduction]`) são escritas no **stderr** por padrão, mantendo stdout limpo para pipes. Use `--quiet` para suprimir, ou `--stats` para redirecionar ao stdout.

---

## Uso da biblioteca

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

### Estimativa de contagem de tokens

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## Formato de saída

```
<D>                  ← Dicionário de símbolos (omitido sem substituições)
{sym}=termo-repetido
</D>
<H>                  ← Cabeçalho de metadados tipo YAML
t: título do documento
s: resumo em uma linha
k: [palavra-chave1, palavra-chave2]
</H>
<B>                  ← Corpo do documento (comprimido + substituído)
...conteúdo...
</B>
```

O bloco `<D>` usa caracteres da Área de Uso Privado Unicode (`U+E000–U+F8FF`) como identificadores de símbolo compactos, evitando colisões com padrões de texto visível. O dicionário suporta até **6.400 termos únicos** por documento.

---

## Níveis de fidelidade

| Nível | Caso de uso típico | Compressão aplicada |
|-------|-------------------|---------------------|
| `Lossless` | Documentos legais/auditoria | Nenhuma — conteúdo original garantido |
| `Semantic` | Pipelines RAG gerais | Remoção de stopwords + poda por importância |
| `Compressed` | Resumo, orçamentos apertados | Compressão máxima, extração da primeira frase |

---

## Compressão adaptativa

O compressor monitora o uso do orçamento em tempo real e escala automaticamente:

| Uso do orçamento | Estágio | O que acontece |
|-----------------|---------|----------------|
| 0–60% | `StopwordOnly` | Stopwords inglês/coreano removidas |
| 60–80% | `PruneLowImportance` | 20% inferiores dos parágrafos por importância removidos |
| 80–95% | `DeduplicateAndLinearize` | Frases duplicadas removidas; tabelas linearizadas |
| 95%+ | `MaxCompression` | Cada parágrafo truncado à primeira frase |

> O modo `Lossless` ignora incondicionalmente todos os estágios de compressão.

Durante o streaming, quando o uso do orçamento ultrapassa 80%, os nós restantes são automaticamente alternados para o modo `Compressed`.

---

## Formatos de entrada

| `InputFormat` | Analisador |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + tabelas GFM |
| `Html` | saneamento ammonia → remoção de tags → pipeline de texto puro |
| `PlainText` | Divisão de parágrafos por linha em branco |

---

## Tratamento de erros

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* usar output */ }
    Err(TranspileError::Parse(msg))            => eprintln!("falha de análise: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("termos únicos demais: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("compressão em modo lossless"),
    Err(e)                                     => eprintln!("erro: {e}"),
}
```

---

## Desempenho

Medido em build release (`cargo build --release`), Apple M-series, 48 documentos Markdown/HTML/PlainText:

| Métrica | Medido | Notas |
|---------|--------|-------|
| Throughput | **10.975 tok/ms** | ≈75× mais rápido que a linha de base Python |
| Redução Semantic | **33,9%** (Markdown) | Objetivo 15–30% atingido |
| Redução Compressed | **39,7%** (Markdown) | Adaptativo ao orçamento, ≥ PruneLowImportance garantido |
| Cobertura de palavras Lossless | **98,8% em média** | Em todos os formatos e idiomas |
| Redução HTML | **97,6%** | Remoção de overhead de marcação nav/scripts/estilos |
| Suporte multilíngue | 15 idiomas testados | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99,4% cobertura média |

Execute a suite de avaliação você mesmo:

```bash
cargo run --release --example eval
```

---

## Contribuir

Relatórios de bugs, solicitações de recursos e pull requests são bem-vindos.

```bash
# Clonar e compilar
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# Executar testes
cargo test

# Executar benchmarks (relatório HTML → target/criterion/)
cargo bench

# Lint e formatação
cargo clippy -- -D warnings
cargo fmt
```

**Diretrizes**

- Manter MSRV no Rust 1.75 — evitar recursos introduzidos depois disso.
- Novos comportamentos de compressão não devem afetar o modo `Lossless`.
- Cada PR deve incluir testes para qualquer nova lógica no módulo relevante (`ir`, `compressor`, `symbol`, `renderer`).
- Executar `cargo clippy -- -D warnings` e `cargo fmt` antes de enviar.

---

## Licença

Apache-2.0 — veja [LICENSE](LICENSE).
