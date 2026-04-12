# Transformers

Transformers هي مكتبة تعلم آلي متطورة من إنشاء Hugging Face. توفر آلاف النماذج المدربة مسبقاً لمهام النص والرؤية والصوت.

## ما الذي يمكن لـ Transformers فعله؟

تقدم المكتبة الوظائف التالية:

- **الاستدلال (Inference)**: تحميل النماذج المدربة مسبقاً ببضعة أسطر من الكود.
- **الضبط الدقيق (Fine-tuning)**: تكييف النماذج الموجودة مع بياناتك وحالات استخدامك.
- **التصدير**: تحويل النماذج إلى ONNX أو TorchScript أو TFLite للنشر.

## البنى المعمارية المدعومة

| البنية المعمارية | المهام الرئيسية |
|---|---|
| BERT | تصنيف النصوص، NER، الأسئلة والأجوبة |
| GPT-2 / GPT-J | توليد النصوص |
| T5 | الترجمة، التلخيص، الأسئلة والأجوبة |
| ViT | تصنيف الصور |
| Whisper | التعرف على الكلام |
| CLIP | متعدد الوسائط رؤية-لغة |

## التثبيت

```bash
pip install transformers
# مع دعم PyTorch
pip install transformers[torch]
# مع دعم TensorFlow
pip install transformers[tf-cpu]
```

## الاستخدام الأساسي

### خطوط أنابيب الاستدلال (Pipelines)

أسرع طريقة لاستخدام نموذج هي عبر خطوط الأنابيب:

```python
from transformers import pipeline

# تحليل المشاعر
classifier = pipeline("sentiment-analysis")
result = classifier("أحب هذا المنتج كثيراً!")
print(result)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# توليد النصوص
generator = pipeline("text-generation", model="gpt2")
text = generator("التعلم الآلي هو", max_length=50)
print(text[0]["generated_text"])
```

### تحميل النماذج والمحللات اللغوية

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

model_name = "CAMeL-Lab/bert-base-arabic-camelbert-da"
tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForSequenceClassification.from_pretrained(model_name)

inputs = tokenizer("مرحباً، كيف حالك؟", return_tensors="pt")
with torch.no_grad():
    outputs = model(**inputs)

logits = outputs.logits
predicted_class = logits.argmax(-1).item()
print(model.config.id2label[predicted_class])
```

## الضبط الدقيق (Fine-tuning)

يُكيِّف الضبط الدقيق نموذجاً مدرباً مسبقاً لمهمة محددة.

### باستخدام Trainer من Hugging Face

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

## التكميم (Quantization)

تقليل حجم النموذج وتسريع الاستدلال:

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

## التكامل مع Hugging Face Hub

```python
from huggingface_hub import login

login()  # تسجيل الدخول بالرمز المميز

# رفع النموذج إلى Hub
model.push_to_hub("my-username/my-finetuned-model")
tokenizer.push_to_hub("my-username/my-finetuned-model")

# استخدام نموذج خاص
model = AutoModel.from_pretrained("my-username/private-model")
```

## موارد إضافية

- [الوثائق الرسمية](https://huggingface.co/docs/transformers)
- [دورة Hugging Face](https://huggingface.co/course)
- [منتدى المجتمع](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
