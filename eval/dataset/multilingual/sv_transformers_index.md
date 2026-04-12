# Transformers

Transformers är ett toppmodernt maskininlärningsbibliotek skapat av Hugging Face. Det erbjuder tusentals förtränade modeller för text-, bild- och ljuduppgifter.

## Vad kan Transformers göra?

Biblioteket erbjuder följande funktioner:

- **Inferens**: Ladda förtränade modeller med bara några rader kod.
- **Finjustering**: Anpassa befintliga modeller till dina egna data och användningsfall.
- **Export**: Konvertera modeller till ONNX, TorchScript eller TFLite för driftsättning.

## Stödda arkitekturer

| Arkitektur | Huvuduppgifter |
|---|---|
| BERT | Textklassificering, NER, QA |
| GPT-2 / GPT-J | Textgenerering |
| T5 | Översättning, sammanfattning, QA |
| ViT | Bildklassificering |
| Whisper | Taligenkänning |
| CLIP | Multimodal vision-språk |

## Installation

```bash
pip install transformers
# Med PyTorch-stöd
pip install transformers[torch]
# Med TensorFlow-stöd
pip install transformers[tf-cpu]
```

## Grundläggande användning

### Inferenspipelines

Det snabbaste sättet att använda en modell är via pipelines:

```python
from transformers import pipeline

# Sentimentanalys
klassificerare = pipeline("sentiment-analysis")
resultat = klassificerare("Jag älskar den här produkten!")
print(resultat)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Textgenerering
generator = pipeline("text-generation", model="gpt2")
text = generator("Maskininlärning är", max_length=50)
print(text[0]["generated_text"])
```

### Ladda modell och tokenizer

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

modellnamn = "KB/bert-base-swedish-cased"
tokenizer = AutoTokenizer.from_pretrained(modellnamn)
modell = AutoModelForSequenceClassification.from_pretrained(modellnamn)

indata = tokenizer("Hej, hur mår du?", return_tensors="pt")
with torch.no_grad():
    utdata = modell(**indata)

logits = utdata.logits
förutsagd_klass = logits.argmax(-1).item()
print(modell.config.id2label[förutsagd_klass])
```

## Finjustering (Fine-tuning)

Finjustering anpassar en förtränad modell för en specifik uppgift.

### Med Hugging Face Trainer

```python
from transformers import TrainingArguments, Trainer

träningsargument = TrainingArguments(
    output_dir="./resultat",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./loggar",
    evaluation_strategy="epoch",
    save_strategy="epoch",
    load_best_model_at_end=True,
)

tränare = Trainer(
    model=modell,
    args=träningsargument,
    train_dataset=träningsdataset,
    eval_dataset=utvärderingsdataset,
    compute_metrics=beräkna_metriker,
)

tränare.train()
```

## Kvantisering

Minska modellstorleken och påskynda inferens:

```python
from transformers import BitsAndBytesConfig
import torch

kvantiseringsconfig = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

modell = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=kvantiseringsconfig,
    device_map="auto",
)
```

## Integration med Hugging Face Hub

```python
from huggingface_hub import login

login()  # Logga in med token

# Ladda upp modell till Hub
modell.push_to_hub("mitt-användarnamn/min-finjusterade-modell")
tokenizer.push_to_hub("mitt-användarnamn/min-finjusterade-modell")

# Använd privat modell
modell = AutoModel.from_pretrained("mitt-användarnamn/privat-modell")
```

## Ytterligare resurser

- [Officiell dokumentation](https://huggingface.co/docs/transformers)
- [Hugging Face-kurs](https://huggingface.co/course)
- [Communityforum](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
