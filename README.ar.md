# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**محوّل وثائق مُحسَّن للرموز في مسارات LLM**

الوثائق الخام (Markdown وHTML والنص العادي) ← تنسيق جسري منظم `<D>?<H><B>` — مع ضغط تكيّفي يحافظ على ميزانية الرموز.

```
<H>
t: اتفاقية ترخيص البرمجيات
s: شروط الترخيص السنوية بين المرخِّص والمرخَّص له
k: [ترخيص, عقد, برمجيات]
</H>
<B>
# أطراف العقد
تُبرم هذه الاتفاقية بين المرخِّص والمرخَّص له.
...
</B>
```

---

## جدول المحتويات

- [لماذا](#لماذا)
- [التثبيت](#التثبيت)
- [استخدام CLI](#استخدام-cli)
- [استخدام المكتبة](#استخدام-المكتبة)
- [تنسيق الإخراج](#تنسيق-الإخراج)
- [مستويات الدقة](#مستويات-الدقة)
- [الضغط التكيّفي](#الضغط-التكيّفي)
- [تنسيقات الإدخال](#تنسيقات-الإدخال)
- [معالجة الأخطاء](#معالجة-الأخطاء)
- [الأداء](#الأداء)
- [المساهمة](#المساهمة)
- [الرخصة](#الرخصة)

---

## لماذا

تعمل نماذج اللغة الكبيرة بشكل أفضل عندما يكون السياق نظيفاً وكثيفاً. تتولى هذه المكتبة العمل الميكانيكي:

- **التحليل الهيكلي** — Markdown/HTML/نص عادي ← عُقد IR مكتوبة (عناوين، فقرات، جداول، قوائم، كتل برمجية)
- **الضغط التكيّفي** — يتصاعد تلقائياً عبر 4 مراحل مع امتلاء ميزانية الرموز
- **استبدال الرموز** — مصطلحات النطاق المتكررة ← أحرف Unicode PUA، يفكّها رأس القاموس `<D>`
- **تحويل الجداول إلى خطي** — جداول Markdown ← تسلسلات `Key:Val` مضغوطة (≤5 صفوف) أو صفوف مفصولة بـ pipe للجداول الكبيرة
- **الإخراج المتدفق** — يُسلّم دفق Tokio أول قطعة فوراً، مما يُقلّص TTFT

---

## التثبيت

### المكتبة (حزمة Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

يتطلب **Rust 1.75+**.

### ملف CLI الثنائي + تكامل الأدوات

```bash
# Homebrew (macOS)
brew install epicsagas/tap/llm-transpile

# ملف ثنائي جاهز (أسرع، بدون تجميع)
cargo binstall llm-transpile

# من crates.io
cargo install llm-transpile
```

تهيئة تكاملات الأدوات:

```bash
transpile install
```

يُطلق `transpile install` معالجاً تفاعلياً يكتشف الأدوات المثبتة ويهيّئها:

| الأداة | طريقة التكامل | الوظيفة |
|--------|--------------|---------|
| **Claude Code** | خطاف PostToolUse | ضغط تلقائي لملفات `.md/.html/.txt` عند القراءة |
| **Gemini CLI** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |
| **Codex CLI** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |
| **Cursor** | قاعدة `.mdc` (`alwaysApply`) | يُشغّل `transpile` قبل قراءة ملفات الوثائق |
| **OpenCode** | `SKILL.md` | يستدعي النموذج `transpile` تلقائياً على امتدادات الملفات |

**التثبيت / إلغاء التثبيت الانتقائي**

```bash
transpile install claude gemini    # أدوات محددة فقط
transpile install --all            # كل شيء دفعة واحدة
transpile install --dry-run        # معاينة التغييرات
transpile install --list           # عرض حالة التكاملات

transpile uninstall cursor         # إزالة واحدة
transpile uninstall --all          # إزالة الكل
transpile uninstall --dry-run      # معاينة الإزالات
```

**إضافة Claude Code** (بديل — يتطلب دعم الإضافات)

```
/plugin marketplace add epicsagas/claude-plugins
/plugin install transpile@epicsagas
```

من الكود المصدري:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
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

## استخدام المكتبة

### متزامن

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

### متدفق (Tokio)

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

### تقدير عدد الرموز

```rust
let n = llm_transpile::token_count("Hello, world!");
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
use llm_transpile::TranspileError;

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

---

## المساهمة

نرحب بتقارير الأخطاء وطلبات الميزات وطلبات السحب.

```bash
# الاستنساخ والبناء
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# تشغيل الاختبارات
cargo test

# تشغيل المعايير (تقرير HTML ← target/criterion/)
cargo bench

# الفحص والتنسيق
cargo clippy -- -D warnings
cargo fmt
```

**الإرشادات**

- الحفاظ على MSRV عند Rust 1.75 — تجنّب الميزات المُقدَّمة بعد ذلك.
- يجب ألا يؤثر سلوك الضغط الجديد على وضع `Lossless`.
- يجب أن يشمل كل PR اختبارات لأي منطق جديد في الوحدة ذات الصلة (`ir`، `compressor`، `symbol`، `renderer`).
- تشغيل `cargo clippy -- -D warnings` و`cargo fmt` قبل التقديم.

---

## الرخصة

Apache-2.0 — انظر [LICENSE](LICENSE).
