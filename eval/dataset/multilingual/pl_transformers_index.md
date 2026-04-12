# Transformers

Transformers to najnowocześniejsza biblioteka uczenia maszynowego stworzona przez Hugging Face. Oferuje tysiące wstępnie wytrenowanych modeli do zadań związanych z tekstem, obrazem i dźwiękiem.

## Co potrafi Transformers?

Biblioteka oferuje następujące funkcje:

- **Wnioskowanie**: Ładuj wstępnie wytrenowane modele za pomocą kilku linii kodu.
- **Dostrajanie**: Adaptuj istniejące modele do swoich danych i przypadków użycia.
- **Eksport**: Konwertuj modele do ONNX, TorchScript lub TFLite dla wdrożeń produkcyjnych.

## Obsługiwane architektury

| Architektura | Główne zadania |
|---|---|
| BERT | Klasyfikacja tekstu, NER, QA |
| GPT-2 / GPT-J | Generowanie tekstu |
| T5 | Tłumaczenie, streszczanie, QA |
| ViT | Klasyfikacja obrazów |
| Whisper | Rozpoznawanie mowy |
| CLIP | Multimodalne zadania wizja-język |

## Instalacja

```bash
pip install transformers
# Z obsługą PyTorch
pip install transformers[torch]
# Z obsługą TensorFlow
pip install transformers[tf-cpu]
```

## Podstawowe użycie

### Potoki wnioskowania (Pipelines)

Najszybszym sposobem na użycie modelu są potoki:

```python
from transformers import pipeline

# Analiza sentymentu
klasyfikator = pipeline("sentiment-analysis")
wynik = klasyfikator("Uwielbiam ten produkt!")
print(wynik)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Generowanie tekstu
generator = pipeline("text-generation", model="gpt2")
tekst = generator("Uczenie maszynowe to", max_length=50)
print(tekst[0]["generated_text"])
```

### Ładowanie modelu i tokenizatora

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

nazwa_modelu = "dkleczek/bert-base-polish-cased-v1"
tokenizator = AutoTokenizer.from_pretrained(nazwa_modelu)
model = AutoModelForSequenceClassification.from_pretrained(nazwa_modelu)

wejscie = tokenizator("Cześć, jak się masz?", return_tensors="pt")
with torch.no_grad():
    wyjscie = model(**wejscie)

logits = wyjscie.logits
przewidywana_klasa = logits.argmax(-1).item()
print(model.config.id2label[przewidywana_klasa])
```

## Dostrajanie modeli (Fine-tuning)

Dostrajanie adaptuje wstępnie wytrenowany model do konkretnego zadania.

### Przy użyciu Trainer z Hugging Face

```python
from transformers import TrainingArguments, Trainer

argumenty_treningu = TrainingArguments(
    output_dir="./wyniki",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./logi",
    evaluation_strategy="epoch",
    save_strategy="epoch",
    load_best_model_at_end=True,
)

trener = Trainer(
    model=model,
    args=argumenty_treningu,
    train_dataset=zbior_treningowy,
    eval_dataset=zbior_ewaluacyjny,
    compute_metrics=oblicz_metryki,
)

trener.train()
```

## Kwantyzacja

Redukcja rozmiaru modelu i przyspieszenie wnioskowania:

```python
from transformers import BitsAndBytesConfig
import torch

config_kwantyzacji = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

model = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=config_kwantyzacji,
    device_map="auto",
)
```

## Integracja z Hugging Face Hub

```python
from huggingface_hub import login

login()  # Logowanie z tokenem

# Przesyłanie modelu do Hub
model.push_to_hub("moj-uzytkownik/moj-dostrojony-model")
tokenizator.push_to_hub("moj-uzytkownik/moj-dostrojony-model")

# Używanie prywatnego modelu
model = AutoModel.from_pretrained("moj-uzytkownik/prywatny-model")
```

## Dodatkowe zasoby

- [Oficjalna dokumentacja](https://huggingface.co/docs/transformers)
- [Kurs Hugging Face](https://huggingface.co/course)
- [Forum społeczności](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
