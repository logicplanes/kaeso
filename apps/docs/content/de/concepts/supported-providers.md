---
title: Unterstützte Anbieter
description: Jeden Anbieter von Kaeso nachschlagen, mit ID, Formaten, Host und der Herkunft seiner Schlüssel.
order: 2
---

Diese Anbieter sind in Kaeso eingebaut, nach Namen sortiert. Unter [Anbieter](providers.md) steht, wie Sie einen davon nutzen und worin sie sich unterscheiden.

## Spalten

- **Anbieter.** Der Name, den die Konsole zeigt, dann die ID. Die ID bezeichnet den Anbieter in Modellnamen wie `groq/llama-3.3-70b-versatile` und in [Einstellungen](../self-hosting/configuration.md#anbietereinstellungen).
- **Formate.** Die API-Formate, in denen Kaeso den Anbieter aufruft. Ein Modell nutzt das erste, außer sein Wert **Sendet als** nennt ein anderes ([Werte eines Modells](../console/models-and-aliases.md#werte-eines-modells-ändern)). **Embeddings** heißt, dass der Anbieter Embeddings-Modelle hat, die `/v1/embeddings` bedienen. **Token-Zählung** heißt, dass er Token für `/v1/messages/count_tokens` und `/v1/responses/input_tokens` zählt.
- **Host.** Der Host der Standard-Basis-URL des Anbieters, den die Installation erreichen muss. Eine [Basis-URL, die Sie setzen](../self-hosting/configuration.md#anbietereinstellungen), ersetzt ihn.
- **Schlüssel.** Wo Sie einen Schlüssel erstellen und womit er beginnt ([Anbieter-Schlüssel](../console/provider-keys.md)).

## Anbieter

| Anbieter | Formate | Host | Schlüssel |
| --- | --- | --- | --- |
| Anthropic `anthropic` | Messages, Token-Zählung | `api.anthropic.com` | [platform.claude.com/settings/keys](https://platform.claude.com/settings/keys), `sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys), `sk-` |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com/keys](https://console.groq.com/keys), `gsk_` |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, Token-Zählung | `api.openai.com` | [platform.openai.com/api-keys](https://platform.openai.com/api-keys), `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | `openrouter.ai` | [openrouter.ai/settings/keys](https://openrouter.ai/settings/keys), `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai), `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys), `tgp_v1_` |

## Hinweise

Was Sie über manche Anbieter noch wissen sollten:

- **OpenRouter.** Modell-IDs nennen den Hersteller des Modells, etwa `anthropic/claude-sonnet-4.5`. Rufen Sie sie als `openrouter/anthropic/claude-sonnet-4.5` auf, damit sie nie beim eigenen Anbieter des Herstellers landen ([Wie eine Anfrage ein Modell nennt](../console/models-and-aliases.md#wie-eine-anfrage-ein-modell-nennt)). Kaeso berechnet die Kosten mit den Preisen des Katalogs, nicht mit dem `usage.cost`, das OpenRouter meldet. OpenRouter listet seine Modelle, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel.
- **SpaceX.** Die API von xAI, mit Modellen wie Grok.
