# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Оптимизированный по токенам транспилятор документов для LLM-конвейеров**

Исходные документы (Markdown, HTML, обычный текст) → структурированный формат моста `<D>?<H><B>` — с адаптивным сжатием для соблюдения токенного бюджета.

```
<H>
t: Лицензионное соглашение на программное обеспечение
s: Годовые условия лицензии между лицензиаром и лицензиатом
k: [лицензия, договор, программное обеспечение]
</H>
<B>
# Стороны договора
Настоящее соглашение заключается между Лицензиаром и Лицензиатом.
...
</B>
```

---

<details>
<summary>Содержание</summary>

- [Зачем использовать](#зачем-использовать)
- [Установка](#установка)
- [Использование CLI](#использование-cli)
- [Использование библиотеки](#использование-библиотеки)
- [Формат вывода](#формат-вывода)
- [Уровни точности](#уровни-точности)
- [Адаптивное сжатие](#адаптивное-сжатие)
- [Форматы входных данных](#форматы-входных-данных)
- [Обработка ошибок](#обработка-ошибок)
- [Производительность](#производительность)
- [Участие в разработке](#участие-в-разработке)
- [Лицензия](#лицензия)
</details>

---

## Зачем использовать

LLM работают лучше, когда контекст чистый и плотный. Эта библиотека берёт на себя механическую работу:

- **Структурный разбор** — Markdown/HTML/обычный текст → типизированные IR-узлы (заголовки, абзацы, таблицы, списки, блоки кода)
- **Адаптивное сжатие** — автоматически переходит через 4 стадии по мере заполнения токенного бюджета
- **Замена символов** — повторяющиеся термины предметной области → символы Unicode PUA, декодируемые заголовком словаря `<D>`
- **Линеаризация таблиц** — таблицы Markdown → компактные последовательности `Key:Val` (≤5 строк) или строки через pipe для больших таблиц
- **Потоковый вывод** — поток Tokio доставляет первый блок немедленно, минимизируя TTFT

---

## Установка

### Библиотека (Rust-крейт)

```toml
[dependencies]
llm-transpile = "0.1"
```

Требуется **Rust 1.75+**.

### CLI-бинарник + интеграция инструментов

```bash
# Homebrew (macOS)
brew tap epicsagas/tap
brew install llm-transpile

# Готовый бинарник (быстрее, без компиляции)
cargo binstall llm-transpile

# Из crates.io
cargo install llm-transpile
```

Настройка интеграций:

```bash
transpile install
```

`transpile install` запускает интерактивный мастер, который обнаруживает и настраивает установленные инструменты:

| Инструмент | Метод интеграции | Функция |
|------------|-----------------|---------|
| **Claude Code** | Хук PostToolUse | Автоматически сжимает `.md/.html/.txt` при чтении |
| **Gemini CLI** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |
| **Codex CLI** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |
| **Cursor** | Правило `.mdc` (`alwaysApply`) | Запускает `transpile` перед чтением документов |
| **OpenCode** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |

**Выборочная установка / удаление**

```bash
transpile install claude gemini    # только конкретные инструменты
transpile install --all            # всё сразу
transpile install --dry-run        # предварительный просмотр изменений
transpile install --list           # статус интеграций

transpile uninstall cursor         # удалить один
transpile uninstall --all          # удалить всё
transpile uninstall --dry-run      # предварительный просмотр удаления
```

**Плагин Claude Code**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Из исходного кода:

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

---

## Использование CLI

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Путь к входному файлу (читает из stdin если не указан)
  -f, --format <FORMAT>    Формат входных данных: markdown | html | plaintext  [по умолчанию: markdown]
                           Определяется автоматически по расширению при использовании --input
  -l, --fidelity <LEVEL>  Уровень сжатия: lossless | semantic | compressed  [по умолчанию: semantic]
  -b, --budget <N>         Верхний предел токенного бюджета (без ограничений если не указан)
  -c, --count              Выводит только количество токенов и завершается
  -j, --json               Вывод в JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Подавляет строку статистики в stderr
      --stats              Выводит строку статистики в stdout после содержимого
  -h, --help               Показать справку
  -V, --version            Показать версию
```

**Примеры**

```bash
# Конвертировать Markdown-файл (формат определяется автоматически по .md)
transpile --input doc.md

# Читать из stdin — чистый stdout, статистика в stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Чистый pipe — полное подавление статистики
transpile --input doc.md --quiet | send_to_llm_api

# Проверить количество токенов без конвертации
transpile --input doc.md --count

# JSON-вывод для скриптов и конвейеров
transpile --input doc.md --json | jq '.reduction_pct'

# Захват содержимого + статистики в одном потоке
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — без сжатия, полное сохранение содержимого (юридические/аудиторские документы)
transpile --input contract.md --fidelity lossless

# Агрессивное сжатие до 512 токенов
transpile --input article.md --fidelity compressed --budget 512
```

> Статистика (`[273 → 150 tok  45.1% reduction]`) по умолчанию выводится в **stderr**, чтобы stdout оставался чистым для pipe. Используйте `--quiet` для подавления или `--stats` для перенаправления в stdout.

---

## Использование библиотеки

### Синхронно

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

### Потоково (Tokio)

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

### Оценка количества токенов

```rust
let n = llm_transpile::token_count("Hello, world!");
```

---

## Формат вывода

```
<D>                  ← Словарь символов (опускается при отсутствии замен)
{sym}=повторяемый-термин
</D>
<H>                  ← Заголовок метаданных в стиле YAML
t: заголовок документа
s: однострочное резюме
k: [ключевое_слово1, ключевое_слово2]
</H>
<B>                  ← Тело документа (сжато + заменено)
...содержимое...
</B>
```

Блок `<D>` использует символы зоны частного использования Unicode (`U+E000–U+F8FF`) в качестве компактных идентификаторов символов, избегая коллизий с видимыми текстовыми шаблонами. Словарь поддерживает до **6 400 уникальных терминов** на документ.

---

## Уровни точности

| Уровень | Типичный сценарий | Применяемое сжатие |
|---------|------------------|--------------------|
| `Lossless` | Юридические/аудиторские документы | Нет — оригинальное содержимое гарантировано |
| `Semantic` | Общие RAG-конвейеры | Удаление стоп-слов + отсечение по важности |
| `Compressed` | Резюмирование, жёсткий бюджет | Максимальное сжатие, извлечение первого предложения |

---

## Адаптивное сжатие

Компрессор отслеживает использование бюджета в реальном времени и автоматически эскалирует:

| Использование бюджета | Стадия | Что происходит |
|----------------------|--------|----------------|
| 0–60% | `StopwordOnly` | Стоп-слова английского/корейского удаляются |
| 60–80% | `PruneLowImportance` | Удаляется нижние 20% абзацев по важности |
| 80–95% | `DeduplicateAndLinearize` | Удаляются дублирующиеся предложения; таблицы линеаризуются |
| 95%+ | `MaxCompression` | Каждый абзац сокращается до первого предложения |

> Режим `Lossless` безусловно обходит все стадии сжатия.

При потоковой передаче, когда использование бюджета превышает 80%, оставшиеся узлы автоматически переключаются в режим `Compressed`.

---

## Форматы входных данных

| `InputFormat` | Парсер |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + GFM-таблицы |
| `Html` | санация через ammonia → удаление тегов → конвейер обычного текста |
| `PlainText` | Разбивка на абзацы по пустым строкам |

---

## Обработка ошибок

```rust
use llm_transpile::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* использовать вывод */ }
    Err(TranspileError::Parse(msg))            => eprintln!("ошибка разбора: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("слишком много уникальных терминов: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("сжатие в режиме lossless"),
    Err(e)                                     => eprintln!("ошибка: {e}"),
}
```

---

## Производительность

Измерено на сборке release (`cargo build --release`), Apple M-series, 48 документов Markdown/HTML/PlainText:

| Метрика | Измерено | Примечания |
|---------|----------|------------|
| Пропускная способность | **10 975 tok/ms** | ≈75× быстрее базовой реализации на Python |
| Сокращение Semantic | **33,9%** (Markdown) | Цель 15–30% достигнута |
| Сокращение Compressed | **39,7%** (Markdown) | Адаптивно к бюджету, ≥ PruneLowImportance гарантировано |
| Покрытие слов Lossless | **98,8% среднее** | Все форматы и языки |
| Сокращение HTML | **97,6%** | Удаление накладных расходов разметки nav/скриптов/стилей |
| Многоязычная поддержка | 15 языков протестировано | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99,4% покрытие слов в среднем |

Запустите набор тестов самостоятельно:

```bash
cargo run --release --example eval
```

---

## Участие в разработке

Приветствуются отчёты об ошибках, запросы функций и pull request'ы.

```bash
# Клонировать и собрать
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# Запустить тесты
cargo test

# Запустить бенчмарки (HTML-отчёт → target/criterion/)
cargo bench

# Линтинг и форматирование
cargo clippy -- -D warnings
cargo fmt
```

**Рекомендации**

- Поддерживать MSRV на уровне Rust 1.75 — избегать функций, введённых после этой версии.
- Новое поведение сжатия не должно влиять на режим `Lossless`.
- Каждый PR должен включать тесты для новой логики в соответствующем модуле (`ir`, `compressor`, `symbol`, `renderer`).
- Перед отправкой выполнить `cargo clippy -- -D warnings` и `cargo fmt`.

---

## Лицензия

Apache-2.0 — см. [LICENSE](LICENSE).
