---
title: Chat-Completions-Inferenz-Clouds
description: Was die Inferenz-Clouds offener Modelle zu Chat Completions ergänzen, und wie der Codec ihnen begegnet.
order: 6
nav: Inferenz-Clouds
---

Inferenz-Clouds, die offene Modelle betreiben, sprechen [OpenAI Chat Completions](openai-chat.md) mit eigenen Unterschieden. Schlagen Sie hier nach, was jede ergänzt, bevor Sie ihr Profil ändern; ihre Profile stehen in der Tabelle der [Anbieterprofile](openai-chat-providers.md#anbieterprofile), und die anderen Anbieter stehen unter [Chat-Completions-Anbieter](openai-chat-providers.md).

Ihre Fixtures liegen in `fixtures/openai_chat/providers/<id>/` ([Fixtures](fixtures.md)).

## Amazon Bedrock

Quellen: die Anleitungen von AWS zu [Endpunkten](https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html), [Chat Completions](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions.html), [Messages](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html) und [API-Schlüsseln](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html) und seine Tabelle zur [API-Kompatibilität](https://docs.aws.amazon.com/bedrock/latest/userguide/models-api-compatibility.html), gelesen am 2026-11-14. Nicht mit einem erfolgreichen Aufruf getestet: AWS veröffentlicht keine Antwort dieser Pfade, daher enthält `fixtures/openai_chat/providers/amazon-bedrock/` die Anfragen der Anleitungen und Fehler, die live mit erfundenen Schlüsseln beantwortet wurden. Jeder Schlüssel ruft eine Region auf, unter `https://bedrock-runtime.{region}.amazonaws.com`, mit `Authorization: Bearer`; der Eintrag `Regional` des Anbieters in `provider.rs` listet die Regionen und gibt jedem Dialekt dort seinen API-Stamm.

| Feld bei Amazon Bedrock | KPP | Hinweise |
|---|---|---|
| Chat Completions unter `/openai/v1/chat/completions` | `EgressCtx::api_root` | `/openai/v1` tritt an die Stelle von `/v1`; der Body ist der von OpenAI. Claude-Modelle gehen stattdessen an `/anthropic/v1/messages` ([Messages-Anbieter](anthropic-messages-providers.md#amazon-bedrock)). |
| `amazon-bedrock-guardrailConfig` | Extras | Bedrocks eigenes Body-Feld für eine Guardrail (`requests/guardrail-config-doc`); abgelehnt, wenn die Anfrage an einen anderen Dialekt geht. |
| 401 `permission_denied_error` mit `code: "access_denied"` | `KppError` | `Authentication` aus dem Status; ein fehlerhafter, ein unsignierter und ein unbekannter Schlüssel haben je eine eigene Meldung (`errors/recorded-*-key*`). |
| `{"Message": …}` mit `x-amzn-ErrorType` | `KppError` | Die Form, mit der bedrock-runtime seine eigenen Operationen beantwortet; die Meldung wird aus `Message` gelesen, die Art aus dem Status (`errors/recorded-native-invalid-api-key`). |
| `GET /openai/v1/models` | — | Nicht angeboten (404 `UnknownOperationException`), daher ist eine Modellprüfung unklar und sendet nichts. |

Ein Modell, für das der Katalog kein Format nennt, bedient Bedrock nur über seine eigenen APIs InvokeModel und Converse, deren AWS-Event-Streams KPP nicht liest; es wird vor dem Senden abgelehnt. Bedrocks Responses-Pfad wird nicht genutzt: Responses-Apps erreichen Bedrock über Chat Completions. Token-Zählungen werden abgelehnt: bedrock-runtime zählt sie nur im eigenen Format, unter `/model/{modelId}/count-tokens`.

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

## Cerebras

Quellen: die Seiten [Chat-Completions-Referenz](https://inference-docs.cerebras.ai/api-reference/chat-completions) und [OpenAI-Kompatibilität](https://inference-docs.cerebras.ai/resources/openai) von Cerebras, gelesen am 2026-11-02, und die Live-Aufnahmen in `fixtures/openai_chat/providers/cerebras/`, aufgenommen mit `gpt-oss-120b` und `qwen-3.8-27b`. Kaesos Basis-URL ist `https://api.cerebras.ai`, die `Authorization: Bearer` annimmt.

| Cerebras-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils, wie bei Groq. Thinking, das eine App in einem anderen Format zurückschickt, erreicht Cerebras als `reasoning`, das es annimmt (`responses/recorded-reasoning-sent-back`). Das `reasoning_content` einer Chat-App, etwa von Grok CLI, beantwortet Cerebras mit 400, deshalb geht es ebenfalls als `reasoning` (`replay_reasoning_as_read()`). |
| `time_info`, `system_fingerprint`, `choices[].reasoning_logprobs` | Extras | Gegenüber anderen Dialekten weggelassen. |
| `completion_tokens_details.accepted_prediction_tokens` und `rejected_prediction_tokens`, `prompt_tokens_details.image_tokens` | `raw`-Usage | Die gecachten und die Reasoning-Tokens sind wie unter [Nutzung](openai-chat.md#nutzung-completionusage) typisiert. |
| `usage` und `time_info` im Abschluss-Chunk, auch ohne `stream_options.include_usage` | `Usage` | Cerebras sendet nie einen Chunk mit `choices: []`. |
| `reasoning_effort` | `reasoning.effort` | `gpt-oss-120b` nimmt `low`, `medium` und `high`, `qwen-3.8-27b` zusätzlich `none`, wie KPMR sie führt; andere Stufen lehnt Kaeso vor dem Senden ab, und Cerebras beantwortet sie mit 400 (`errors/recorded-reasoning-effort-*`). `max_completion_tokens` zählt das Reasoning mit. |
| `reasoning_format`, `clear_thinking` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| Felder, die Cerebras nicht deklariert, etwa `safety_identifier`, `metadata`, `store` oder `web_search_options` | gesendet | Cerebras antwortet mit 400 `property '…' is unsupported`, das die App erreicht. Das Profil sendet die ID des Endnutzers als `user` (`end_user_as_user()`), sodass das `metadata.user_id` von Claude Code dort ankommt. |
| `logit_bias`, `logprobs`, `top_logprobs`, beide Penalties, `seed`, `stop`, `prompt_cache_key`, `parallel_tool_calls`, `service_tier`, `prediction` | gesendet | Cerebras nimmt sie an (`responses/recorded-logit-bias-honoured` zeigt den Bias in Aktion). Eine `stop`-Sequenz beendet auch das Reasoning, sodass eine Antwort ohne `content` bleiben kann (`responses/recorded-sampling`). |
| die Rolle `developer`, eine `system`-Nachricht nach der ersten Runde | `Role::System` | Qwens Chat-Template lehnt beide mit 400 ab. `system_role()` sendet `system`, und `leading_system_only()` verschiebt den Text einer späteren Systemnachricht einer App in einem anderen Format, etwa Claude Code oder Codex, in die nächste Benutzerrunde. |
| eine abschließende Assistentennachricht | abgelehnt | Cerebras beantwortet sie neu und wiederholt sie (`responses/recorded-prefill`), deshalb lehnt `without_prefill()` sie vor dem Senden ab. |
| `refusal` in einer Assistentennachricht, auch `null` | abgelehnt | Cerebras antwortet mit 400 (`errors/recorded-assistant-refusal`); das Profil lehnt es vor dem Senden ab. |
| eine Funktion ohne `parameters` | `parameters` | `with_tool_parameters()` sendet das leere Objekt-Schema, das das Chat-Template von `gpt-oss-120b` verlangt (`errors/recorded-tool-without-parameters`). |
| `tools` zusammen mit `response_format` bei `gpt-oss-120b`, ein Bild als `https`-URL oder an ein Modell ohne Bilder | gesendet | Cerebras antwortet mit 400 (`errors/recorded-tools-with-response-format`, `errors/recorded-image-url`, `errors/recorded-image-gpt-oss`), das die App erreicht. |
| Fehler mit OpenAIs `message`, `type`, `param` und `code` auf oberster Ebene, ohne das Objekt `error` | `KppError` | Meldung und `param` stammen von Cerebras, die Art folgt dem Status: Ein falscher Schlüssel (401 `wrong_api_key`) ist ein abgelehnter Schlüssel, das Token-Limit pro Minute des Kontos (429 `too_many_tokens_error`, `retry-after: 60`) ein Ratenlimit. Chat-Apps erhalten den Fehler in OpenAIs Form. |
| `GET /v1/models` | `ModelList` | Die Modelle, die der Schlüssel aufrufen darf, mit `created: 0` und `owned_by: "Cerebras"`; ein falscher Schlüssel erhält 401, sodass eine Modellprüfung ihn erkennt. |

Cerebras hat keinen Endpunkt für Responses, Messages, Embeddings oder Token-Zählung (alle antworten mit 404), deshalb sind `embeddings` und `count_tokens` `None`.

## Azure

Quellen: Azures v1-OpenAPI-Dokument (`azure-v1-v1-generated.json` in [azure-rest-api-specs](https://github.com/Azure/azure-rest-api-specs/tree/main/specification/ai/data-plane/OpenAI.v1)) und die Seiten [API-Lebenszyklus](https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle), [Endpunkte](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints), [Content-Streaming](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/content-streaming) und [Inhaltsfilter](https://learn.microsoft.com/en-us/azure/foundry-classic/openai/concepts/content-filter-annotations), gelesen am 2026-11-14. Es stand kein Schlüssel zur Verfügung: Eine Antwort in `fixtures/openai_chat/providers/azure/` ist eine Antwort, die Azure gesendet hat, veröffentlicht in einer Stack-Overflow-Antwort, die übrigen und die Streams sind die Beispiele der Dokumentation; die Schlüsselfehler wurden live mit einem erfundenen Schlüssel beantwortet. Jeder Schlüssel trägt den Endpunkt seiner Ressource, etwa `https://example.openai.azure.com`, und Kaeso ruft `{endpoint}/openai/v1/…` mit dem Schlüssel in `api-key` auf, ohne `api-version`.

| Azure-Feld | KPP | Hinweise |
|---|---|---|
| `model` | der Name des Deployments | Der `upstream_model` des Modells, wenn es einen hat, sonst seine ID. |
| `prompt_filter_results` (oder `prompt_annotations`), `choices[].content_filter_results` | Extras | Kommen für Chat-Apps unverändert zurück, gegenüber anderen Dialekten ausgelassen (`responses/greeting-published`). |
| ein erster Chunk mit leerem `id`, `object` und `model` und `choices: []` | Start | ID und Modell der Antwort kommen aus dem nächsten Chunk (`streams/filter-pass-doc`). |
| Chunks, deren Choice nur `content_filter_results` und `content_filter_offsets` enthält | Extras | Auch nach `finish_reason: "stop"`; sie tragen keinen Text. |
| `finish_reason: "content_filter"` | `StopReason::ContentFilter` | Auch in einem Chunk ohne `delta` (`streams/filter-blocked-doc`) und in einer Antwort ohne `content` (`responses/filtered-blocklist-doc`). |
| ein 400 mit `code: "content_filter"` und `innererror` | `KppError` | `ContentFiltered`, mit Azures Meldung (`errors/content-filter-prompt-doc`). |
| `error.code` als Zahl in einem String, etwa `"401"` | `KppError` | Die Art folgt dem Status (`errors/recorded-invalid-api-key`). |
| `{"statusCode": 401, "message": …}` | `KppError` | Azures Antwort auf ein Entra-ID-Token, das es nicht annimmt (`errors/recorded-entra-token`); bei einem Schlüssel, der sich mit Entra ID anmeldet, ein abgelehnter Schlüssel, der die Rolle nennt, die der Service Principal braucht. |
| `GET /openai/v1/models` | `ModelList` | Die Modelle, die eine Ressource bereitstellen kann, nicht ihre Deployments; eine Modellprüfung zeigt daher nur, ob der Schlüssel und sein Endpunkt funktionieren (`ModelListing::Catalogue`). |

Die v1-API bietet Responses mit OpenAIs Anfragekörper an, dazu `content_filters` in Antworten, die in den Extras bleiben; Kaeso sendet ein Modell als Responses, wenn sein Wert **Sendet als** das sagt (`fixtures/openai_responses/providers/azure/`). Claude-Deployments beantworten nur Anthropics Messages API unter `{endpoint}/anthropic`, und Bilder, Sprache, Transkriptionen und Videos gibt es in der v1-API nur in der Vorschau, daher ruft Kaeso keines davon auf. Token-Zählungen werden abgelehnt.

## DeepInfra

Quellen: das [OpenAPI document](https://api.deepinfra.com/openapi.json) von DeepInfra und seine Anleitungen zu [Chat Completions](https://docs.deepinfra.com/chat/overview.md), [Streaming](https://docs.deepinfra.com/chat/streaming.md), [Tool-Aufrufen](https://docs.deepinfra.com/chat/tool-calling.md), [Prompt-Caching](https://docs.deepinfra.com/chat/prompt-caching.md) und [Authentifizierung](https://docs.deepinfra.com/account/authentication.md), gelesen am 2026-11-14. Es gab keinen Schlüssel: Eine Antwort in `fixtures/openai_chat/providers/deepinfra/` ist eine Antwort, die DeepInfra gesendet hat, veröffentlicht im Issue eines anderen Projekts, die übrigen und der Stream sind aus den Anleitungen gebaut; die Modellliste und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Die Anleitungen von DeepInfra schreiben die Basis-URL als `https://api.deepinfra.com/v1/openai`; sein OpenAPI-Dokument bietet dieselbe API unter `/v1/…` an, daher ist Kaesos Basis-URL `https://api.deepinfra.com`, die `Authorization: Bearer` annimmt.

| DeepInfra-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils; Assistant-Nachrichten nehmen es zurück. |
| `max_completion_tokens` | `max_tokens` | Das Schema hat nur `max_tokens` (`max_tokens()`). |
| die Rolle `developer` | `Role::System` | Das Schema führt `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| `user` | `user` | Das Schema hat kein `safety_identifier`, daher geht die ID des Endnutzers als `user` (`end_user_as_user()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()`. |
| `parallel_tool_calls`, `top_logprobs`, `logit_bias`, `metadata`, `store` und die anderen Felder, die das Schema nicht kennt | abgelehnt | DeepInfra verwirft nicht deklarierte Felder kommentarlos, daher lehnt das Profil sie ab; `parallel_tool_calls: true` erlaubt nur parallele Aufrufe und wird weggelassen, ebenso ein [neutraler Wert](sampling.md#neutrale-werte) wie `store: false`. |
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

## Groq

Quellen: die [API-Referenz](https://console.groq.com/docs/api-reference) und der [Reasoning-Leitfaden](https://console.groq.com/docs/reasoning) von Groq, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/groq/`.

| Groq-Feld | KPP | Hinweise |
|---|---|---|
| `reasoning_format`, `include_reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `user`, `safety_identifier` (Anfrage) | KPP `user` | Groq nimmt `user` an und lehnt `safety_identifier` ab, daher sendet das Profil die ID des Endnutzers als `user` (`end_user_as_user()`). |
| `prompt_cache_key` (Anfrage) | Extras | Groq lehnt ihn ab. Den eigenen Schlüssel einer Chat-App lehnt das Gateway daher vor dem Senden ab; den Schlüssel einer OpenAI-Responses-App, den Codex bei jeder Anfrage sendet, lässt es weg und nennt ihn in `kaeso-left-out` (`without_cache_key()`). |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). |
| `delta.channel` (`analysis` beim Reasoning von GPT-OSS) | Extras des Chunks | Chat-Apps erhalten es mit dem Reasoning zurück; gegenüber anderen Dialekten weggelassen. |
| `x_groq`, `usage_breakdown`, `service_tier` (`on_demand`) | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time` | `raw` der Nutzung | Die Token-Zahlen, `reasoning_tokens` eingeschlossen, sind wie unter [Nutzung](openai-chat.md#nutzung-completionusage) typisiert. |
| `usage` im Chunk mit `finish_reason`, auch ohne `stream_options.include_usage` | `Usage` | Jeder Dialekt erhält die Nutzung, ob Groq zusätzlich den Nutzungs-Chunk sendet, den `include_usage` anfordert, oder nicht. |
| zusätzliche Modellfelder (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features` und weitere) | Modell-Extras | |

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

## IONOS AI Model Hub

Quellen: die Referenz [create chat completion](https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/chat-completions/create-chat-completion) von IONOS (OpenAPI 3.0.3), seine [Fehlercodes](https://docs.ionos.com/cloud/ai/ai-model-hub/error-codes) und seine Modellseiten, gelesen am 2026-11-14. Es gab kein Token, daher sind die Antworten in `fixtures/openai_chat/providers/ionos/` die Beispiele der Modellseiten und der Stream ist um den letzten Chunk gebaut, den die Dokumentation zeigt; die Schlüsselfehler kamen live, ohne gültiges Token. Kaesos Basis-URL ist `https://openai.inference.de-txl.ionos.com` in Berlin, die `Authorization: Bearer` mit einem IONOS-Cloud-API-Token annimmt.

| IONOS-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `message.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils; GPT OSS sendet dasselbe Thinking in beiden, Qwen3.8 nur in `reasoning` (`responses/gpt-oss-reasoning-doc`, `responses/qwen-reasoning-doc`). |
| die Rolle `developer` | `Role::System` | Nur manche Modelle nehmen `developer` an; IONOS empfiehlt `system` (`system_role()`). |
| `user` | `user` | Das Schema hat kein `safety_identifier` (`end_user_as_user()`). |
| `prompt_cache_key` | weggelassen | IONOS speichert Präfixe selbst im Cache und hat kein solches Feld (`without_cache_key()`). |
| `store`, `metadata`, `service_tier` und die anderen Felder, die das Schema nicht deklariert | abgelehnt | Die Dokumentation sagt nicht, ob IONOS sie ablehnt oder verwirft. |
| `stop_reason`, `token_ids`, `prompt_logprobs`, `prompt_token_ids`, `kv_transfer_params` und die `null`-Felder der Nachricht | Extras | Gegenüber anderen Dialekten weggelassen (`responses/mistral-image-doc`). |
| `{"httpStatus", "messages": [{"errorCode", "message"}]}` | `KppError` | Die eigene Fehlerform von IONOS, die Meldungen verbunden; die Art folgt dem Status (`errors/recorded-invalid-api-key`, `errors/validation-error-doc`). Nur 529 kommt in OpenAIs Form. |
| `GET /v1/models` | `ModelList` | Braucht ein Token und liefert nur IDs; es gibt kein `GET /v1/models/{model}`. |

Die Responses API von IONOS speichert keinen Zustand und wird nicht genutzt: Responses-Apps erreichen IONOS über Chat Completions. Seine Bilder (FLUX), Transkriptionen (Whisper) und sein Rerank werden nicht genutzt, bis KPMR diese Modelle listet; Sprache, Video, Messages und Token-Zählungen bietet es nicht an.

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

## Novita AI

Quellen: die Referenzen von Novita AI zu [Chat Completion](https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md), [Embeddings](https://docs.novita.ai/api-reference/model-apis-llm-create-embeddings.md), [Modellliste](https://docs.novita.ai/api-reference/model-apis-llm-list-models.md) und [Fehlercodes](https://docs.novita.ai/api-reference/basic-error-code.md) und seine Anleitungen zu [Reasoning](https://docs.novita.ai/guides/llm-reasoning.md), [Interleaved Thinking](https://docs.novita.ai/guides/llm-interleaved-thinking.md), [Prompt-Cache](https://docs.novita.ai/guides/llm-prompt-cache.md) und die [FAQ](https://docs.novita.ai/guides/LLM-FAQ.md), gelesen am 2026-11-14. Es gab keinen Schlüssel: Die Antworten in `fixtures/openai_chat/providers/novita-ai/` sind Beispiele aus den Anleitungen, der Stream ist aus der Referenz gebaut; die Modellliste, ein Modell und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Die Basis-URL ist `https://api.novita.ai/openai`, unter der die Referenz `/v1/chat/completions`, `/v1/embeddings` und `/v1/models` anbietet, mit `Authorization: Bearer`.

| Novita-AI-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Nur mit `separate_reasoning: true`, das das Profil sendet (`separate_reasoning()`); ohne bleibt das Reasoning in `content`. Das eigene `separate_reasoning` einer Chat-App bleibt erhalten. |
| `enable_thinking` | `ReasoningConfig` | `thinking_switch()`; das Budget einer Anthropic-App geht als `thinking_budget`, das die Referenz nicht nennt. |
| `max_completion_tokens` | `max_tokens` | Die Referenz hat nur `max_tokens`, das auch Reasoning-Tokens zählt. |
| die Rolle `developer` | `Role::System` | Die Referenz führt `system`, `user` und `assistant`; ihre Anleitungen senden auch `tool`-Nachrichten. |
| `user`, `safety_identifier` | weggelassen | Die Referenz hat kein Feld für den Endnutzer (`without_end_user()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()`. |
| `store`, `metadata`, `prompt_cache_key`, `parallel_tool_calls`, `service_tier` und die anderen Felder, die die Referenz nicht nennt | abgelehnt | Die Referenz sagt nicht, ob Novita AI nicht deklarierte Felder ablehnt, daher lehnt das Profil sie ab; `parallel_tool_calls: true` wird weggelassen. `tool_choice`, das in der Referenz ebenfalls fehlt, wird gesendet, da Tool-Aufrufe es brauchen. |
| `reasoning_details` und `name` an der Antwortnachricht, `cache_creation_Prompt_tokens` und `cache_read_Prompt_tokens` in `prompt_tokens_details` | Extras und rohe Nutzung | Gegenüber anderen Dialekten weggelassen (`responses/interleaved-tool-call-doc`, `responses/cached-doc`). |
| Fehler als `{"code", "reason", "message", "metadata"}` | `KppError` | Die Art folgt dem Status: 401 `FAILED_TO_AUTH`, 403 `INVALID_API_KEY` oder `NOT_ENOUGH_BALANCE`, 404 `MODEL_NOT_FOUND`, 429, 503 (`errors/`). |
| `GET /v1/models` mit Preisen in 1/10.000 USD, `pricing`, `features` und `endpoints` | `ModelList` | Ohne Schlüssel beantwortet und nur mit Chat-Modellen (`models/list-recorded`); `GET /v1/models/{model}` antwortet auch für Embedding-Modelle (`models/retrieve-embedding-recorded`), daher findet eine Modellprüfung ein Modell, aber keinen falschen Schlüssel (`ModelListing::Partial`). |

Der Anthropic-kompatible Endpunkt von Novita AI liegt unter `/anthropic`, außerhalb seiner OpenAI-Basis-URL, und `/v1/responses` ist nicht dokumentiert, daher ruft Kaeso nur Chat Completions auf und lehnt Token-Zählungen ab. `/v1/completions`, `/v1/rerank`, die Batch-API, sein Bildmodell in OpenAIs Images API und seine eigenen Task-APIs für Sprache, Transkription und Video werden nicht genutzt.

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

## Ollama Cloud

Quellen: Ollamas Seiten zu [Cloud](https://docs.ollama.com/cloud), [Authentifizierung](https://docs.ollama.com/api/authentication), [OpenAI-Kompatibilität](https://docs.ollama.com/api/openai-compatibility) und [Fehlern](https://docs.ollama.com/api/errors), gelesen am 2026-10-30, und die Live-Aufnahmen in `fixtures/openai_chat/providers/ollama-cloud/`, aufgenommen mit `gpt-oss:20b` und `gemma4:31b` im kostenlosen Tarif.

| Ollama-Cloud-Feld | KPP | Hinweise |
|---|---|---|
| Modell-IDs mit Tag, etwa `gpt-oss:20b` | `model` | Unverändert gesendet, auch in `GET /v1/models/{model}`. |
| `max_completion_tokens` (ignoriert), `max_tokens` (beachtet) | `max_output_tokens` | `max_tokens()` im Profil sendet das Ausgabelimit jeder App als `max_tokens`. |
| `reasoning: {effort}` (Anfrage) | Extras | Ollamas Schreibweise neben `reasoning_effort`; abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` im Profil, wie bei Groq. Reasoning-Chunks tragen daneben `content: ""`, das keinen Textblock öffnet. Reasoning, das eine App in einem anderen Format zurückschickt, erreicht Ollama als `reasoning` im Assistenten-Zug. |
| `system_fingerprint` (`fp_ollama`) | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.prompt_tokens_details.cached_tokens` | `Usage.cache_read_tokens` | Ollama meldet keine Reasoning-Token. |
| der Abschluss-Chunk mit leerem `delta`, dann ein Usage-Chunk mit `choices: []` | `Usage` | Wie OpenAI sie sendet. |
| `logprobs`, `top_logprobs`, `logit_bias` | abgelehnt | Ollama nimmt sie an und liefert nichts dafür; das Profil lehnt sie vor dem Senden ab. |
| `tool_choice` außer `auto`, `response_format` | gesendet | Ollamas Cloud-Modelle ignorierten beide in einem Live-Test am 2026-10-30; Kaeso sendet sie wie geschrieben. |
| ein `refusal`-Inhaltsteil in einer Assistentennachricht | gesendet | Ollama antwortet mit 400 `invalid message format` (`errors/recorded-content-array-all-roles`), das die App in ihrem eigenen Format erhält. |
| Fehler in OpenAIs Hülle mit `type` `api_error`, `not_found_error` oder `invalid_request_error` | `KppError` | Die Art folgt dem Status: 401 für einen falschen Schlüssel, 402 für ein Modell, das der Tarif nicht enthält (`errors/recorded-model-outside-plan`), 404 für ein unbekanntes Modell. |
| `GET /v1/models`: OpenAIs Liste, `owned_by: "ollama"` und ein `models`-Array auf oberster Ebene im Modellkatalog-Format von Codex | `ModelList`, Extras | Ollama beantwortet die Liste und `GET /v1/models/{model}`, ohne den Schlüssel zu prüfen, daher erkennt eine Modellprüfung keinen falschen Schlüssel. |

## OVHcloud AI Endpoints

Quellen: das [OpenAPI document](https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/gpt-oss-120b/openapi.json) von AI Endpoints und seine Anleitungen [Getting started](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-getting-started.md), [Capabilities](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-capabilities.md), [Function calling](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-function-calling.md) und [Structured output](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-structured-output.md), gelesen am 2026-11-14. AI Endpoints antwortet ohne Schlüssel, mit 2 Anfragen pro Minute je IP und Modell, daher wurden die Antworten, der Stream, die Modellliste und die Fehler in `fixtures/openai_chat/providers/ovhcloud/` am 2026-11-14 live ohne Schlüssel aufgezeichnet, der Schlüsselfehler mit einem erfundenen Schlüssel. Kaesos Basis-URL ist `https://oai.endpoints.kepler.ai.cloud.ovh.net`, die dokumentierte Basis ohne ihr `/v1`, die `Authorization: Bearer` annimmt. OVHcloud hostet die Modelle in Frankreich.

| AI-Endpoints-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils; Assistant-Nachrichten nehmen es zurück (`requests/recorded-tool-result`). Eine Antwort, die beim Thinking abgeschnitten wird, hat kein `content` (`responses/recorded-thinking-length`). |
| die Rolle `developer` | `Role::System` | Mistral Small 3.2 beantwortet sie mit 400 (`errors/recorded-developer-role-mistral`), daher sendet `system_role()` `system`. |
| `user`, `safety_identifier` | weggelassen | `user` bekommt 400, und das Schema deklariert kein anderes Feld für den Endnutzer (`without_end_user()`). |
| `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_key`, `web_search_options`, `functions`, `function_call` | abgelehnt | Nicht deklariert, und der Parser lässt mindestens `metadata`, `store`, `prediction`, `prompt_cache_key` und `web_search_options` kommentarlos durch; der Cache-Schlüssel einer Responses-App wird weggelassen (`without_cache_key()`). |
| andere nicht deklarierte Felder wie `service_tier`, `verbosity` und `top_k` | wie geschrieben gesendet | AI Endpoints antwortet mit 400 und nennt jedes davon (`errors/recorded-service-tier`). |
| `tool_calls: []`, `"name": "assistant"` in Deltas, `system_fingerprint` im Usage-Chunk | Extras | Gegenüber anderen Dialekten weggelassen. |
| ein bloßes `{"message": …}` oder der Fehler von vLLM als JSON-Text darin | `KppError` | Die Meldung wird mit dem Status zitiert, das verschachtelte `error.message` aus dem Text gelesen. Ein 403 „Forbidden: authentication failed.“ ist ein abgelehnter Schlüssel (`refused_key_messages`, `errors/recorded-invalid-api-key`). |
| `GET /v1/models` mit `pricing` in USD pro Token, `context_length` und `max_completion_tokens` | `ModelList` | Listet jede Art von Modell und lehnt einen falschen Schlüssel mit 403 ab (`models/list-recorded`). |

Die Responses API von AI Endpoints verlangt `store: false` und wird nicht genutzt: Responses-Apps erreichen es über Chat Completions. Es bietet weder Messages noch Token-Zählungen. Modell-IDs unterscheiden Groß- und Kleinschreibung; einen kleingeschriebenen Alias beantwortet es mit der gelisteten ID in `model`, daher listet KPMR die IDs, die `GET /v1/models` nennt. Seine Transkriptionen werden erst genutzt, wenn KPMR die Modelle mit ihren Preisen listet.

## SambaNova

Quellen: das [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) von SambaNova (SambaNova cloud API 1.2.0) und seine Seiten [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility), [Textgenerierung](https://docs.sambanova.ai/docs/en/features/text-generation) und [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes), gelesen am 2026-11-09. Es gab keinen Schlüssel: `fixtures/openai_chat/providers/sambanova/` enthält die Beispielantwort und den Beispiel-Stream des Dokuments und daraus gebaute Antworten, die Modellliste, wie SambaCloud sie ohne Schlüssel liefert, und Fehler, die live ohne gültigen Schlüssel kamen. Kaesos Basis-URL ist `https://api.sambanova.ai`, die `Authorization: Bearer` annimmt.

| SambaNova-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `delta.reasoning`, `channel` | `Block::Reasoning`, Extras | Über `reasoning_fields` des Profils. Assistentennachrichten nehmen `reasoning` zurück. |
| `chat_template_kwargs` (`enable_thinking`), `top_k`, `repetition_penalty` | Extras | DeepSeek-V3.2 und Gemma 4 denken nur mit `enable_thinking`, das nur eine Chat-App senden kann. Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht. |
| `frequency_penalty`, `presence_penalty` | abgelehnt | SambaNova nimmt sie an und ignoriert sie, daher lehnt das Profil sie vor dem Senden ab; `0` wird weggelassen ([neutrale Werte](sampling.md#neutrale-werte)). |
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

## Together AI

Quellen: die [Chat-Completions-Referenz](https://docs.together.ai/reference/chat-completions-1) und die [OpenAI-Kompatibilität](https://docs.together.ai/docs/inference/openai-compatibility) von Together AI, gelesen am 2026-10-19, die SDK-Typen in `togethercomputer/together-py` und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/togetherai/`.

| Together-AI-Feld | KPP | Hinweise |
|---|---|---|
| `top_k`, `min_p`, `repetition_penalty`, `safety_model`, `context_length_exceeded_behavior` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning_content` oder `message.reasoning`, je nach Modell, und dasselbe in Deltas | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). Ein gestreamtes Stück Reasoning kommt mit `content: ""`, das keinen Textblock öffnet. |
| `matched_stop` und `seed` an einer Choice, `text` an einer Stream-Choice, `token_id` in einem Delta, `prompt`, `metadata` und `warnings` auf oberster Ebene | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.reasoning_tokens` neben `completion_tokens_details.reasoning_tokens` | `raw` der Nutzung | Die Token-Zahlen sind wie unter [Nutzung](openai-chat.md#nutzung-completionusage) typisiert. |
| Fehler in der OpenAI-Hülle mit eigenen `type`- und `code`-Werten von Together AI (`start_generation_failed`, `model_not_available`), manchmal mit einer `id` auf oberster Ebene | `KppError` | Die Art folgt dem Status; ein falscher Schlüssel ist ein 401 `invalid_api_key`, also `Authentication`. |
| `GET /v1/models`: ein bloßes Array von Modellen mit `type`, `display_name`, `organization`, `context_length`, `config` und `pricing` | `ModelList`, Modell-Extras | `GET /v1/models/{model}` antwortet für IDs mit `/` mit 404, daher stützt sich eine Modellprüfung auf die Modellliste. |

## Volcengine Ark

Quellen: die Seite [base URL and authentication](https://docs.volcengine.com/docs/zh/ark/base-url-and-authentication.md) von Ark, seine Referenz [Chat API](https://docs.volcengine.com/docs/zh/ark/chat-api.md) (aktualisiert am 2026-11-12), sein Leitfaden [deep thinking](https://docs.volcengine.com/docs/zh/ark/deep-thinking.md), seine Seiten zur [Modellliste](https://docs.volcengine.com/docs/zh/ark/models-api.md) und zu den [Fehlercodes](https://docs.volcengine.com/docs/zh/ark/error-codes.md), gelesen am 2026-11-14. Nicht mit einem erfolgreichen Aufruf getestet: Die Antworten, der Stream und die Liste in `fixtures/openai_chat/providers/volcengine/` sind Beispiele der Referenz und des Leitfadens, ein Stream wurde im Issue eines anderen Projekts veröffentlicht, und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://ark.cn-beijing.volces.com/api/v3`, mit `Authorization: Bearer`.

| Volcengine-Ark-Feld | KPP | Hinweise |
|---|---|---|
| `<base>/chat/completions`, `<base>/models` | die API-Wurzel des Anbieters | Die API von Ark liegt unter `/api/v3`, nicht `/v1`; die Basis-URL endet dort und die Wurzel ist leer ([API-Wurzeln](#api-wurzeln)). |
| `thinking: {"type": "enabled" \| "disabled"}` | `reasoning` | `thinking_type()`: Die Modelle denken von sich aus, und `minimal` schaltet das Thinking von Seed aus, ist bei DeepSeek V4.1 Flash aber `low`, daher geht ausgeschaltetes Thinking nur als Schalter. |
| `reasoning_effort` (`none` bis `max`) | `reasoning.effort` | Jedes Modell mit Thinking-Stufen nimmt alle sieben Werte an und bildet sie auf seine eigenen Stufen ab. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils; Assistant-Nachrichten nehmen es zurück. |
| `message.encrypted_content`, `delta.encrypted_content` | Extras und ein Opaque des Reasoning-Blocks | `encrypted_reasoning_field`: Eine Chat-App erhält und sendet es unverändert; andere Apps tragen es in ihrem Reasoning-Träger, und Kaeso schreibt es wieder an die Assistant-Nachricht, wie Ark es bei Tool-Aufrufen über mehrere Züge verlangt (`responses/tools-encrypted-content-doc`, `requests/tools-encrypted-content-replay-doc`). |
| `max_completion_tokens`, `max_tokens` | `max_output_tokens` | `max_completion_tokens` zählt Thinking und Antwort, bis 65536; `max_tokens` bei den meisten Modellen nur die Antwort. Wie von der App geschrieben gesendet, Kaesos eigene Grenze als `max_completion_tokens`; der Anbieter hat `reasoning_beyond_output_limit`. |
| die ID eines Endnutzers | weggelassen | Kein `user` oder `safety_identifier` (`without_end_user()`). |
| `service_tier` (`auto`, `default`, `fast`, `flex`) | Extras | Unverändert gesendet; OpenAIs `priority` und `scale` werden abgelehnt (`refusing_values`). |
| `web_search_options` | Extras | Nur Doubao Search; `{}` schaltet es ein. `search_context_size` und `user_location` bewirken nichts und werden abgelehnt. Antworten tragen `url_citation`-Annotationen ohne Offsets und `usage.tool_usage.web_search` (`responses/web-search-doc`). |
| `seed`, `n`, `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_retention`, `functions`, `function_call` | abgelehnt | Nicht im Schema, und Ark dokumentiert nicht, ob es nicht deklarierte Felder ablehnt. |
| `prompt_cache_key`, `video_url`-Teile, `logprobs`, `logit_bias`, `parallel_tool_calls` | unverändert gesendet | Im Schema; Reasoning-Modelle lehnen manche davon selbst ab. |
| IDs ohne `chatcmpl-`, `service_tier` in Antworten, `usage.prompt_tokens_details.audio_tokens` | Extras und rohe Usage | Bleiben für Chat-Apps erhalten. |
| Fehler in OpenAIs Hülle mit dem `code` von Ark und einem HTTP-artigen `type`, die Request-ID am Ende der Meldung | `KppError` | Die Art folgt dem Status: 401 ist ein abgelehnter Schlüssel (`errors/recorded-*-api-key`). |
| `GET /models` mit `name`, `version`, `domain`, `token_limits`, `modalities` und `features` von Ark, ohne `owned_by` | `ModelList` | Braucht einen Schlüssel. Sie listet Basismodellversionen, nicht die Endpunkte des Kontos (`ep-…`), daher ist die Liste unvollständig (`models/list-doc`). |

Die Embeddings von Ark bedienen nur `/embeddings/multimodal`, das einen Vektor für die ganze Eingabe liefert, daher hat der Anbieter keinen Embeddings-Dialekt. Seine Endpunkte für Responses und Anthropic Messages werden nicht genutzt, und es gibt keine Token-Zählungen. BytePlus ModelArk bietet dieselbe API unter `https://ark.ap-southeast.bytepluses.com/api/v3` an, mit eigenen Schlüsseln und Modell-IDs, über die Basis-URL.

### API-Wurzeln

Ein Anbieter, dessen API nicht unter `/v1` liegt, nennt seine API-Wurzel in `Provider::api_root`: das, was in den Pfaden jedes Dialekts an die Stelle von `/v1` tritt. Das Gateway gibt sie den Codecs mit `EgressCtx::with_api_root` (`pipeline::egress_ctx`, und `availability::models_ctx` für Modellliste und Modellprüfungen). Chat Completions, Responses, Messages und Embeddings senden ihren `/v1/…`-Pfad unter der Wurzel, und die OpenAI-Modellliste sendet `/v1/models` als `<root>/models`. `Some("")` heißt, dass die Basis-URL selbst an der Wurzel endet, wie `…/api/v3` von Volcengine Ark, daher wird `/v1/chat/completions` zu `/chat/completions`; `None`, für jeden anderen Anbieter, behält `/v1`. Der Kontobereich eines Schlüssels, wie `/client/v4/accounts/{account_id}/ai/v1` von Cloudflare AI Gateway, wird danach angewandt und tritt an die Stelle dieser Wurzel; die Basis-URL eines eigenen Anbieters nennt ihre Wurzel selbst. Weil die Standard-Basis-URL an der Wurzel endet, ist die Basis-URL eines Administrators die dokumentierte, so wie sie ist, etwa die von BytePlus ModelArk.

## StepFun

Quellen: die [Chat-Completions-Referenz](https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create) von StepFun, seine Anleitungen zu [Reasoning](https://platform.stepfun.ai/docs/en/guides/developer/reasoning), [Prompt-Cache](https://platform.stepfun.ai/docs/en/guides/developer/prompt-cache), [Tool-Aufrufen](https://platform.stepfun.ai/docs/en/guides/developer/tool-call) und [Videos](https://platform.stepfun.ai/docs/en/guides/developer/video-chat) und seine [Fehlercodes](https://platform.stepfun.ai/docs/en/api-reference/error-codes), dazu die [Referenz](https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create) und die [Fehlercodes](https://platform.stepfun.com/docs/zh/api-reference/error-codes) seines Dienstes in China, gelesen am 2026-11-14. Es gab keinen Schlüssel: Die Antworten und der Text-Stream in `fixtures/openai_chat/providers/stepfun-ai/` sind die Beispiele der Referenz und der Anleitungen, die Antworten und der Stream mit Reasoning sind aus der Referenz gebaut, und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Kaesos Basis-URL ist `https://api.stepfun.ai` (`https://api.stepfun.com` für den Dienst in China), die `Authorization: Bearer` annimmt.

| StepFun-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning`, `delta.reasoning` und `reasoning_content` mit `reasoning_format: "deepseek-style"` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Die Referenz des Dienstes in China sendet beide Felder mit demselben Text: Das erste wird gelesen, das andere bleibt ein Extra (`responses/both-reasoning-fields-schema`). |
| `reasoning_format` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `usage.cached_tokens` neben den Zählungen | `cache_read_tokens` | Gelesen, wenn `prompt_tokens_details.cached_tokens`, das die Referenz des Dienstes in China nennt, fehlt (`responses/cached-doc`); `prompt_tokens` enthält die Token aus dem Cache. |
| `max_completion_tokens`, die Rolle `developer` | `max_tokens`, `Role::System` | Die Referenz hat nur `max_tokens` und die Rollen `system`, `user`, `assistant` und `tool` (`max_tokens()`, `system_role()`). |
| `video_url`-Teile | `Block::Video` | `with_video_url()`. |
| `user`, `safety_identifier`, das `prompt_cache_key` einer Responses-App | weggelassen | Die Referenz hat kein Feld für den Endnutzer oder einen Cache-Schlüssel, und StepFun speichert Prompts selbst im Cache (`without_end_user()`, `without_cache_key()`). |
| `stream_options` | weggelassen | Jeder Chunk trägt die laufende Nutzung, ohne dass danach gefragt wird (`without_stream_options()`, `streams/text-doc`). |
| `presence_penalty`, `seed`, `logprobs`, `logit_bias`, `parallel_tool_calls`, `prompt_cache_key`, `store` und die anderen Felder, die die Referenz nicht nennt | abgelehnt | Ob StepFun nicht deklarierte Felder ablehnt oder verwirft, ist nicht dokumentiert, daher lehnt das Profil sie vor dem Senden ab; `parallel_tool_calls: true` erlaubt nur parallele Aufrufe und wird weggelassen, ebenso ein [neutraler Wert](sampling.md#neutrale-werte) wie `store: false` und `name` in Nachrichten ([Namen von Teilnehmern](openai-chat.md#namen-von-teilnehmern)). `tool_choice`, das die Anleitungen von StepFun senden, wird gesendet. |
| `finish_reason: ""` und `role: ""` vor dem letzten Chunk | keiner | Als kein Wert gelesen, wie bei anderen Anbietern. |
| ein Tool-Aufruf mit `finish_reason: "stop"` | `StopReason::EndTurn` | Beide dokumentierten Antworten mit Tool-Aufruf enden so (`responses/tools-doc`). Die Chat-Zuordnung liest `stop` als Ende einer Runde, daher erhält eine Anthropic-App den `tool_use`-Block mit `stop_reason: "end_turn"`. |
| Fehler als `{"error": {"message", "type"}}` ohne `code` | `KppError` | Die Art folgt dem Status, und die Kennung von StepFun steht in `type`. Die 429 `project_credit_limit_exceeded` und `member_project_credit_limit_exceeded` des Dienstes in China sind Abrechnungsfehler, keine Ratenlimits. |
| `GET /v1/models` | `ModelList` | Braucht einen Schlüssel und listet Chat- und Sprachmodelle gemeinsam (`models/list-doc`), daher erkennt eine Modellprüfung einen falschen Schlüssel. |

Der Anthropic-Messages-Endpunkt von StepFun, seine Responses API und seine Token-Zählung (`/v1/token/count`) werden nicht genutzt: Jede App erreicht StepFun über Chat Completions, und Kaeso lehnt Token-Zählungen ab. `input_audio` nimmt eine Data-URL ohne `format` an, für die Audio-Chat-Modelle von StepFun, die KPMR nicht listet; seine APIs für Sprache, Transkription und Echtzeit werden ebenfalls nicht genutzt.

## Tencent TokenHub

Quellen: die Seiten [API usage](https://cloud.tencent.com/document/product/1823/130078), [Feldreferenz zu Chat Completions](https://cloud.tencent.com/document/product/1823/135872), [Thinking](https://cloud.tencent.com/document/product/1823/131208), [Hunyuan-Anleitung](https://cloud.tencent.com/document/product/1823/132252), [Fehlercodes](https://cloud.tencent.com/document/product/1823/131595) und [Modellliste](https://cloud.tencent.com/document/product/1823/130051) von TokenHub, gelesen am 2026-11-14. Es gab keinen Schlüssel: Die Antworten und der Stream in `fixtures/openai_chat/providers/tencent-tokenhub/` sind die Beispiele der Hunyuan-Anleitung, zwei Chunks eines echten Streams aus dem Issue eines anderen Projekts stimmen mit dem Stream der Anleitung überein, und die Schlüsselfehler kamen live, ohne gültigen Schlüssel. Jeder Pfad liegt unter `/v1` auf dem Host, daher ist Kaesos Basis-URL `https://tokenhub.tencentmaas.com` (Guangzhou), die `Authorization: Bearer` annimmt; andere Regionen und die internationale Website von Tencent Cloud sind andere Hosts mit jeweils eigenen Schlüsseln.

| TokenHub-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils; Assistant-Nachrichten nehmen es zurück, was Hy4 und Hy3 brauchen, um ihr Thinking über Runden zu behalten (`requests/reasoning-sent-back-doc`). |
| die Rolle `developer` | `Role::System` | Die Referenz führt `system`, `user`, `assistant` und `tool`, daher sendet `system_role()` `system`. |
| `max_tokens`, `max_completion_tokens` | wie geschrieben | TokenHub nimmt beide an und bevorzugt `max_completion_tokens`. |
| `video_url`-Teile | `Block::Video` | `with_video_url()`. |
| `audio`, `modalities`, `logit_bias`, `metadata`, `store`, `service_tier` und die anderen Felder, die die Referenz nicht kennt | abgelehnt | TokenHub ignoriert nicht deklarierte Felder kommentarlos („会被静默忽略“), daher lehnt das Profil sie ab. |
| `thinking`, `enable_thinking`, `thinking_budget`, `interleaved_thinking`, `reasoning_split`, `web_search_options`, `extra_body` | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere reine Chat-Felder. |
| `reasoning_effort` `no_think` | nicht gesendet | Hy4 und Hy3 schalten das Thinking mit `no_think` ab, das keine KPP-Stufe hat; der Katalog führt `high` (Hy4) und `low`, `high` (Hy3), daher wird `none` zur niedrigsten Stufe des Modells, genannt in `kaeso-adapted` ([Modelle, die immer Reasoning nutzen](reasoning.md#modelle-die-immer-reasoning-nutzen)). |
| `reasoning_details`, `search_info`, `usage.cache_read_tokens`, `usage.cache_write_tokens` | Extras und rohe Nutzung | Gegenüber anderen Dialekten weggelassen. |
| Fehler mit `message_zh`, sechsstelligem `code`, `type`, `source`, `request_id` | `KppError` | Die Art folgt dem Status; `type` unterscheidet sich zwischen Referenz und Live-Antworten, und ein 429 kann `code` als Zahl ohne `type` tragen (`errors/rate-limit-doc`). Der `code` gibt die Art, wo der Status weniger sagt: `400003` (Eingabe über dem Kontext) `ContextTooLong`, `400004` (unbekanntes Modell) `NotFound`, `451001` `ContentFiltered`. |
| `GET /v1/models` mit `name` und `status` (`online`, `pre-offline`) | `ModelList` | Braucht den Schlüssel (`models/list-doc`); es gibt kein `GET /v1/models/{model}`. |

Die Endpunkte für Responses und Anthropic Messages von TokenHub bedienen nur manche Modelle und werden nicht genutzt: Responses- und Messages-Apps erreichen TokenHub über Chat Completions, und Kaeso lehnt Token-Zählungen ab, die TokenHub nicht anbietet. Seine Bilder, Videos, Sprache und die multimodalen Embeddings nutzen eigene Formate.

## Venice

Quellen: das [OpenAPI document](https://docs.venice.ai/swagger.yaml) von Venice (Version `20261113.174621`) und seine Seiten [API spec](https://docs.venice.ai/api-reference/api-spec), [OpenAI-Migrationsanleitung](https://docs.venice.ai/guides/getting-started/openai-migration), [Fehlercodes](https://docs.venice.ai/api-reference/error-codes) und [Reasoning-Modelle](https://docs.venice.ai/guides/features/reasoning-models), gelesen am 2026-11-14. Es gab keinen Schlüssel: Die Antworten und Streams in `fixtures/openai_chat/providers/venice/` sind das Beispiel des OpenAPI document und Antworten, die die aufgezeichneten Tests eines anderen Projekts veröffentlichen, und der Schlüsselfehler kam live ohne gültigen Schlüssel.

| Venice-Feld | KPP | Hinweise |
|---|---|---|
| `venice_parameters` (Anfrage) | Extras | Venice stellt einen eigenen System-Prompt voran, wenn `include_venice_system_prompt` nicht `false` ist, daher schreibt `with_option_defaults` des Profils diesen Schlüssel, wenn die Anfrage ihn nicht setzt, auch in die eigenen `venice_parameters` einer Chat-App, deren übrige Schlüssel unverändert gehen. Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. |
| `user` | weggelassen | Venice verwirft es (`without_end_user()`); sein eigenes `anon_user_id` bleibt in den Extras. |
| `metadata`, `store` | abgelehnt | Venice nimmt sie an, ohne etwas damit zu tun. |
| andere Felder, die Venice nicht kennt | gesendet | Venice lehnt sie selbst mit 400 ab (`additionalProperties: false`), daher lehnt das Profil nichts weiter ab. |
| `video_url`-Teile, `input_audio` in anderen Formaten als `wav` und `mp3` | `Block::Video`, `Block::Audio` | `with_video_url()`, `any_audio_format()`. |
| `cost`, die `venice_parameters` der Antwort, `choices[].stop_reason`, `prompt_logprobs` | Extras | Gegenüber anderen Dialekten weggelassen. |
| ein letzter Stream-Chunk mit `choices: []`, `usage` und `cost` | Usage | Venice sendet ihn in jedem Stream, auch ohne `stream_options.include_usage`. |
| Fehler `{"error": "<Text>"}` und `{"error", "details", "issues"}` | `KppError` | Die Art folgt dem Status, und der Text ist die Meldung (`errors/recorded-invalid-api-key`, `errors/unknown-model-published`). |
| `GET /v1/models` | `ModelList` | Listet nur Textmodelle (`models/list-recorded`), daher ist die Liste unvollständig. |

Die Responses API von Venice ist in der Beta und wird nicht genutzt: Responses- und Messages-Apps erreichen Venice über Chat Completions, und Kaeso lehnt Token-Zählungen ab, die Venice nicht anbietet.

## Baidu Qianfan

Quellen: Qianfans Referenzen für [Chat](https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4) und [Bildverständnis](https://cloud.baidu.com/doc/qianfan-api/s/ymwbl48ky), seine [gemeinsamen Header](https://cloud.baidu.com/doc/qianfan-api/s/Nmwbl43w1), [Fehlercodes](https://cloud.baidu.com/doc/qianfan-api/s/rmwbl43x7) und die Anleitungen zu [Denken](https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv) und [Websuche](https://cloud.baidu.com/doc/qianfan-docs/s/Wm8r4sw29), gelesen am 14.11.2026, und `fixtures/openai_chat/providers/qianfan/`: Anfragen und Antworten der Referenz, eine veröffentlichte Antwort, der Stream der Websuche-Anleitung (ohne ihre ausgelassene Mitte) und Fehler zu abgelehnten Schlüsseln und des Gateways, live ohne gültigen Schlüssel beantwortet.

| Feld von Qianfan | KPP | Anmerkungen |
|---|---|---|
| `<base>/chat/completions`, `<base>/embeddings`, `<base>/models` | API-Wurzel des Anbieters | Qianfans API liegt unter `/v2`; die Basis-URL endet dort, und die Wurzel ist leer ([API-Wurzeln](#api-wurzeln)). |
| Rolle `system`, kein `developer` | `Role::System` | `system_role()`. |
| `user` | Endnutzer | `end_user_as_user()`. |
| `thinking: {"type": "enabled" \| "disabled"}`, `reasoning_effort` | `reasoning` | `thinking_type()`; `low` und `medium` wirken als `high`, `xhigh` als `max`. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | `reasoning_fields`. |
| `max_completion_tokens`, `max_tokens` | `max_output_tokens` | Beide zählen Denken und Antwort. |
| `choices[].flag`, `ban_round`, `search_results`, `delta_tag` | Extras | Qianfans Sicherheitsmarken und Quellen der Websuche; Rundreise für Chat-Apps. |
| `web_search`, `penalty_score`, `enable_thinking`, `thinking_budget`, `metadata` | Extras | Wie von einer Chat-App geschrieben gesendet. |
| `logprobs`, `top_logprobs`, `logit_bias`, `store`, `service_tier`, `prompt_cache_key`, `prompt_cache_retention`, `verbosity`, `modalities`, `audio`, `prediction`, `web_search_options`, `functions`, `function_call` | abgelehnt | Nicht in der Referenz, und Qianfan dokumentiert nicht, ob es nicht deklarierte Felder ablehnt. |
| Fehler im Umschlag von OpenAI mit Qianfans `code`; BCE-Gateway-Fehler als `{"requestId", "code", "message"}` | `KppError` | `tokens_too_long` und `characters_too_long` sind ein zu langer Kontext, `account_overdue` ein Abrechnungsfehler, `model_offline` nicht gefunden. Ein falscher Schlüssel bekommt beim Chat 401 `invalid_iam_token` und bei der Modellliste 429 `OverRateLimit` (`errors/recorded-*`). |
| `GET /models` mit Qianfans `status`, `pricing`, `features` und `endpoints` | `ModelList` | Braucht einen Schlüssel; unvollständig, da bereitgestellte Dienste fehlen (`models/list-doc`). |
