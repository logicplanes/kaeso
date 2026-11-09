---
title: Messages-Anbieter
description: Wie der Messages-Codec seine Anfragen an jeden Anbieter anpasst, der das Format spricht, und was jeder hinzufügt.
order: 4
nav: Messages-Anbieter
---

Andere Anbieter als Anthropic sprechen [Anthropic Messages](anthropic-messages.md) mit eigenen Abweichungen. Schlagen Sie hier nach, wie der Codec jede Anfrage an einen Anbieter anpasst und was jeder Anbieter hinzufügt, bevor Sie einen Anbieter hinzufügen oder sein Profil ändern.

Andere Anbieter als Anthropic, die Messages bereitstellen, zeichnen in `fixtures/anthropic_messages/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln von [Anthropic Messages](anthropic-messages.md); diese Seite nennt, was jeder Anbieter hinzufügt.

Jeder Anbieter in `PROVIDERS` nennt in seinem Feld `messages` ein `kaeso_codecs::anthropic_messages::MessagesProfile`, und das Gateway übergibt es dem Encoder mit `EgressCtx::with_messages_profile`, der es auf jede Anfrage anwendet, gleich in welchem Format die App sie gesendet hat. `MessagesProfile::ANTHROPIC`, der Standard, sendet Anfragen so, wie die App sie geschrieben hat; `MessagesProfile::compatible(name)` ist dasselbe mit dem Namen des Anbieters. Eigene Anbieter erhalten `ANTHROPIC`.

| Einstellung | Was der Encoder tut | Warum das erlaubt ist |
|---|---|---|
| `refusing(fields)` | Lehnt eine Anfrage ab, die eines dieser Felder setzt, mit ihren Namen bei Anthropic: `top_k`, `stop_sequences` (so benannt, wie die App es geschrieben hat, etwa `stop` einer Chat-App) oder ein Feld einer Anthropic-App auf oberster Ebene, etwa `container`: `` `top_k` is not supported by Example Cloud; remove it from the request. `` Ein Feld, das beim Wechsel des Formats ausgelassen werden darf, etwa Claude Codes `context_management`, das nur Thinking löscht ([Extras](cross-format.md#extras)), wird stattdessen ausgelassen und in `kaeso-left-out` genannt; jeder andere Wert davon wird mit dem Hinweis für Claude Code abgelehnt. | Diese Felder sind semantisch, und ein Anbieter, der sie annimmt, ohne etwas damit zu tun, würde sie stillschweigend verwerfen. |
| `sampling_fixed_at(values)` | Lehnt eine `temperature` oder ein `top_p` ab, das vom Wert abweicht, mit dem der Anbieter immer sampelt (`FixedValue`, in Tausendsteln); dieser Wert selbst geht so, wie er geschrieben ist. | Der Anbieter ignoriert das Feld, daher wird nur sein eigener Wert eingehalten. |
| `auto_tool_choice_only()` | Lehnt ein `tool_choice` außer `auto` ab, aus jedem Format. | Der Anbieter würde Tools selbst wählen, gleich worum die App gebeten hat. |
| `with_video(media_types)` | Sendet `Block::Video` als `video`-Block; eine base64-Quelle eines anderen Medientyps ist `Unsupported` (`` video media type `…` ``), ebenso eine Datei-ID oder Textquelle. | Nur ein Anbieter, der den Block dokumentiert, bekommt ihn. |
| `raw_predict()` | Sendet eine Generierung an `{base_path}/{model}:rawPredict` (`:streamRawPredict`, wenn sie streamt) ohne `model` und eine Token-Zählung an `{base_path}/count-tokens:rawPredict` mit `model`; beide Bodys bekommen `anthropic_version` `vertex-2023-10-16`, außer die App hat eine eigene gesendet. Fehler in Googles Form werden als Gemini-Fehler gelesen. | So bietet Googles Plattform Claude an ([Claude verwenden](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/use-claude), [Token zählen](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/count-tokens), Anthropics SDK). Gemini Enterprise Agent Platform nutzt es und lehnt `mcp_servers` und `container` ab, die [Anthropic](https://platform.claude.com/docs/en/build-with-claude/claude-models-on-google-cloud/google-managed) dort als nicht verfügbar führt. |

Ablehnungen geschehen, während die Anfrage für das Ziel angepasst wird, vor dem Kodieren; daher wird nichts gesendet, und die Ablehnung erreicht die App in ihrem eigenen Format.

| Anbieter | Einstellungen | Quellen |
|---|---|---|
| MiniMax (`minimax`) | lehnt `top_k`, `stop_sequences`, `mcp_servers`, `container` und `context_management` ab; `sampling_fixed_at([("temperature", 1), ("top_p", 0.95)])`; `auto_tool_choice_only()`; `with_video(MINIMAX_VIDEO_MEDIA_TYPES)` (MP4, AVI, MOV, MKV, aus `MediaSource`) | MiniMax' Seite zur [Anthropic-API-Kompatibilität](https://platform.minimax.io/docs/api-reference/text-anthropic-api), gelesen am 2026-11-08, deren Tabelle `temperature`, `top_p`, `top_k`, `stop_sequences`, `mcp_servers`, `context_management` und `container` als ignoriert führt, sagt, dass MiniMax immer eine `temperature` von 1 und ein `top_p` von 0.95 verwendet, und für `tool_choice` nur `auto` unterstützt |

## Vercel

Quellen: Vercels Seiten zur [Anthropic Messages API](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api) und zum [Extended Thinking](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api/reasoning), gelesen am 2026-10-29, und die Live-Aufzeichnungen in `fixtures/anthropic_messages/providers/vercel/`. Ein Vercel-Modell wird als Messages gesendet, wenn sein Wert **Sendet als** das sagt; Vercels `generation`-Dialekt ist Chat Completions.

| Vercel-Feld | KPP | Hinweise |
|---|---|---|
| `provider_metadata` in der Antwort | Extras | Anthropic-Apps erhalten es unverändert zurück. |
| `thinking`-Blöcke ohne `signature` | `Block::Reasoning` | Anthropic-Apps erhalten sie so, wie Vercel sie gesendet hat. |
| Fehler in OpenAIs Hülle | `KppError` | Siehe [Fehler](anthropic-messages.md#fehler). |
| `POST /v1/messages/count_tokens` | — | Vercel zählt nur für Claude-Modelle genau und schätzt den Rest, daher lehnt Kaeso Token-Zählungen für jedes Vercel-Modell ab. |

## Z.ai

Quellen: Z.ais Seiten zu den [Fehlercodes](https://docs.z.ai/api-reference/api-code), zum [Schnellstart](https://docs.z.ai/devpack/quick-start) und zu den [Thinking-Einstellungen](https://docs.z.ai/devpack/latest-model), gelesen am 2026-11-08, und die live aufgezeichneten und erprobten Fehler in `fixtures/anthropic_messages/providers/zai/`. Messages ist Z.ais `generation`-Dialekt, unter `https://api.z.ai/api/anthropic`. Z.ai dokumentiert keine Anthropic-Antwort, keinen Stream und keine Liste der Felder, die es annimmt; daher gehen Anfragen so wie an Anthropic, nichts wird für Z.ai abgelehnt, und Z.ais eigene Fehler erreichen die App. Seine Seite zu den Thinking-Einstellungen nennt `thinking.type` `adaptive` und `output_config.effort`, zu denen die Effort-Stufe eines anderen Dialekts wird.

| Z.ai-Feld | KPP | Hinweise |
|---|---|---|
| Z.ais numerischer Code als `error.type`, in OpenAIs Hülle (`errors/recorded-invalid-api-key`) | Art des `KppError` | Z.ai beantwortet manche Codes mit einem Status, der in Anthropics Tabelle etwas anderes bedeutet, etwa 429 für ein leeres Guthaben. Der Code gewinnt: `1113`, `1309` und `1314` → `Billing`; `1211` → `NotFound`; `1220`, `1311` und `1315` → `PermissionDenied`; `1261` → `ContextTooLong`; `1301` → `ContentFiltered`; `1305` → `Overloaded`; `1000`, `1001`, `1003` und `1005` → `Authentication`. Andere Codes folgen dem Status. |
| `{"code", "msg", "success": false}`, auch mit HTTP 200 (`errors/list-invalid-api-key`) | `KppError` | Z.ais Plattform-Hülle, die einen Pfad, den Z.ai nicht bedient, darunter `GET /v1/models`, beantwortet, bevor der Schlüssel geprüft wird. `decode_models` und `decode_model` lesen sie unabhängig vom Status als Fehler, sodass eine Modellprüfung mit einem falschen Schlüssel den abgelehnten Schlüssel meldet. |
| `POST /v1/messages/count_tokens` | — | Nicht dokumentiert; Kaeso lehnt Token-Zählungen für Z.ai-Modelle ab, bevor etwas gesendet wird. |
## MiniMax

Quellen: MiniMax' Seite zur [Anthropic-API-Kompatibilität](https://platform.minimax.io/docs/api-reference/text-anthropic-api) und sein [OpenAPI-Dokument](https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json), gelesen am 2026-11-08, die Beispiele aus diesem Dokument in `fixtures/anthropic_messages/providers/minimax/` und ein live mit einem erfundenen Schlüssel aufgezeichneter Fehler. Messages ist MiniMax' `generation`-Dialekt, unter `https://api.minimax.io/anthropic`, für `MiniMax-M3` und `MiniMax-M3.1-Flash`.

| MiniMax-Feld | KPP | Hinweise |
|---|---|---|
| Antwort-`id` aus 32 Hex-Ziffern ohne `msg_`, kein `stop_sequence` | `id`, Form `none` | Anthropic-Apps erhalten die Antwort so, wie MiniMax sie gesendet hat (`responses/deep-thinking`). |
| `thinking`-Blöcke mit MiniMax' eigener `signature`, und `signature_delta` | `Block::Reasoning` mit einer opaken Anthropic-Signatur vom Ursprung `minimax` | MiniMax bittet, die Blöcke in späteren Runden unverändert zurückzusenden. Andere Formate senden sie nur an MiniMax zurück ([Reasoning-Blobs](reasoning.md#reasoning-blobs)); die eigene Wiedergabe einer Anthropic-App geht so, wie sie geschrieben ist, und Anthropic lehnt MiniMax' Signaturen ab. |
| `service_tier` in `message_start` | Extras | `streams/image-url-stream`. |
| `video`-Inhaltsblöcke (Anfrage), base64 oder URL, auch `mm_file://` | `Block::Video` | MiniMax' eigener Block (`requests/video-url`); nur MiniMax' Profil sendet ihn, Anthropic lehnt ihn ab, und Gemini bekommt ihn als `inlineData` oder `fileData`-URL. |
| `thinking.type` `adaptive` und `output_config.effort` (Anfrage) | `reasoning` | MiniMax dokumentiert nur `adaptive` und `disabled`; `MiniMax-M3.1-Flash` lehnt `disabled` mit 400 ab, das die App erreicht. |
| `POST /v1/messages/count_tokens` | — | MiniMax' dokumentierte Antwort legt die Zahlen unter `usage` statt auf die oberste Ebene, daher lehnt Kaeso Token-Zählungen für MiniMax-Modelle ab, bevor etwas gesendet wird. |
| `GET /v1/models`, `GET /v1/models/{model_id}` | `ModelList`, `ModelInfo` | Anthropics Form (`models/list-documented`, `models/retrieve-documented`); ein falscher Schlüssel erhält 401, daher erkennt **Verfügbarkeit prüfen** ihn. |
| Fehler | `KppError` | Anthropics Form mit einer eigenen `request_id` von MiniMax (`errors/recorded-invalid-api-key`). |

## Fireworks AI

Quellen: Fireworks' Referenz [Create a Message](https://docs.fireworks.ai/api-reference/anthropic-messages) und die Seite [Anthropic compatibility](https://docs.fireworks.ai/tools-sdks/anthropic-compatibility), gelesen am 2026-11-09, und `fixtures/anthropic_messages/providers/fireworks-ai/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Ein Fireworks-Modell sendet als Messages, wenn sein Wert **Sendet als** das sagt; der Schlüssel geht als `Authorization: Bearer`.

| Fireworks-Feld | KPP | Hinweise |
|---|---|---|
| `output_config.effort`, `thinking` mit `budget_tokens` | `reasoning` | Fireworks bildet beide auf sein `reasoning_effort` ab. |
| `message_start` mit Usage null, die Zahlen im einen `message_delta` | `Usage` | Wie die Kompatibilitätsseite beschreibt; `cache_creation_input_tokens` ist immer 0. |
| `document`-Blöcke, `image`-Blöcke mit einer Datei-Quelle, Server-Tools, ein erzwungenes `tool_choice` oder ein Prefill bei aktivem Thinking | gesendet | Fireworks antwortet mit 400, das die App erreicht. |
| Fehler in Anthropics Hülle mit einer `request_id` (`errors/recorded-invalid-api-key`) | `KppError` | Ein unbekanntes Modell oder ein unbekannter Pfad erhält stattdessen Fireworks' eigenen Body `{"error": {"code": …}}`, der wie OpenAIs gelesen wird. |
| `POST /v1/messages/count_tokens` | — | Fireworks antwortet mit 404, daher lehnt Kaeso Token-Zählungen ab, bevor etwas gesendet wird. |

## SambaNova

Quellen: die Seiten [Messages API](https://docs.sambanova.ai/docs/en/features/messages) und [Anthropic compatibility](https://docs.sambanova.ai/docs/en/features/anthropic-compatibility) von SambaNova und das Schema `Message` seines [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json), gelesen am 2026-11-09, und `fixtures/anthropic_messages/providers/sambanova/`, daraus gebaut, mit dem live erhaltenen Fehler ohne gültigen Schlüssel. Ein SambaNova-Modell sendet als Messages, wenn sein Wert **Sendet als** das sagt; der Schlüssel geht als `Authorization: Bearer`.

| SambaNova-Feld | KPP | Hinweise |
|---|---|---|
| `thinking`-Blöcke ohne `signature` | `Block::Reasoning` | Reasoning-Modelle senden sie ohne Thinking-Einstellung (`responses/thinking`). |
| Server-Tools, `document`-Blöcke, Bilder per URL | gesendet | SambaNova antwortet mit 400, das die App erreicht. |
| Fehler in Anthropics Hülle mit einer `request_id` (`errors/recorded-invalid-api-key`) | `KppError` | |
| `POST /v1/messages/count_tokens` | `input_tokens` | Anthropics Form. Kaeso zählt dort die Tokens jedes SambaNova-Modells, in welchem Format das Modell auch sendet. |

## Nächste Schritte

- [Anthropic Messages](anthropic-messages.md): Finden Sie, wohin jedes Feld des Formats Anthropic Messages in KPP geht.
- [Fixtures und Aufzeichnungen](fixtures.md): Zeichnen Sie die Fixtures eines Anbieters mit `cargo xtask record anthropic_messages --provider <id>` auf.
