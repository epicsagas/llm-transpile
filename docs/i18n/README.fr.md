# llm-transpile

[![Crates.io](https://img.shields.io/crates/v/llm-transpile.svg)](https://crates.io/crates/llm-transpile)
[![docs.rs](https://docs.rs/llm-transpile/badge.svg)](https://docs.rs/llm-transpile)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Transpilateur de documents optimise en tokens pour les pipelines LLM**

Documents bruts (Markdown, HTML, texte brut) → format pont structure `<D>?<H><B>` — avec compression adaptative pour rester dans le budget de tokens.

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
- [Licence](#licence)
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

---

## Installation

### Bibliothèque (crate Rust)

```toml
[dependencies]
llm-transpile = "0.1"
```

Requiert **Rust 1.92+**.

### Binaire CLI + intégration d'outils

**macOS / Linux**

```bash
brew install epicsagas/tap/llm-transpile
```

Pas de Homebrew ? Utilisez le script d'installation :

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/llm-transpile/releases/latest/download/install.sh | sh
```

**Windows**

```powershell
irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
```

**Via la toolchain Rust**

```bash
cargo binstall llm-transpile   # binaire précompilé (rapide)
cargo install llm-transpile    # compiler depuis les sources
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

Tous les outils autres que Claude utilisent un fichier skill qui apprend au LLM à exécuter `TRANSPILE_AGENT=<agent> transpile --input <file>` automatiquement — aucune vérification de taille nécessaire, l'extension seule suffit pour le déclencher.

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

**Plugin Claude Code**

```
/plugin marketplace add epicsagas/plugins
/plugin install transpile@epicsagas
```

Auto-installe le binaire et configure le hook PostToolUse au prochain démarrage de session — aucune configuration supplémentaire requise.

Depuis les sources :

```bash
git clone https://github.com/epicsagas/llm-transpile
cd llm-transpile
cargo install --path .
transpile install
```

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
transpile stats                # aujourd'hui
transpile stats --days 7       # N derniers jours
transpile stats --agent claude # filtrer par agent
```

Exemple de sortie :

```
transpile stats — 7 derniers jours

  Date        Agent      Appels  Tokens entrée  Tokens sortie  Économisés  Réduction
  ──────────────────────────────────────────────────────────────────────────────────
  2026-04-13  claude        5      14 965          10 872       4 093      27.3%
  2026-04-13  gemini        2       4 800           3 500       1 300      27.1%
  ──────────────────────────────────────────────────────────────────────────────────
  Total                     7      19 765          14 372       5 393      27.3%
```

**Champs de l'enregistrement JSONL**

| Champ | Type | Description |
|-------|------|-------------|
| `ts` | ISO 8601 | Horodatage de l'invocation |
| `agent` | string | Outil ayant déclenché l'appel (`claude`, `gemini`, `codex`, `opencode`) |
| `file` | string | Chemin du fichier d'entrée (vide si lecture depuis stdin) |
| `format` | string | `markdown`, `html`, ou `plaintext` |
| `fidelity` | string | `lossless`, `semantic`, ou `compressed` |
| `input_tok` | integer | Nombre de tokens avant transpilation |
| `output_tok` | integer | Nombre de tokens après transpilation |
| `reduction_pct` | float | Pourcentage de tokens économisés |
| `saved` | integer | Tokens économisés en valeur absolue (`input_tok − output_tok`) |

**Variable d'environnement `TRANSPILE_AGENT`**

Le champ `agent` est renseigné depuis la variable d'environnement `TRANSPILE_AGENT`. Chaque intégration la définit automatiquement (`claude`, `gemini`, `codex`, `opencode`, `cursor`). Vous pouvez aussi la définir manuellement :

```bash
TRANSPILE_AGENT=claude transpile --input doc.md
```

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

Consultez [CONTRIBUTING.md](../../CONTRIBUTING.md) pour les directives complètes. Les PR sont les bienvenus — consultez les issues ouverts étiquetés `good first issue`.

---

## Licence

Apache-2.0 — voir [LICENSE](LICENSE).
