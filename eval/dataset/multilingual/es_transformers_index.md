# Transformers

Transformers es una biblioteca de aprendizaje automático de última generación creada por Hugging Face. Ofrece miles de modelos preentrenados para realizar tareas en texto, visión y audio.

## ¿Qué puede hacer Transformers?

La biblioteca ofrece las siguientes funcionalidades:

- **Inferencia en producción**: Carga modelos preentrenados con unas pocas líneas de código.
- **Ajuste fino**: Adapta modelos existentes a tus datos y casos de uso específicos.
- **Exportación**: Convierte modelos a ONNX, TorchScript o TFLite para despliegue.

## Modelos compatibles

Transformers soporta arquitecturas de modelos como:

| Arquitectura | Tareas principales |
|---|---|
| BERT | Clasificación, NER, QA |
| GPT-2 / GPT-J | Generación de texto |
| T5 | Traducción, resumen, QA |
| ViT | Clasificación de imágenes |
| Whisper | Reconocimiento de voz |
| CLIP | Visión-lenguaje |

## Instalación

```bash
pip install transformers
# Con soporte para PyTorch
pip install transformers[torch]
# Con soporte para TensorFlow
pip install transformers[tf-cpu]
```

## Uso básico

### Pipeline de inferencia

La forma más rápida de usar un modelo es a través de los pipelines:

```python
from transformers import pipeline

# Clasificación de sentimientos
clasificador = pipeline("sentiment-analysis")
resultado = clasificador("Me encanta este producto")
print(resultado)
# [{'label': 'POSITIVE', 'score': 0.9998}]

# Generación de texto
generador = pipeline("text-generation", model="gpt2")
texto = generador("El aprendizaje automático es", max_length=50)
print(texto[0]["generated_text"])
```

### Carga de modelos y tokenizadores

```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

modelo_nombre = "distilbert-base-uncased-finetuned-sst-2-english"
tokenizador = AutoTokenizer.from_pretrained(modelo_nombre)
modelo = AutoModelForSequenceClassification.from_pretrained(modelo_nombre)

entradas = tokenizador("Esto es increíble", return_tensors="pt")
with torch.no_grad():
    salidas = modelo(**entradas)

logits = salidas.logits
clase_predicha = logits.argmax(-1).item()
print(modelo.config.id2label[clase_predicha])
```

## Ajuste fino (Fine-tuning)

El ajuste fino permite adaptar un modelo preentrenado a una tarea específica.

### Con el Trainer de Hugging Face

```python
from transformers import TrainingArguments, Trainer

argumentos = TrainingArguments(
    output_dir="./resultados",
    num_train_epochs=3,
    per_device_train_batch_size=16,
    per_device_eval_batch_size=64,
    warmup_steps=500,
    weight_decay=0.01,
    logging_dir="./logs",
    logging_steps=10,
    evaluation_strategy="epoch",
)

entrenador = Trainer(
    model=modelo,
    args=argumentos,
    train_dataset=dataset_entrenamiento,
    eval_dataset=dataset_evaluacion,
)

entrenador.train()
```

## Cuantización y optimización

Para reducir el tamaño del modelo y acelerar la inferencia:

```python
from transformers import BitsAndBytesConfig

config_cuant = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_compute_dtype=torch.float16,
)

modelo = AutoModelForCausalLM.from_pretrained(
    "meta-llama/Llama-2-7b-hf",
    quantization_config=config_cuant,
)
```

## Integración con Hugging Face Hub

```python
from huggingface_hub import login

login()  # Autenticarse con token

# Subir modelo al Hub
modelo.push_to_hub("mi-usuario/mi-modelo-ajustado")
tokenizador.push_to_hub("mi-usuario/mi-modelo-ajustado")

# Descargar y usar modelo privado
modelo = AutoModel.from_pretrained("mi-usuario/mi-modelo-privado")
```

## Recursos adicionales

- [Documentación oficial](https://huggingface.co/docs/transformers)
- [Curso de Hugging Face](https://huggingface.co/course)
- [Foro de la comunidad](https://discuss.huggingface.co)
- [GitHub](https://github.com/huggingface/transformers)
