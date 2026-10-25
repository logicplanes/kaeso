---
title: Chat-Completions-Anbieter
description: Wie der Chat-Codec seine Anfragen an jeden Anbieter dieses Formats anpasst, und was jeder Anbieter ergänzt.
order: 6
nav: Chat-Anbieter
---

Auch andere Anbieter als OpenAI sprechen [OpenAI Chat Completions](openai-chat.md), mit eigenen Abweichungen. Schlagen Sie hier nach, wie der Codec jede Anfrage an einen Anbieter anpasst und was jeder Anbieter ergänzt, bevor Sie einen Anbieter hinzufügen oder sein Profil ändern.

## Anbieterprofile

Andere Anbieter sprechen dieses Format mit eigenen Abweichungen. Jeder Anbieter in `PROVIDERS` (`crates/kaeso-core/src/provider.rs`) nennt im Feld `chat` ein `kaeso_codecs::openai_chat::ChatProfile`. Das Profil enthält den Namen des Anbieters und die Einstellungen für die Anfragen, die Kaeso ihm sendet (`ChatProfile::request`, ein `RequestProfile`), gesetzt mit den Buildern unten.

Das Gateway übergibt das Profil mit `EgressCtx::with_chat_profile` dem Encoder, der es auf jede Anfrage anwendet, gleich in welchem Format die App sie gesendet hat. `ChatProfile::OPENAI`, der Standard, ändert nichts, ebenso `ChatProfile::compatible(name)`, solange keine Einstellung hinzukommt.

| Einstellung | Was der Encoder tut | Warum das erlaubt ist |
|---|---|---|
| `max_tokens()` | Sendet das Ausgabelimit als `max_tokens` und ein `max_completion_tokens: null` als `max_tokens: null`. Eine Chat-Anfrage mit beiden Feldern und verschiedenen Werten ist `Unsupported` (`max_tokens`). | Nur die Schreibweise ändert sich; der Wert bleibt gleich. |
| `system_role()` | Sendet `developer`-Nachrichten als `system`-Nachrichten. | Die Schreibweise der Rolle ist Form; beide tragen die Anweisungen. |
| `random_seed()` | Sendet `seed` als `random_seed`, auch ein `seed: null`. Ein negativer Seed oder eine Anfrage mit `seed` und `random_seed` (beide auch als `null`) ist `Unsupported` (`seed`). Die Regel folgt Mistrals `random_seed`, dem sein OpenAPI-Dokument `minimum: 0` gibt. | `seed` bleibt semantisch: Es wird umbenannt oder abgelehnt, nie verworfen. |
| `without_end_user()` | Lässt KPP `user` weg, statt es als `user` oder `safety_identifier` zu senden, zusammen mit einem zweiten `user`, das eine Chat-App neben `safety_identifier` gesendet hat, und ihren `null`-Schreibweisen. | `user` ist ein Hinweis ([Feldklassen](index.md#feldklassen)). |
| `without_prefill()` | Lehnt eine Unterhaltung ab, die mit einer Assistentennachricht endet, außer sie ruft Tools auf oder ist die Nachricht einer Chat-App mit `prefix: true`: `Unsupported` (`assistant prefill`, Param `messages[i]`). | Ein Anbieter, der eine solche Nachricht nur mit `prefix: true` fortsetzt, wiederholt das Vorausgefüllte in seiner Antwort, sodass die App keine Fortsetzung erhielte. |
| `greedy_top_p()` | Sendet `top_p: 1`, sobald die Anfrage `temperature: 0` hat, gleich welches `top_p` sie setzt. | Bei `temperature: 0` nimmt das Modell immer das wahrscheinlichste Token, deshalb ändert `top_p` nichts, und der Wert der App geht nicht verloren; ein Anbieter mit niedrigerem Standard-`top_p` lehnt Greedy Sampling ohne `top_p: 1` ab. |
| `refusing(fields)` | Lehnt eine Anfrage ab, deren Body einen dieser Schlüssel auf oberster Ebene enthielte, etwa `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | Extras sind semantisch, daher werden sie abgelehnt, nicht verworfen. |
| `refusing_in_messages(fields)` | Dasselbe für Schlüssel von System-, Benutzer- und Assistentennachrichten. Tool-Nachrichten werden nicht geprüft. | Wie oben. |

Einstellungen für die Antworten, die Kaeso liest (`ChatProfile::reply`, ein `ReplyProfile`):

| Einstellung | Was der Decoder tut | Warum es erlaubt ist |
|---|---|---|
| `thinking_parts()` | Liest einen `thinking`-Teil eines `content`-Arrays (`{type: "thinking", thinking: [Textteile], closed}`, sonst nichts) als `Block::Reasoning`, dessen `text` jeden Textteil enthält, und gestreamte als `BlockStart(Reasoning)` und `BlockDelta(ReasoningText)`. Der Reasoning-Block endet, wenn die Antwort, eine Ablehnung oder ein Tool-Aufruf beginnt, und leere Textstücke eines Streams vor dem ersten echten öffnen keinen Textblock (Form `et`). Ein `thinking`-Teil mit anderen Schlüsseln oder anderen inneren Teilen bleibt wie ohne die Einstellung. | Nichts wird verworfen: Chat-Apps erhalten dieselben Teile zurück, und andere Formate erhalten das Denken in ihrer eigenen Reasoning-Form, mit dem ursprünglichen Block in einem [Reasoning-Blob](reasoning.md#reasoning-blobs). |
| `reasoning_fields(fields)` | Liest den ersten dieser Nachrichtenschlüssel, der einen nicht leeren String enthält, etwa DeepSeeks `reasoning_content` oder Groqs `reasoning`, als `Block::Reasoning` vor den übrigen Blöcken der Antwort, mit Form `reasoning_field`, die den Schlüssel nennt. In einem Stream wird dieser Schlüssel jedes Deltas zu `BlockDelta(ReasoningText)` eines einzigen Reasoning-Blocks, mit Form `rf` am Chunk. Der Reasoning-Block endet, wenn die Antwort, eine Ablehnung oder ein Tool-Aufruf beginnt, und leere Textstücke vor dem ersten echten öffnen keinen Textblock (Form `et`). Ein leerer String, `null` oder ein anderer Typ bleibt ein Extra. | Nichts wird verworfen: Chat-Apps erhalten den Schlüssel so zurück, wie er kam, die übrigen Schlüssel des Deltas (Groqs `channel`) bleiben Extras des Chunks, und andere Formate erhalten das Reasoning in ihrer eigenen Reasoning-Form, mit dem ursprünglichen Block in einem [Reasoning-Blob](reasoning.md#reasoning-blobs). Ohne die Einstellung wäre jedes gestreamte Stück Reasoning ein Chunk ohne KPP-Ereignis und würde als eigener unbekannter Block protokolliert. |
| `error_detail(path)` | Hängt den String an diesem Pfad eines Fehler-Bodys, etwa OpenRouters `error.metadata.raw`, an die Meldung des Fehlers an: `<message>: <detail>`. Ein fehlender, leerer oder nicht als String vorliegender Wert, oder einer, den die Meldung schon enthält, ändert nichts. Stream-Fehler erhalten dasselbe. | Der Body bleibt, wie er war: Chat-Apps erhalten den Fehler des Anbieters unverändert, und andere Formate erhalten den Grund in der Meldung, wo eine allgemeine (`Provider returned error`) ihnen nichts sagen würde. Das Gateway entfernt schlüsselähnliche Zeichenfolgen aus jeder Fehlermeldung eines Anbieters. |

`refusing` und `refusing_in_messages` nehmen jeweils die ganze Liste; ein zweiter Aufruf ersetzt den ersten. Eine Ablehnung nennt das Feld so, wie die App es geschrieben hat. Den Schlüssel einer Chat-App nennt sie mit eigenem Namen und Pfad (`logit_bias`, `messages[1].name`), einen Ablehnungsblock aus einem anderen Format mit seinem KPP-Pfad (`messages[1].blocks[0]`).

Die Ablehnung geschieht beim Kodieren der Anfrage, an der Anfrage, die der Encoder gebaut hat, und bevor sie geschrieben wird. Es wird also nichts gesendet, und die Ablehnung erreicht die App in ihrem eigenen Format.

Jeder eingebaute Anbieter, dessen Profil von dem von OpenAI abweicht, steht unten mit der Quelle jeder Einstellung: der Dokumentation des Anbieters, seinem OpenAPI-Dokument oder einer Live-Aufnahme in `fixtures/openai_chat/providers/<id>/`.

| Anbieter | Einstellungen | Quellen |
|---|---|---|
| DeepSeek (`deepseek`) | `max_tokens()`, `system_role()`, `reasoning_fields(["reasoning_content"])` | DeepSeeks API-Referenz und Leitfaden zum Thinking-Modus (`reasoning_content`), `responses/recorded-thinking-reasoning-content` und `streams/recorded-thinking-stream`; `fixtures/openai_chat/providers/deepseek/responses/recorded-max-completion-tokens-limit`: DeepSeek ignoriert `max_completion_tokens` (20 verlangt, 133 Completion-Token), während seine API-Referenz `max_tokens` dokumentiert; `fixtures/openai_chat/providers/deepseek/errors/recorded-plain-developer-string`: DeepSeek beantwortet die Rolle `developer` mit 422 `unknown variant` |
| Groq (`groq`) | `system_role()`, `reasoning_fields(["reasoning"])`; lehnt `logit_bias`, `logprobs` und `top_logprobs` ab, dazu `name` und `refusal` in Nachrichten | Groqs Seite zur OpenAI-Kompatibilität (https://console.groq.com/docs/openai, die `logprobs`, `logit_bias`, `top_logprobs` und `messages[].name` als nicht unterstützt nennt); `fixtures/openai_chat/providers/groq/errors/`: `recorded-logit-bias`, `recorded-logprobs-top-logprobs-doc`, `recorded-tools-results-parallel-error` (`refusal` in einer Assistentennachricht) und `recorded-tools-single` (Qwens Chat-Template lehnt die Rolle `developer` ab); Groqs Reasoning-Leitfaden (`reasoning`), `responses/recorded-reasoning-format-parsed` und `streams/recorded-stream-doc` |
| Mistral (`mistral`) | `max_tokens()`, `system_role()`, `random_seed()`, `without_end_user()`, `without_prefill()`, `greedy_top_p()`, `thinking_parts()`; lehnt `audio`, `function_call`, `functions`, `logit_bias`, `moderation`, `prompt_cache_options`, `prompt_cache_retention`, `store`, `verbosity` und `web_search_options` ab, dazu `name` und `refusal` in Nachrichten | Mistrals OpenAPI-Dokument (https://docs.mistral.ai/openapi.yaml, gelesen am 2026-10-19), in dem `ChatCompletionRequest` nicht deklarierte Felder verbietet und `random_seed` `minimum: 0` hat; die `422 extra_forbidden`-Aufnahmen in `fixtures/openai_chat/providers/mistral/errors/`, eine je abgelehntem oder umbenanntem Feld; `responses/recorded-audio-field`, in der Mistral `audio` annimmt und nur mit Text antwortet; dazu `recorded-assistant-prefill` (400 `invalid_request_message_order`) mit `responses/recorded-assistant-prefix`, dessen Antwort das Vorausgefüllte wiederholt; `responses/recorded-reasoning-effort-high` und `streams/recorded-reasoning-effort-high-stream` für `thinking_parts()`; `errors/greedy-sampling` (400 `invalid_request_greedy_sampling`) für `greedy_top_p()` |
| OpenRouter (`openrouter`) | `reasoning_fields(["reasoning"])`, `error_detail(["error", "metadata", "raw"])` | OpenRouters Leitfaden zu Reasoning-Token (`reasoning`, daneben `reasoning_details`); `responses/recorded-free-reasoning` und `streams/recorded-free-reasoning-stream`; OpenRouters Seite zu Fehlern und Debugging (`metadata.raw`, der Fehler des vorgelagerten Anbieters), `errors/recorded-upstream-rate-limited` und `errors/recorded-routing-fields` |
| SpaceX (`xai`) | `reasoning_fields(["reasoning_content"])` | xAIs Chat-Completions-Referenz (`reasoning_content`); `responses/recorded-reasoning-content` und `streams/recorded-reasoning-content-stream` |
| Together AI (`togetherai`) | `reasoning_fields(["reasoning_content", "reasoning"])` | Together AIs Chat-Completions-Referenz; der Schlüssel hängt vom Modell ab: `responses/recorded-sampling-extras` (`reasoning_content`), `responses/recorded-reasoning-reply` und `streams/recorded-reasoning-stream` (`reasoning`) |

## Anbieter mit diesem Format

Andere Anbieter als OpenAI, die Chat Completions bereitstellen, zeichnen in `fixtures/openai_chat/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln oben; dieser Abschnitt nennt, was jeder Anbieter hinzufügt.

### DeepSeek

Quellen: die [API-Referenz](https://api-docs.deepseek.com/api/create-chat-completion), der [Leitfaden zum Thinking-Modus](https://api-docs.deepseek.com/guides/thinking_mode/) und die [Fehlercodes](https://api-docs.deepseek.com/quick_start/error_codes) von DeepSeek, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/deepseek/`.

| DeepSeek-Feld | KPP | Hinweise |
|---|---|---|
| `thinking` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück, und ein Assistant-Turn einer Chat-App, der eines trägt, schickt es an DeepSeek zurück. Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item; schicken sie es zurück, bleibt es aus der Anfrage an DeepSeek weg ([Reasoning](reasoning.md)), und DeepSeek antwortet ohne. |
| `usage.prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` | `raw` der Nutzung | Die Treffer entsprechen `prompt_tokens_details.cached_tokens`, das als Cache-Lesezugriff typisiert ist. |
| `: keep-alive`-Zeilen und leere Zeilen, solange der Server ausgelastet ist | `Extra` | Nur im selben Dialekt, wie andere SSE-Kommentare. |
| HTTP 402 | `Billing` | Ein aufgebrauchtes Guthaben, was auch immer `code` sagt. |
| HTTP 422 für eine `developer`-Nachricht | `InvalidRequest` | DeepSeek kennt die Rolle nicht; die App erhält den Fehler von DeepSeek. |
| zusätzliche Modellfelder (`name`, `context_window`, `max_output_tokens`, `input_modalities`, `output_modalities`, `effort`, `api_capabilities`) | Modell-Extras | Die Modellliste von DeepSeek hat kein `created`. |

### SpaceX

Quellen: die [Chat-Completions-Referenz](https://docs.x.ai/developers/rest-api-reference/inference/chat-completions) und die [Modellreferenz](https://docs.x.ai/developers/rest-api-reference/inference/models) von xAI, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/xai/`. Das eigene Format von SpaceX ist [OpenAI Responses](openai-responses.md#anbieter-mit-diesem-format); ein Modell erreicht Chat Completions über sein `egress_dialect`.

| SpaceX-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). |
| `citations`, `system_fingerprint`, `service_tier` | Extras | Gegenüber anderen Dialekten weggelassen. |
| `max_completion_tokens` (Anfrage) | `sampling.max_output_tokens` | SpaceX wendet es nur auf die sichtbare Ausgabe an, nicht auf Reasoning-Token (Chat-Completions-Referenz; `responses/recorded-max-completion-tokens-reasoning`: 20 verlangt, 18 sichtbare und 271 Reasoning-Token). Die Nutzung zählt beides, daher kann die Ausgabe das Limit übersteigen; das Gateway reserviert für ein Reasoning-Modell von SpaceX (`reasoning_beyond_output_limit` in `PROVIDERS`) unter harten Budgets und Token-Raten das Ausgabelimit des Modells ([Limits und Budgets](../../console/limits-and-budgets.md)). |
| `usage.num_sources_used`, `cost_in_usd_ticks`, `prompt_tokens_details.text_tokens` und `image_tokens` | `raw` der Nutzung | Die Token-Zahlen sind wie oben typisiert. SpaceX lässt das Reasoning aus `completion_tokens` heraus und zählt es in `total_tokens`, daher ist die Ausgabe `completion_tokens` plus `reasoning_tokens`. |
| `{"error": "<message>"}`, bei einem 400 mit `"code": "invalid-argument"` | `KppError` | Der String ist die Meldung, und Apps erhalten sie in der OpenAI-Fehlerform mit dem Status von SpaceX. Ein falscher Schlüssel ist ein 400, dessen Meldung mit `Incorrect API key provided` beginnt; die `refused_key_messages` von SpaceX nennen sie, daher meldet das Gateway einen abgelehnten Schlüssel. |
| `presence_penalty` für ein Reasoning-Modell, `reasoning_effort` für ein Modell ohne Reasoning | | SpaceX antwortet mit 400, und dieser Fehler erreicht die App. |
| `search_parameters` (Anfrage) | Extras | SpaceX antwortet mit 410, weil die Live-Suche abgekündigt ist; die App erhält den 410. |
| zusätzliche Modellfelder (`aliases`, Preise in US-Cent pro 100 Millionen Token, `capabilities.reasoning_effort`, `pricing` für Bilder) | Modell-Extras | |

### Together AI

Quellen: die [Chat-Completions-Referenz](https://docs.together.ai/reference/chat-completions-1) und die [OpenAI-Kompatibilität](https://docs.together.ai/docs/inference/openai-compatibility) von Together AI, gelesen am 2026-10-19, die SDK-Typen in `togethercomputer/together-py` und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/togetherai/`.

| Together-AI-Feld | KPP | Hinweise |
|---|---|---|
| `top_k`, `min_p`, `repetition_penalty`, `safety_model`, `context_length_exceeded_behavior` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning_content` oder `message.reasoning`, je nach Modell, und dasselbe in Deltas | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). Ein gestreamtes Stück Reasoning kommt mit `content: ""`, das keinen Textblock öffnet. |
| `matched_stop` und `seed` an einer Choice, `text` an einer Stream-Choice, `token_id` in einem Delta, `prompt`, `metadata` und `warnings` auf oberster Ebene | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.reasoning_tokens` neben `completion_tokens_details.reasoning_tokens` | `raw` der Nutzung | Die Token-Zahlen sind wie oben typisiert. |
| Fehler in der OpenAI-Hülle mit eigenen `type`- und `code`-Werten von Together AI (`start_generation_failed`, `model_not_available`), manchmal mit einer `id` auf oberster Ebene | `KppError` | Die Art folgt dem Status; ein falscher Schlüssel ist ein 401 `invalid_api_key`, also `Authentication`. |
| `GET /v1/models`: ein bloßes Array von Modellen mit `type`, `display_name`, `organization`, `context_length`, `config` und `pricing` | `ModelList`, Modell-Extras | `GET /v1/models/{model}` antwortet für IDs mit `/` mit 404, daher stützt sich eine Modellprüfung auf die Modellliste. |

### Groq

Quellen: die [API-Referenz](https://console.groq.com/docs/api-reference) und der [Reasoning-Leitfaden](https://console.groq.com/docs/reasoning) von Groq, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/groq/`.

| Groq-Feld | KPP | Hinweise |
|---|---|---|
| `reasoning_format`, `include_reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). |
| `delta.channel` (`analysis` beim Reasoning von GPT-OSS) | Extras des Chunks | Chat-Apps erhalten es mit dem Reasoning zurück; gegenüber anderen Dialekten weggelassen. |
| `x_groq`, `usage_breakdown`, `service_tier` (`on_demand`) | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time` | `raw` der Nutzung | Die Token-Zahlen, `reasoning_tokens` eingeschlossen, sind wie oben typisiert. |
| `usage` im Chunk mit `finish_reason`, auch ohne `stream_options.include_usage` | `Usage` | Jeder Dialekt erhält die Nutzung, ob Groq zusätzlich den Nutzungs-Chunk sendet, den `include_usage` anfordert, oder nicht. |
| zusätzliche Modellfelder (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features` und weitere) | Modell-Extras | |

### OpenRouter

Quellen: die [API-Referenz](https://openrouter.ai/docs/api_reference/overview), [Streaming](https://openrouter.ai/docs/api_reference/streaming), [Fehler und Debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging) und [Reasoning-Token](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens) von OpenRouter, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/openrouter/`, alle mit `:free`-Modellen.

| OpenRouter-Feld | KPP | Hinweise |
|---|---|---|
| `provider`, `models`, `transforms`, `usage`, `reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils. Chat-Apps erhalten sie unverändert zurück; Apps von Anthropic Messages erhalten einen `thinking`-Block und Apps von Responses ein Reasoning-Item ([Reasoning](reasoning.md)). |
| `message.reasoning_details`, `delta.reasoning_details` | Extras | Dasselbe Reasoning in OpenRouters strukturierter Form; Chat-Apps erhalten es unverändert zurück, andere Dialekte erhalten den Text aus `reasoning`. |
| `provider`, `native_finish_reason` an einer Antwort | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.cost`, `is_byok`, `cost_details`, die zusätzlichen Token-Angaben | `raw` der Nutzung | Kaeso berechnet Kosten mit den Preisen des Katalogs, nicht mit `usage.cost`. |
| der letzte Stream-Chunk, der die Choice mit `finish_reason` neben `usage` wiederholt | `Usage` | Gelesen wie ein Nutzungs-Chunk mit `choices: []`. |
| `: OPENROUTER PROCESSING`-Kommentare | `Extra` | Nur im selben Dialekt, wie andere SSE-Kommentare. |
| `error.metadata.raw`, der eigene Fehler des vorgelagerten Anbieters, neben der allgemeinen `message` `Provider returned error` | `KppError.message` | Über `error_detail` des Profils erhalten Apps von Anthropic Messages und Responses sowie das Anfrageprotokoll `Provider returned error: <Grund>`, etwa `google/gemma-4-31b-it:free is temporarily rate-limited upstream…`. Chat-Apps erhalten den Body so, wie OpenRouter ihn geschickt hat. |
| ein `error`-Objekt nach der 200, im Body oder in einem Chunk neben `choices` | `KppError` | Siehe die [Zeilen oben](openai-chat.md#streams-createchatcompletionstreamresponse); ein numerischer `code` bestimmt die Art wie ein Status. |
| `GET /api/v1/models`: kein `object`, Modelle ohne `object` und `owned_by`, mit `total_count` und `links` | `ModelList`, Extras | `GET /api/v1/models/{model}` antwortet für jedes Modell mit 404, daher stützt sich eine Modellprüfung auf die Modellliste, die OpenRouter ohne Blick auf den Schlüssel ausliefert. |
