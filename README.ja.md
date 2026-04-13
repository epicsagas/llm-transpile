# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**LLMパイプライン向けトークン最適化ドキュメントトランスパイラー**

生ドキュメント（Markdown、HTML、プレーンテキスト）→ 構造化ブリッジフォーマット `<D>?<H><B>` — トークン予算内に収める適応型圧縮付き。

```
<H>
t: ソフトウェアライセンス契約
s: ライセンサーとライセンシー間の年間ライセンス条件
k: [ライセンス, 契約, ソフトウェア]
</H>
<B>
# 契約当事者
本契約はライセンサーとライセンシーの間で締結されます。
...
</B>
```

---

## 目次

- [なぜ使うのか](#なぜ使うのか)
- [インストール](#インストール)
- [CLI使用法](#cli使用法)
- [ライブラリ使用法](#ライブラリ使用法)
- [出力フォーマット](#出力フォーマット)
- [忠実度レベル](#忠実度レベル)
- [適応型圧縮](#適応型圧縮)
- [入力フォーマット](#入力フォーマット)
- [エラー処理](#エラー処理)
- [パフォーマンス](#パフォーマンス)
- [コントリビュート](#コントリビュート)
- [ライセンス](#ライセンス)

---

## なぜ使うのか

LLMはコンテキストがクリーンで密度が高いほど性能が向上します。このライブラリが機械的な作業を担当します:

- **構造的パース** — Markdown/HTML/プレーンテキスト → 型付きIRノード（見出し、段落、表、リスト、コードブロック）
- **適応型圧縮** — トークン予算が埋まるにつれて4段階を自動的にエスカレーション
- **シンボル置換** — 繰り返されるドメイン用語 → Unicode PUA文字、`<D>`辞書ヘッダーで復元
- **表の線形化** — Markdown表 → コンパクトな`Key:Val`シーケンス（≤5行）または大きな表はパイプ区切り行
- **ストリーミング出力** — TokioストリームがTTFTを最小化するために最初のチャンクを即座に配信

---

## インストール

### ライブラリ（Rustクレート）

```toml
[dependencies]
llm-transpile = "0.1"
```

**Rust 1.75+** が必要。

### CLIバイナリ + ツール連携

```bash
# Homebrew (macOS)
brew install epicsagas/tap/llm-transpile

# ビルド済みバイナリ（コンパイル不要で高速）
cargo binstall llm-transpile

# crates.ioからインストール
cargo install llm-transpile
```

ツール連携の設定:

```bash
transpile install
```

`transpile install` はインストール済みツールを検出して設定する対話型ウィザードを起動します:

| ツール | 連携方法 | 動作 |
|--------|---------|------|
| **Claude Code** | PostToolUseフック | Read時に`.md/.html/.txt`ファイルを自動圧縮 |
| **Gemini CLI** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |
| **Codex CLI** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |
| **Cursor** | `.mdc`ルール（`alwaysApply`） | ドキュメントファイル読み込み前に`transpile`を実行 |
| **OpenCode** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |

**選択的インストール / アンインストール**

```bash
transpile install claude gemini    # 特定ツールのみ
transpile install --all            # すべてインストール
transpile install --dry-run        # 変更のプレビュー
transpile install --list           # 連携状態の確認

transpile uninstall cursor         # 一つ削除
transpile uninstall --all          # すべて削除
transpile uninstall --dry-run      # 削除のプレビュー
```

**Claude Codeプラグイン**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

ソースからインストール:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## CLI使用法

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       入力ファイルパス（省略時はstdinから読み込み）
  -f, --format <FORMAT>    入力フォーマット: markdown | html | plaintext  [デフォルト: markdown]
                           --input使用時はファイル拡張子から自動検出
  -l, --fidelity <LEVEL>  圧縮レベル: lossless | semantic | compressed  [デフォルト: semantic]
  -b, --budget <N>         トークン予算の上限（省略時は無制限）
  -c, --count              入力トークン数のみ出力して終了
  -j, --json               JSON形式で出力 {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              stderrの統計行を非表示
      --stats              コンテンツの後に統計をstdoutに出力
  -h, --help               ヘルプを表示
  -V, --version            バージョンを表示
```

**例**

```bash
# Markdownファイルを変換（.md拡張子からフォーマット自動検出）
transpile --input doc.md

# stdinから読み込み — クリーンなstdout、統計はstderrへ
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# パイプ接続 — 統計を完全に非表示
transpile --input doc.md --quiet | send_to_llm_api

# 変換なしでトークン数を確認
transpile --input doc.md --count

# スクリプト/パイプライン向けJSON出力
transpile --input doc.md --json | jq '.reduction_pct'

# コンテンツ + 統計を一つのストリームでキャプチャ
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — 圧縮なし、完全なコンテンツを保持（法律/監査文書）
transpile --input contract.md --fidelity lossless

# 512トークン予算への積極的な圧縮
transpile --input article.md --fidelity compressed --budget 512
```

> 統計（`[273 → 150 tok  45.1% reduction]`）はデフォルトで**stderr**に出力されるため、stdoutはパイプ用にクリーンな状態を保ちます。`--quiet`で非表示、`--stats`でstdoutに出力できます。

---

## ライブラリ使用法

### 同期処理

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

### ストリーミング（Tokio）

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

### トークン数の推定

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## 出力フォーマット

```
<D>                  ← シンボル辞書（置換がない場合は省略）
{sym}=繰り返し用語
</D>
<H>                  ← YAMLライクなメタデータヘッダー
t: ドキュメントタイトル
s: 一行サマリー
k: [キーワード1, キーワード2]
</H>
<B>                  ← ドキュメント本文（圧縮 + 置換適用済み）
...コンテンツ...
</B>
```

`<D>`ブロックはUnicode私用領域文字（`U+E000–U+F8FF`）をシンボルハンドルとして使用し、可視テキストパターンとの衝突を回避します。辞書はドキュメントあたり最大**6,400個**の固有用語をサポートします。

---

## 忠実度レベル

| レベル | 典型的な使用ケース | 適用される圧縮 |
|--------|-------------------|---------------|
| `Lossless` | 法律/監査文書 | なし — 元のコンテンツを保証 |
| `Semantic` | 汎用RAGパイプライン | ストップワード削除 + 低重要度の刈り込み |
| `Compressed` | 要約、厳しい予算 | 最大圧縮、最初の文を抽出 |

---

## 適応型圧縮

コンプレッサーはリアルタイムで予算使用量を監視し、自動的にエスカレーションします:

| 予算使用量 | ステージ | 動作 |
|-----------|---------|------|
| 0–60% | `StopwordOnly` | 英語/韓国語ストップワードを除去 |
| 60–80% | `PruneLowImportance` | 重要度の低い下位20%の段落を削除 |
| 80–95% | `DeduplicateAndLinearize` | 重複文を削除; 表を線形化 |
| 95%+ | `MaxCompression` | 各段落を最初の文に短縮 |

> `Lossless`モードはすべての圧縮ステージを無条件でバイパスします。

ストリーミング中、予算使用量が80%を超えると、残りのノードは自動的に`Compressed`モードに切り替わります。

---

## 入力フォーマット

| `InputFormat` | パーサー |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFMテーブル |
| `Html` | ammoniaサニタイズ → タグ除去 → プレーンテキストパイプライン |
| `PlainText` | 空行による段落分割 |

---

## エラー処理

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* 出力を使用 */ }
    Err(TranspileError::Parse(msg))            => eprintln!("パース失敗: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("固有用語が多すぎる: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("Losslessモードでの圧縮"),
    Err(e)                                     => eprintln!("エラー: {e}"),
}
```

---

## パフォーマンス

リリースビルド（`cargo build --release`）、Apple Mシリーズ、Markdown/HTML/PlainText 48ドキュメント測定:

| 指標 | 測定値 | 備考 |
|------|--------|------|
| スループット | **10,975 tok/ms** | Pythonパーシングベースラインの≈75倍高速 |
| Semantic削減率 | **33.9%**（Markdown） | 15–30%目標達成 |
| Compressed削減率 | **39.7%**（Markdown） | 予算適応型、PruneLowImportance以上保証 |
| Lossless単語カバレッジ | **98.8% 平均** | 全フォーマット・言語 |
| HTML削減率 | **97.6%** | ナビ/スクリプト/スタイルのマークアップオーバーヘッド除去 |
| 多言語サポート | 15言語テスト済み | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 平均99.4%単語カバレッジ |

評価スイートを自分で実行:

```bash
cargo run --release --example eval
```

---

## コントリビュート

バグレポート、機能リクエスト、プルリクエストを歓迎します。

```bash
# クローンとビルド
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# テスト実行
cargo test

# ベンチマーク実行（HTMLレポート → target/criterion/）
cargo bench

# リントとフォーマット
cargo clippy -- -D warnings
cargo fmt
```

**ガイドライン**

- MSRVをRust 1.75に維持 — それ以降に導入された機能は使用しないこと。
- 新しい圧縮動作は`Lossless`モードに影響を与えてはなりません。
- 各PRには関連モジュール（`ir`、`compressor`、`symbol`、`renderer`）の新しいロジックのテストを含めること。
- 提出前に`cargo clippy -- -D warnings`と`cargo fmt`を実行すること。

---

## ライセンス

Apache-2.0 — [LICENSE](LICENSE)を参照。
