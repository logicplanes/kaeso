---
title: Unterstützte Anbieter
description: Jeden Anbieter von Kaeso nachschlagen, mit ID, Formaten, Host und der Herkunft seiner Schlüssel.
order: 2
---

Diese Anbieter sind in Kaeso eingebaut, nach Namen sortiert. Unter [Anbieter](providers.md) steht, wie Sie einen davon nutzen und worin sie sich unterscheiden. Eine Organisation kann außerdem einen eigenen Server hinzufügen, der eines dieser Formate spricht ([Eigene Anbieter](../console/custom-providers.md)).

## Spalten

- **Anbieter.** Der Name, den die Konsole zeigt, dann die ID. Die ID bezeichnet den Anbieter in Modellnamen wie `groq/llama-3.3-70b-versatile` und in [Einstellungen](../self-hosting/configuration.md#anbietereinstellungen).
- **Formate.** Die API-Formate, in denen Kaeso den Anbieter aufruft. Ein Modell nutzt das erste, außer sein Wert **Sendet als** nennt ein anderes ([Werte eines Modells](../console/models-and-aliases.md#werte-eines-modells-ändern)). **Embeddings** heißt, dass der Anbieter Embeddings-Modelle hat, die `/v1/embeddings` bedienen. **Token-Zählung** heißt, dass er Token für `/v1/messages/count_tokens` und `/v1/responses/input_tokens` zählt.
- **Host.** Der Host der Standard-Basis-URL des Anbieters, den die Installation erreichen muss. Eine [Basis-URL, die Sie setzen](../self-hosting/configuration.md#anbietereinstellungen), ersetzt ihn.
- **Schlüssel.** Wo Sie einen Schlüssel erstellen und womit er beginnt ([Anbieter-Schlüssel](../console/provider-keys.md)).

## Anbieter

| Anbieter | Formate | Host | Schlüssel |
| --- | --- | --- | --- |
| Anthropic `anthropic` | Messages, Token-Zählung | `api.anthropic.com` | [platform.claude.com](https://platform.claude.com/settings/keys), `sk-ant-` |
| Cohere `cohere` | Chat Completions, Embeddings | `api.cohere.ai` | [dashboard.cohere.com](https://dashboard.cohere.com/api-keys), kein festes Präfix |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com/api_keys), `sk-` |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com](https://console.groq.com/keys), `gsk_` |
| Mistral `mistral` | Chat Completions, Embeddings | `api.mistral.ai` | [console.mistral.ai](https://console.mistral.ai/api-keys), kein festes Präfix |
| Ollama Cloud `ollama-cloud` | Chat Completions | `ollama.com` | [ollama.com](https://ollama.com/settings/keys), kein festes Präfix |
| OpenAI `openai` | Responses, Chat Completions, Embeddings, Token-Zählung | `api.openai.com` | [platform.openai.com](https://platform.openai.com/api-keys), `sk-` |
| OpenRouter `openrouter` | Chat Completions, Embeddings | `openrouter.ai` | [openrouter.ai](https://openrouter.ai/settings/keys), `sk-or-v1-` |
| SpaceX `xai` | Responses, Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai/team/default/api-keys), `xai-` |
| Together AI `togetherai` | Chat Completions, Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys), `tgp_v1_` |
| Vercel `vercel` | Chat Completions, Responses, Messages, Embeddings | `ai-gateway.vercel.sh` | [vercel.com](https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys), `vck_` |

## Hinweise

Was Sie über manche Anbieter noch wissen sollten. Die [Anbieterprofile](../development/kpp/openai-chat-providers.md#anbieterprofile) nennen jedes Feld von Chat Completions, das ein Anbieter umbenennt oder ablehnt:

- **Cohere.** Coheres [Compatibility API](https://docs.cohere.com/docs/compatibility-api), die einen Teil von Coheres eigener API abdeckt: Dokumente, Zitate und Rerank sind darüber nicht erreichbar. Cohere lehnt einige Felder ab, etwa `logit_bias` und `parallel_tool_calls`, und erhält einen einzelnen `stop`-String als Liste. Seine Reasoning-Modelle denken nur mit `reasoning_effort` `none` oder `high`. Seine Embed-Modelle wie `embed-v4.0` stehen nicht auf models.dev: Fügen Sie sie als eigene Modelle hinzu, die als Embeddings senden, mit ihrem Preis.
- **DeepSeek.** Erhält `developer`-Nachrichten als `system`-Nachrichten.
- **Groq.** Erhält `developer`-Nachrichten als `system`-Nachrichten und lehnt manche Felder ab, etwa `logit_bias`.
- **Mistral.** Erhält `developer`-Nachrichten als `system`-Nachrichten und manche Felder unter eigenem Namen, etwa `seed` als `random_seed`. Mistral lehnt andere ab, etwa `logit_bias`, ebenso ein Gespräch, das mit einer Assistentennachricht endet.
- **Ollama Cloud.** Modelle, die Ollama auf eigenen Servern betreibt, etwa `gpt-oss:120b`. Modell-IDs behalten Ollamas Tag nach einem Doppelpunkt. Erhält das Ausgabelimit jeder App als `max_tokens`, weil Ollama `max_completion_tokens` ignoriert, und lehnt `logprobs`, `top_logprobs` und `logit_bias` ab, die Ollama annimmt, ohne etwas damit zu tun. Das Reasoning eines Modells erreicht Apps in jedem Format. Ollamas kostenloser Tarif enthält nur einige Modelle: Für die übrigen antwortet Ollama mit 402, das Apps als abgelehnten Anbieter-Schlüssel erhalten. Ollama listet seine Modelle, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel. Ein lokaler Ollama-Server braucht keinen Schlüssel; fügen Sie ihn als [eigenen Anbieter](../console/custom-providers.md) hinzu, der Chat Completions spricht.
- **OpenAI.** Ein Anbieter-Schlüssel kann auch ein ChatGPT-Konto sein, dessen Anfragen sein ChatGPT-Abo nach den Regeln von OpenAI dafür nutzen ([Mit einem ChatGPT-Konto anmelden](../console/provider-keys.md#mit-einem-chatgpt-konto-anmelden)).
- **OpenRouter.** Modell-IDs nennen den Hersteller des Modells, etwa `anthropic/claude-sonnet-4.5`. Rufen Sie sie als `openrouter/anthropic/claude-sonnet-4.5` auf, damit sie nie beim eigenen Anbieter des Herstellers landen ([Wie eine Anfrage ein Modell nennt](../console/models-and-aliases.md#wie-eine-anfrage-ein-modell-nennt)). Kaeso berechnet die Kosten mit den Preisen des Katalogs, nicht mit dem `usage.cost`, das OpenRouter meldet. OpenRouter listet seine Modelle, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel.
- **SpaceX.** Die API von xAI, mit Modellen wie Grok. Ein Anbieter-Schlüssel kann als experimentelle Option auch ein Grok-Konto sein, dessen Abo API-Zugang enthält ([Mit einem Grok-Konto anmelden](../console/provider-keys.md#mit-einem-grok-konto-anmelden)).
- **Vercel.** Vercel AI Gateway, ein weiteres Gateway, dessen eigenes Routing und eigene Fallbacks hinter Kaeso laufen. Modell-IDs nennen den Hersteller des Modells, etwa `openai/gpt-5-nano`. Rufen Sie sie als `vercel/openai/gpt-5-nano` auf, damit sie nie beim eigenen Anbieter des Herstellers landen ([Wie eine Anfrage ein Modell nennt](../console/models-and-aliases.md#wie-eine-anfrage-ein-modell-nennt)). Der Wert **Sendet als** eines Modells kann Responses oder Messages wählen, die Vercel ebenfalls anbietet; über Messages reicht Vercel Anthropics `cache_control` an Claude-Modelle weiter. Kaeso berechnet die Kosten mit den Preisen des Katalogs, nicht mit dem `usage.cost`, das Vercel meldet und das Vercels eigene Aufschläge nicht enthält. Vercel zählt Token nur für Claude-Modelle genau und schätzt sie für die übrigen, daher lehnt Kaeso die Token-Zählung für jedes Vercel-Modell ab. Vercel antwortet zu einem Modell, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel. Ein Vercel-Konto im kostenlosen Tarif kann keine kostenpflichtigen Modelle wie Claude aufrufen: Vercel antwortet mit 403, das Apps als abgelehnten Anbieter-Schlüssel erhalten, und das Anfrageprotokoll behält Vercels Grund.

## Marken

Alle Marken und Logos gehören ihren Inhabern. Kaeso ist mit keinem dieser Anbieter verbunden und wird von keinem unterstützt oder gesponsert. Die Konsole zeigt das Zeichen eines Anbieters nur, um den Anbieter zu nennen, zu dem ein Schlüssel, ein Modell oder eine Anfrage gehört. Anthropic, Cohere und Groq beschränken in ihren Bedingungen die Verwendung ihrer Logos; Kaeso zeigt ihre Zeichen unverändert und nur zu diesem Zweck. [NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md#trademarks) nennt die Quelle jedes Zeichens.
