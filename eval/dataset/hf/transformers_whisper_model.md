# Whisper

[Whisper](https://huggingface.co/papers/2212.04356) is a encoder-decoder (sequence-to-sequence) transformer pretrained on 680,000 hours of labeled audio data. This amount of pretraining data enables zero-shot performance on audio tasks in English and many other languages. The decoder allows Whisper to map the encoders learned speech representations to useful outputs, such as text, without additional fine-tuning. Whisper just works out of the box.

You can find all the original Whisper checkpoints under the [Whisper](https://huggingface.co/collections/openai/whisper-release-6501bba2cf999715fd953013) collection.

> [!TIP]
> Click on the Whisper models in the right sidebar for more examples of how to apply Whisper to different audio tasks.

The example below demonstrates how to automatically transcribe speech into text with [`Pipeline`] or the [`AutoModel`] class.

```py
import torch
from transformers import pipeline

pipeline = pipeline(
    task="automatic-speech-recognition",
    model="openai/whisper-large-v3-turbo",
    dtype=torch.float16,
    device=0
)
pipeline("https://huggingface.co/datasets/Narsil/asr_dummy/resolve/main/mlk.flac")
```

```py
# pip install datasets
import torch
from datasets import load_dataset
from transformers import AutoProcessor, WhisperForConditionalGeneration

processor = AutoProcessor.from_pretrained(
    "openai/whisper-large-v3-turbo",
)
model = WhisperForConditionalGeneration.from_pretrained(
    "openai/whisper-large-v3-turbo",
    dtype=torch.float16,
    device_map="auto",
    attn_implementation="sdpa"
)

ds = load_dataset("hf-internal-testing/librispeech_asr_dummy", "clean", split="validation")
audio_sample = ds[0]["audio"]

input_features = processor(
    audio_sample["array"],
    sampling_rate=audio_sample["sampling_rate"],
    return_tensors="pt"
).input_features

input_features = input_features.to(model.device, dtype=torch.float16)
predicted_ids = model.generate(input_features, cache_implementation="static")
transcription = processor.batch_decode(predicted_ids, skip_special_tokens=True)
transcription[0]
```

## Notes

- Whisper relies a custom [`generate`] for inference, make sure to check the docs below.
- The [`WhisperProcessor`] can be used for preparing audio and decoding predicted ids back into text.

## WhisperConfig

[[autodoc]] WhisperConfig

## WhisperTokenizer

[[autodoc]] WhisperTokenizer
    - set_prefix_tokens
    - get_special_tokens_mask
    - save_vocabulary
    - batch_decode
    - decode
    - basic_normalize
    - normalize

## WhisperTokenizerFast

[[autodoc]] WhisperTokenizerFast
    - set_prefix_tokens
    - get_special_tokens_mask
    - save_vocabulary
    - batch_decode
    - decode
    - basic_normalize
    - normalize

## WhisperFeatureExtractor

[[autodoc]] WhisperFeatureExtractor
    - __call__

## WhisperProcessor

[[autodoc]] WhisperProcessor
    - __call__
    - from_pretrained
    - save_pretrained
    - batch_decode
    - decode

## WhisperModel

[[autodoc]] WhisperModel
    - forward
    - _mask_input_features

## WhisperForConditionalGeneration

[[autodoc]] WhisperForConditionalGeneration
    - forward
    - generate

## WhisperForCausalLM

[[autodoc]] WhisperForCausalLM
    - forward

## WhisperForAudioClassification

[[autodoc]] WhisperForAudioClassification
    - forward
