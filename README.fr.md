# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Transpilateur de documents optimisé en tokens pour les pipelines LLM**

Documents bruts (Markdown, HTML, texte brut) → format pont structuré `<D>?<H><B>` — avec compression adaptative pour rester dans le budget de tokens.

```
<H>
t: Contrat de Licence Logicielle
s: Conditions de licence annuelles entre donneur et preneur de licence
k: [licence, contrat, logiciel]
</H>
<B>
# Parties contractantes
Le présent accord est conclu entre le Donneur de licence et le Preneur de licence.
...
</B>
```

---

## Table des matières

- [Pourquoi](#pourquoi)
- [Installation](#installation)
- [Utilisation CLI](#utilisation-cli)
- [Utilisation de la bibliothèque](#utilisation-de-la-bibliothèque)
- [Format de sortie](#format-de-sortie)
- [Niveaux de fidélité](#niveaux-de-fidélité)
- [Compression adaptative](#compression-adaptative)
- [Formats d'entrée](#formats-dentrée)
- [Gestion des erreurs](#gestion-des-erreurs)
- [Performance](#performance)
- [Contribuer](#contribuer)
- [Licence](#licence)

---

## Pourquoi

Les LLM fonctionnent mieux lorsque le contexte est propre et dense. Cette bibliothèque gère le travail mécanique :

- **Analyse structurelle** — Markdown/HTML/texte brut → nœuds IR typés (titres, paragraphes, tableaux, listes, blocs de code)
- **Compression adaptative** — monte automatiquement en 4 étapes au fur et à mesure que le budget de tokens se remplit
- **Substitution de symboles** — termes de domaine répétés → caractères Unicode PUA, décodés par l'en-tête de dictionnaire `<D>`
- **Linéarisation des tableaux** — tableaux Markdown → séquences compactes `Key:Val` (≤5 lignes) ou lignes séparées par des pipes pour les grands tableaux
- **Sortie en streaming** — le flux Tokio livre le premier bloc immédiatement, minimisant le TTFT

---

## Installation

### Bibliothèque (crate Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requiert **Rust 1.75+**.

### Binaire CLI + intégration d'outils

```bash
# Homebrew (macOS)
brew install epicsagas/tap/llm-transpile

# Binaire précompilé (plus rapide, sans compilation)
cargo binstall llm-transpile

# Depuis crates.io
cargo install llm-transpile
```

Configurer les intégrations d'outils :

```bash
transpile install
```

`transpile install` lance un assistant interactif qui détecte et configure les outils installés :

| Outil | Méthode d'intégration | Fonction |
|-------|----------------------|---------|
| **Claude Code** | Hook PostToolUse | Auto-compresse les fichiers `.md/.html/.txt` à la lecture |
| **Gemini CLI** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |
| **Codex CLI** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |
| **Cursor** | Règle `.mdc` (`alwaysApply`) | Déclenche `transpile` avant la lecture des fichiers document |
| **OpenCode** | `SKILL.md` | Le LLM invoque automatiquement `transpile` sur les extensions de fichier |

**Installation / désinstallation sélective**

```bash
transpile install claude gemini    # outils spécifiques uniquement
transpile install --all            # tout à la fois
transpile install --dry-run        # aperçu des changements
transpile install --list           # afficher l'état des intégrations

transpile uninstall cursor         # supprimer un outil
transpile uninstall --all          # tout supprimer
transpile uninstall --dry-run      # aperçu des suppressions
```

**Plugin Claude Code** (alternative — nécessite Claude Code avec support de plugins)

```
/plugin marketplace add epicsagas/claude-plugins
/plugin install transpile@epicsagas
```

Depuis les sources :

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
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

## Utilisation de la bibliothèque

### Synchrone

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

### Streaming (Tokio)

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

### Estimation du nombre de tokens

```rust
let n = llm_transpile::token_count("Hello, world!");
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
use llm_transpile::TranspileError;

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

---

## Contribuer

Les rapports de bugs, demandes de fonctionnalités et pull requests sont les bienvenus.

```bash
# Cloner et compiler
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo build

# Exécuter les tests
cargo test

# Exécuter les benchmarks (rapport HTML → target/criterion/)
cargo bench

# Lint et formatage
cargo clippy -- -D warnings
cargo fmt
```

**Directives**

- Maintenir le MSRV à Rust 1.75 — éviter les fonctionnalités introduites après.
- Les nouveaux comportements de compression ne doivent pas affecter le mode `Lossless`.
- Chaque PR doit inclure des tests pour toute nouvelle logique dans le module concerné (`ir`, `compressor`, `symbol`, `renderer`).
- Exécuter `cargo clippy -- -D warnings` et `cargo fmt` avant de soumettre.

---

## Licence

Apache-2.0 — voir [LICENSE](LICENSE).
