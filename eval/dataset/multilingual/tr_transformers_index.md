# Transformers

Transformers, Hugging Face tarafından oluşturulmuş son teknoloji bir makine öğrenmesi kütüphanesidir. Metin, görüntü ve ses görevleri için binlerce önceden eğitilmiş model sunar.

## Transformers Ne Yapabilir?

Kütüphane aşağıdaki işlevleri sunar:

- **Çıkarım (Inference)**: Birkaç satır kodla önceden eğitilmiş modelleri yükleyin ve kullanın.
- **İnce Ayar (Fine-tuning)**: Mevcut modelleri kendi verilerinize ve kullanım durumlarınıza uyarlayın.
- **Dışa Aktarma**: Modelleri ONNX, TorchScript veya TFLite formatına dönüştürerek dağıtın.

## Desteklenen Mimariler

| Mimari | Ana Görevler |
|---|---|
| BERT | Metin sınıflandırma, NER, QA |
| GPT-2 / GPT-J | Metin üretimi |
| T5 | Çeviri, özetleme, QA |
| ViT | Görüntü sınıflandırma |
| Whisper | Konuşma tanıma |
| CLIP | Görüntü-dil çok modlu |

## Kurulum

```bash
pip install transformers
# PyTorch desteğiyle
pip install transformers[torch]
# TensorFlow desteğiyle
pip install transformers[tf-cpu]
```

## Temel Kullanım

### Çıkarım Boru Hatları (Pipelines)

Bir modeli kullanmanın en hızlı yolu pipeline'lar aracılığıyladır:

```python
from transformers import pipeline

# Duygu analizi
siniflandirici = pipeline("sentiment-analysis")
sonuc = siniflandirici("Bu ürünü çok sevdim!")
print(sonuc)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Metin üretimi
uretici = pipeline("text-generation", model="gpt2")
metin = uretici("Makine öğrenmesi", max_length=50)
print(metin[0]["generated_text"])
```

### Model ve Tokenizer Yükleme

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

model_adi = "dbmdz/bert-base-turkish-cased"
tokenizer = AutoTokenizer.from_pretrained(model_adi)
model = AutoModelForSequenceClassification.from_pretrained(model_adi)

girisler = tokenizer("Merhaba, nasılsın?", return_tensors="pt")
with torch.no_grad():
    cikislar = model(**girisler)

logits = cikislar.logits
tahmin_edilen_sinif = logits.argmax(-1).item()
print(model.config.id2label[tahmin_edilen_sinif])
```

## İnce Ayar (Fine-tuning)

İnce ayar, önceden eğitilmiş bir modeli belirli bir göreve uyarlar.

### Hugging Face Trainer ile

```python
from transformers import TrainingArguments, Trainer

egitim_argumanlari = TrainingArguments(
    output_dir="./sonuclar",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./loglar",
    evaluation_strategy="epoch",
    save_strategy="epoch",
    load_best_model_at_end=True,
)

egitici = Trainer(
    model=model,
    args=egitim_argumanlari,
    train_dataset=egitim_veri_seti,
    eval_dataset=degerlendrme_veri_seti,
    compute_metrics=metrikleri_hesapla,
)

egitici.train()
```

## Kuantizasyon

Model boyutunu küçültme ve çıkarım hızlandırma:

```python
from transformers import BitsAndBytesConfig
import torch

kuantizasyon_config = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

model = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=kuantizasyon_config,
    device_map="auto",
)
```

## Hugging Face Hub Entegrasyonu

```python
from huggingface_hub import login

login()  # Token ile giriş

# Modeli Hub'a yükle
model.push_to_hub("kullanici-adim/ince-ayarli-modelim")
tokenizer.push_to_hub("kullanici-adim/ince-ayarli-modelim")

# Özel model kullan
model = AutoModel.from_pretrained("kullanici-adim/ozel-model")
```

## Ek Kaynaklar

- [Resmi Belgeler](https://huggingface.co/docs/transformers)
- [Hugging Face Kursu](https://huggingface.co/course)
- [Topluluk Forumu](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
