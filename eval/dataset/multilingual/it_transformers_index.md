# Transformers

Transformers è una libreria di machine learning all'avanguardia creata da Hugging Face. Offre migliaia di modelli pre-addestrati per attività su testo, visione e audio.

## Cosa può fare Transformers?

La libreria offre le seguenti funzionalità:

- **Inferenza**: Carica modelli pre-addestrati con poche righe di codice.
- **Fine-tuning**: Adatta i modelli esistenti ai tuoi dati e casi d'uso.
- **Esportazione**: Converti i modelli in ONNX, TorchScript o TFLite per il deployment.

## Architetture supportate

| Architettura | Attività principali |
|---|---|
| BERT | Classificazione testo, NER, QA |
| GPT-2 / GPT-J | Generazione testo |
| T5 | Traduzione, riepilogo, QA |
| ViT | Classificazione immagini |
| Whisper | Riconoscimento vocale |
| CLIP | Visione-linguaggio multimodale |

## Installazione

```bash
pip install transformers
# Con supporto PyTorch
pip install transformers[torch]
# Con supporto TensorFlow
pip install transformers[tf-cpu]
```

## Utilizzo di base

### Pipeline di inferenza

Il modo più rapido per usare un modello è tramite le pipeline:

```python
from transformers import pipeline

# Analisi del sentimento
classificatore = pipeline("sentiment-analysis")
risultato = classificatore("Adoro questo prodotto!")
print(risultato)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Generazione testo
generatore = pipeline("text-generation", model="gpt2")
testo = generatore("Il machine learning è", max_length=50)
print(testo[0]["generated_text"])
```

### Caricamento di modelli e tokenizer

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

nome_modello = "dbmdz/bert-base-italian-cased"
tokenizer = AutoTokenizer.from_pretrained(nome_modello)
modello = AutoModelForSequenceClassification.from_pretrained(nome_modello)

input_data = tokenizer("Ciao, come stai?", return_tensors="pt")
with torch.no_grad():
    output = modello(**input_data)

logits = output.logits
classe_predetta = logits.argmax(-1).item()
print(modello.config.id2label[classe_predetta])
```

## Fine-tuning

Il fine-tuning adatta un modello pre-addestrato a un compito specifico.

### Con il Trainer di Hugging Face

```python
from transformers import TrainingArguments, Trainer

training_args = TrainingArguments(
    output_dir="./risultati",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./log",
    evaluation_strategy="epoch",
    save_strategy="epoch",
    load_best_model_at_end=True,
)

trainer = Trainer(
    model=modello,
    args=training_args,
    train_dataset=dataset_train,
    eval_dataset=dataset_eval,
    compute_metrics=calcola_metriche,
)

trainer.train()
```

## Quantizzazione

Per ridurre la dimensione del modello e accelerare l'inferenza:

```python
from transformers import BitsAndBytesConfig
import torch

config_quantizzazione = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

modello = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=config_quantizzazione,
    device_map="auto",
)
```

## Integrazione con Hugging Face Hub

```python
from huggingface_hub import login

login()  # Autenticarsi con token

# Caricare il modello sull'Hub
modello.push_to_hub("mio-username/mio-modello-finetunato")
tokenizer.push_to_hub("mio-username/mio-modello-finetunato")

# Utilizzare modello privato
modello = AutoModel.from_pretrained("mio-username/modello-privato")
```

## Risorse aggiuntive

- [Documentazione ufficiale](https://huggingface.co/docs/transformers)
- [Corso Hugging Face](https://huggingface.co/course)
- [Forum della community](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
