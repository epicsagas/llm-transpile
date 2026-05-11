# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**LLM पाइपलाइन के लिए टोकन-अनुकूलित दस्तावेज़ ट्रांसपाइलर**

कच्चे दस्तावेज़ (Markdown, HTML, सादा टेक्स्ट) → संरचित ब्रिज फॉर्मेट `<D>?<H><B>` — टोकन बजट में रहने वाले अनुकूली संपीड़न के साथ।

```
<H>
t: सॉफ़्टवेयर लाइसेंस अनुबंध
s: लाइसेंसर और लाइसेंसी के बीच वार्षिक लाइसेंस शर्तें
k: [लाइसेंस, अनुबंध, सॉफ़्टवेयर]
</H>
<B>
# अनुबंध पक्ष
यह अनुबंध लाइसेंसर और लाइसेंसी के बीच संपन्न होता है।
...
</B>
```

---

<details>
<summary>विषय-सूची</summary>
- [क्यों](#क्यों)
- [स्थापना](#स्थापना)
- [अपडेट करना](#अपडेट-करना)
- [CLI उपयोग](#cli-उपयोग)
- [उपयोग सांख्यिकी](#उपयोग-सांख्यिकी)
- [लाइब्रेरी उपयोग](#लाइब्रेरी-उपयोग)
- [आउटपुट फॉर्मेट](#आउटपुट-फॉर्मेट)
- [फिडेलिटी स्तर](#फिडेलिटी-स्तर)
- [अनुकूली संपीड़न](#अनुकूली-संपीड़न)
- [इनपुट फॉर्मेट](#इनपुट-फॉर्मेट)
- [त्रुटि प्रबंधन](#त्रुटि-प्रबंधन)
- [प्रदर्शन](#प्रदर्शन)
- [योगदान](#योगदान)
- [लाइसेंस](#लाइसेंस)
</details>

---

## क्यों

LLM तब बेहतर काम करते हैं जब संदर्भ स्वच्छ और घना हो। यह लाइब्रेरी यांत्रिक कार्य संभालती है:

| | सुविधा | यह क्यों मायने रखती है |
|--|--------|------------------------|
| 🏗️ | **संरचनात्मक पार्सिंग** | Markdown/HTML/सादा टेक्स्ट → टाइप किए गए IR नोड्स (शीर्षक, पैराग्राफ, तालिकाएं, सूचियां, कोड ब्लॉक) |
| 📉 | **अनुकूली संपीड़न** | टोकन बजट भरते ही 4 चरणों के माध्यम से स्वचालित रूप से बढ़ता है |
| 🔣 | **प्रतीक प्रतिस्थापन** | दोहराए जाने वाले डोमेन शब्द → Unicode PUA वर्ण, `<D>` शब्दकोश हेडर द्वारा डिकोड किए गए |
| 📊 | **तालिका रैखिकीकरण** | Markdown तालिकाएं → संक्षिप्त `Key:Val` अनुक्रम (≤5 पंक्तियां) या बड़ी तालिकाओं के लिए pipe-विभाजित पंक्तियां |
| 🌊 | **स्ट्रीमिंग आउटपुट** | Tokio स्ट्रीम TTFT को न्यूनतम करते हुए पहला chunk तुरंत देता है |

---

## स्थापना

### लाइब्रेरी (Rust क्रेट)

```toml
[dependencies]
llm-transpile = "0.1"
```

**Rust 1.92+** आवश्यक।

### CLI बाइनरी + टूल इंटीग्रेशन

**macOS / Linux**

```bash
brew install epicsagas/tap/llm-transpile
```

Homebrew नहीं है? इंस्टॉलर स्क्रिप्ट का उपयोग करें:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

**Windows**

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

**Rust टूलचेन के माध्यम से**

```bash
cargo binstall llm-transpile   # पूर्व-निर्मित बाइनरी (तेज़)
cargo install llm-transpile    # सोर्स से बिल्ड
```

टूल इंटीग्रेशन कॉन्फ़िगर करें:

```bash
transpile install
```

`transpile install` एक इंटरैक्टिव विज़ार्ड लॉन्च करता है जो इंस्टॉल किए गए टूल का पता लगाता और कॉन्फ़िगर करता है:

| टूल | इंटीग्रेशन विधि | कार्य |
|-----|----------------|-------|
| **Claude Code** | PostToolUse हुक | Read पर `.md/.html/.txt` फ़ाइलें स्वचालित संपीड़ित |
| **Gemini CLI** | `SKILL.md` | LLM दस्तावेज़ एक्सटेंशन पर `transpile` स्वचालित चलाता है |
| **Codex CLI** | `SKILL.md` | LLM दस्तावेज़ एक्सटेंशन पर `transpile` स्वचालित चलाता है |
| **Cursor** | `.mdc` नियम (`alwaysApply`) | दस्तावेज़ फ़ाइलें पढ़ने से पहले `transpile` ट्रिगर करता है |
| **OpenCode** | `SKILL.md` | LLM दस्तावेज़ एक्सटेंशन पर `transpile` स्वचालित चलाता है |

Claude को छोड़कर सभी टूल एक स्किल फ़ाइल का उपयोग करते हैं जो LLM को `TRANSPILE_AGENT=<agent> transpile --input <file>` स्वचालित रूप से चलाना सिखाती है — साइज़ जांच की आवश्यकता नहीं, केवल एक्सटेंशन पर्याप्त है।

**चयनात्मक इंस्टॉल / अनइंस्टॉल**

```bash
transpile install claude gemini    # केवल विशिष्ट टूल
transpile install --all            # सब एक साथ
transpile install --dry-run        # बदलावों का पूर्वावलोकन
transpile install --list           # सभी इंटीग्रेशन की स्थिति देखें

transpile uninstall cursor         # एक हटाएं
transpile uninstall --all          # सब हटाएं
transpile uninstall --dry-run      # हटाने का पूर्वावलोकन
```

**Claude Code प्लगइन**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

अगले सेशन स्टार्ट पर बाइनरी और PostToolUse हुक स्वचालित रूप से इंस्टॉल हो जाते हैं — कोई अतिरिक्त सेटअप आवश्यक नहीं।

सोर्स से:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## अपडेट करना

| विधि | कमांड |
|------|-------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShell इंस्टॉलर | ऊपर दी गई इंस्टॉल कमांड दोबारा चलाएं |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## CLI उपयोग

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       इनपुट फ़ाइल पथ (छोड़ने पर stdin से पढ़ता है)
  -f, --format <FORMAT>    इनपुट फॉर्मेट: markdown | html | plaintext  [डिफ़ॉल्ट: markdown]
                           --input के साथ फ़ाइल एक्सटेंशन से स्वचालित पहचान
  -l, --fidelity <LEVEL>  संपीड़न स्तर: lossless | semantic | compressed  [डिफ़ॉल्ट: semantic]
  -b, --budget <N>         टोकन बजट की ऊपरी सीमा (छोड़ने पर असीमित)
  -c, --count              केवल इनपुट टोकन गिनती प्रिंट करके बाहर निकलें
  -j, --json               JSON के रूप में आउटपुट {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              stderr पर आंकड़े की पंक्ति दबाएं
      --stats              सामग्री के बाद आंकड़े stdout पर प्रिंट करें
  -h, --help               सहायता प्रिंट करें
  -V, --version            संस्करण प्रिंट करें
```

**उदाहरण**

```bash
# Markdown फ़ाइल रूपांतरित करें (.md एक्सटेंशन से फॉर्मेट स्वचालित पहचाना जाता है)
transpile --input doc.md

# stdin से पढ़ें — स्वच्छ stdout, stderr पर आंकड़े
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# स्वच्छ पाइप — आंकड़े पूरी तरह दबाएं
transpile --input doc.md --quiet | send_to_llm_api

# बिना रूपांतरण के टोकन गिनती जांचें
transpile --input doc.md --count

# स्क्रिप्ट और पाइपलाइन के लिए JSON आउटपुट
transpile --input doc.md --json | jq '.reduction_pct'

# एक स्ट्रीम में सामग्री + आंकड़े कैप्चर करें
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — कोई संपीड़न नहीं, पूर्ण सामग्री संरक्षित (कानूनी/ऑडिट दस्तावेज़)
transpile --input contract.md --fidelity lossless

# 512 टोकन बजट में आक्रामक संपीड़न
transpile --input article.md --fidelity compressed --budget 512
```

> आंकड़े (`[273 → 150 tok  45.1% reduction]`) डिफ़ॉल्ट रूप से **stderr** पर लिखे जाते हैं, जिससे stdout पाइपिंग के लिए स्वच्छ रहता है। दबाने के लिए `--quiet` या stdout पर पुनर्निर्देशित करने के लिए `--stats` का उपयोग करें।

---

## उपयोग सांख्यिकी

प्रत्येक `transpile` कॉल स्वचालित रूप से `~/.agents/transpile/stats/YYYY-MM-DD.jsonl` में एक रिकॉर्ड जोड़ता है। `transpile stats` सबकमांड उन फ़ाइलों को पढ़ता है और एक सारांश तालिका प्रिंट करता है।

```
transpile stats                # आज
transpile stats --days 7       # अंतिम N दिन
transpile stats --agent claude # एजेंट के अनुसार फ़िल्टर
```

आउटपुट उदाहरण:

```
transpile stats — अंतिम 7 दिन

  दिनांक      एजेंट     कॉल   इनपुट टोकन  आउटपुट टोकन  बचत    कमी
  ──────────────────────────────────────────────────────────────────────────
  2026-04-13  claude        5     14 965       10 872      4 093    27.3%
  2026-04-13  gemini        2      4 800        3 500      1 300    27.1%
  ──────────────────────────────────────────────────────────────────────────
  कुल                      7     19 765       14 372      5 393    27.3%
```

**JSONL रिकॉर्ड फ़ील्ड**

| फ़ील्ड | प्रकार | विवरण |
|--------|--------|--------|
| `ts` | ISO 8601 | कॉल का टाइमस्टैम्प |
| `agent` | स्ट्रिंग | कॉल शुरू करने वाला टूल (`claude`, `gemini`, `codex`, `opencode`) |
| `file` | स्ट्रिंग | इनपुट फ़ाइल पथ (stdin से पढ़ने पर रिक्त) |
| `format` | स्ट्रिंग | `markdown`, `html`, या `plaintext` |
| `fidelity` | स्ट्रिंग | `lossless`, `semantic`, या `compressed` |
| `input_tok` | पूर्णांक | ट्रांसपाइलेशन से पहले टोकन गिनती |
| `output_tok` | पूर्णांक | ट्रांसपाइलेशन के बाद टोकन गिनती |
| `reduction_pct` | दशमलव | टोकन बचत प्रतिशत |
| `saved` | पूर्णांक | बचाए गए टोकन (`input_tok − output_tok`) |

**`TRANSPILE_AGENT` पर्यावरण चर**

`agent` फ़ील्ड `TRANSPILE_AGENT` पर्यावरण चर से भरा जाता है। प्रत्येक इंटीग्रेशन इसे स्वचालित रूप से सेट करता है (`claude`, `gemini`, `codex`, `opencode`, `cursor`)। आप इसे मैन्युअल रूप से भी सेट कर सकते हैं:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## लाइब्रेरी उपयोग

### समकालिक

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

### स्ट्रीमिंग (Tokio)

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

### टोकन गिनती अनुमान

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## आउटपुट फॉर्मेट

```
<D>                  ← प्रतीक शब्दकोश (कोई प्रतिस्थापन नहीं होने पर छोड़ा गया)
{sym}=दोहराया-शब्द
</D>
<H>                  ← YAML जैसा मेटाडेटा हेडर
t: दस्तावेज़ शीर्षक
s: एक-पंक्ति सारांश
k: [कीवर्ड1, कीवर्ड2]
</H>
<B>                  ← दस्तावेज़ मुख्य भाग (संपीड़ित + प्रतिस्थापित)
...सामग्री...
</B>
```

`<D>` ब्लॉक Unicode निजी उपयोग क्षेत्र वर्णों (`U+E000–U+F8FF`) को प्रतीक हैंडल के रूप में उपयोग करता है, दृश्यमान टेक्स्ट पैटर्न के साथ टकराव से बचता है। शब्दकोश प्रति दस्तावेज़ अधिकतम **6,400 अनन्य शब्दों** का समर्थन करता है।

---

## फिडेलिटी स्तर

| स्तर | विशिष्ट उपयोग मामला | लागू संपीड़न |
|------|-------------------|-------------|
| `Lossless` | कानूनी/ऑडिट दस्तावेज़ | कोई नहीं — मूल सामग्री गारंटीड |
| `Semantic` | सामान्य RAG पाइपलाइन | स्टॉपवर्ड हटाना + कम-महत्व की छंटाई |
| `Compressed` | सारांश, सख्त बजट | अधिकतम संपीड़न, पहला वाक्य निष्कर्षण |

---

## अनुकूली संपीड़न

कंप्रेसर रियल टाइम में बजट उपयोग की निगरानी करता है और स्वचालित रूप से बढ़ता है:

| बजट उपयोग | चरण | क्या होता है |
|-----------|-----|------------|
| 0–60% | `StopwordOnly` | अंग्रेज़ी/कोरियाई स्टॉपवर्ड हटाए जाते हैं |
| 60–80% | `PruneLowImportance` | महत्व के आधार पर नीचे के 20% पैराग्राफ हटाए जाते हैं |
| 80–95% | `DeduplicateAndLinearize` | डुप्लीकेट वाक्य हटाए जाते हैं; तालिकाएं रैखिकीकृत होती हैं |
| 95%+ | `MaxCompression` | प्रत्येक पैराग्राफ पहले वाक्य तक छोटा किया जाता है |

> `Lossless` मोड बिना शर्त सभी संपीड़न चरणों को बायपास करता है।

स्ट्रीमिंग के दौरान, जब बजट उपयोग 80% से अधिक होता है, शेष नोड्स स्वचालित रूप से `Compressed` मोड में बदल जाते हैं।

---

## इनपुट फॉर्मेट

| `InputFormat` | पार्सर |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM तालिकाएं |
| `Html` | ammonia सैनिटाइज़ेशन → टैग हटाना → सादा टेक्स्ट पाइपलाइन |
| `PlainText` | रिक्त पंक्ति पैराग्राफ विभाजन |

---

## त्रुटि प्रबंधन

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* आउटपुट उपयोग करें */ }
    Err(TranspileError::Parse(msg))            => eprintln!("पार्स विफल: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("बहुत अधिक अनन्य शब्द: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("lossless मोड में संपीड़न"),
    Err(e)                                     => eprintln!("त्रुटि: {e}"),
}
```

---

## प्रदर्शन

रिलीज़ बिल्ड (`cargo build --release`) पर मापा गया, Apple M-series, Markdown/HTML/PlainText में 48 दस्तावेज़:

| मीट्रिक | मापा गया | नोट्स |
|---------|---------|-------|
| थ्रूपुट | **10,975 tok/ms** | Python पार्सिंग बेसलाइन से ≈75× तेज़ |
| Semantic कमी | **33.9%** (Markdown) | 15–30% लक्ष्य प्राप्त |
| Compressed कमी | **39.7%** (Markdown) | बजट-अनुकूली, ≥ PruneLowImportance गारंटीड |
| Lossless शब्द कवरेज | **98.8% औसत** | सभी फॉर्मेट और भाषाओं में |
| HTML कमी | **97.6%** | nav/स्क्रिप्ट/स्टाइल मार्कअप ओवरहेड हटाना |
| बहुभाषी समर्थन | 15 भाषाएं परीक्षित | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99.4% औसत शब्द कवरेज |

मूल्यांकन सूट स्वयं चलाएं:

```bash
cargo run --release --example eval
```

---

## योगदान

पूर्ण दिशानिर्देशों के लिए [CONTRIBUTING.md](../../CONTRIBUTING.md) देखें। PR का स्वागत है — `good first issue` लेबल वाले खुले issues देखें।

---

## लाइसेंस

Apache-2.0 — [LICENSE](../../LICENSE) देखें।
