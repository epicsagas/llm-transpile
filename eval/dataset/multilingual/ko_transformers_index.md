# Transformers

Transformers는 Hugging Face에서 만든 최첨단 머신러닝 라이브러리입니다. 텍스트, 비전, 오디오 작업을 위한 수천 개의 사전학습 모델을 제공합니다.

## Transformers로 할 수 있는 일

이 라이브러리는 다음과 같은 기능을 제공합니다.

- **추론 (Inference)**: 몇 줄의 코드로 사전학습 모델을 즉시 사용할 수 있습니다.
- **파인튜닝 (Fine-tuning)**: 기존 모델을 자신의 데이터와 목적에 맞게 조정합니다.
- **내보내기 (Export)**: ONNX, TorchScript, TFLite 형식으로 변환하여 배포합니다.

## 지원하는 모델 아키텍처

| 아키텍처 | 주요 작업 |
|---|---|
| BERT | 텍스트 분류, NER, 질의응답 |
| GPT-2 / GPT-J | 텍스트 생성 |
| T5 | 번역, 요약, 질의응답 |
| ViT | 이미지 분류 |
| Whisper | 음성 인식 |
| CLIP | 비전-언어 멀티모달 |

## 설치

```bash
pip install transformers
# PyTorch 지원
pip install transformers[torch]
# TensorFlow 지원
pip install transformers[tf-cpu]
```

## 기본 사용법

### 파이프라인 추론

가장 빠른 사용 방법은 pipeline을 통해서입니다.

```python
from transformers import pipeline

# 감성 분류
classifier = pipeline("sentiment-analysis")
result = classifier("이 제품이 정말 마음에 들어요")
print(result)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# 텍스트 생성
generator = pipeline("text-generation", model="gpt2")
output = generator("머신러닝은", max_length=50)
print(output[0]["generated_text"])
```

### 모델과 토크나이저 직접 로드

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

model_name = "klue/bert-base"
tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForSequenceClassification.from_pretrained(model_name)

inputs = tokenizer("안녕하세요, 반갑습니다.", return_tensors="pt")
with torch.no_grad():
    outputs = model(**inputs)

logits = outputs.logits
predicted_class = logits.argmax(-1).item()
print(model.config.id2label[predicted_class])
```

## 파인튜닝

파인튜닝은 사전학습된 모델을 특정 태스크에 맞게 추가 학습하는 과정입니다.

### Trainer API 사용

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

## 양자화 (Quantization)

모델 크기를 줄이고 추론 속도를 높이기 위한 양자화:

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

## LoRA를 활용한 효율적인 파인튜닝 (PEFT)

```python
from peft import LoraConfig, get_peft_model

lora_config = LoraConfig(
    r=16,
    lora_alpha=32,
    target_modules=["q_proj", "v_proj"],
    lora_dropout=0.05,
    bias="none",
    task_type="CAUSAL_LM",
)

model = get_peft_model(model, lora_config)
model.print_trainable_parameters()
# 학습 가능한 파라미터: 4,194,304 (전체의 0.06%)
```

## Hugging Face Hub 연동

```python
from huggingface_hub import login

login()  # 토큰으로 로그인

# Hub에 모델 업로드
model.push_to_hub("my-username/my-finetuned-model")
tokenizer.push_to_hub("my-username/my-finetuned-model")

# 사설 모델 사용
model = AutoModel.from_pretrained("my-username/private-model")
```

## 주요 자료

- [공식 문서](https://huggingface.co/docs/transformers)
- [Hugging Face 강좌](https://huggingface.co/course)
- [커뮤니티 포럼](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
