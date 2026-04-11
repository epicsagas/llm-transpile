# Model Cards

## What are Model Cards?

Model cards are files that accompany the models and provide handy information. Under the hood, model cards are simple Markdown files with additional metadata. Model cards are essential for discoverability, reproducibility, and sharing! You can find a model card as the `README.md` file in any model repo.

The model card should describe:
- the model
- its intended uses & potential limitations, including biases and ethical considerations as detailed in [Mitchell, 2018](https://arxiv.org/abs/1810.03993)
- the training params and experimental info (you can embed or link to an experiment tracking platform for reference)
- which datasets were used to train your model
- the model's evaluation results

The model card template is available [here](https://github.com/huggingface/huggingface_hub/blob/main/src/huggingface_hub/templates/modelcard_template.md).

How to fill out each section of the model card is described in [the Annotated Model Card](https://huggingface.co/docs/hub/model-card-annotated).

Model Cards on the Hub have two key parts, with overlapping information:
- [Metadata](#model-card-metadata)
- [Text descriptions](#model-card-text)

## Model card metadata

A model repo will render its `README.md` as a model card. The model card is a [Markdown](https://en.wikipedia.org/wiki/Markdown) file, with a [YAML](https://en.wikipedia.org/wiki/YAML) section at the top that contains metadata about the model.

The metadata you add to the model card supports discovery and easier use of your model. For example:
* Allowing users to filter models at https://huggingface.co/models.
* Displaying the model's license.
* Adding datasets to the metadata will add a message reading `Datasets used to train:` to your model page and link the relevant datasets, if they're available on the Hub.

Dataset and language identifiers are those listed on the [Datasets](https://huggingface.co/datasets) and [Languages](https://huggingface.co/languages) pages.

### Adding metadata to your model card

There are a few different ways to add metadata to your model card including:
- Using the metadata UI
- Directly editing the YAML section of the `README.md` file
- Via the [`huggingface_hub`](https://huggingface.co/docs/huggingface_hub) Python library

#### Using the metadata UI

You can add metadata to your model card using the metadata UI. To access the metadata UI, go to the model page and click on the `Edit model card` button in the top right corner of the model card. This will open an editor showing the model card `README.md` file, as well as a UI for editing the metadata.

This UI will allow you to add key metadata to your model card and many of the fields will autocomplete based on the information you provide. Using the UI is the easiest way to add metadata to your model card, but it doesn't support all of the metadata fields. If you want to add metadata that isn't supported by the UI, you can edit the YAML section of the `README.md` file directly.

#### Editing the YAML section of the `README.md` file

You can also directly edit the YAML section of the `README.md` file. If the model card doesn't already have a YAML section, you can add one by adding three `---` at the top of the file, then include all of the relevant metadata, and close the section with another group of `---` like the example below:

```yaml
---
language:
- "List of ISO 639-1 code for your language"
- lang1
- lang2
thumbnail: "url to a thumbnail used in social sharing"
tags:
- tag1
- tag2
license: "any valid license identifier"
datasets:
- dataset1
- dataset2
base_model: "base model Hub identifier"
---
```

### Specifying a library

You can specify the supported libraries in the model card metadata section. The library will be specified in the following order of priority:

1. Specifying `library_name` in the model card (recommended if your model is not a `transformers` model):

```yaml
library_name: flair
```

2. Having a tag with the name of a library that is supported:

```yaml
tags:
- flair
```

If it's not specified, the Hub will try to automatically detect the library type.

### Specifying a base model

If your model is a fine-tune, an adapter, or a quantized version of a base model, you can specify the base model in the model card metadata section. The `base_model` field can either be a single model ID, or a list of one or more base_models (specified by their Hub identifiers).

```yaml
base_model: HuggingFaceH4/zephyr-7b-beta
```

This metadata will be used to display the base model on the model page.

For a merge of two or more models:

```yaml
base_model:
- Endevor/InfinityRP-v1-7B
- l3utterfly/mistral-7b-v0.1-layla-v4
```

The Hub will infer the type of relationship from the current model to the base model (`"adapter", "merge", "quantized", "finetune"`) but you can also set it explicitly if needed: `base_model_relation: quantized` for instance.

### Specifying a new version

If a new version of your model is available in the Hub, you can specify it in a `new_version` field.

```yaml
new_version: l3utterfly/mistral-7b-v0.1-layla-v4
```

### Specifying a dataset

You can specify the datasets used to train your model in the model card metadata section.

```yaml
datasets:
- stanfordnlp/imdb
- HuggingFaceFW/fineweb
```

### Specifying a task (`pipeline_tag`)

You can specify the `pipeline_tag` in the model card metadata. The `pipeline_tag` indicates the type of task the model is intended for. This tag will be displayed on the model page and users can filter models on the Hub by task.

For `transformers` models, the pipeline tag is automatically inferred from the model's `config.json` file but you can override it in the model card metadata if required.

### Specifying a license

You can specify the license in the model card metadata section.

If required, you can also specify a custom license by adding `other` as the license value:

```yaml
# Example from https://huggingface.co/coqui/XTTS-v1
---
license: other
license_name: coqui-public-model-license
license_link: https://coqui.ai/cpml
---
```

### Evaluation Results

You can specify your **model's evaluation results** in a structured way in the model card metadata. Here is a partial example of a model-index:

```yaml
---
model-index:
- name: Yi-34B
  results:
  - task:
      type: text-generation
    dataset:
      name: ai2_arc
      type: ai2_arc
    metrics:
    - name: AI2 Reasoning Challenge (25-Shot)
      type: AI2 Reasoning Challenge (25-Shot)
      value: 64.59
    source:
      name: Open LLM Leaderboard
      url: https://huggingface.co/spaces/open-llm-leaderboard/open_llm_leaderboard
---
```

### CO2 Emissions

The model card is also a great place to show information about the CO2 impact of your model. Visit our [guide on tracking and reporting CO2 emissions](./model-cards-co2) to learn more.

### Linking a Paper

If the model card includes a link to a Paper page (either on HF or an Arxiv abstract/PDF), the Hugging Face Hub will extract the arXiv ID and include it in the model tags with the format `arxiv:`.

## Model Card text

Details on how to fill out the human-readable portion of the model card is available in the [Annotated Model Card](./model-card-annotated).

## FAQ

### How are model tags determined?

Each model page lists all the model's tags in the page header, below the model name. These are primarily computed from the model card metadata, although some are added automatically.

### Can I add custom tags to my model?

Yes, you can add custom tags to your model by adding them to the `tags` field in the model card metadata. For example, you could indicate that your model is focused on finance by adding a `finance` tag.

### How can I indicate that my model is not suitable for all audiences?

You can add a `not-for-all-audiences` tag to your model card metadata. When this tag is present, a message will be displayed on the model page indicating that the model is not for all audiences.

### Can I write LaTeX in my model card?

Yes! The Hub uses the [KaTeX](https://katex.org/) math typesetting library to render math formulas server-side before parsing the Markdown.

You have to use the following delimiters:
- `$$ ... $$` for display mode
- `\\(...\\)` for inline mode (no space between the slashes and the parenthesis).
