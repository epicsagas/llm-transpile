# DreamBooth

[DreamBooth](https://huggingface.co/papers/2208.12242) is a training technique that updates the entire diffusion model by training on just a few images of a subject or style. It works by associating a special word in the prompt with the example images.

If you're training on a GPU with limited vRAM, you should try enabling the `gradient_checkpointing` and `mixed_precision` parameters in the training command. You can also reduce your memory footprint by using memory-efficient attention with [xFormers](../optimization/xformers).

This guide will explore the [train_dreambooth.py](https://github.com/huggingface/diffusers/blob/main/examples/dreambooth/train_dreambooth.py) script to help you become more familiar with it, and how you can adapt it for your own use-case.

Before running the script, make sure you install the library from source:

```bash
git clone https://github.com/huggingface/diffusers
cd diffusers
pip install .
```

Navigate to the example folder with the training script and install the required dependencies for the script you're using:

```bash
cd examples/dreambooth
pip install -r requirements.txt
```

> [!TIP]
> Accelerate is a library for helping you train on multiple GPUs/TPUs or with mixed-precision. It'll automatically configure your training setup based on your hardware and environment. Take a look at the Accelerate [Quick tour](https://huggingface.co/docs/accelerate/quicktour) to learn more.

Initialize an Accelerate environment:

```bash
accelerate config
```

To setup a default Accelerate environment without choosing any configurations:

```bash
accelerate config default
```

## Script parameters

> [!WARNING]
> DreamBooth is very sensitive to training hyperparameters, and it is easy to overfit. Read the [Training Stable Diffusion with Dreambooth using Diffusers](https://huggingface.co/blog/dreambooth) blog post for recommended settings for different subjects to help you choose the appropriate hyperparameters.

Some basic and important parameters to know and specify are:

- `--pretrained_model_name_or_path`: the name of the model on the Hub or a local path to the pretrained model
- `--instance_data_dir`: path to a folder containing the training dataset (example images)
- `--instance_prompt`: the text prompt that contains the special word for the example images
- `--train_text_encoder`: whether to also train the text encoder
- `--output_dir`: where to save the trained model
- `--push_to_hub`: whether to push the trained model to the Hub
- `--checkpointing_steps`: frequency of saving a checkpoint as the model trains

### Min-SNR weighting

The [Min-SNR](https://huggingface.co/papers/2303.09556) weighting strategy can help with training by rebalancing the loss to achieve faster convergence. The training script supports predicting `epsilon` (noise) or `v_prediction`, but Min-SNR is compatible with both prediction types.

```bash
accelerate launch train_dreambooth.py \
    --snr_gamma=5.0
```

### Prior preservation loss

Prior preservation loss is a method that uses a model's own generated samples to help it learn how to generate more diverse images. Because these generated sample images belong to the same class as the images you provided, they help the model retain what it has learned about the class.

- `--with_prior_preservation`: whether to use prior preservation loss
- `--prior_loss_weight`: controls the influence of the prior preservation loss on the model
- `--class_data_dir`: path to a folder containing the generated class sample images
- `--class_prompt`: the text prompt describing the class of the generated sample images

```bash
accelerate launch train_dreambooth.py \
    --with_prior_preservation \
    --prior_loss_weight=1.0 \
    --class_data_dir="path/to/class/images" \
    --class_prompt="text prompt describing class"
```

### Train text encoder

To improve the quality of the generated outputs, you can also train the text encoder in addition to the UNet. This requires additional memory and you'll need a GPU with at least 24GB of vRAM.

```bash
accelerate launch train_dreambooth.py \
    --train_text_encoder
```

## Launch the script

For this guide, you'll download some images of a [dog](https://huggingface.co/datasets/diffusers/dog-example) and store them in a directory.

```py
from huggingface_hub import snapshot_download

local_dir = "./dog"
snapshot_download(
    "diffusers/dog-example",
    local_dir=local_dir,
    repo_type="dataset",
    ignore_patterns=".gitattributes",
)
```

Set the environment variable `MODEL_NAME` to a model id on the Hub or a path to a local model, `INSTANCE_DIR` to the path where you just downloaded the dog images to, and `OUTPUT_DIR` to where you want to save the model. You'll use `sks` as the special word to tie the training to.

**On a 16GB GPU**, you can use bitsandbytes 8-bit optimizer and gradient checkpointing:

```bash
accelerate launch train_dreambooth.py \
    --gradient_checkpointing \
    --use_8bit_adam \
```

**On a 12GB GPU**, you'll need bitsandbytes 8-bit optimizer, gradient checkpointing, xFormers, and set the gradients to `None`:

```bash
accelerate launch train_dreambooth.py \
    --use_8bit_adam \
    --gradient_checkpointing \
    --enable_xformers_memory_efficient_attention \
    --set_grads_to_none \
```

**On a 8GB GPU**, you'll need [DeepSpeed](https://www.deepspeed.ai/) to offload some of the tensors from the vRAM to either the CPU or NVME to allow training with less GPU memory.

Full training command example:

```bash
export MODEL_NAME="stable-diffusion-v1-5/stable-diffusion-v1-5"
export INSTANCE_DIR="./dog"
export OUTPUT_DIR="path_to_saved_model"

accelerate launch train_dreambooth.py \
    --pretrained_model_name_or_path=$MODEL_NAME \
    --instance_data_dir=$INSTANCE_DIR \
    --output_dir=$OUTPUT_DIR \
    --instance_prompt="a photo of sks dog" \
    --resolution=512 \
    --train_batch_size=1 \
    --gradient_accumulation_steps=1 \
    --learning_rate=5e-6 \
    --lr_scheduler="constant" \
    --lr_warmup_steps=0 \
    --max_train_steps=400 \
    --push_to_hub
```

Once training is complete, you can use your newly trained model for inference:

```py
from diffusers import DiffusionPipeline
import torch

pipeline = DiffusionPipeline.from_pretrained("path_to_saved_model", torch_dtype=torch.float16, use_safetensors=True).to("cuda")
image = pipeline("A photo of sks dog in a bucket", num_inference_steps=50, guidance_scale=7.5).images[0]
image.save("dog-bucket.png")
```

## LoRA

LoRA is a training technique for significantly reducing the number of trainable parameters. As a result, training is faster and it is easier to store the resulting weights because they are a lot smaller (~100MBs). Use the [train_dreambooth_lora.py](https://github.com/huggingface/diffusers/blob/main/examples/dreambooth/train_dreambooth_lora.py) script to train with LoRA.

## Stable Diffusion XL

Stable Diffusion XL (SDXL) is a powerful text-to-image model that generates high-resolution images, and it adds a second text-encoder to its architecture. Use the [train_dreambooth_lora_sdxl.py](https://github.com/huggingface/diffusers/blob/main/examples/dreambooth/train_dreambooth_lora_sdxl.py) script to train a SDXL model with LoRA.

## DeepFloyd IF

DeepFloyd IF is a cascading pixel diffusion model with three stages. The first stage generates a base image and the second and third stages progressively upscales the base image into a high-resolution 1024x1024 image.

### Training tips

Training the DeepFloyd IF model can be challenging, but here are some tips that we've found helpful:

- LoRA is sufficient for training the stage 1 model because the model's low resolution makes representing finer details difficult regardless.
- For common or simple objects, you don't necessarily need to finetune the upscaler.
- For finer details like faces, fully training the stage 2 upscaler is better than training the stage 2 model with LoRA.
- Lower learning rates should be used to train the stage 2 model.
- The [`DDPMScheduler`] works better than the DPMSolver used in the training scripts.

## Next steps

Congratulations on training your DreamBooth model! To learn more about how to use your new model, the following guide may be helpful:

- Learn how to [load a DreamBooth](../using-diffusers/dreambooth) model for inference if you trained your model with LoRA.
