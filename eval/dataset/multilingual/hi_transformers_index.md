# Transformers

Transformers, Hugging Face द्वारा बनाई गई एक अत्याधुनिक मशीन लर्निंग लाइब्रेरी है। यह टेक्स्ट, विज़न और ऑडियो कार्यों के लिए हजारों पूर्व-प्रशिक्षित मॉडल प्रदान करती है।

## Transformers क्या कर सकता है?

यह लाइब्रेरी निम्नलिखित कार्यक्षमताएं प्रदान करती है:

- **इन्फरेंस**: कुछ ही कोड लाइनों में पूर्व-प्रशिक्षित मॉडल लोड करें।
- **फाइन-ट्यूनिंग**: मौजूदा मॉडल को अपने डेटा और उपयोग के मामलों के अनुकूल बनाएं।
- **एक्सपोर्ट**: डिप्लॉयमेंट के लिए मॉडल को ONNX, TorchScript या TFLite में बदलें।

## समर्थित आर्किटेक्चर

| आर्किटेक्चर | मुख्य कार्य |
|---|---|
| BERT | टेक्स्ट वर्गीकरण, NER, QA |
| GPT-2 / GPT-J | टेक्स्ट जनरेशन |
| T5 | अनुवाद, सारांश, QA |
| ViT | इमेज वर्गीकरण |
| Whisper | स्पीच रेकग्निशन |
| CLIP | विज़न-लैंग्वेज मल्टीमॉडल |

## इंस्टॉलेशन

```bash
pip install transformers
# PyTorch सपोर्ट के साथ
pip install transformers[torch]
# TensorFlow सपोर्ट के साथ
pip install transformers[tf-cpu]
```

## बुनियादी उपयोग

### इन्फरेंस पाइपलाइन

मॉडल उपयोग करने का सबसे तेज़ तरीका पाइपलाइन के माध्यम से है:

```python
from transformers import pipeline

# भावना विश्लेषण
classifier = pipeline("sentiment-analysis")
result = classifier("मुझे यह उत्पाद बहुत पसंद है!")
print(result)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# टेक्स्ट जनरेशन
generator = pipeline("text-generation", model="gpt2")
text = generator("मशीन लर्निंग है", max_length=50)
print(text[0]["generated_text"])
```

### मॉडल और टोकनाइज़र लोड करना

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

model_name = "ai4bharat/indic-bert"
tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForSequenceClassification.from_pretrained(model_name)

inputs = tokenizer("नमस्ते, आप कैसे हैं?", return_tensors="pt")
with torch.no_grad():
    outputs = model(**inputs)

logits = outputs.logits
predicted_class = logits.argmax(-1).item()
print(model.config.id2label[predicted_class])
```

## फाइन-ट्यूनिंग

फाइन-ट्यूनिंग एक पूर्व-प्रशिक्षित मॉडल को किसी विशिष्ट कार्य के लिए अनुकूलित करती है।

### Hugging Face Trainer के साथ

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

## क्वांटाइज़ेशन

मॉडल का आकार कम करें और इन्फरेंस तेज़ करें:

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

## Hugging Face Hub के साथ एकीकरण

```python
from huggingface_hub import login

login()  # टोकन के साथ लॉगिन

# Hub पर मॉडल अपलोड करें
model.push_to_hub("my-username/my-finetuned-model")
tokenizer.push_to_hub("my-username/my-finetuned-model")

# प्राइवेट मॉडल उपयोग करें
model = AutoModel.from_pretrained("my-username/private-model")
```

## अतिरिक्त संसाधन

- [आधिकारिक दस्तावेज़ीकरण](https://huggingface.co/docs/transformers)
- [Hugging Face कोर्स](https://huggingface.co/course)
- [कम्युनिटी फोरम](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
