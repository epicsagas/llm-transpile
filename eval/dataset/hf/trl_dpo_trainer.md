# DPO Trainer

## Overview

TRL supports the Direct Preference Optimization (DPO) Trainer for training language models, as described in the paper [Direct Preference Optimization: Your Language Model is Secretly a Reward Model](https://huggingface.co/papers/2305.18290) by Rafael Rafailov, Archit Sharma, Eric Mitchell, Stefano Ermon, Christopher D. Manning, Chelsea Finn.

The abstract from the paper is the following:

> While large-scale unsupervised language models (LMs) learn broad world knowledge and some reasoning skills, achieving precise control of their behavior is difficult due to the completely unsupervised nature of their training. Existing methods for gaining such steerability collect human labels of the relative quality of model generations and fine-tune the unsupervised LM to align with these preferences, often with reinforcement learning from human feedback (RLHF). However, RLHF is a complex and often unstable procedure, first fitting a reward model that reflects the human preferences, and then fine-tuning the large unsupervised LM using reinforcement learning to maximize this estimated reward without drifting too far from the original model. In this paper we introduce a new parameterization of the reward model in RLHF that enables extraction of the corresponding optimal policy in closed form, allowing us to solve the standard RLHF problem with only a simple classification loss. The resulting algorithm, which we call Direct Preference Optimization (DPO), is stable, performant, and computationally lightweight, eliminating the need for sampling from the LM during fine-tuning or performing significant hyperparameter tuning. Our experiments show that DPO can fine-tune LMs to align with human preferences as well as or better than existing methods. Notably, fine-tuning with DPO exceeds PPO-based RLHF in ability to control sentiment of generations, and matches or improves response quality in summarization and single-turn dialogue while being substantially simpler to implement and train.

## Quick start

This example demonstrates how to train a language model using the [`DPOTrainer`] from TRL. We train a [Qwen 3 0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) model on the [UltraFeedback dataset](https://huggingface.co/datasets/openbmb/UltraFeedback).

```python
from trl import DPOTrainer
from datasets import load_dataset

trainer = DPOTrainer(
    model="Qwen/Qwen3-0.6B",
    train_dataset=load_dataset("trl-lib/ultrafeedback_binarized", split="train"),
)
trainer.train()
```

## Expected dataset type and format

DPO requires a [preference](dataset_formats#preference) dataset. The [`DPOTrainer`] is compatible with both [standard](dataset_formats#standard) and [conversational](dataset_formats#conversational) dataset formats.

```python
# Standard format - explicit prompt (recommended)
preference_example = {"prompt": "The sky is", "chosen": " blue.", "rejected": " green."}

# Conversational format - explicit prompt (recommended)
preference_example = {
    "prompt": [{"role": "user", "content": "What color is the sky?"}],
    "chosen": [{"role": "assistant", "content": "It is blue."}],
    "rejected": [{"role": "assistant", "content": "It is green."}]
}
```

## Looking deeper into the DPO method

Direct Preference Optimization (DPO) is a training method designed to align a language model with preference data. Instead of supervised input-output pairs, the model is trained on pairs of completions to the same prompt, where one completion is preferred over the other. The objective directly optimizes the model to widen the margin between the log-likelihoods of preferred and dispreferred completions, relative to a reference model, without requiring an explicit reward model.

### Computing the loss

The loss used in DPO is defined as:

$$
\mathcal{L}_{\mathrm{DPO}}(\theta) = -\mathbb{E}_{(x,y^{+},y^{-})}\!\left[\log \sigma\!\left(\beta\Big(\log\frac{\pi_{\theta}(y^{+}\!\mid x)}{\pi_{\mathrm{ref}}(y^{+}\!\mid x)}-\log \frac{\pi_{\theta}(y^{-}\!\mid x)}{\pi_{\mathrm{ref}}(y^{-}\!\mid x)}\Big)\right)\right]
$$

where $x$ is the prompt, $y^+$ is the preferred completion and $y^-$ is the dispreferred completion. $\pi_{\theta}$ is the policy model being trained, $\pi_{\mathrm{ref}}$ is the reference model, $\sigma$ is the sigmoid function, and $\beta > 0$ is a hyperparameter that controls the strength of the preference signal.

#### Loss Types

Several formulations of the objective have been proposed in the literature.

| `loss_type=` | Description |
| --- | --- |
| `"sigmoid"` (default) | Given the preference data, we can fit a binary classifier according to the Bradley-Terry model and in fact the [DPO](https://huggingface.co/papers/2305.18290) authors propose the sigmoid loss on the normalized likelihood via the `logsigmoid` to fit a logistic regression. |
| `"hinge"` | The [RSO](https://huggingface.co/papers/2309.06657) authors propose to use a hinge loss on the normalized likelihood from the [SLiC](https://huggingface.co/papers/2305.10425) paper. In this case, the `beta` is the reciprocal of the margin. |
| `"ipo"` | The [IPO](https://huggingface.co/papers/2310.12036) authors argue the logit transform can overfit and propose the identity transform to optimize preferences directly. |
| `"exo_pair"` | The [EXO](https://huggingface.co/papers/2402.00856) authors propose reverse-KL preference optimization. |
| `"nca_pair"` | The [NCA](https://huggingface.co/papers/2402.05369) authors shows that NCA optimizes the absolute likelihood for each response rather than the relative likelihood. |
| `"robust"` | The [Robust DPO](https://huggingface.co/papers/2403.00409) authors propose an unbiased DPO loss under noisy preferences. |
| `"bco_pair"` | The [BCO](https://huggingface.co/papers/2404.04656) authors train a binary classifier whose logit serves as a reward. |
| `"sppo_hard"` | The [SPPO](https://huggingface.co/papers/2405.00675) authors claim that SPPO is capable of solving the Nash equilibrium iteratively. |
| `"aot"` or `"aot_unpaired"` | The [AOT](https://huggingface.co/papers/2406.05882) authors propose Distributional Preference Alignment via Optimal Transport. |
| `"apo_zero"` or `"apo_down"` | The [APO](https://huggingface.co/papers/2408.06266) method introduces an anchored objective. |
| `"discopop"` | The [DiscoPOP](https://huggingface.co/papers/2406.08414) paper uses LLMs to discover more efficient offline preference optimization losses. |
| `"sft"` | SFT (Supervised Fine-Tuning) loss is the negative log likelihood loss, used to train the model to generate preferred responses. |

## Logged metrics

While training and evaluating we record the following reward metrics:

* `global_step`: The total number of optimizer steps taken so far.
* `epoch`: The current epoch number, based on dataset iteration.
* `num_tokens`: The total number of tokens processed so far.
* `loss`: The average cross-entropy loss computed over non-masked tokens.
* `logps/chosen`: The average log-probability assigned by the model to the tokens in the chosen completion.
* `logps/rejected`: The average log-probability assigned by the model to the tokens in the rejected completion.
* `rewards/chosen`: The average implicit reward computed for the chosen completion.
* `rewards/rejected`: The average implicit reward computed for the rejected completion.
* `rewards/margins`: The average implicit reward margin between the chosen and rejected completions.
* `rewards/accuracies`: The proportion of examples where the implicit reward for the chosen completion is higher.

## Customization

### Multi-loss combinations

The DPO trainer supports combining multiple loss functions with different weights, enabling more sophisticated optimization strategies like MPO (Mixed Preference Optimization):

```python
# MPO: Combines DPO (sigmoid) for preference and BCO (bco_pair) for quality
training_args = DPOConfig(
    loss_type=["sigmoid", "bco_pair", "sft"],
    loss_weights=[0.8, 0.2, 1.0]
)
```

### Train adapters with PEFT

```python
from datasets import load_dataset
from trl import DPOTrainer
from peft import LoraConfig

dataset = load_dataset("trl-lib/ultrafeedback_binarized", split="train")
trainer = DPOTrainer(
    "Qwen/Qwen3-0.6B",
    train_dataset=dataset,
    peft_config=LoraConfig(),
)
trainer.train()
```

> [!TIP]
> When training adapters, you typically use a higher learning rate (approximately 1e-5) than full fine-tuning since only new parameters are being learned.

## Training Vision Language Models

[`DPOTrainer`] fully supports training Vision-Language Models (VLMs). To train a VLM, provide a dataset with either an `image` column (single image per sample) or an `images` column (list of images per sample).

```python
from trl import DPOConfig, DPOTrainer
from datasets import load_dataset

trainer = DPOTrainer(
    model="Qwen/Qwen2.5-VL-3B-Instruct",
    args=DPOConfig(max_length=None),
    train_dataset=load_dataset("HuggingFaceH4/rlaif-v_formatted", split="train"),
)
trainer.train()
```

## DPOTrainer

[[autodoc]] DPOTrainer
    - train
    - save_model
    - push_to_hub

## DPOConfig

[[autodoc]] DPOConfig
