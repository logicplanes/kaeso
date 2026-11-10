---
title: Chat-Completions-Inferenz-Clouds
description: Was die Inferenz-Clouds offener Modelle zu Chat Completions ergänzen, und wie der Codec ihnen begegnet.
order: 6
nav: Inferenz-Clouds
---

Inferenz-Clouds, die offene Modelle betreiben, sprechen [OpenAI Chat Completions](openai-chat.md) mit eigenen Unterschieden. Schlagen Sie hier nach, was jede ergänzt, bevor Sie ihr Profil ändern; ihre Profile stehen in der Tabelle der [Anbieterprofile](openai-chat-providers.md#anbieterprofile), und die anderen Anbieter stehen unter [Chat-Completions-Anbieter](openai-chat-providers.md).

Ihre Fixtures liegen in `fixtures/openai_chat/providers/<id>/` ([Fixtures](fixtures.md)).

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
