# Overview

Quantization lowers the memory requirements of loading and using a model by storing the weights in a lower precision while trying to preserve as much accuracy as possible. Weights are typically stored in full-precision (fp32) floating point representations, but half-precision (fp16 or bf16) are increasingly popular data types given the large size of models today. Some quantization methods can reduce the precision even further to integer representations, like int8 or int4.

Transformers supports many quantization methods, each with their pros and cons, so you can pick the best one for your specific use case. Some methods require calibration for greater accuracy and extreme compression (1-2 bits), while other methods work out of the box with on-the-fly quantization.

Use the Space below to help you pick a quantization method depending on your hardware and number of bits to quantize to.

| Quantization Method | On the fly quantization | CPU | CUDA GPU | ROCm GPU | Metal (Apple Silicon) | Intel GPU | Torch compile() | Bits | PEFT Fine Tuning | Serializable with Transformers | Transformers Support | Link to library |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| [AQLM](./aqlm) | No | Yes | Yes | No | No | Yes | Yes | 1/2 | Yes | Yes | Yes | https://github.com/Vahe1994/AQLM |
| [AutoRound](./auto_round) | No | Yes | Yes | No | No | Yes | No | 2/3/4/8 | No | Yes | Yes | https://github.com/intel/auto-round |
| [AWQ](./awq) | No | Yes | Yes | Yes | No | Yes | ? | 4 | Yes | Yes | Yes | https://github.com/casper-hansen/AutoAWQ |
| [bitsandbytes](./bitsandbytes) | Yes | Yes | Yes | Partial | Partial | Yes | Yes | 4/8 | Yes | Yes | Yes | https://github.com/bitsandbytes-foundation/bitsandbytes |
| [compressed-tensors](./compressed_tensors) | No | Yes | Yes | Yes | No | No | No | 1/8 | Yes | Yes | Yes | https://github.com/neuralmagic/compressed-tensors |
| [EETQ](./eetq) | Yes | No | Yes | No | No | No | ? | 8 | Yes | Yes | Yes | https://github.com/NetEase-FuXi/EETQ |
| [GGUF / GGML (llama.cpp)](../gguf) | Yes | Yes | Yes | No | Yes | Yes | No | 1/8 | No | See Notes | See Notes | https://github.com/ggerganov/llama.cpp |
| [GPT-QModel](./gptq) | No | Yes | Yes | Yes | Yes | Yes | No | 2/3/4/8 | Yes | Yes | Yes | https://github.com/ModelCloud/GPTQModel |
| [HIGGS](./higgs) | Yes | No | Yes | No | No | No | Yes | 2/4 | No | Yes | Yes | https://github.com/HanGuo97/flute |
| [HQQ](./hqq) | Yes | Yes | Yes | No | No | Yes | Yes | 1/8 | Yes | No | Yes | https://github.com/mobiusml/hqq/ |
| [optimum-quanto](./quanto) | Yes | Yes | Yes | No | Yes | Yes | Yes | 2/4/8 | No | No | Yes | https://github.com/huggingface/optimum-quanto |
| [torchao](./torchao) | Yes | Yes | Yes | No | Partial | Yes | | 4/8 | | Partial | Yes | https://github.com/pytorch/ao |
| [VPTQ](./vptq) | No | No | Yes | Partial | No | No | Yes | 1/8 | No | Yes | Yes | https://github.com/microsoft/VPTQ |

## Resources

If you are new to quantization, we recommend checking out these beginner-friendly quantization courses in collaboration with DeepLearning.AI.

* [Quantization Fundamentals with Hugging Face](https://www.deeplearning.ai/short-courses/quantization-fundamentals-with-hugging-face/)
* [Quantization in Depth](https://www.deeplearning.ai/short-courses/quantization-in-depth)

## User-Friendly Quantization Tools

If you are looking for a user-friendly quantization experience, you can use the following community spaces and notebooks:

* [Bitsandbytes Space](https://huggingface.co/spaces/bnb-community/bnb-my-repo)
* [GGUF Space](https://huggingface.co/spaces/ggml-org/gguf-my-repo)
* [MLX Space](https://huggingface.co/spaces/mlx-community/mlx-my-repo)
* [AutoQuant Notebook](https://colab.research.google.com/drive/1b6nqC7UZVt8bx4MksX7s656GXPM-eWw4?usp=sharing#scrollTo=ZC9Nsr9u5WhN)
