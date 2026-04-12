# Transformers

Transformers is een geavanceerde machine learning-bibliotheek van Hugging Face. De bibliotheek biedt duizenden voorgetrainde modellen voor tekst-, beeld- en audiotaken.

## Wat kan Transformers doen?

De bibliotheek biedt de volgende functionaliteit:

- **Inferentie**: Laad voorgetrainde modellen met slechts enkele regels code.
- **Fine-tuning**: Pas bestaande modellen aan voor uw eigen data en use cases.
- **Export**: Converteer modellen naar ONNX, TorchScript of TFLite voor implementatie.

## Ondersteunde architecturen

| Architectuur | Hoofdtaken |
|---|---|
| BERT | Tekstclassificatie, NER, QA |
| GPT-2 / GPT-J | Tekstgeneratie |
| T5 | Vertaling, samenvatting, QA |
| ViT | Beeldclassificatie |
| Whisper | Spraakherkenning |
| CLIP | Beeld-taal multimodaal |

## Installatie

```bash
pip install transformers
# Met PyTorch-ondersteuning
pip install transformers[torch]
# Met TensorFlow-ondersteuning
pip install transformers[tf-cpu]
```

## Basisgebruik

### Inferentiepipelines

De snelste manier om een model te gebruiken is via pipelines:

```python
from transformers import pipeline

# Sentimentanalyse
classificator = pipeline("sentiment-analysis")
resultaat = classificator("Ik vind dit product geweldig!")
print(resultaat)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Tekstgeneratie
generator = pipeline("text-generation", model="gpt2")
tekst = generator("Machine learning is", max_length=50)
print(tekst[0]["generated_text"])
```

### Model en tokenizer laden

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

modelnaam = "GroNLP/bert-base-dutch-cased"
tokenizer = AutoTokenizer.from_pretrained(modelnaam)
model = AutoModelForSequenceClassification.from_pretrained(modelnaam)

invoer = tokenizer("Hallo, hoe gaat het?", return_tensors="pt")
with torch.no_grad():
    uitvoer = model(**invoer)

logits = uitvoer.logits
voorspelde_klasse = logits.argmax(-1).item()
print(model.config.id2label[voorspelde_klasse])
```

## Fine-tuning

Fine-tuning past een voorgetraind model aan voor een specifieke taak.

### Met de Trainer van Hugging Face

```python
from transformers import TrainingArguments, Trainer

trainingsargumenten = TrainingArguments(
    output_dir="./resultaten",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./logs",
    evaluation_strategy="epoch",
    save_strategy="epoch",
    load_best_model_at_end=True,
)

trainer = Trainer(
    model=model,
    args=trainingsargumenten,
    train_dataset=trainingsdataset,
    eval_dataset=evaluatiedataset,
    compute_metrics=bereken_metrics,
)

trainer.train()
```

## Kwantisatie

Verminder de modelgrootte en versnel inferentie:

```python
from transformers import BitsAndBytesConfig
import torch

kwantisatieconfig = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

model = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=kwantisatieconfig,
    device_map="auto",
)
```

## Integratie met Hugging Face Hub

```python
from huggingface_hub import login

login()  # Inloggen met token

# Model uploaden naar Hub
model.push_to_hub("mijn-gebruiker/mijn-finetuned-model")
tokenizer.push_to_hub("mijn-gebruiker/mijn-finetuned-model")

# Privémodel gebruiken
model = AutoModel.from_pretrained("mijn-gebruiker/privémodel")
```

## Aanvullende bronnen

- [Officiële documentatie](https://huggingface.co/docs/transformers)
- [Hugging Face cursus](https://huggingface.co/course)
- [Communityforum](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
