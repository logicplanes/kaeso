---
title: Chat-Completions-Inferenz-Clouds
description: Was die Inferenz-Clouds offener Modelle zu Chat Completions ergänzen, und wie der Codec ihnen begegnet.
order: 6
nav: Inferenz-Clouds
---

Inferenz-Clouds, die offene Modelle betreiben, sprechen [OpenAI Chat Completions](openai-chat.md) mit eigenen Unterschieden. Schlagen Sie hier nach, was jede ergänzt, bevor Sie ihr Profil ändern; ihre Profile stehen in der Tabelle der [Anbieterprofile](openai-chat-providers.md#anbieterprofile), und die anderen Anbieter stehen unter [Chat-Completions-Anbieter](openai-chat-providers.md).

Ihre Fixtures liegen in `fixtures/openai_chat/providers/<id>/` ([Fixtures](fixtures.md)).

## Baseten

Quellen: die Übersicht [Model APIs](https://docs.baseten.co/inference/model-apis/overview) von Baseten, seine OpenAPI-Dokumente für Chat Completions, Messages und Responses, seine Leitfäden zu [Reasoning](https://docs.baseten.co/inference/model-apis/reasoning) und [Streaming](https://docs.baseten.co/inference/streaming) und seine Seite zu [Inferenzfehlern](https://docs.baseten.co/inference/errors), gelesen am 2026-11-10. Nicht mit einem erfolgreichen Aufruf getestet: Die Antwort in `fixtures/openai_chat/providers/baseten/` ist das eigene Beispiel des Reasoning-Leitfadens und der Stream der eigene Auszug des Streaming-Leitfadens, aus dem Schema ergänzt; der Fehler kam live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://inference.baseten.co`, die `Authorization: Bearer` annimmt.

| Baseten-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Thinking, das eine App zurücksendet, erreicht Baseten als `reasoning_content`, das Beam in früheren Assistant-Nachrichten verlangt. |
| `chat_template_args`, `reasoning` mit `max_tokens`, `top_k`, `min_p`, `repetition_penalty` und die anderen Anfragefelder von Baseten | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `max_completion_tokens` | `max_tokens` | Das Schema hat nur `max_tokens` (`max_tokens()`). |
| die Rolle `developer` | `Role::System` | Das Schema führt `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| `user` | `user` | Das Schema hat kein `safety_identifier`, daher geht die ID des Endnutzers als `user` (`end_user_as_user()`). |
| Fehler als `{"error": "<message>"}` | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-invalid-api-key`). |
| ein Stream, der ohne `[DONE]` endet | `KppError` | Baseten kann einen Stream ohne Fehlerereignis vorzeitig beenden; Apps erhalten den Fehler des Stream-Decoders, dass der Stream vor dem Ende der Antwort abbrach. |

Der Messages-Endpunkt von Baseten wird nicht genutzt: Sein Schema nimmt nur Text- und Tool-Blöcke an, ohne Thinking oder Bilder. Auch seine Responses API, die nichts speichert, wird nicht genutzt: Messages- und Responses-Apps erreichen Baseten über Chat Completions. Model APIs bieten weder Embeddings noch Bilder, Audio oder Token-Zählungen an.

## DeepInfra

Quellen: das [OpenAPI document](https://api.deepinfra.com/openapi.json) von DeepInfra und seine Anleitungen zu [Chat Completions](https://docs.deepinfra.com/chat/overview.md), [Streaming](https://docs.deepinfra.com/chat/streaming.md), [Tool-Aufrufen](https://docs.deepinfra.com/chat/tool-calling.md), [Prompt-Caching](https://docs.deepinfra.com/chat/prompt-caching.md) und [Authentifizierung](https://docs.deepinfra.com/account/authentication.md), gelesen am 2026-11-14. Es gab keinen Schlüssel: Eine Antwort in `fixtures/openai_chat/providers/deepinfra/` ist eine Antwort, die DeepInfra gesendet hat, veröffentlicht im Issue eines anderen Projekts, die übrigen und der Stream sind aus den Anleitungen gebaut; die Modellliste und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Die Anleitungen von DeepInfra schreiben die Basis-URL als `https://api.deepinfra.com/v1/openai`; sein OpenAPI-Dokument bietet dieselbe API unter `/v1/…` an, daher ist Kaesos Basis-URL `https://api.deepinfra.com`, die `Authorization: Bearer` annimmt.

| DeepInfra-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils; Assistant-Nachrichten nehmen es zurück. |
| `max_completion_tokens` | `max_tokens` | Das Schema hat nur `max_tokens` (`max_tokens()`). |
| die Rolle `developer` | `Role::System` | Das Schema führt `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| `user` | `user` | Das Schema hat kein `safety_identifier`, daher geht die ID des Endnutzers als `user` (`end_user_as_user()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()`. |
| `parallel_tool_calls`, `top_logprobs`, `logit_bias`, `metadata`, `store` und die anderen Felder, die das Schema nicht kennt | abgelehnt | DeepInfra verwirft nicht deklarierte Felder kommentarlos, daher lehnt das Profil sie ab; `parallel_tool_calls: true` erlaubt nur parallele Aufrufe und wird weggelassen. |
| `top_k`, `min_p`, `repetition_penalty`, `fail_fast`, `models`, `prompt_cache_options`, `cache_salt`, `chat_template_kwargs` und das Objekt `reasoning` | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `usage.estimated_cost`, `prompt_tokens_details` mit `cache_write_tokens`, `name: null` | Extras und rohe Nutzung | Gegenüber anderen Dialekten weggelassen (`responses/greeting-published`, `responses/cached-doc`). |
| ein 403 `{"detail": {"error": …}}` für einen Schlüssel ohne den Scope `inference` | `KppError` | `PermissionDenied`, mit der Meldung von DeepInfra (`errors/missing-scope-doc`). |
| `GET /v1/models` mit `root`, `parent` und DeepInfras `metadata` | `ModelList` | Ohne Schlüssel beantwortet (`models/list-recorded`), daher kann eine Modellprüfung einen falschen Schlüssel nicht erkennen. Es gibt kein `GET /v1/models/{model}`. |

Die Responses API von DeepInfra speichert nichts und wird nicht genutzt: Responses-Apps erreichen DeepInfra über Chat Completions. Sein Anthropic-kompatibler Endpunkt und die Token-Zählung liegen unter `/anthropic/v1`, außerhalb des `/v1` der anderen Formate, daher lehnt Kaeso Token-Zählungen ab. Seine Bilder, Sprache, Transkriptionen und Videos in OpenAIs Formaten werden nicht genutzt, bis KPMR die Modelle mit ihren Preisen listet.

## Fireworks AI

Quellen: Fireworks' Referenz [Create Chat Completion](https://docs.fireworks.ai/api-reference/post-chatcompletions) (sein OpenAPI-Dokument) und die Seiten [OpenAI compatibility](https://docs.fireworks.ai/tools-sdks/openai-compatibility) und [Reasoning](https://docs.fireworks.ai/guides/reasoning), gelesen am 2026-11-09. Es gab keinen Schlüssel, daher sind die Antworten und der Stream in `fixtures/openai_chat/providers/fireworks-ai/` daraus gebaut; die Fehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://api.fireworks.ai/inference`, die `Authorization: Bearer` mit einem `fw_`-Schlüssel annimmt.

| Fireworks-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Thinking, das eine App in einem anderen Format zurückschickt, erreicht Fireworks als `reasoning_content`, wie es sein Reasoning-Leitfaden bei Tool-Aufrufen verlangt. Die Dokumentation beschreibt keine `<think>`-Tags in `content`. |
| `reasoning_effort` mit `xhigh`, `max`, Booleans oder einer Token-Zahl; `thinking` in Anthropics Form; `reasoning_history` | `reasoning.effort`, Extras | Die Effort-Stufen des Katalogs bestimmen, was andere Formate senden; die eigenen Werte einer Chat-App gehen wie geschrieben. |
| `top_k`, `min_p`, `typical_p`, `repetition_penalty`, `mirostat_*`, `context_length_exceeded_behavior`, `prompt_cache_isolation_key`, `perf_metrics_in_response`, `raw_output` und Fireworks' weitere Anfragefelder | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `max_tokens` über dem, was das Kontextfenster übrig lässt | gesendet | Fireworks senkt es, statt es abzulehnen, außer die Anfrage setzt `context_length_exceeded_behavior: "error"`. |
| `user` | `user` | Fireworks' Schema hat kein `safety_identifier`, daher sendet das Profil die ID des Endnutzers als `user` (`end_user_as_user()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()`; die Videomodelle von Fireworks nehmen sie an (`requests/video-url`). |
| `perf_metrics` in einer Antwort oder im Abschluss-Chunk | Extras | Gegenüber anderen Dialekten weggelassen. |
| Usage standardmäßig in einem letzten Chunk mit `choices: []` | `Usage` | Auch ohne `stream_options.include_usage`. |
| Fehler im `error`-Objekt von OpenAI mit einem `code` in Großbuchstaben wie `UNAUTHORIZED`, `type: "error"` und einer `request_id` auf oberster Ebene | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | Ein falscher Schlüssel erhält 401 (`errors/list-invalid-api-key`), sodass eine Modellprüfung ihn erkennt. Fireworks dokumentiert auf diesem Pfad keine Liste; die Konsole nimmt seine Modelle aus KPMR. |

Fireworks dokumentiert keinen Endpunkt für Bilder, Sprache, Transkription, Video oder Token-Zählung, daher bleiben diese leer und `count_tokens` ist `None`. Seine Responses API meldet die Usage als `prompt_tokens` und `completion_tokens` statt unter OpenAIs Namen, daher sendet Kaeso ihm kein Responses; Responses-Apps erreichen Fireworks über Chat Completions.

## Hugging Face

Quellen: die Übersicht [Inference Providers](https://huggingface.co/docs/inference-providers/index) von Hugging Face, seine Referenz [chat completion](https://huggingface.co/docs/inference-providers/tasks/chat-completion) mit den Schemas in `huggingface.js` und seine Leitfäden [Claude Code](https://huggingface.co/docs/inference-providers/integrations/claude-code) und [Responses API](https://huggingface.co/docs/inference-providers/guides/responses-api), gelesen am 2026-11-10. Nicht mit einem erfolgreichen Aufruf getestet: Die Antwort und der Stream in `fixtures/openai_chat/providers/huggingface/` sind aus der Referenz gebaut, mit dem Thinking in dem Feld, das der Anbieter hinter dem Router dokumentiert; die Modellliste und der Fehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://router.huggingface.co`, die `Authorization: Bearer` mit einem `hf_`-Token annimmt.

| Hugging-Face-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `message.reasoning` und ihre Deltas | `Block::Reasoning` | Über `reasoning_fields` des Profils; der Router gibt das Feld des Anbieters hinter ihm weiter. |
| `max_completion_tokens` | `max_tokens` | Das Schema hat nur `max_tokens` (`max_tokens()`). |
| die Rolle `developer` | `Role::System` | `role` geht unverändert an Anbieter, die nicht alle `developer` annehmen, daher sendet `system_role()` `system`. |
| die ID eines Endnutzers | weggelassen | Das Schema hat kein `user` oder `safety_identifier` (`without_end_user()`). |
| ein Suffix `:provider`, `:fastest`, `:cheapest` oder `:preferred` an der Modell-ID | `model` | Unverändert gesendet; die Antwort nennt das Modell ohne Suffix (`responses/provider-suffix`). |
| `tool_prompt` | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| Fehler als `{"error": "<message>"}` | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` mit `architecture` und einem Array `providers` mit Status, Kontextlänge, Preisen und Geschwindigkeit | `ModelList` | Ohne Schlüssel beantwortet (`models/list-recorded`), daher kann eine Modellprüfung einen falschen Schlüssel nicht erkennen. |

Die OpenAI-kompatible API des Routers ist nur für Chat: `/v1/embeddings`, `/v1/images/generations` und `/v1/audio/*` antworten mit 404. Seine Responses API wird nicht genutzt, da sie ein abgelehntes Token mit 200 und `status: "failed"` beantwortet. Auch sein Messages-Endpunkt wird nicht genutzt: Er beantwortet ein abgelehntes Token mit `api_error`, das Apps als fehlgeschlagenen Anbieter statt als abgelehnten Schlüssel erreichen würde. `/v1/messages/count_tokens` antwortet, ohne das Token zu prüfen, und ist nicht dokumentiert, daher lehnt Kaeso Token-Zählungen ab.

## Nebius Token Factory

Quellen: das [OpenAPI document](https://api.tokenfactory.nebius.com/openapi.json) von Nebius (Version `20261105-4c58718c5`), sein Beispiel zur [Textgenerierung](https://docs.tokenfactory.nebius.com/api-reference/examples/text-generation) und seine [Übersicht](https://docs.tokenfactory.nebius.com/ai-models-inference/overview), gelesen am 2026-11-09. Es gab keinen Schlüssel, daher sind die Antworten und der Stream in `fixtures/openai_chat/providers/nebius/` daraus gebaut, eine davon ist das Beispiel der Dokumentation selbst; die Fehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://api.tokenfactory.nebius.com`, die `Authorization: Bearer` annimmt.

| Nebius-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content`, `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils; das Feld hängt vom Modell ab, wie bei Together AI. |
| die Rolle `developer` | `Role::System` | Nebius führt nur `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| `user` | `user` | Das Schema von Nebius hat kein `safety_identifier`, daher geht die ID des Endnutzers als `user` (`end_user_as_user()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()` (`requests/video-url`). |
| vLLM-Sampling-Felder wie `top_k` und `repetition_penalty`, `extra_body`, `partial` und Kimis `tools` in Nachrichten | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `service_tier` in jeder Antwort, `stop_reason`, `function_call: null`, `tool_calls: []`, `system_fingerprint: null` | Extras | Gegenüber anderen Dialekten weggelassen (`responses/text-generation-doc`). |
| `response_format` mit einem JSON-Schema | gesendet | Die Beschreibung von Nebius erlaubt nur `json_object` und `text`; Nebius beantwortet ein Schema selbst. |
| Fehler mit OpenAIs `error`-Objekt und einem `detail` auf oberster Ebene | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | Ein falscher Schlüssel erhält 401 nur mit `{"detail": "…"}` (`errors/list-invalid-api-key`), das Apps in OpenAIs Form mit der Meldung von Nebius erhalten, sodass eine Modellprüfung ihn erkennt. |

Nebius bietet weder Bilder, Audio, Messages noch Token-Zählungen an: Sein OpenAPI-Dokument hat keinen solchen Pfad, und `/v1/images/generations`, `/v1/audio/transcriptions` und `/v1/messages` antworten mit 404. Seine Responses API wird nicht genutzt: Responses-Apps erreichen Nebius über Chat Completions.

## NVIDIA

Quellen: die Referenz [LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) von NVIDIA und die OpenAPI-Dokumente seiner Modellseiten wie [`nvidia/nemotron-3-super-120b-a12b`](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer), gelesen am 2026-11-10. Nicht mit einem erfolgreichen Aufruf getestet: Die Antwort und der Stream in `fixtures/openai_chat/providers/nvidia/` sind daraus gebaut, die Anfrage des Streams ist das eigene Beispiel der Referenz; die Modellliste und die Fehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://integrate.api.nvidia.com`, die `Authorization: Bearer` mit einem `nvapi-`-Schlüssel annimmt.

| NVIDIA-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. |
| `chat_template_kwargs` (`enable_thinking`, `low_effort`), `reasoning_budget` | Extras | Die eigenen Thinking-Schalter der Modelle; die einer Chat-App gehen unverändert, abgelehnt, wenn die Anfrage an einen anderen Dialekt geht. |
| `max_completion_tokens` | `max_tokens` | Die Anfrageschemas haben nur `max_tokens` (`max_tokens()`). |
| die Rolle `developer` | `Role::System` | Die Schemas führen `system`, `context`, `user` und `assistant`, daher sendet `system_role()` `system`. |
| die ID eines Endnutzers | weggelassen | Die Schemas haben kein `user` oder `safety_identifier` (`without_end_user()`). |
| Fehler als Problemdokumente nach RFC 9457 mit `status`, `title` und `detail` | `KppError` | Die Art folgt dem Status: Ein Schlüssel, den NVIDIA nicht kennt, erhält 403 (`errors/recorded-invalid-api-key`), ein ausgemustertes Modell 410 (`errors/recorded-retired-model`). |
| eine 202 mit einem Header `NVCF-REQID` | — | Die Antwort von NVIDIA, wenn das Ergebnis noch nicht fertig ist, zum Abfragen; Kaeso fragt nicht ab. |
| `GET /v1/models` | `ModelList` | Ohne Schlüssel beantwortet (`models/list-recorded`), daher kann eine Modellprüfung einen falschen Schlüssel nicht erkennen. |

NVIDIA bietet weder Responses noch Messages oder Token-Zählungen an: `/v1/responses` und `/v1/messages/count_tokens` antworten mit 404, und `/v1/messages` antwortet für jedes Modell mit `model not found`. Seine Embedding-Modelle nehmen `input_type` und `truncate` an, die in den Extras bleiben ([OpenAI Embeddings](openai-embeddings.md#nvidia)).

## SambaNova

Quellen: das [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) von SambaNova (SambaNova cloud API 1.2.0) und seine Seiten [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility), [Textgenerierung](https://docs.sambanova.ai/docs/en/features/text-generation) und [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes), gelesen am 2026-11-09. Es gab keinen Schlüssel: `fixtures/openai_chat/providers/sambanova/` enthält die Beispielantwort und den Beispiel-Stream des Dokuments und daraus gebaute Antworten, die Modellliste, wie SambaCloud sie ohne Schlüssel liefert, und Fehler, die live ohne gültigen Schlüssel kamen. Kaesos Basis-URL ist `https://api.sambanova.ai`, die `Authorization: Bearer` annimmt.

| SambaNova-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `delta.reasoning`, `channel` | `Block::Reasoning`, Extras | Über `reasoning_fields` des Profils. Assistentennachrichten nehmen `reasoning` zurück. |
| `chat_template_kwargs` (`enable_thinking`), `top_k`, `repetition_penalty` | Extras | DeepSeek-V3.2 und Gemma 4 denken nur mit `enable_thinking`, das nur eine Chat-App senden kann. Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht. |
| `frequency_penalty`, `presence_penalty` | abgelehnt | SambaNova nimmt sie an und ignoriert sie, daher lehnt das Profil sie vor dem Senden ab. |
| die ID eines Endnutzers, das `prompt_cache_key` einer Responses-App | weggelassen | Das Anfrageschema hat keines von beiden und lehnt nicht deklarierte Eigenschaften ab (`without_end_user()`, `without_cache_key()`). Andere nicht deklarierte Felder gehen wie geschrieben, für SambaNovas eigenen 400. |
| `video_url`-Teile | `Block::Video` | `with_video_url()`; Gemma 4 nimmt Videos an. |
| ein `created` als Gleitkommazahl, `system_fingerprint` | Extras | `responses/palindrome-poem-doc`. |
| `usage` mit SambaNovas Zeitfeldern (`time_to_first_token`, `total_latency`, `acceptance_rate`, `is_last_response` und weitere) | `Usage`, `raw`-Usage | In Antworten und im letzten Chunk eines Streams, der `choices: []` hat (`streams/stream-length-doc`). |
| Fehler mit OpenAIs `error`-Objekt und einer `request_id` auf oberster Ebene; 410 `model_deprecated` | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel, 410 nicht gefunden. |
| `GET /v1/models`, `GET /v1/models/{id}` | `ModelList`, `ModelInfo` | Ohne Schlüssel beantwortet, mit `context_length`, `max_completion_tokens` und `pricing` (`models/list-recorded`), daher kann eine Modellprüfung einen falschen Schlüssel nicht erkennen. |

SambaCloud bietet weder Embeddings noch Whisper an; SambaNova dokumentiert beides nur für SambaStack, und SambaCloud antwortet für deren Modelle mit `model_not_found` (`errors/recorded-embeddings-model-not-found`). Seine Responses API wird nicht genutzt: Responses-Apps erreichen SambaNova über Chat Completions.

## Scaleway

Quellen: die [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/) von Scaleway, sein [Reasoning-Leitfaden](https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/), sein [Audio-Leitfaden](https://www.scaleway.com/en/docs/generative-apis/how-to/query-audio-models/) und seine [Fehlerseite](https://www.scaleway.com/en/docs/generative-apis/api-cli/understanding-errors/), gelesen am 2026-11-09. Es gab keinen Schlüssel, daher sind die Antworten und der Stream in `fixtures/openai_chat/providers/scaleway/` daraus gebaut, die ersten Chunks des Streams aus dem Beispiel des Reasoning-Leitfadens; die Fehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://api.scaleway.ai`, die `Authorization: Bearer` mit einem geheimen IAM-Schlüssel annimmt und das Standardprojekt des Schlüssels erreicht.

| Scaleway-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Nur ältere Modelle setzen das Thinking in `<think>`-Tags in `content`, und diese haben das Ende ihrer Laufzeit erreicht. |
| die Rolle `developer` | `Role::System` | Scaleway führt nur `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| die ID eines Endnutzers | weggelassen | Das Anfrageschema hat kein `user` oder `safety_identifier` (`without_end_user()`). |
| `input_audio`-Teile in `wav` oder `mp3` | `Block::Audio` | Wie bei OpenAI (`requests/input-audio`). |
| `token_ids` in Stream-Choices | Extras | Gegenüber anderen Dialekten weggelassen. |
| Fehler mit dem HTTP-Status als Zahl in `error.code`, einem `type` wie `unauthorized` oder `forbidden` und `param: ""` | `KppError` | Die Art folgt dem Status. Ein Schlüssel, den Scaleway nicht kennt, erhält 403 `forbidden` (`errors/recorded-invalid-api-key`), wie ein Schlüssel ohne Berechtigung für Generative APIs, daher erhalten Apps ihn als Schlüssel, dem die Anfrage nicht erlaubt ist. |
| die ältere Fehlerform `{"object": "error", "message", "type", "param", "code"}` | `KppError` | Ältere Modelle senden sie noch (`errors/legacy-model-error-doc`); Apps erhalten einen OpenAI-Fehler mit ihrer Meldung. |

Scaleway bietet weder Messages noch Bilder, Sprache oder Token-Zählungen an: `/v1/messages`, `/v1/images/generations` und `/v1/audio/speech` antworten mit 404 `route_not_found`. Seine zustandslose Responses API, nur mit Funktionswerkzeugen, wird nicht genutzt: Responses-Apps erreichen Scaleway über Chat Completions.

## SiliconFlow

Quellen: die Referenz [chat completions](https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions) von SiliconFlow, sein Leitfaden [reasoning](https://docs.siliconflow.com/en/userguide/capabilities/reasoning) und seine Seite [error handling](https://docs.siliconflow.com/en/faqs/error-code), gelesen am 2026-11-10. Nicht mit einem erfolgreichen Aufruf getestet: Die Anfragen in `fixtures/openai_chat/providers/siliconflow/` sind die eigenen Beispiele des Reasoning-Leitfadens, die Antwort und der Stream sind aus der Referenz gebaut; der Fehler kam live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://api.siliconflow.com`, die `Authorization: Bearer` mit einem `sk-`-Schlüssel annimmt.

| SiliconFlow-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Stream-Deltas tragen `content: null` neben dem Feld, das sie füllen. |
| `enable_thinking`, `thinking_budget` | `reasoning` | `thinking_switch()`, wie bei QwenCloud: Das Thinking einer App schaltet sie ein oder aus; die eigenen Werte einer Chat-App gehen unverändert. |
| `max_tokens` | `max_tokens` | Es begrenzt nur die Antwort, daher `max_tokens()` und `reasoning_beyond_output_limit` des Anbieters. |
| die Rolle `developer` | `Role::System` | Die Referenz führt `system`, `user` und `assistant`, daher sendet `system_role()` `system`. |
| die ID eines Endnutzers | weggelassen | Die Referenz hat kein `user` oder `safety_identifier` (`without_end_user()`). |
| `min_p`, `top_k` | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| Fehler als `{"code", "message", "data"}` | `KppError` | Die Meldung von SiliconFlow; die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-invalid-api-key`). |

SiliconFlow bietet weder Responses noch Token-Zählungen an: `/v1/responses` und `/v1/messages/count_tokens` antworten mit 404. Sein Messages-Endpunkt wird nicht genutzt, da seine Referenz nur Text- und Tool-Blöcke dokumentiert. Seine Embeddings, Sprache und Transkriptionen nutzen die Formen von OpenAI ([OpenAI Embeddings](openai-embeddings.md#siliconflow)); seine Bilder und Videos nicht.
