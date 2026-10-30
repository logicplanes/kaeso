---
title: Supported providers
description: Look up every provider Kaeso calls, with its id, formats, host and where its keys come from.
order: 2
---

Kaeso has the providers below built in, listed by name. [Providers](providers.md) explains how to use one and what differs between them. An organization can also add a server of its own that speaks one of these formats ([custom providers](../console/custom-providers.md)).

## Columns

- **Provider.** The name the console shows, then the id. The id names the provider in model names, such as `groq/llama-3.3-70b-versatile`, and in [settings](../self-hosting/configuration.md#provider-settings).
- **Formats.** The API formats Kaeso calls the provider with. A model uses the first one unless its **Sends as** value names another ([a model's values](../console/models-and-aliases.md#change-a-models-values)). **Embeddings** means the provider has embeddings models, which serve `/v1/embeddings`. **Token counts** means it counts tokens for `/v1/messages/count_tokens` and `/v1/responses/input_tokens`.
- **Host.** The host of the provider's default base URL, which the install must reach. A [base URL you set](../self-hosting/configuration.md#provider-settings) replaces it.
- **Key.** Where you create a key, and how the key starts ([provider keys](../console/provider-keys.md)).

## Providers

| Provider | Formats | Host | Key |
| --- | --- | --- | --- |
| Anthropic `anthropic` | Messages, token counts | `api.anthropic.com` | [platform.claude.com](https://platform.claude.com/settings/keys), `sk-ant-` |
| Cohere `cohere` | Chat Completions, Embeddings | `api.cohere.ai` | [dashboard.cohere.com](https://dashboard.cohere.com/api-keys), no fixed prefix |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com/api_keys), `sk-` |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com](https://console.groq.com/keys), `gsk_` |
| Mistral `mistral` | Chat Completions, Embeddings | `api.mistral.ai` | [console.mistral.ai](https://console.mistral.ai/api-keys), no fixed prefix |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, token counts | `api.openai.com` | [platform.openai.com](https://platform.openai.com/api-keys), `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | `openrouter.ai` | [openrouter.ai](https://openrouter.ai/settings/keys), `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai/team/default/api-keys), `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys), `tgp_v1_` |
| Vercel `vercel` | Chat Completions, Responses, Messages, Embeddings | `ai-gateway.vercel.sh` | [vercel.com](https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys), `vck_` |

## Notes

What else to know about some providers. [Provider profiles](../development/kpp/openai-chat-providers.md#provider-profiles) lists every Chat Completions field a provider renames or refuses:

- **Cohere.** Cohere's [Compatibility API](https://docs.cohere.com/docs/compatibility-api), which covers part of Cohere's own API: documents, citations and rerank aren't reachable through it. Cohere refuses some fields, such as `logit_bias` and `parallel_tool_calls`, and gets a single `stop` string as a list. Its reasoning models think at `reasoning_effort` `none` or `high` only. Its Embed models, such as `embed-v4.0`, aren't on models.dev: add them as custom models that send as Embeddings, with their price.
- **DeepSeek.** Gets `developer` messages as `system` messages.
- **Groq.** Gets `developer` messages as `system` messages, and refuses some fields, such as `logit_bias`.
- **Mistral.** Gets `developer` messages as `system` messages, and some fields under its own names, such as `seed` as `random_seed`. It refuses others, such as `logit_bias`, and a conversation that ends with an assistant message.
- **OpenAI.** A provider key can also be a ChatGPT account, whose requests use its ChatGPT plan under OpenAI's rules for it ([sign in with a ChatGPT account](../console/provider-keys.md#sign-in-with-a-chatgpt-account)).
- **OpenRouter.** Model ids name the model's maker, such as `anthropic/claude-sonnet-4.5`. Call them as `openrouter/anthropic/claude-sonnet-4.5`, so they never reach the maker's own provider ([how a request names a model](../console/models-and-aliases.md#how-a-request-names-a-model)). Kaeso records costs at the catalogue's prices, not at the `usage.cost` OpenRouter reports. OpenRouter lists its models without checking the key, so **Check availability** can't spot a wrong key; the first request with it reports the refused key.
- **SpaceX.** xAI's API, with models such as Grok. A provider key can also be a Grok account whose plan includes API access, as an experimental option ([sign in with a Grok account](../console/provider-keys.md#sign-in-with-a-grok-account)).
- **Vercel.** Vercel AI Gateway, another gateway whose own routing and fallbacks run behind Kaeso. Model ids name the model's maker, such as `openai/gpt-5-nano`. Call them as `vercel/openai/gpt-5-nano`, so they never reach the maker's own provider ([how a request names a model](../console/models-and-aliases.md#how-a-request-names-a-model)). A model's **Sends as** value can choose Responses or Messages, which Vercel serves too; on Messages, Vercel passes Anthropic's `cache_control` on to Claude models. Kaeso records costs at the catalogue's prices, not at the `usage.cost` Vercel reports, which leaves out Vercel's own surcharges. Vercel counts tokens exactly only for Claude models and estimates the rest, so Kaeso refuses token counts for every Vercel model. Vercel answers for a model without checking the key, so **Check availability** can't spot a wrong key; the first request with it reports the refused key. A Vercel account on the free tier can't call paid models such as Claude: Vercel answers 403, which apps get as a refused provider key, and the request log keeps Vercel's reason.
