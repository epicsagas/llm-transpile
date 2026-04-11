# Pipeline

The [`Pipeline`] is a simple but powerful inference API that is readily available for a variety of machine learning tasks with any model from the Hugging Face [Hub](https://hf.co/models).

Tailor the [`Pipeline`] to your task with task specific parameters such as adding timestamps to an automatic speech recognition (ASR) pipeline for transcribing meeting notes. [`Pipeline`] supports GPUs, Apple Silicon, and half-precision weights to accelerate inference and save memory.

Transformers has two pipeline classes, a generic [`Pipeline`] and many individual task-specific pipelines like [`TextGenerationPipeline`]. Load these individual pipelines by setting the task identifier in the `task` parameter in [`Pipeline`]. You can find the task identifier for each pipeline in their API documentation.

Each task is configured to use a default pretrained model and preprocessor, but this can be overridden with the `model` parameter if you want to use a different model.

For example, to use the [`TextGenerationPipeline`] with [Gemma 2](./model_doc/gemma2), set `task="text-generation"` and `model="google/gemma-2-2b"`.

```py
from transformers import pipeline

pipeline = pipeline(task="text-generation", model="google/gemma-2-2b")
pipeline("the secret to baking a really good cake is ")
[{'generated_text': 'the secret to baking a really good cake is 1. the right ingredients 2. the'}]
```

When you have more than one input, pass them as a list.

```py
from transformers import pipeline
from accelerate import Accelerator

device = Accelerator().device
pipeline = pipeline(task="text-generation", model="google/gemma-2-2b", device=device)
pipeline(["the secret to baking a really good cake is ", "a baguette is "])
[[{'generated_text': 'the secret to baking a really good cake is 1. the right ingredients 2. the'}],
 [{'generated_text': 'a baguette is 100% bread.\n\na baguette is 100%'}]]
```

This guide will introduce you to the [`Pipeline`], demonstrate its features, and show how to configure its various parameters.

## Tasks

[`Pipeline`] is compatible with many machine learning tasks across different modalities. Pass an appropriate input to the pipeline and it will handle the rest.

Here are some examples of how to use [`Pipeline`] for different tasks and modalities.

```py
from transformers import pipeline

pipeline = pipeline(task="automatic-speech-recognition", model="openai/whisper-large-v3")
pipeline("https://huggingface.co/datasets/Narsil/asr_dummy/resolve/main/mlk.flac")
{'text': ' I have a dream that one day this nation will rise up and live out the true meaning of its creed.'}
```

```py
from transformers import pipeline

pipeline = pipeline(task="image-classification", model="google/vit-base-patch16-224")
pipeline(images="https://huggingface.co/datasets/huggingface/documentation-images/resolve/main/pipeline-cat-chonk.jpeg")
[{'label': 'lynx, catamount', 'score': 0.43350091576576233},
 {'label': 'cougar, puma, catamount, mountain lion, painter, panther, Felis concolor', 'score': 0.034796204417943954},
 {'label': 'snow leopard, ounce, Panthera uncia', 'score': 0.03240183740854263},
 {'label': 'Egyptian cat', 'score': 0.02394474856555462},
 {'label': 'tiger cat', 'score': 0.02288915030658245}]
```

```py
from transformers import pipeline

pipeline = pipeline(task="visual-question-answering", model="Salesforce/blip-vqa-base")
pipeline(
    image="https://huggingface.co/datasets/huggingface/documentation-images/resolve/main/transformers/tasks/idefics-few-shot.jpg",
    question="What is in the image?",
)
[{'answer': 'statue of liberty'}]
```

## Parameters

At a minimum, [`Pipeline`] only requires a task identifier, model, and the appropriate input. But there are many parameters available to configure the pipeline with, from task-specific parameters to optimizing performance.

### Device

[`Pipeline`] is compatible with many hardware types, including GPUs, CPUs, Apple Silicon, and more. Configure the hardware type with the `device` parameter. By default, [`Pipeline`] runs on a CPU which is given by `device=-1`.

To run [`Pipeline`] on a GPU, set `device` to the associated CUDA device id. For example, `device=0` runs on the first GPU.

```py
from transformers import pipeline

pipeline = pipeline(task="text-generation", model="google/gemma-2-2b", device=0)
pipeline("the secret to baking a really good cake is ")
```

You could also let [Accelerate](https://hf.co/docs/accelerate/index) automatically choose how to load and store the model weights on the appropriate device. Set `device_map="auto"` to let Accelerate choose the device.

```py
from transformers import pipeline

pipeline = pipeline(task="text-generation", model="google/gemma-2-2b", device_map="auto")
pipeline("the secret to baking a really good cake is ")
```

To run [`Pipeline`] on Apple silicon, set `device="mps"`.

```py
from transformers import pipeline

pipeline = pipeline(task="text-generation", model="google/gemma-2-2b", device="mps")
pipeline("the secret to baking a really good cake is ")
```

### Batch inference

[`Pipeline`] can also process batches of inputs with the `batch_size` parameter. Batch inference may improve speed, especially on a GPU, but it isn't guaranteed. Other variables such as hardware, data, and the model itself can affect whether batch inference improves speed. For this reason, batch inference is disabled by default.

```py
from transformers import pipeline
from accelerate import Accelerator

device = Accelerator().device
pipeline = pipeline(task="text-generation", model="google/gemma-2-2b", device=device, batch_size=2)
pipeline(["the secret to baking a really good cake is", "a baguette is", "paris is the", "hotdogs are"])
```

Keep the following general rules of thumb in mind for determining whether batch inference can help improve performance.

1. The only way to know for sure is to measure performance on your model, data, and hardware.
2. Don't batch inference if you're constrained by latency (a live inference product for example).
3. Don't batch inference if you're using a CPU.
4. Don't batch inference if you don't know the `sequence_length` of your data. Measure performance, iteratively add to `sequence_length`, and include out-of-memory (OOM) checks to recover from failures.
5. Do batch inference if your `sequence_length` is regular, and keep pushing it until you reach an OOM error. The larger the GPU, the more helpful batch inference is.
6. Do make sure you can handle OOM errors if you decide to do batch inference.

### Task-specific parameters

[`Pipeline`] accepts any parameters that are supported by each individual task pipeline. Make sure to check out each individual task pipeline to see what type of parameters are available.

The examples below demonstrate some of the task-specific parameters available.

Pass the `return_timestamps="word"` parameter to [`Pipeline`] to return when each word was spoken.

```py
from transformers import pipeline

pipeline = pipeline(task="automatic-speech-recognition", model="openai/whisper-large-v3")
pipeline(audio="https://huggingface.co/datasets/Narsil/asr_dummy/resolve/main/mlk.flac", return_timestamp="word")
{'text': ' I have a dream that one day this nation will rise up and live out the true meaning of its creed.',
 'chunks': [{'text': ' I', 'timestamp': (0.0, 1.1)},
            {'text': ' have', 'timestamp': (1.1, 1.44)},
            {'text': ' a', 'timestamp': (1.44, 1.62)},
            {'text': ' dream', 'timestamp': (1.62, 1.92)},
            ...]}
```

## Chunk batching

There are some instances where you need to process data in chunks.

- for some data types, a single input (for example, a really long audio file) may need to be chunked into multiple parts before it can be processed
- for some tasks, like zero-shot classification or question answering, a single input may need multiple forward passes which can cause issues with the `batch_size` parameter

The [ChunkPipeline](https://github.com/huggingface/transformers/blob/99e0ab6ed888136ea4877c6d8ab03690a1478363/src/transformers/pipelines/base.py#L1387) class is designed to handle these use cases.

## Large datasets

For inference with large datasets, you can iterate directly over the dataset itself. This avoids immediately allocating memory for the entire dataset, and you don't need to worry about creating batches yourself.

```py
from transformers.pipelines.pt_utils import KeyDataset
from transformers import pipeline
from accelerate import Accelerator
from datasets import load_dataset

device = Accelerator().device
dataset = datasets.load_dataset("imdb", name="plain_text", split="unsupervised")
pipeline = pipeline(task="text-classification", model="distilbert/distilbert-base-uncased-finetuned-sst-2-english", device=device)
for out in pipeline(KeyDataset(dataset, "text"), batch_size=8, truncation="only_first"):
    print(out)
```

## Large models

[Accelerate](https://hf.co/docs/accelerate/index) enables a couple of optimizations for running large models with [`Pipeline`].

- The `device_map="auto"` setting is useful for automatically distributing the model across the fastest devices (GPUs) first before dispatching to other slower devices if available (CPU, hard drive).
- [`Pipeline`] supports half-precision weights (torch.float16), which can be significantly faster and save memory.
- [`Pipeline`] also accepts quantized models to reduce memory usage even further.

```py
import torch
from transformers import pipeline, BitsAndBytesConfig

pipeline = pipeline(
    model="google/gemma-7b",
    dtype=torch.bfloat16,
    device_map="auto",
    model_kwargs={"quantization_config": BitsAndBytesConfig(load_in_8bit=True)}
)
pipeline("the secret to baking a good cake is ")
```
