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

**Транспайлер документов, оптимизированный для токенов в LLM-пайплайнах**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

Исходные документы (Markdown, HTML, обычный текст) → структурированный формат моста `<D>?<H><B>` — с адаптивным сжатием для соблюдения токенного бюджета.

---

<details>
<summary>Содержание</summary>

- [Зачем использовать](#зачем-использовать)
- [Установка](#установка)
- [Обновление](#обновление)
- [Использование CLI](#использование-cli)
- [Статистика использования](#статистика-использования)
- [Использование библиотеки](#использование-библиотеки)
- [Формат вывода](#формат-вывода)
- [Уровни точности](#уровни-точности)
- [Адаптивное сжатие](#адаптивное-сжатие)
- [Форматы входных данных](#форматы-входных-данных)
- [Обработка ошибок](#обработка-ошибок)
- [Производительность](#производительность)
- [Участие в разработке](#участие-в-разработке)
- [Лицензия](#лицензия)- [Бенчмаркинг](#бенчмаркинг)

</details>

---

## Зачем использовать

LLM работают лучше, когда контекст чистый и плотный. Эта библиотека берёт на себя механическую работу:

| | Функция | Почему это важно |
|--|---------|------------------|
| 🏗️ | **Структурный разбор** | Markdown/HTML/обычный текст → типизированные IR-узлы (заголовки, абзацы, таблицы, списки, блоки кода) |
| 📉 | **Адаптивное сжатие** | Автоматически переходит через 4 стадии по мере заполнения токенного бюджета |
| 🔣 | **Замена символов** | Повторяющиеся термины предметной области → символы Unicode PUA, декодируемые заголовком словаря `<D>` |
| 📊 | **Линеаризация таблиц** | Таблицы Markdown → компактные последовательности `Key:Val` (≤5 строк) или строки через pipe для больших таблиц |
| 🌊 | **Потоковый вывод** | Поток Tokio доставляет первый блок немедленно, минимизируя TTFT |

### Бенчмарки

37 документов, 4 формата, 5 языков — Apple M-series, сборка `--release`. Полный отчёт: [`eval/EVAL_REPORT.md`](../../eval/EVAL_REPORT.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown (EN) | 29.8% | 42.0% | 99.7% | 895 tok/ms |
| Markdown (ML) | 43.1% | 43.9% | 97.3% | 3,483 tok/ms |
| HTML | 97.7% | 97.7% | 93.0% | 5,879 tok/ms |
| PlainText | 17.7% | 47.7% | 100.0% | 189 tok/ms |
| **Overall** | **79.2%** | **81.1%** | **98.4%** | **2,258 tok/ms** |

> Сокращение HTML отражает удаление избыточной разметки (навигация, скрипты, стили), а не только сжатие текста.

---

## Установка

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Бинарник и хук PostToolUse автоматически устанавливаются при следующем запуске сессии — дополнительная настройка не требуется.

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

Хук PostToolUse регистрируется автоматически — дальнейшие действия не требуются.

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

Нет Homebrew? Используйте скрипт установки:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### Через инструментальный цепочку Rust

```bash
cargo binstall llm-transpile   # готовый бинарник (быстро)
cargo install llm-transpile    # сборка из исходного кода
```

### После установки

Настройка интеграций:

```bash
transpile install
```

`transpile install` запускает интерактивный мастер, который обнаруживает и настраивает установленные инструменты:

| Инструмент | Метод интеграции | Функция |
|------------|-----------------|---------|
| **Antigravity** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |
| **Cursor** | Правило `.mdc` (`alwaysApply`) | Запускает `transpile` перед чтением документов |
| **OpenCode** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |
| **Cline** | `SKILL.md` | LLM автоматически вызывает `transpile` для документов |

Все инструменты используют файл навыка, который обучает LLM автоматически выполнять `TRANSPILE_AGENT=<agent> transpile --input <file>` — проверка размера не требуется, одного расширения файла достаточно для активации.

**Выборочная установка / удаление**

```bash
transpile install antigravity cursor    # только конкретные инструменты
transpile install --all            # всё сразу
transpile install --dry-run        # предварительный просмотр изменений
transpile install --list           # статус интеграций

transpile uninstall cursor         # удалить один
transpile uninstall --all          # удалить всё
transpile uninstall --dry-run      # предварительный просмотр удаления
```

### Библиотека (Rust-крейт)

```toml
[dependencies]
llm-transpile = "0.1"
```

Требуется **Rust 1.92+**.

### Antigravity (Gemini CLI)

```bash
agy plugins install https://github.com/epicsagas/llm-transpile
```

Автоматически устанавливает плагин (хуки) и регистрирует его при следующем запуске сессии.


### Бенчмаркинг


```bash
# Запустить бенчмарк для каталога тестовых файлов
transpile bench run --dataset ./eval                    # генерирует лог JSONL
transpile bench run --dataset ./eval --report           # запуск + открыть HTML-отчет
transpile bench report                                  # перегенерировать отчет из логов
```

HTML-отчет бенчмарка включает:

- **Карточки KPI** — semantic сокращение, compressed сокращение, пропускная способность (tok/ms), охват слов, всего входящих токенов, количество запусков
- **7 графиков** — тренд сокращения, пропускная способность по запускам, рассеивание semantic к пропускной способности, диаграмма размаха по формату, распределение форматов, гистограмма размеров токенов, диаграмма охвата слов
- **Таблица запусков** — сводка по запускам с агрегированными метриками
- **Таблица записей** — детали по каждому файлу с фильтрами по формату, запуску и имени
- **Тема** — темный/светлый режим с сохранением настроек
- **Двуязычный** — автоматическое определение корейской локали; ручной переключатель KO/EN


---

---

## Обновление

| Метод | Команда |
|-------|---------|
| Homebrew | `brew upgrade llm-transpile` |
| curl / PowerShell установщик | Повторно выполните команду установки выше |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
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

## Статистика использования

Каждый вызов `transpile` автоматически добавляет запись в `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`. Подкоманда `transpile stats` считывает эти файлы и выводит сводную таблицу.

```
transpile stats show                # за сегодня
transpile stats show --days 7       # за последние N дней
transpile stats show --agent claude # фильтр по агенту
```

Пример вывода:

```
transpile stats — за последние 7 дней

  Дата        Агент      Вызовы  Вход. токены  Выход. токены  Сэкономлено  Сокращение
  ──────────────────────────────────────────────────────────────────────────────────
  2026-04-13  claude         5      14 965        10 872        4 093       27.3%
  2026-04-13  antigravity         2       4 800         3 500        1 300       27.1%
  ──────────────────────────────────────────────────────────────────────────────────
  Итого                      7      19 765        14 372        5 393       27.3%
```

**Поля записи JSONL**

| Поле | Тип | Описание |
|------|-----|----------|
| `ts` | ISO 8601 | Временная метка вызова |
| `agent` | строка | Инструмент, инициировавший вызов (`claude`, `antigravity`, `codex`, `opencode`) |
| `file` | строка | Путь к входному файлу (пусто при чтении из stdin) |
| `format` | строка | `markdown`, `html` или `plaintext` |
| `fidelity` | строка | `lossless`, `semantic` или `compressed` |
| `input_tok` | целое | Количество токенов до транспиляции |
| `output_tok` | целое | Количество токенов после транспиляции |
| `reduction_pct` | число | Процент сэкономленных токенов |
| `saved` | целое | Абсолютное количество сэкономленных токенов (`input_tok − output_tok`) |

**Переменная окружения `TRANSPILE_AGENT`**

Поле `agent` заполняется из переменной окружения `TRANSPILE_AGENT`. Каждая интеграция устанавливает её автоматически (`claude`, `antigravity`, `codex`, `opencode`, `cursor`). Также можно задать вручную:

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## Использование библиотеки

### Синхронно

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

### Потоково (Tokio)

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

### Оценка количества токенов

```rust
let n = llm_transpiler::token_count("Hello, world!");
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
use llm_transpiler::TranspileError;

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

Детализация по файлам, методология и известные ограничения: [`eval/EVAL_REPORT.md`](../../eval/EVAL_REPORT.md)

---

## Участие в разработке

См. [CONTRIBUTING.md](../../CONTRIBUTING.md) для полных рекомендаций. PR приветствуются — см. открытые issues с меткой `good first issue`.

---

## Лицензия

Apache-2.0 — см. [LICENSE](../../LICENSE).
