---
title: Unterstützte Anbieter
description: Jeden Anbieter von Kaeso nachschlagen, mit seiner ID, den Formaten, in denen Kaeso ihn aufruft, und der Herkunft seiner Schlüssel.
order: 2
---

Diese Anbieter sind in Kaeso eingebaut, nach Namen sortiert. Unter [Anbieter](providers.md) steht, wie Sie einen davon nutzen und worin sie sich unterscheiden.

## Spalten

- **Anbieter.** Der Name, den die Konsole zeigt, dann die ID. Die ID bezeichnet den Anbieter in Modellnamen wie `groq/llama-3.3-70b-versatile` und in [Einstellungen](../self-hosting/configuration.md#anbietereinstellungen).
- **Formate.** Die API-Formate, in denen Kaeso den Anbieter aufruft. Ein Modell nutzt das erste, außer sein Wert **Sendet als** nennt ein anderes ([Werte eines Modells](../console/models-and-aliases.md#werte-eines-modells-ändern)). **Embeddings** heißt, dass der Anbieter Embeddings-Modelle hat, die `/v1/embeddings` bedienen. **Token-Zählung** heißt, dass er Token für `/v1/messages/count_tokens` und `/v1/responses/input_tokens` zählt.
- **Schlüssel.** Wo Sie einen Schlüssel erstellen und womit er beginnt ([Anbieter-Schlüssel](../console/provider-keys.md)).

## Anbieter

| Anbieter | Formate | Schlüssel |
| --- | --- | --- |
| Anthropic `anthropic` | Messages, Token-Zählung | console.anthropic.com, `sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | platform.deepseek.com, `sk-` |
| Groq `groq` | Chat Completions | console.groq.com, `gsk_` |
| Mistral `mistral` | Chat Completions, Embeddings | console.mistral.ai |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, Token-Zählung | platform.openai.com, `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | openrouter.ai, `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | console.x.ai, `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | api.together.ai, `tgp_v1_` |

## Hinweise

Was Sie über manche Anbieter noch wissen sollten:

- **Mistral.** Kaeso lehnt manche Felder von Chat Completions ab, die Mistral nicht annimmt ([Anbieterprofile](../development/kpp/openai-chat.md)).
- **OpenRouter.** Modell-IDs nennen den Hersteller des Modells, etwa `anthropic/claude-sonnet-4.5`. Rufen Sie sie mit vorangestelltem Anbieter auf, `openrouter/anthropic/claude-sonnet-4.5`, damit sie nie beim eigenen Anbieter des Herstellers landen.
- **SpaceX.** Die API von xAI, mit Modellen wie Grok.
