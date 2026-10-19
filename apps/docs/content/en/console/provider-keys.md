---
title: Provider keys
description: Connect your organization's provider keys, and choose which projects use each one.
order: 3
---

Provider keys are your organization's keys for the providers Kaeso calls. Kaeso stores them encrypted under the install's master key and never shows them again. Apps never see them: they call Kaeso with [API keys](api-keys.md) instead.

## Add a key

1. On **Providers**, choose **Add key**, or **Connect** next to a provider that has no key yet.
2. **Which provider is the key for?** Choose one from the [list below](#providers).
3. Paste the key and give it a name, such as `Production`.
4. If the provider already has a default key, choose whether the new key becomes the default.

Each action on provider keys needs its own right:

| Action | Right |
| --- | --- |
| Add a key | **Add provider keys** |
| Replace, rename or assign a key, or make it the default | **Change provider keys** |
| Delete a key | **Delete provider keys** |

Every action except deleting also needs a recent sign-in.

## Providers

| Provider | Id | Where to create a key | Starts with |
| --- | --- | --- | --- |
| OpenAI | `openai` | platform.openai.com, under API keys | `sk-` |
| Anthropic | `anthropic` | console.anthropic.com, under API keys | `sk-ant-` |
| Hugging Face | `huggingface` | huggingface.co/settings/tokens: a fine-grained token that may make calls to Inference Providers | `hf_` |

Hugging Face routes each request to one of its inference providers. A model id can name one with a suffix, such as `meta-llama/Llama-3.1-8B-Instruct:cerebras`, or pick by `:fastest` or `:cheapest`; in Kaeso, the prefixed name is `huggingface/meta-llama/Llama-3.1-8B-Instruct:cerebras`. When a model's organization has the name of a provider, as in `openai/gpt-oss-120b`, and the project also has that provider's `gpt-oss-120b`, the name means OpenAI's model; write `huggingface/openai/gpt-oss-120b` for Hugging Face's. When the Hugging Face account has no credits left, its requests fail with a billing error.

## Choose which key a project uses

Each provider has one default key, which every project without an assigned key uses. The first key you add for a provider becomes its default. To move the default, choose **Make default** in another key's menu.

To give some projects a different key, choose **Assign to projects** in that key's menu and pick the projects. They then use that key for its provider instead of the default. A project uses one key per provider.

## Replace, rename or delete a key

The list shows each key's name, its last four characters, the projects that use it, who added it and when it was last used. A key's menu has these actions:

| Action | What it does |
| --- | --- |
| Replace | Takes a new secret for the same key, for example after you rotated it at the provider |
| Rename | Changes the name the console shows |
| Delete | Removes the key. Its projects use the default again |

When you delete the default, the provider's most recently added other key becomes the default. If there is none, requests to that provider fail. The dialog tells you which applies.

> [!NOTE]
> A project's key decides which models the provider serves it. Adding, replacing, assigning or deleting a key, or making it the default, resets the [availability](models-and-aliases.md#availability) of that provider's models to **Not checked** in every project.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models a project may call.
- [API keys](api-keys.md): Create the keys your apps call Kaeso with.
