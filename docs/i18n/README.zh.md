# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**面向 LLM 流水线的令牌优化文档转译器**

原始文档（Markdown、HTML、纯文本）→ 结构化桥接格式 `<D>?<H><B>` — 支持自适应压缩以控制令牌预算。

```
<H>
t: 软件许可协议
s: 许可方与被许可方之间的年度许可条款
k: [许可证, 合同, 软件]
</H>
<B>
# 合同当事方
本协议由许可方与被许可方签订。
...
</B>
```

---

<details>
<summary>目录</summary>

- [为什么使用](#为什么使用)
- [安装](#安装)
- [更新](#更新)
- [CLI 用法](#cli-用法)
- [使用统计](#使用统计)
- [库用法](#库用法)
- [输出格式](#输出格式)
- [保真度级别](#保真度级别)
- [自适应压缩](#自适应压缩)
- [输入格式](#输入格式)
- [错误处理](#错误处理)
- [性能](#性能)
- [贡献](#贡献)
- [许可证](#许可证)
</details>

---

## 为什么使用

当上下文简洁且密度高时，LLM 表现更好。本库处理机械性工作：

| | 特性 | 重要性 |
|--|------|--------|
| 🏗️ | **结构化解析** | Markdown/HTML/纯文本 → 类型化 IR 节点（标题、段落、表格、列表、代码块） |
| 📉 | **自适应压缩** | 随着令牌预算消耗，自动升级压缩阶段（共 4 个阶段） |
| 🔣 | **符号替换** | 重复的领域术语 → Unicode PUA 字符，通过 `<D>` 字典头恢复 |
| 📊 | **表格线性化** | Markdown 表格 → 紧凑的 `Key:Val` 序列（≤5 行）或管道分隔行 |
| 🌊 | **流式输出** | Tokio 流立即交付第一个块，最小化 TTFT |

---

## 安装

### 库（Rust crate）

```toml
[dependencies]
llm-transpile = "0.1"
```

需要 **Rust 1.92+**。

### CLI 二进制文件 + 工具集成

**macOS / Linux**

```bash
brew install epicsagas/tap/llm-transpile
```

没有 Homebrew？使用安装脚本：

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

**Windows**

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

**通过 Rust 工具链**

```bash
cargo binstall llm-transpile   # 预构建二进制文件（更快）
cargo install llm-transpile    # 从源码构建
```

然后配置工具集成：

```bash
transpile install
```

`transpile install` 启动一个交互式向导，检测并配置已安装的工具：

| 工具 | 集成方式 | 功能 |
|------|---------|------|
| **Claude Code** | PostToolUse 钩子 | Read 时自动压缩 `.md/.html/.txt` 文件 |
| **Gemini CLI** | `SKILL.md` | LLM 自动对文档扩展名调用 `transpile` |
| **Codex CLI** | `SKILL.md` | LLM 自动对文档扩展名调用 `transpile` |
| **Cursor** | `.mdc` 规则（`alwaysApply`） | 读取文档文件前触发 `transpile` |
| **OpenCode** | `SKILL.md` | LLM 自动对文档扩展名调用 `transpile` |

所有非 Claude 工具均使用技能文件，教导 LLM 自动运行 `TRANSPILE_AGENT=<agent> transpile --input <file>` — 无需大小检查，仅凭扩展名即可触发。

**选择性安装 / 卸载**

```bash
transpile install claude gemini    # 仅特定工具
transpile install --all            # 全部安装
transpile install --dry-run        # 预览变更
transpile install --list           # 查看集成状态

transpile uninstall cursor         # 移除一个
transpile uninstall --all          # 移除全部
transpile uninstall --dry-run      # 预览移除
```

**Claude Code 插件**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

下次会话启动时自动安装二进制文件并配置 PostToolUse 钩子 — 无需额外设置。

从源码安装：

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## 更新

| 方式 | 命令 |
|------|------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShell 安装脚本 | 重新运行上方的安装命令 |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## CLI 用法

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       输入文件路径（省略时从 stdin 读取）
  -f, --format <FORMAT>    输入格式: markdown | html | plaintext  [默认: markdown]
                           使用 --input 时从文件扩展名自动检测
  -l, --fidelity <LEVEL>  压缩级别: lossless | semantic | compressed  [默认: semantic]
  -b, --budget <N>         令牌预算上限（省略时无限制）
  -c, --count              仅打印输入令牌数后退出
  -j, --json               以 JSON 格式输出 {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              抑制 stderr 统计行
      --stats              在内容后将统计行打印到 stdout
  -h, --help               打印帮助
  -V, --version            打印版本
```

**示例**

```bash
# 转换 Markdown 文件（从 .md 扩展名自动检测格式）
transpile --input doc.md

# 从 stdin 读取 — 干净的 stdout，统计输出到 stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# 管道连接 — 完全抑制统计
transpile --input doc.md --quiet | send_to_llm_api

# 不转换仅检查令牌数
transpile --input doc.md --count

# 脚本/流水线的 JSON 输出
transpile --input doc.md --json | jq '.reduction_pct'

# 在一个流中捕获内容 + 统计
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — 无压缩，完整保留内容（法律/审计文档）
transpile --input contract.md --fidelity lossless

# 压缩到 512 令牌预算
transpile --input article.md --fidelity compressed --budget 512
```

> 统计（`[273 → 150 tok  45.1% reduction]`）默认写入 **stderr**，保持 stdout 干净以便管道使用。使用 `--quiet` 抑制，或 `--stats` 重定向到 stdout。

---

## 使用统计

每次 `transpile` 调用都会自动追加一条记录到 `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`。`transpile stats` 子命令读取这些文件并打印汇总表。

```
transpile stats                # 今天
transpile stats --days 7       # 最近 N 天
transpile stats --agent claude # 按代理筛选
```

示例输出：

```
transpile stats — 最近 7 天

  日期       代理       调用次数   输入令牌   输出令牌   节省    缩减率
  ──────────────────────────────────────────────────────────────────────────
  2026-04-13  claude          5      14 965       10 872   4 093      27.3%
  2026-04-13  gemini          2       4 800        3 500   1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────
  合计                       7      19 765       14 372   5 393      27.3%
```

**JSONL 记录字段**

| 字段 | 类型 | 说明 |
|------|------|------|
| `ts` | ISO 8601 | 调用时间戳 |
| `agent` | 字符串 | 触发调用的工具（`claude`、`gemini`、`codex`、`opencode`） |
| `file` | 字符串 | 输入文件路径（从 stdin 读取时为空） |
| `format` | 字符串 | `markdown`、`html` 或 `plaintext` |
| `fidelity` | 字符串 | `lossless`、`semantic` 或 `compressed` |
| `input_tok` | 整数 | 转译前令牌数 |
| `output_tok` | 整数 | 转译后令牌数 |
| `reduction_pct` | 浮点数 | 令牌节省百分比 |
| `saved` | 整数 | 节省的令牌绝对值（`input_tok − output_tok`） |

**`TRANSPILE_AGENT` 环境变量**

`agent` 字段从 `TRANSPILE_AGENT` 环境变量中获取。每个集成会自动设置此变量（`claude`、`gemini`、`codex`、`opencode`、`cursor`）。您也可以手动设置：

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## 库用法

### 同步

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

### 流式（Tokio）

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

### 令牌数估算

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## 输出格式

```
<D>                  ← 符号字典（无替换时省略）
{sym}=重复术语
</D>
<H>                  ← 类 YAML 元数据头
t: 文档标题
s: 单行摘要
k: [关键词1, 关键词2]
</H>
<B>                  ← 文档正文（已压缩 + 已替换）
...内容...
</B>
```

`<D>` 块使用 Unicode 私用区字符（`U+E000–U+F8FF`）作为紧凑符号句柄，避免与可见文本模式冲突。字典每文档最多支持 **6,400 个**唯一术语。

---

## 保真度级别

| 级别 | 典型使用场景 | 应用的压缩 |
|------|------------|-----------|
| `Lossless` | 法律/审计文档 | 无 — 保证原始内容 |
| `Semantic` | 通用 RAG 流水线 | 停用词移除 + 低重要性修剪 |
| `Compressed` | 摘要生成，严格预算 | 最大压缩，首句提取 |

---

## 自适应压缩

压缩器实时监控预算使用情况并自动升级：

| 预算使用率 | 阶段 | 操作 |
|-----------|------|------|
| 0–60% | `StopwordOnly` | 移除英语/韩语停用词 |
| 60–80% | `PruneLowImportance` | 移除重要性最低的 20% 段落 |
| 80–95% | `DeduplicateAndLinearize` | 移除重复句子；表格线性化 |
| 95%+ | `MaxCompression` | 每段落截断为首句 |

> `Lossless` 模式无条件跳过所有压缩阶段。

流式处理时，当预算使用率超过 80%，剩余节点自动切换为 `Compressed` 模式。

---

## 输入格式

| `InputFormat` | 解析器 |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM 表格 |
| `Html` | ammonia 净化 → 标签剥离 → 纯文本流水线 |
| `PlainText` | 空行段落分割 |

---

## 错误处理

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* 使用输出 */ }
    Err(TranspileError::Parse(msg))            => eprintln!("解析失败: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("唯一术语过多: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("Lossless 模式下的压缩"),
    Err(e)                                     => eprintln!("错误: {e}"),
}
```

---

## 性能

发布构建（`cargo build --release`），Apple M 系列，48 个 Markdown/HTML/PlainText 文档测量：

| 指标 | 测量值 | 备注 |
|------|--------|------|
| 吞吐量 | **10,975 tok/ms** | 约为 Python 解析基线的 75 倍 |
| Semantic 缩减率 | **33.9%**（Markdown） | 达成 15–30% 目标 |
| Compressed 缩减率 | **39.7%**（Markdown） | 预算自适应，保证 ≥ PruneLowImportance |
| Lossless 词汇覆盖率 | **98.8% 平均** | 覆盖所有格式和语言 |
| HTML 缩减率 | **97.6%** | 移除导航/脚本/样式标记开销 |
| 多语言支持 | 15 种语言已测试 | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 平均 99.4% 词汇覆盖率 |

自行运行评估套件：

```bash
cargo run --release --example eval
```

---

## 贡献

参见 [CONTRIBUTING.md](../../CONTRIBUTING.md) 了解完整指南。欢迎提交 PR — 请查看标记为 `good first issue` 的开放议题。

---

## 许可证

Apache-2.0 — 参见 [LICENSE](LICENSE)。
