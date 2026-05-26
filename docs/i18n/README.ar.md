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

**مترجم مستندات محسن للرموز (Tokens) في خطوط أنابيب نماذج اللغة الكبيرة (LLMs)**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

الوثائق الخام (Markdown وHTML والنص العادي) ← تنسيق جسري منظم `<D>?<H><B>` — مع ضغط تكيّفي يحافظ على ميزانية الرموز.

---

<details>
<summary> جدول المحتويات </summary>
- [لماذا](#لماذا)
- [التثبيت](#التثبيت)
- [التحديث](#التحديث)
- [استخدام CLI](#استخدام-cli)
- [إحصائيات الاستخدام](#إحصائيات-الاستخدام)
- [استخدام المكتبة](#استخدام-المكتبة)
- [تنسيق الإخراج](#تنسيق-الإخراج)
- [مستويات الدقة](#مستويات-الدقة)
- [الضغط التكيّفي](#الضغط-التكيّفي)
- [تنسيقات الإدخال](#تنسيقات-الإدخال)
- [معالجة الأخطاء](#معالجة-الأخطاء)
- [الأداء](#الأداء)
- [المساهمة](#المساهمة)
- [الرخصة](#الرخصة)- [قياس الأداء](#قياس-الأداء-benchmarking)

</details>

---

## لماذا

تعمل نماذج اللغة الكبيرة بشكل أفضل عندما يكون السياق نظيفاً وكثيفاً. تتولى هذه المكتبة العمل الميكانيكي:

| | الميزة | لماذا هي مهمة |
|--|--------|--------------|
| 🏗️ | **التحليل الهيكلي** | Markdown/HTML/نص عادي ← عُقد IR مكتوبة (عناوين، فقرات، جداول، قوائم، كتل برمجية) |
| 📉 | **الضغط التكيّفي** | يتصاعد تلقائياً عبر 4 مراحل مع امتلاء ميزانية الرموز |
| 🔣 | **استبدال الرموز** | مصطلحات النطاق المتكررة ← أحرف Unicode PUA، يفكّها رأس القاموس `<D>` |
| 📊 | **تحويل الجداول إلى خطي** | جداول Markdown ← تسلسلات `Key:Val` مضغوطة (≤5 صفوف) أو صفوف مفصولة بـ pipe للجداول الكبيرة |
| 🌊 | **الإخراج المتدفق** | يُسلّم دفق Tokio أول قطعة فوراً، مما يُقلّص TTFT |

### قياسات الأداء

37 وثيقة، 4 تنسيقات، 5 لغات — Apple M-series، بناء `--release`. التقرير الكامل: [`eval/EVAL_REPORT.md`](../../eval/EVAL_REPORT.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown (EN) | 29.8% | 42.0% | 99.7% | 895 tok/ms |
| Markdown (ML) | 43.1% | 43.9% | 97.3% | 3,483 tok/ms |
| HTML | 97.7% | 97.7% | 93.0% | 5,879 tok/ms |
| PlainText | 17.7% | 47.7% | 100.0% | 189 tok/ms |
| **Overall** | **79.2%** | **81.1%** | **98.4%** | **2,258 tok/ms** |

> نسبة تخفيض HTML تعكس إزالة التكلفة الإضافية للترميز (التنقل والسكريبتات والأنماط)، وليس ضغط النص فحسب.

---

## التثبيت

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

يُثبّت الملف الثنائي ويهيّئ خطاف PostToolUse تلقائياً عند بدء الجلسة التالية — لا حاجة لإعداد إضافي.

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

يُسجّل خطاف PostToolUse تلقائياً — لا حاجة لخطوات إضافية.

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

لا تستخدم Homebrew؟ استخدم نص التثبيت:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### عبر سلسلة أدوات Rust

```bash
cargo binstall llm-transpile   # ملف ثنائي جاهز (أسرع)
cargo install llm-transpile    # بناء من المصدر
```

### بعد التثبيت

تهيئة تكاملات الأدوات:

```bash
transpile install
```

يُطلق `transpile install` معالجاً تفاعلياً يكتشف الأدوات المثبتة ويهيّئها:

| الأداة | طريقة التكامل | الوظيفة |
|--------|--------------|---------|
| **Antigravity** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |
| **Cursor** | قاعدة `.mdc` (`alwaysApply`) | يُشغّل `transpile` قبل قراءة ملفات الوثائق |
| **OpenCode** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |
| **Cline** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |

جميع الأدوات تستخدم ملف مهارة يُعلّم النموذج تشغيل `TRANSPILE_AGENT=<agent> transpile --input <file>` تلقائياً — لا حاجة لفحص الحجم، الامتداد وحده يُفعّله.

**التثبيت / إلغاء التثبيت الانتقائي**

```bash
transpile install antigravity cursor    # أدوات محددة فقط
transpile install --all            # كل شيء دفعة واحدة
transpile install --dry-run        # معاينة التغييرات
transpile install --list           # عرض حالة التكاملات

transpile uninstall cursor         # إزالة واحدة
transpile uninstall --all          # إزالة الكل
transpile uninstall --dry-run      # معاينة الإزالات
```

### المكتبة (حزمة Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

يتطلب **Rust 1.92+**.

### Antigravity (Gemini CLI)

```bash
agy plugins install https://github.com/epicsagas/llm-transpile
```

يثبّت البرنامج المساعد (الخطافات) تلقائياً ويسجّله عند بدء الجلسة التالية.


### قياس الأداء (Benchmarking)


```bash
# تشغيل القياس على مجلد ملفات تجريبية
transpile bench run --dataset ./eval                    # إنشاء سجل JSONL
transpile bench run --dataset ./eval --report           # تشغيل + فتح تقرير HTML
transpile bench report                                  # إعادة إنشاء تقرير من السجلات
```

يتضمن تقرير HTML الخاص بالقياس:

- **بطاقات الأداء (KPIs)** — تخفيض الدلالة (semantic)، التخفيض المضغوط (compressed)، الإنتاجية (tok/ms)، تغطية الكلمات، إجمالي رموز الإدخال، وعدد التشغيلات
- **7 رسوم بيانية** — اتجاه التخفيض، الإنتاجية لكل تشغيل، التشتت، الصندوق المخطط (Box plot)، توزيع التنسيقات، والمدرج التكراري للرموز
- **جدول التشغيلات** — ملخص لكل تشغيل مع مقاييس مجمعة
- **جدول السجلات** — تفاصيل لكل ملف مع الفلاتر
- **السمة (Theme)** — الوضع الداكن/الفاتح مع حفظ التفضيل
- **ثنائية اللغة** — اكتشاف تلقائي للغة الكورية؛ تبديل يدوي بين الكورية والإنجليزية


---

---

## التحديث

| الطريقة | الأمر |
|---------|-------|
| Homebrew | `brew upgrade llm-transpile` |
| مثبّت curl / PowerShell | أعد تشغيل أمر التثبيت أعلاه |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## استخدام CLI

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       مسار ملف الإدخال (يقرأ من stdin إذا حُذف)
  -f, --format <FORMAT>    تنسيق الإدخال: markdown | html | plaintext  [افتراضي: markdown]
                           يُكتشف تلقائياً من امتداد الملف عند استخدام --input
  -l, --fidelity <LEVEL>  مستوى الضغط: lossless | semantic | compressed  [افتراضي: semantic]
  -b, --budget <N>         الحد الأعلى لميزانية الرموز (غير محدود إذا حُذف)
  -c, --count              يطبع عدد رموز الإدخال فحسب ثم يخرج
  -j, --json               إخراج بتنسيق JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              يُكتم سطر الإحصائيات في stderr
      --stats              يطبع سطر الإحصائيات في stdout بعد المحتوى
  -h, --help               طباعة المساعدة
  -V, --version            طباعة الإصدار
```

**أمثلة**

```bash
# تحويل ملف Markdown (يُكتشف التنسيق تلقائياً من الامتداد .md)
transpile --input doc.md

# القراءة من stdin — stdout نظيف، الإحصائيات في stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# توصيل نظيف — إخفاء الإحصائيات كلياً
transpile --input doc.md --quiet | send_to_llm_api

# التحقق من عدد الرموز بدون تحويل
transpile --input doc.md --count

# إخراج JSON للسكريبتات والمسارات
transpile --input doc.md --json | jq '.reduction_pct'

# التقاط المحتوى + الإحصائيات في دفق واحد
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — بدون ضغط، الحفاظ الكامل على المحتوى (وثائق قانونية/تدقيق)
transpile --input contract.md --fidelity lossless

# ضغط مكثف ضمن ميزانية 512 رمزاً
transpile --input article.md --fidelity compressed --budget 512
```

> تُكتب الإحصائيات (`[273 → 150 tok  45.1% reduction]`) في **stderr** افتراضياً، مما يُبقي stdout نظيفاً للتوصيل. استخدم `--quiet` لإخفائها، أو `--stats` لإعادة توجيهها إلى stdout.

---

## إحصائيات الاستخدام

كل استدعاء لـ `transpile` يُضيف تلقائياً سجلاً إلى `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`. تقرأ الأوامر الفرعية `transpile stats` هذه الملفات وتطبع جدولاً ملخصاً.

```
transpile stats show                # اليوم
transpile stats show --days 7       # آخر N أيام
transpile stats show --agent claude # تصفية حسب الأداة
```

مثال على الإخراج:

```
transpile stats — آخر 7 أيام

  التاريخ     الأداة     الاستدعاءات  رموز الإدخال  رموز الإخراج  الموفّرة  النسبة
  ──────────────────────────────────────────────────────────────────────────────────
  2026-04-13  claude         5        14 965        10 872       4 093     27.3%
  2026-04-13  antigravity         2         4 800         3 500       1 300     27.1%
  ──────────────────────────────────────────────────────────────────────────────────
  الإجمالي                  7        19 765        14 372       5 393     27.3%
```

**حقول سجل JSONL**

| الحقل | النوع | الوصف |
|-------|-------|-------|
| `ts` | ISO 8601 | الطابع الزمني للاستدعاء |
| `agent` | نص | الأداة التي بدأت الاستدعاء (`claude`، `antigravity`، `codex`، `opencode`) |
| `file` | نص | مسار ملف الإدخال (فارغ عند القراءة من stdin) |
| `format` | نص | `markdown` أو `html` أو `plaintext` |
| `fidelity` | نص | `lossless` أو `semantic` أو `compressed` |
| `input_tok` | عدد صحيح | عدد الرموز قبل التحويل |
| `output_tok` | عدد صحيح | عدد الرموز بعد التحويل |
| `reduction_pct` | عدد عشري | نسبة الرموز الموفّرة |
| `saved` | عدد صحيح | الرموز الموفّرة المطلقة (`input_tok − output_tok`) |

**متغير البيئة `TRANSPILE_AGENT`**

يُملأ حقل `agent` من متغير البيئة `TRANSPILE_AGENT`. تعيّن كل تكامل هذه القيمة تلقائياً (`claude`، `antigravity`، `codex`، `opencode`، `cursor`). يمكن أيضاً تعيينها يدوياً:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## استخدام المكتبة

### متزامن

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

### متدفق (Tokio)

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

### تقدير عدد الرموز

```rust
let n = llm_transpiler::token_count("Hello, world!");
```

---

## تنسيق الإخراج

```
<D>                  ← قاموس الرموز (يُحذف عند عدم وجود استبدالات)
{sym}=مصطلح-متكرر
</D>
<H>                  ← رأس بيانات وصفية بأسلوب YAML
t: عنوان الوثيقة
s: ملخص من سطر واحد
k: [كلمة_مفتاحية1, كلمة_مفتاحية2]
</H>
<B>                  ← جسم الوثيقة (مضغوط + مستبدَل)
...المحتوى...
</B>
```

يستخدم الكتلة `<D>` أحرفاً من نطاق الاستخدام الخاص في Unicode (`U+E000–U+F8FF`) كمعرّفات رموز مضغوطة، تجنّباً للتعارض مع أنماط النص المرئي. يدعم القاموس ما يصل إلى **6,400 مصطلح فريد** لكل وثيقة.

---

## مستويات الدقة

| المستوى | حالة الاستخدام النموذجية | الضغط المُطبَّق |
|---------|------------------------|----------------|
| `Lossless` | الوثائق القانونية/التدقيق | لا شيء — المحتوى الأصلي مضمون |
| `Semantic` | مسارات RAG العامة | إزالة الكلمات الوقفية + تقليص المحتوى منخفض الأهمية |
| `Compressed` | التلخيص، الميزانيات الضيقة | ضغط أقصى، استخراج الجملة الأولى |

---

## الضغط التكيّفي

يراقب المُضغِّط استخدام الميزانية في الوقت الفعلي ويتصاعد تلقائياً:

| استخدام الميزانية | المرحلة | ما يحدث |
|-----------------|---------|---------|
| 0–60% | `StopwordOnly` | إزالة الكلمات الوقفية الإنجليزية/الكورية |
| 60–80% | `PruneLowImportance` | إزالة أدنى 20% من الفقرات حسب الأهمية |
| 80–95% | `DeduplicateAndLinearize` | إزالة الجمل المكررة؛ تحويل الجداول إلى خطي |
| 95%+ | `MaxCompression` | اختزال كل فقرة إلى الجملة الأولى |

> يتجاوز وضع `Lossless` جميع مراحل الضغط بشكل غير مشروط.

أثناء البث، عندما يتجاوز استخدام الميزانية 80%، تنتقل العُقد المتبقية تلقائياً إلى وضع `Compressed`.

---

## تنسيقات الإدخال

| `InputFormat` | المُحلِّل |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + جداول GFM |
| `Html` | تعقيم ammonia ← إزالة الوسوم ← مسار النص العادي |
| `PlainText` | تقسيم الفقرات بالأسطر الفارغة |

---

## معالجة الأخطاء

```rust
use llm_transpiler::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* استخدام الإخراج */ }
    Err(TranspileError::Parse(msg))            => eprintln!("فشل التحليل: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("مصطلحات فريدة أكثر من اللازم: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("الضغط في وضع lossless"),
    Err(e)                                     => eprintln!("خطأ: {e}"),
}
```

---

## الأداء

قياس في بناء إصدار (`cargo build --release`)، Apple M-series، 48 وثيقة عبر Markdown/HTML/PlainText:

| المقياس | المقيَّس | ملاحظات |
|---------|---------|---------|
| الإنتاجية | **10,975 tok/ms** | ≈75 مرة أسرع من الخط الأساسي Python |
| تخفيض Semantic | **33.9%** (Markdown) | تحقق هدف 15–30% |
| تخفيض Compressed | **39.7%** (Markdown) | تكيّفي مع الميزانية، ≥ PruneLowImportance مضمون |
| تغطية كلمات Lossless | **98.8% متوسط** | عبر جميع التنسيقات واللغات |
| تخفيض HTML | **97.6%** | إزالة التكلفة الإضافية للترميز nav/السكريبتات/الأنماط |
| الدعم متعدد اللغات | 15 لغة مُختبَرة | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.4% تغطية كلمات متوسطة |

شغّل مجموعة التقييم بنفسك:

```bash
cargo run --release --example eval
```

تفصيل كل ملف، المنهجية، والقيود المعروفة: [`eval/EVAL_REPORT.md`](../../eval/EVAL_REPORT.md)

---

## المساهمة

انظر [CONTRIBUTING.md](../../CONTRIBUTING.md) للإرشادات الكاملة. نرحب بطلبات السحب — راجع المشاكل المفتوحة المُوسومة بـ `good first issue`.

---

## الرخصة

Apache-2.0 — انظر [LICENSE](../../LICENSE).
