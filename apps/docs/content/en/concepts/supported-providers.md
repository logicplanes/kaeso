---
title: Supported providers
description: Look up every provider Kaeso calls, with its id, the formats Kaeso calls it with and where its keys come from.
order: 2
---

Kaeso has the providers below built in, listed by name. [Providers](providers.md) explains how to use one and what differs between them.

## Columns

- **Provider.** The name the console shows, then the id. The id names the provider in model names, such as `groq/llama-3.3-70b-versatile`, and in [settings](../self-hosting/configuration.md#provider-settings).
- **Formats.** The API formats Kaeso calls the provider with. A model uses the first one unless its **Sends as** value names another ([a model's values](../console/models-and-aliases.md#change-a-models-values)). **Embeddings** means the provider has embeddings models, which serve `/v1/embeddings`. **Token counts** means it counts tokens for `/v1/messages/count_tokens` and `/v1/responses/input_tokens`.
- **Key.** Where you create a key, and how the key starts ([provider keys](../console/provider-keys.md)).

## Providers

| Provider | Formats | Key |
| --- | --- | --- |
| Anthropic `anthropic` | Messages, token counts | console.anthropic.com, `sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | platform.deepseek.com, `sk-` |
| Groq `groq` | Chat Completions | console.groq.com, `gsk_` |
| Mistral `mistral` | Chat Completions, Embeddings | console.mistral.ai |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, token counts | platform.openai.com, `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | openrouter.ai, `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | console.x.ai, `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | api.together.ai, `tgp_v1_` |

## Notes

What else to know about some providers:

- **Mistral.** Kaeso refuses some Chat Completions fields that Mistral doesn't take ([provider profiles](../development/kpp/openai-chat.md)).
- **OpenRouter.** Model ids name the model's maker, such as `anthropic/claude-sonnet-4.5`. Call them with the provider in front, `openrouter/anthropic/claude-sonnet-4.5`, so they never reach the maker's own provider.
- **SpaceX.** xAI's API, with models such as Grok.
