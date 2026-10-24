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
| Anthropic `anthropic` | Messages, Token-Zählung | `api.anthropic.com` | [platform.claude.com](https://platform.claude.com/settings/keys), `sk-ant-` |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com/api_keys), `sk-` |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com](https://console.groq.com/keys), `gsk_` |
| Mistral `mistral` | Chat Completions, Embeddings | `api.mistral.ai` | [console.mistral.ai](https://console.mistral.ai/api-keys), kein festes Präfix |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, Token-Zählung | `api.openai.com` | [platform.openai.com](https://platform.openai.com/api-keys), `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | `openrouter.ai` | [openrouter.ai](https://openrouter.ai/settings/keys), `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai/team/default/api-keys), `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys), `tgp_v1_` |

## Hinweise

Was Sie über manche Anbieter noch wissen sollten. Die [Anbieterprofile](../development/kpp/openai-chat-providers.md#anbieterprofile) nennen jedes Feld von Chat Completions, das ein Anbieter umbenennt oder ablehnt:

- **DeepSeek.** Erhält `developer`-Nachrichten als `system`-Nachrichten.
- **Groq.** Erhält `developer`-Nachrichten als `system`-Nachrichten und lehnt manche Felder ab, etwa `logit_bias`.
- **Mistral.** Erhält manche Felder unter eigenem Namen, etwa `seed` als `random_seed`, und lehnt andere ab, etwa `logit_bias`, ebenso ein Gespräch, das mit einer Assistentennachricht endet.
- **OpenAI.** Ein Anbieter-Schlüssel kann auch ein ChatGPT-Konto sein, dessen Anfragen sein ChatGPT-Abo nach den Regeln von OpenAI dafür nutzen ([Mit einem ChatGPT-Konto anmelden](../console/provider-keys.md#mit-einem-chatgpt-konto-anmelden)).
- **OpenRouter.** Modell-IDs nennen den Hersteller des Modells, etwa `anthropic/claude-sonnet-4.5`. Rufen Sie sie als `openrouter/anthropic/claude-sonnet-4.5` auf, damit sie nie beim eigenen Anbieter des Herstellers landen ([Wie eine Anfrage ein Modell nennt](../console/models-and-aliases.md#wie-eine-anfrage-ein-modell-nennt)). Kaeso berechnet die Kosten mit den Preisen des Katalogs, nicht mit dem `usage.cost`, das OpenRouter meldet. OpenRouter listet seine Modelle, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel.
- **SpaceX.** Die API von xAI, mit Modellen wie Grok. Ein Anbieter-Schlüssel kann als experimentelle Option auch ein Grok-Konto sein, dessen Abo API-Zugang enthält ([Mit einem Grok-Konto anmelden](../console/provider-keys.md#mit-einem-grok-konto-anmelden)).
