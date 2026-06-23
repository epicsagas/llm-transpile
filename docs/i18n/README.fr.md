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

**Transpilateur de documents optimisé pour les tokens des pipelines LLM**

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [हिन्दी](README.hi.md)

</div>

Documents bruts (Markdown, HTML, texte brut) → format pont structure `<D>?<H><B>` — avec compression adaptative pour rester dans le budget de tokens.

---

<details>
<summary>Table des matières</summary>
- [Pourquoi](#pourquoi)
- [Installation](#installation)
- [Mise à jour](#mise-à-jour)
- [Utilisation CLI](#utilisation-cli)
- [Statistiques d'utilisation](#statistiques-dutilisation)
- [Utilisation de la bibliothèque](#utilisation-de-la-bibliothèque)
- [Format de sortie](#format-de-sortie)
- [Niveaux de fidélité](#niveaux-de-fidélité)
- [Compression adaptative](#compression-adaptative)
- [Formats d'entrée](#formats-dentrée)
- [Gestion des erreurs](#gestion-des-erreurs)
- [Performance](#performance)
- [Contribuer](#contribuer)
- [Licence](#licence)- [Analyse de performance](#analyse-de-performance-benchmarking)

</details>

---

## Pourquoi

Les LLM fonctionnent mieux lorsque le contexte est propre et dense. Cette bibliothèque gère le travail mécanique :

| | Fonctionnalité | Pourquoi c'est important |
|--|----------------|--------------------------|
| 🏗️ | **Analyse structurelle** | Markdown/HTML/texte brut → nœuds IR typés (titres, paragraphes, tableaux, listes, blocs de code) |
| 📉 | **Compression adaptative** | Monte automatiquement en 4 étapes au fur et à mesure que le budget de tokens se remplit |
| 🔣 | **Substitution de symboles** | Termes de domaine répétés → caractères Unicode PUA, décodés par l'en-tête de dictionnaire `<D>` |
| 📊 | **Linéarisation des tableaux** | Tableaux Markdown → séquences compactes `Key:Val` (≤5 lignes) ou lignes séparées par des pipes pour les grands tableaux |
| 🌊 | **Sortie en streaming** | Le flux Tokio livre le premier bloc immédiatement, minimisant le TTFT |

### Benchmarks

37 documents, 4 formats, 5 langues — Apple M-series, build `--release`. Rapport complet : [`docs/EVALUATION.md`](../EVALUATION.md)

| Format | Semantic reduction | Compressed reduction | Lossless word coverage | Throughput |
|--------|-------------------:|--------------------:|----------------------:|-----------:|
| Markdown | 27.4% | 69.4% | 99.0% | — |
| HTML | 98.7% | 99.3% | 99.0% | — |
| PlainText | -3.5% | 30.4% | 99.0% | — |
| **Overall (BPE)** | **81.5%** | **91.8%** | **99.0%** | **~1,070 tok/ms** |

> La réduction HTML reflète la suppression du balisage superflu (nav, scripts, styles), et non uniquement la compression du texte.

---

## Installation

### Claude Code

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Auto-installe le binaire et configure le hook PostToolUse au prochain démarrage de session — aucune configuration supplémentaire requise.

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

Le hook PostToolUse est enregistré automatiquement — aucune étape supplémentaire nécessaire.

### macOS / Linux

```bash
brew install epicsagas/tap/llm-transpile
```

Pas de Homebrew ? Utilisez le script d'installation :

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

### Via la toolchain Rust

```bash
cargo binstall llm-transpile   # binaire précompilé (rapide)
cargo install llm-transpile    # compiler depuis les sources
```

### Après l'installation

Configurer les intégrations d'outils :

```bash
transpile install
```

`transpile install` lance un assistant interactif qui détecte et configure les outils installés :

| Outil | Méthode d'intégration | Fonction |
|-------|----------------------|---------|
| **Antigravity** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |
| **Cursor** | Règle `.mdc` (`alwaysApply`) | Déclenche `transpile` avant la lecture des fichiers document |
| **OpenCode** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |
| **Cline** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |

Tous les outils utilisent un fichier skill qui apprend au LLM à exécuter `TRANSPILE_AGENT=<agent> transpile --input <file>` automatiquement — aucune vérification de taille nécessaire, l'extension seule suffit pour le déclencher.

**Installation / désinstallation sélective**

```bash
transpile install antigravity cursor    # outils spécifiques uniquement
transpile install --all            # tout à la fois
transpile install --dry-run        # aperçu des changements
transpile install --list           # afficher l'état des intégrations

transpile uninstall cursor         # supprimer un outil
transpile uninstall --all          # tout supprimer
transpile uninstall --dry-run      # aperçu des suppressions
```

### Bibliothèque (crate Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requiert **Rust 1.92+**.

### Antigravity (Gemini CLI)

```bash
agy plugins install https://github.com/epicsagas/llm-transpile
```

Installe automatiquement le plugin (hooks) et l'enregistre au prochain démarrage de session.


### Analyse de performance (Benchmarking)


```bash
# Lancer les tests sur un répertoire de fichiers
transpile bench run --dataset ./eval                    # génère un journal JSONL
transpile bench run --dataset ./eval --report           # exécuter + ouvrir le rapport HTML
transpile bench report                                  # régénérer le rapport depuis les journaux
```

Le rapport HTML de benchmarking comprend :

- **Cartes KPI** — réduction sémantique, réduction compressée, débit (tok/ms), couverture de mots, total des tokens d'entrée, nombre d'exécutions
- **7 graphiques** — tendance de réduction, débit par exécution, dispersion sémantique vs débit, boîte à moustaches par format, distribution des formats, histogramme de taille de token, couverture de mots
- **Tableau des exécutions** — résumé par exécution avec métriques agrégées
- **Tableau des enregistrements** — détail par fichier avec filtre de format, exécution et nom de fichier
- **Thème** — mode sombre / clair avec préférence persistante
- **Bilingue** — auto-détecte la locale coréenne ; bascule manuelle KO/EN


---

---

## Mise à jour

| Méthode | Commande |
|---------|----------|
| Homebrew | `brew upgrade llm-transpile` |
| Installateur curl / PowerShell | Relancer la commande d'installation ci-dessus |
| cargo binstall | `cargo binstall llm-transpile@latest` |
| cargo install | `cargo install llm-transpile@latest` |

```bash
transpile --version
```

---

## Utilisation CLI

```
transpile [OPTIONS]

Options:
  -i, --input <FILE>       Chemin du fichier d'entrée (lit depuis stdin si omis)
  -f, --format <FORMAT>    Format d'entrée: markdown | html | plaintext  [défaut: markdown]
                           Détecté automatiquement depuis l'extension avec --input
  -l, --fidelity <LEVEL>  Niveau de compression: lossless | semantic | compressed  [défaut: semantic]
  -b, --budget <N>         Limite supérieure du budget de tokens (illimité si omis)
  -c, --count              Affiche uniquement le nombre de tokens d'entrée puis quitte
  -j, --json               Sortie en JSON {input_tok, output_tok, reduction_pct, content}
  -q, --quiet              Supprime la ligne de statistiques sur stderr
      --stats              Affiche la ligne de stats sur stdout après le contenu
  -h, --help               Afficher l'aide
  -V, --version            Afficher la version
```

**Exemples**

```bash
# Convertir un fichier Markdown (format détecté automatiquement via .md)
transpile --input doc.md

# Lire depuis stdin — stdout propre, stats sur stderr
cat doc.html | transpile --format html --fidelity compressed --budget 1024

# Pipe propre — supprimer les stats complètement
transpile --input doc.md --quiet | send_to_llm_api

# Vérifier le nombre de tokens sans convertir
transpile --input doc.md --count

# Sortie JSON pour scripts et pipelines
transpile --input doc.md --json | jq '.reduction_pct'

# Capturer contenu + stats dans un flux unique
transpile --input doc.md --stats > output_with_stats.txt

# Lossless — sans compression, contenu complet préservé (documents légaux/audit)
transpile --input contract.md --fidelity lossless

# Compression agressive dans un budget de 512 tokens
transpile --input article.md --fidelity compressed --budget 512
```

> Les statistiques (`[273 → 150 tok  45.1% reduction]`) sont écrites sur **stderr** par défaut, gardant stdout propre pour les pipes. Utilisez `--quiet` pour les supprimer, ou `--stats` pour les rediriger vers stdout.

---

## Statistiques d'utilisation

Chaque invocation de `transpile` ajoute automatiquement un enregistrement à `~/.agents/transpile/stats/YYYY-MM-DD.jsonl`. La sous-commande `transpile stats` lit ces fichiers et affiche un tableau récapitulatif.

```
transpile stats show                # aujourd'hui
transpile stats show --days 7       # N derniers jours
transpile stats show --agent claude # filtrer par agent
```

Exemple de sortie :

```
transpile stats — 7 derniers jours

  Date        Agent      Appels  Tokens entrée  Tokens sortie  Économisés  Réduction
  ──────────────────────────────────────────────────────────────────────────────────
  2026-04-13  claude        5      14 965          10 872       4 093      27.3%
  2026-04-13  antigravity        2       4 800           3 500       1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────────────
  Total                     7      19 765          14 372       5 393      27.3%
```

**Champs de l'enregistrement JSONL**

| Champ | Type | Description |
|-------|------|-------------|
| `ts` | ISO 8601 | Horodatage de l'invocation |
| `agent` | string | Outil ayant déclenché l'appel (`claude`, `antigravity`, `codex`, `opencode`) |
| `file` | string | Chemin du fichier d'entrée (vide si lecture depuis stdin) |
| `format` | string | `markdown`, `html`, ou `plaintext` |
| `fidelity` | string | `lossless`, `semantic`, ou `compressed` |
| `input_tok` | integer | Nombre de tokens avant transpilation |
| `output_tok` | integer | Nombre de tokens après transpilation |
| `reduction_pct` | float | Pourcentage de tokens économisés |
| `saved` | integer | Tokens économisés en valeur absolue (`input_tok − output_tok`) |

**Variable d'environnement `TRANSPILE_AGENT`**

Le champ `agent` est renseigné depuis la variable d'environnement `TRANSPILE_AGENT`. Chaque intégration la définit automatiquement (`claude`, `antigravity`, `codex`, `opencode`, `cursor`). Vous pouvez aussi la définir manuellement :

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

---

## Utilisation de la bibliothèque

### Synchrone

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

### Estimation du nombre de tokens

```rust
let n = llm_transpiler::token_count("Hello, world!");
```

---

## Format de sortie

```
<D>                  ← Dictionnaire de symboles (omis sans substitutions)
{sym}=terme-répété
</D>
<H>                  ← En-tête de métadonnées type YAML
t: titre du document
s: résumé en une ligne
k: [motclé1, motclé2]
</H>
<B>                  ← Corps du document (compressé + substitué)
...contenu...
</B>
```

Le bloc `<D>` utilise des caractères Unicode de la zone d'usage privé (`U+E000–U+F8FF`) comme identifiants de symboles compacts, évitant les collisions avec les patterns de texte visible. Le dictionnaire supporte jusqu'à **6 400 termes uniques** par document.

---

## Niveaux de fidélité

| Niveau | Cas d'usage typique | Compression appliquée |
|--------|--------------------|-----------------------|
| `Lossless` | Documents légaux/audit | Aucune — contenu original garanti |
| `Semantic` | Pipelines RAG généraux | Suppression des mots vides + élagage par importance |
| `Compressed` | Résumé, budgets serrés | Compression maximale, extraction de la première phrase |

---

## Compression adaptative

Le compresseur surveille l'utilisation du budget en temps réel et monte automatiquement en puissance :

| Utilisation du budget | Étape | Ce qui se passe |
|----------------------|-------|-----------------|
| 0–60% | `StopwordOnly` | Mots vides anglais/coréens supprimés |
| 60–80% | `PruneLowImportance` | 20% des paragraphes les moins importants supprimés |
| 80–95% | `DeduplicateAndLinearize` | Phrases dupliquées supprimées ; tableaux linéarisés |
| 95%+ | `MaxCompression` | Chaque paragraphe tronqué à la première phrase |

> Le mode `Lossless` contourne inconditionnellement toutes les étapes de compression.

En streaming, lorsque l'utilisation du budget dépasse 80%, les nœuds restants passent automatiquement en mode `Compressed`.

---

## Formats d'entrée

| `InputFormat` | Analyseur |
|---|---|
| `Markdown` | [pulldown-cmark](https://crates.io/crates/pulldown-cmark) — CommonMark + tableaux GFM |
| `Html` | assainissement ammonia → suppression des balises → pipeline texte brut |
| `PlainText` | Découpage des paragraphes par ligne vide |

---

## Gestion des erreurs

```rust
use llm_transpiler::TranspileError;

match transpile(input, format, fidelity, budget) {
    Ok(output) => { /* utiliser output */ }
    Err(TranspileError::Parse(msg))            => eprintln!("échec d'analyse: {msg}"),
    Err(TranspileError::SymbolOverflow(e))     => eprintln!("trop de termes uniques: {e}"),
    Err(TranspileError::LosslessModeViolation) => eprintln!("compression en mode lossless"),
    Err(e)                                     => eprintln!("erreur: {e}"),
}
```

---

## Performance

Mesuré sur build release (`cargo build --release`), Apple M-series, 48 documents Markdown/HTML/PlainText :

| Métrique | Mesuré | Notes |
|----------|--------|-------|
| Débit | **10 975 tok/ms** | ≈75× plus rapide que la ligne de base Python |
| Réduction Semantic | **33,9%** (Markdown) | Objectif 15–30% atteint |
| Réduction Compressed | **39,7%** (Markdown) | Adaptatif au budget, ≥ PruneLowImportance garanti |
| Couverture de mots Lossless | **98,8% en moyenne** | Tous formats et langues confondus |
| Réduction HTML | **97,6%** | Suppression de l'overhead de balisage nav/scripts/styles |
| Support multilingue | 15 langues testées | AR/DE/ES/FR/HI/IT/JA/KO/NL/PL/PT/RU/SV/TR/ZH — 99,4% couverture moyenne |

Exécutez vous-même la suite d'évaluation :

```bash
cargo run --release --example eval
```

Détail par fichier, méthodologie et limitations connues : [`docs/EVALUATION.md`](../EVALUATION.md)

---

## Contribuer

Consultez [CONTRIBUTING.md](../../CONTRIBUTING.md) pour les directives complètes. Les PR sont les bienvenus — consultez les issues ouverts étiquetés `good first issue`.

---

## Licence

Apache-2.0 — voir [LICENSE](../../LICENSE).
