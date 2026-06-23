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

**LLMパイプライン向けトークン最適化ドキュメントトランスパイラー**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

生ドキュメント（Markdown、HTML、プレーンテキスト）→ 構造化ブリッジフォーマット `<D>?<H><B>` — トークン予算内に収める適応型圧縮付き。

---

<details>
<summary>目次</summary>
- [なぜ使うのか](#なぜ使うのか)
- [インストール](#インストール)
- [アップデート](#アップデート)
- [CLI使用法](#cli使用法)
- [使用統計](#使用統計)
- [ライブラリ使用法](#ライブラリ使用法)
- [出力フォーマット](#出力フォーマット)
- [忠実度レベル](#忠実度レベル)
- [適応型圧縮](#適応型圧縮)
- [入力フォーマット](#入力フォーマット)
- [エラー処理](#エラー処理)
- [パフォーマンス](#パフォーマンス)
- [コントリビュート](#コントリビュート)
- [ライセンス](#ライセンス)- [ベンチマーキング](#ベンチマーキング)

</details>

---

## なぜ使うのか

LLMはコンテキストがクリーンで密度が高いほど性能が向上します。このライブラリが機械的な作業を担当します:

| | 機能 | なぜ重要か |
|--|------|-----------|
| 🏗️ | **構造的パース** | Markdown/HTML/プレーンテキスト → 型付きIRノード（見出し、段落、表、リスト、コードブロック） |
| 📉 | **適応型圧縮** | トークン予算が埋まるにつれて4段階を自動的にエスカレーション |
| 🔣 | **シンボル置換** | 繰り返されるドメイン用語 → Unicode PUA文字、`<D>`辞書ヘッダーで復元 |
| 📊 | **表の線形化** | Markdown表 → コンパクトな`Key:Val`（≤5行）または大きな表はパイプ区切り行 |
| 🌊 | **ストリーミング出力** | TokioストリームがTTFTを最小化するために最初のチャンクを即座に配信 |

### ベンチマーク

48ドキュメント、3フォーマット、15言語 — Apple Mシリーズ、`--release`ビルド。以下の数値は**実際の`cl100k` BPEトークナイザー**で測定したものです（自己言及ヒューリスティックではありません — 分析を参照）。完全な手法とトークンハネスト性の内訳: [`docs/EVALUATION.md`](../EVALUATION.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown | 27.4% | 69.4% | 99.0% | — |
| HTML | 98.7% | 99.3% | 99.0% | — |
| PlainText | −3.5% | 30.4% | 99.0% | — |
| **Overall (BPE)** | **81.5%** | **91.8%** | **99.0%** | **~1,070 tok/ms** |

> ⚠️ 全体の数値はHTMLマークアップ除去が大半を占めています。**Markdownの27.4%が真の圧縮率です。** PlainTextは構造的オーバーヘッドのためSemanticモードではネットマイナスになります。フォーマットごとの実態は[`docs/EVALUATION.md`](../EVALUATION.md)を参照してください。

> HTML削減率はナビ/スクリプト/スタイルのマークアップオーバーヘッド除去を反映しており、本文の圧縮のみを示すものではありません。

---

## インストール

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

次回セッション開始時にバイナリを自動インストールし、PostToolUseフックを設定します — 追加のセットアップは不要です。

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

PostToolUseフックが自動的に登録されます — 追加の手順は不要です。

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

Homebrewがない場合、インストールスクリプトを使用してください:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### Rustツールチェーン

```bash
cargo binstall llm-transpile   # ビルド済みバイナリ（高速）
cargo install llm-transpile    # ソースからビルド
```

### インストール後

ツール連携の設定:

```bash
transpile install
```

`transpile install` はインストール済みツールを検出して設定する対話型ウィザードを起動します:

| ツール | 連携方法 | 動作 |
|--------|---------|------|
| **Antigravity** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |
| **Cursor** | `.mdc`ルール（`alwaysApply`） | ドキュメントファイル読み込み前に`transpile`を実行 |
| **OpenCode** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |
| **Cline** | `SKILL.md` | LLMがドキュメント拡張子で`transpile`を自動実行 |

すべてのツールは、LLMが`TRANSPILE_AGENT=<agent> transpile --input <file>`を自動的に実行するようにガイドするスキルファイルを使用します。サイズチェックは不要で、拡張子だけでトリガーされます。

**選択的インストール / アンインストール**

```bash
transpile install antigravity cursor    # 特定ツールのみ
transpile install --all            # すべてインストール
transpile install --dry-run        # 変更のプレビュー
transpile install --list           # 連携状態の確認

transpile uninstall cursor         # 一つ削除
transpile uninstall --all          # すべて削除
transpile uninstall --dry-run      # 削除のプレビュー
```

### ライブラリ（Rustクレート）

```toml
[dependencies]
llm-transpile = "0.1"
```

**Rust 1.92+** が必要。

---

## アップデート

| 方法 | コマンド |
|------|----------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShellインストーラー | 上記のインストールコマンドを再実行 |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
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

## 使用統計

`transpile`の呼び出しごとに、`~/.agents/transpile/stats/YYYY-MM-DD.jsonl`にレコードが自動的に追記されます。`transpile stats`サブコマンドはこれらのファイルを読み取り、サマリーテーブルを表示します。

```
transpile stats show                # 今日
transpile stats show --days 7       # 過去N日間
transpile stats show --agent claude # エージェントでフィルター
```

出力例:

```
transpile stats — 過去7日間

  日付        エージェント  呼び出し  入力tok    出力tok    削減    削減率
  ──────────────────────────────────────────────────────────────────────────
  2026-04-13  claude          5      14 965       10 872   4 093      27.3%
  2026-04-13  antigravity          2       4 800        3 500   1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────
  合計                         7      19 765       14 372   5 393      27.3%
```

**インタラクティブな HTML ダッシュボード**


```bash
transpile stats report                 # ブラウザで開く（デフォルト: 過去7日間）
transpile stats report --days 30       # 過去30日間
transpile stats report --no-open       # 開かずに生成のみ
transpile stats report --out /tmp/custom.html
```

> レポートはデフォルトで `~/.agents/transpile/reports/` に生成されます。 `--out` で上書きできます。

ダッシュボードに含まれる内容：

- **KPIカード** — 総呼び出し数、節約されたトークン、平均削減率、一意のファイル、エージェント、アクティブ日数
- **6つのチャート** — 日次トークン使用量、忠実度の内訳、入力と出力の傾向、エージェント分布、時間帯別パターン、削減率の分布
- **日付範囲プリセット** — ワンクリックでフィルタリング： `今日` · `1週間` · `2週間` · `1ヶ月` · `90日間`（デフォルト: 1週間）
- **フィルター** — プロジェクト、エージェント、ファイルテキストフィルター、CSVエクスポート
- **テーマ切り替え** — ダーク/ライトモードの永続的な設定
- **バイリンガル** — 韓国語ロケールの自動検出、手動での 韓/EN 切り替え


**JSONLレコードフィールド**

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `ts` | ISO 8601 | 呼び出しのタイムスタンプ |
| `agent` | 文字列 | 呼び出しをトリガーしたツール（`claude`、`antigravity`、`codex`、`opencode`） |
| `file` | 文字列 | 入力ファイルパス（stdin読み込み時は空） |
| `format` | 文字列 | `markdown`、`html`、`plaintext`のいずれか |
| `fidelity` | 文字列 | `lossless`、`semantic`、`compressed`のいずれか |
| `input_tok` | 整数 | トランスパイル前のトークン数 |
| `output_tok` | 整数 | トランスパイル後のトークン数 |
| `reduction_pct` | 浮動小数点 | 削減されたトークンの割合 |
| `saved` | 整数 | 削減されたトークンの絶対数（`input_tok − output_tok`） |

**`TRANSPILE_AGENT`環境変数**

`agent`フィールドは`TRANSPILE_AGENT`環境変数から取得されます。各連携ツールはこれを自動的に設定します（`claude`、`antigravity`、`codex`、`opencode`、`cursor`）。手動で設定することも可能です:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

### ベンチマーキング


```bash
# テストファイルのディレクトリに対してベンチマークを実行
transpile bench run --dataset ./eval                    # JSONL ログを生成
transpile bench run --dataset ./eval --report           # 実行 + HTML レポートを開く
transpile bench report                                  # ログからレポートを再生成
```

HTML ベンチマークレポートに含まれる内容：

- **KPIカード** — semantic 削減率、compressed 削減率、スループット (tok/ms)、単語カバレッジ、総入力トークン、実行回数
- **7つのチャート** — 時間経過に伴う削減傾向、実行ごとのスループット、semantic 対 スループットの散布図、フォーマット別の箱ひげ図、フォーマット分布、トークンサイズのヒストグラム、単語カバレッジのドーナツグラフ
- **実行テーブル** — 集計指標を含む実行ごとのサマリー
- **レコードテーブル** — フォーマット、実行、ファイル名フィルター付きのファイルごとの詳細
- **テーマ切り替え** — ダーク/ライトモードの永続的な設定
- **バイリンガル** — 韓国語ロケールの自動検出、手動での 韓/EN 切り替え


---

---

## ライブラリ使用法

### 同期処理

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

### ストリーミング（Tokio）

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

### トークン数の推定

```rust
let n = llm_transpiler::token_count("Hello, world!");
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
use llm_transpiler::TranspileError;

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

リリースビルド（`cargo build --release`）、Apple Mシリーズ、Markdown/HTML/PlainText 48ドキュメント測定。すべての削減率は**実際の`cl100k` BPEトークナイザー**で測定したものです（自己言及ヒューリスティックではありません）。完全な手法とフォーマットごとの内訳は[`docs/EVALUATION.md`](../EVALUATION.md)を参照してください。

| 指標 | 測定値 | 備考 |
|------|--------|------|
| スループット（Markdown単体ピーク） | **10,975 tok/ms** | Pythonパーシングベースラインの≈75倍高速、単一フォーマットのピーク |
| スループット（データセット全体） | **~1,070 tok/ms** | 48ドキュメント/3フォーマット全体の加重平均（BPE） — ベンチマーク表を参照 |
| Semantic削減率 | **27.4%**（Markdown） | 真の圧縮率、15–30%目標帯域内 |
| Compressed削減率 | **69.4%**（Markdown） | 予算適応型、PruneLowImportance以上保証 |
| Lossless単語カバレッジ | **99.0% 平均** | 全フォーマット・言語 |
| HTML削減率 | **98.7%** | ナビ/スクリプト/スタイルのマークアップオーバーヘッド除去を反映 |
| 多言語サポート | 15言語テスト済み | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 平均99.0%単語カバレッジ |

評価スイートを自分で実行:

```bash
make eval          # 構造化JSON（BPE + ヒューリスティック、`epic eval`が消費）
make eval-report   # 人間が読めるファイル別テーブル + サマリー
```

ファイルごとの内訳、手法、および既知の制限: [`docs/EVALUATION.md`](../EVALUATION.md)

---

## コントリビュート

完全なガイドラインは[CONTRIBUTING.md](../../CONTRIBUTING.md)を参照してください。プルリクエストを歓迎します — `good first issue`ラベルの未解決Issueを確認してください。

---

## ライセンス

Apache-2.0 — [LICENSE](../../LICENSE)を参照。
