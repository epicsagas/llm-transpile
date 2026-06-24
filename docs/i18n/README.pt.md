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

**Transpilador de documentos otimizado para tokens em pipelines de LLM**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

Documentos brutos (Markdown, HTML, texto puro) → formato ponte estruturado `<D>?<H><B>` — com compressão adaptativa para manter o orçamento de tokens.

---

<details>
<summary>Índice</summary>

- [Por quê](#por-quê)
- [Instalação](#instalação)
- [Atualização](#atualização)
- [Uso do CLI](#uso-do-cli)
- [Estatísticas de uso](#estatísticas-de-uso)
- [Uso da biblioteca](#uso-da-biblioteca)
- [Formato de saída](#formato-de-saída)
- [Níveis de fidelidade](#níveis-de-fidelidade)
- [Compressão adaptativa](#compressão-adaptativa)
- [Formatos de entrada](#formatos-de-entrada)
- [Tratamento de erros](#tratamento-de-erros)
- [Desempenho](#desempenho)
- [Contribuir](#contribuir)
- [Licença](#licença)
- [Benchmarking](#benchmarking)

</details>

---

## Por quê

LLMs funcionam melhor quando o contexto é limpo e denso. Esta biblioteca cuida do trabalho mecânico:

| | Recurso | Por que importa |
|--|---------|----------------|
| 🏗️ | **Análise estrutural** | Markdown/HTML/texto puro → nós IR tipados (cabeçalhos, parágrafos, tabelas, listas, blocos de código) |
| 📉 | **Compressão adaptativa** | Escala automaticamente por 4 estágios à medida que o orçamento de tokens se esgota |
| 🔣 | **Substituição de símbolos** | Termos de domínio repetidos → caracteres Unicode PUA, decodificados pelo cabeçalho de dicionário `<D>` |
| 📊 | **Linearização de tabelas** | Tabelas Markdown → sequências compactas `Key:Val` (≤5 linhas) ou linhas separadas por pipes para tabelas maiores |
| 🌊 | **Saída em streaming** | O stream Tokio entrega o primeiro bloco imediatamente, minimizando o TTFT |

### Benchmarks

48 documentos, 3 formatos, 15 idiomas — Apple M-series, build `--release`. Os números abaixo são medidos com o **tokenizador BPE `cl100k` real** (não a heurística autorreferencial — veja a análise). Metodologia completa e detalhamento de honestidade de tokens: [`docs/EVALUATION.md`](../EVALUATION.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown | 27.4% | 69.4% | 99.0% | — |
| HTML | 98.7% | 99.3% | 99.0% | — |
| PlainText | -3.5% | 30.4% | 99.0% | — |
| **Overall (BPE)** | **81.5%** | **91.8%** | **99.0%** | **~1,070 tok/ms** |

> ⚠️ A figura geral é dominada pela remoção de marcação HTML. **27.4% no Markdown é a taxa de compressão genuína.** O PlainText é líquido negativo no modo Semantic devido ao overhead estrutural. Veja [`docs/EVALUATION.md`](../EVALUATION.md) para a realidade por formato.
> A redução HTML reflete a remoção do overhead de marcação (nav, scripts, estilos), não apenas a compressão do texto.

---

## Instalação

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Instala automaticamente o binário e configura o hook PostToolUse no próximo início de sessão — nenhuma configuração adicional necessária.

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

O hook PostToolUse é registrado automaticamente — nenhuma etapa adicional necessária.

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

Sem Homebrew? Use o script instalador:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### Via toolchain Rust

```bash
cargo binstall llm-transpile   # binário pré-compilado (rápido)
cargo install llm-transpile    # compilar do código-fonte
```

### Após instalar

Configurar integrações de ferramentas:

```bash
transpile install
```

`transpile install` inicia um assistente interativo que detecta e configura as ferramentas instaladas:

| Ferramenta | Método de integração | Função |
|------------|---------------------|--------|
| **Antigravity** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |
| **Cursor** | Regra `.mdc` (`alwaysApply`) | Aciona `transpile` antes de ler arquivos de documento |
| **OpenCode** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |
| **Cline** | `SKILL.md` | LLM invoca automaticamente `transpile` em extensões de arquivo |

Todas as ferramentas usam um arquivo de skill que instrui o LLM a executar `TRANSPILE_AGENT=<agent> transpile --input <file>` automaticamente — nenhuma verificação de tamanho necessária, a extensão por si só já aciona.

**Instalação / desinstalação seletiva**

```bash
transpile install antigravity cursor    # ferramentas específicas apenas
transpile install --all            # tudo de uma vez
transpile install --dry-run        # visualizar o que mudaria
transpile install --list           # ver status das integrações

transpile uninstall cursor         # remover uma
transpile uninstall --all          # remover tudo
transpile uninstall --dry-run      # visualizar remoções
```

### Biblioteca (crate Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requer **Rust 1.92+**.

### Antigravity (Gemini CLI)

```bash
agy plugins install https://github.com/epicsagas/llm-transpile
```

Instala automaticamente o plugin (hooks) e o registra na próxima inicialização de sessão.


### Benchmarking


```bash
# Executar benchmarks em um diretório de arquivos de teste
transpile bench run --dataset ./eval                    # gera log JSONL
transpile bench run --dataset ./eval --report           # executa + abre o relatório HTML
transpile bench report                                  # regenerar relatório a partir dos logs
```

O relatório HTML de benchmark inclui:

- **Cartões KPI** — redução semântica, redução comprimida, taxa de transferência (tok/ms), cobertura de palavras, total de tokens de entrada, contagem de execuções
- **7 gráficos** — tendência de redução, taxa por execução, dispersão semântica vs taxa, box plot por formato, distribuição de formatos, histograma de tamanho de token, donut de cobertura de palavras
- **Tabela de execuções** — resumo por execução com métricas agregadas
- **Tabela de registros** — detalhes por arquivo com filtro de formato, execução e nome
- **Tema** — modo escuro/claro com preferência persistente
- **Bilíngue** — detecta automaticamente a localidade coreana; alternância manual KO/EN


---

---

## Atualização

| Método | Comando |
|--------|---------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / instalador PowerShell | Executar o comando de instalação novamente |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
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

## Estatísticas de uso

Cada invocação de `transpile` adiciona automaticamente um registro ao `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`. O subcomando `transpile stats` lê esses arquivos e exibe uma tabela resumida.

```
transpile stats show                # hoje
transpile stats show --days 7       # últimos N dias
transpile stats show --agent claude # filtrar por agente
```

Exemplo de saída:

```
transpile stats — last 7 days

  Date        Agent       Calls   Input tok   Output tok   Saved    Reduction
  ──────────────────────────────────────────────────────────────────────────
  2026-04-13  claude          5      14 965       10 872   4 093      27.3%
  2026-04-13  antigravity          2       4 800        3 500   1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────
  Total                       7      19 765       14 372   5 393      27.3%
```

**Campos do registro JSONL**

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `ts` | ISO 8601 | Timestamp da invocação |
| `agent` | string | Ferramenta que acionou a chamada (`claude`, `antigravity`, `codex`, `opencode`) |
| `file` | string | Caminho do arquivo de entrada (vazio ao ler do stdin) |
| `format` | string | `markdown`, `html` ou `plaintext` |
| `fidelity` | string | `lossless`, `semantic` ou `compressed` |
| `input_tok` | integer | Contagem de tokens antes da transpilação |
| `output_tok` | integer | Contagem de tokens após a transpilação |
| `reduction_pct` | float | Percentual de tokens economizados |
| `saved` | integer | Tokens economizados absolutos (`input_tok − output_tok`) |

**Variável de ambiente `TRANSPILE_AGENT`**

O campo `agent` é preenchido pela variável de ambiente `TRANSPILE_AGENT`. Cada integração define isso automaticamente (`claude`, `antigravity`, `codex`, `opencode`, `cursor`). Também pode ser definido manualmente:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## Uso da biblioteca

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

### Estimativa de contagem de tokens

```rust
let n = llm_transpiler::token_count("Hello, world!");
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
use llm_transpiler::TranspileError;

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

Medido em build release (`cargo build --release`), Apple M-series, 48 documentos Markdown/HTML/PlainText. Todas as figuras de redução são medidas com o **tokenizador BPE `cl100k` real** (não a heurística autorreferencial). Veja [`docs/EVALUATION.md`](../EVALUATION.md):

| Métrica | Medido | Notas |
|---------|--------|-------|
| Throughput (pico Markdown apenas) | **10.975 tok/ms** | ≈75× mais rápido que a linha de base de análise Python; pico de formato único |
| Throughput (agregado do conjunto de dados) | **~1.070 tok/ms** | Ponderado em todos os 48 documentos / 3 formatos (BPE) — veja a tabela de Benchmarks |
| Redução Semantic | **27.4%** (Markdown) | Taxa de compressão genuína; dentro da faixa-alvo de 15–30% |
| Redução Compressed | **69.4%** (Markdown) | Adaptativo ao orçamento, ≥ PruneLowImportance garantido |
| Cobertura de palavras Lossless | **99.0% em média** | Em todos os formatos e idiomas |
| Redução HTML | **98.7%** | Reflete a remoção do overhead de marcação (nav/scripts/estilos) |
| Suporte multilíngue | 15 idiomas testados | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.0% cobertura média de palavras |

Execute a suite de avaliação você mesmo:

```bash
make eval          # JSON estruturado (BPE + heurística; consumido por `epic eval`)
make eval-report   # tabela por arquivo legível por humanos + resumo
```

Detalhamento por arquivo, metodologia e limitações conhecidas: [`docs/EVALUATION.md`](../EVALUATION.md)

---

## Contribuir

Veja [CONTRIBUTING.md](../../CONTRIBUTING.md) para as diretrizes completas. PRs são bem-vindos — confira as issues abertas com o rótulo `good first issue`.

---

## Licença

Apache-2.0 — veja [LICENSE](../../LICENSE).
