# Transformers

Transformers é uma biblioteca de aprendizado de máquina de última geração criada pela Hugging Face. Ela oferece milhares de modelos pré-treinados para tarefas de texto, visão e áudio.

## O que o Transformers pode fazer?

A biblioteca oferece as seguintes funcionalidades:

- **Inferência**: Carregue modelos pré-treinados com poucas linhas de código.
- **Ajuste fino**: Adapte modelos existentes aos seus dados e casos de uso.
- **Exportação**: Converta modelos para ONNX, TorchScript ou TFLite para implantação.

## Arquiteturas suportadas

| Arquitetura | Tarefas principais |
|---|---|
| BERT | Classificação de texto, NER, QA |
| GPT-2 / GPT-J | Geração de texto |
| T5 | Tradução, resumo, QA |
| ViT | Classificação de imagens |
| Whisper | Reconhecimento de fala |
| CLIP | Visão-linguagem multimodal |

## Instalação

```bash
pip install transformers
# Com suporte a PyTorch
pip install transformers[torch]
# Com suporte a TensorFlow
pip install transformers[tf-cpu]
```

## Uso básico

### Pipelines de inferência

A maneira mais rápida de usar um modelo é através de pipelines:

```python
from transformers import pipeline

# Análise de sentimento
classificador = pipeline("sentiment-analysis")
resultado = classificador("Eu adoro este produto!")
print(resultado)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Geração de texto
gerador = pipeline("text-generation", model="gpt2")
texto = gerador("O aprendizado de máquina é", max_length=50)
print(texto[0]["generated_text"])
```

### Carregando modelos e tokenizadores

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

nome_modelo = "neuralmind/bert-base-portuguese-cased"
tokenizador = AutoTokenizer.from_pretrained(nome_modelo)
modelo = AutoModelForSequenceClassification.from_pretrained(nome_modelo)

entradas = tokenizador("Olá, como você está?", return_tensors="pt")
with torch.no_grad():
    saidas = modelo(**entradas)

logits = saidas.logits
classe_prevista = logits.argmax(-1).item()
print(modelo.config.id2label[classe_prevista])
```

## Ajuste fino (Fine-tuning)

O ajuste fino adapta um modelo pré-treinado para uma tarefa específica.

### Com o Trainer da Hugging Face

```python
from transformers import TrainingArguments, Trainer

argumentos_treino = TrainingArguments(
    output_dir="./resultados",
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

treinador = Trainer(
    model=modelo,
    args=argumentos_treino,
    train_dataset=dataset_treino,
    eval_dataset=dataset_avaliacao,
    compute_metrics=calcular_metricas,
)

treinador.train()
```

## Quantização

Para reduzir o tamanho do modelo e acelerar a inferência:

```python
from transformers import BitsAndBytesConfig
import torch

config_quantizacao = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
    bnb_4bit_use_double_quant=True,
)

modelo = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=config_quantizacao,
    device_map="auto",
)
```

## Integração com o Hugging Face Hub

```python
from huggingface_hub import login

login()  # Autenticar com token

# Enviar modelo ao Hub
modelo.push_to_hub("meu-usuario/meu-modelo-ajustado")
tokenizador.push_to_hub("meu-usuario/meu-modelo-ajustado")

# Usar modelo privado
modelo = AutoModel.from_pretrained("meu-usuario/modelo-privado")
```

## Recursos adicionais

- [Documentação oficial](https://huggingface.co/docs/transformers)
- [Curso Hugging Face](https://huggingface.co/course)
- [Fórum da comunidade](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
