# Transformers

Transformers — это библиотека машинного обучения от Hugging Face, предоставляющая тысячи предобученных моделей для работы с текстом, изображениями и аудио.

## Возможности библиотеки

Библиотека предлагает следующие функции:

- **Инференс**: Загружайте предобученные модели и используйте их в несколько строк кода.
- **Дообучение**: Адаптируйте существующие модели под свои данные и задачи.
- **Экспорт**: Конвертируйте модели в ONNX, TorchScript или TFLite для развёртывания.

## Поддерживаемые архитектуры

| Архитектура | Основные задачи |
|---|---|
| BERT | Классификация текста, NER, вопросно-ответные системы |
| GPT-2 / GPT-J | Генерация текста |
| T5 | Перевод, реферирование, QA |
| ViT | Классификация изображений |
| Whisper | Распознавание речи |
| CLIP | Мультимодальные задачи |

## Установка

```bash
pip install transformers
# С поддержкой PyTorch
pip install transformers[torch]
# С поддержкой TensorFlow
pip install transformers[tf-cpu]
```

## Основы использования

### Конвейеры (Pipelines)

Самый быстрый способ начать работу — использовать конвейеры:

```python
from transformers import pipeline

# Анализ тональности текста
classifier = pipeline("sentiment-analysis")
result = classifier("Мне очень нравится этот продукт!")
print(result)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Генерация текста
generator = pipeline("text-generation", model="gpt2")
output = generator("Машинное обучение — это", max_length=50)
print(output[0]["generated_text"])
```

### Загрузка модели и токенизатора

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

model_name = "DeepPavlov/rubert-base-cased-sentence"
tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForSequenceClassification.from_pretrained(model_name)

inputs = tokenizer("Привет, как дела?", return_tensors="pt")
with torch.no_grad():
    outputs = model(**inputs)

logits = outputs.logits
predicted_class = logits.argmax(-1).item()
print(model.config.id2label[predicted_class])
```

## Дообучение модели

Дообучение позволяет адаптировать предобученную модель под конкретную задачу.

### Использование Trainer

```python
from transformers import TrainingArguments, Trainer

training_args = TrainingArguments(
    output_dir="./results",
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
    args=training_args,
    train_dataset=train_dataset,
    eval_dataset=eval_dataset,
    compute_metrics=compute_metrics,
)

trainer.train()
```

## Квантизация

Уменьшение размера модели и ускорение инференса:

```python
from transformers import BitsAndBytesConfig
import torch

quantization_config = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

model = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=quantization_config,
    device_map="auto",
)
```

## Интеграция с Hugging Face Hub

```python
from huggingface_hub import login

login()  # Авторизация с токеном

# Загрузка модели в Hub
model.push_to_hub("my-username/my-finetuned-model")
tokenizer.push_to_hub("my-username/my-finetuned-model")

# Использование приватной модели
model = AutoModel.from_pretrained("my-username/private-model")
```

## Дополнительные ресурсы

- [Официальная документация](https://huggingface.co/docs/transformers)
- [Курс Hugging Face](https://huggingface.co/course)
- [Форум сообщества](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
