---
title: Chat-Completions-Gateways
description: Was die Gateways und Router, die Chat Completions an andere Anbieter weitergeben, ergänzen, und wie der Codec ihnen begegnet.
order: 6
nav: Gateways
---

Gateways und Router, die Anfragen an andere Anbieter weitergeben, sprechen [OpenAI Chat Completions](openai-chat.md) mit eigenen Unterschieden. Schlagen Sie hier nach, was jeder ergänzt, bevor Sie sein Profil ändern; ihre Profile stehen in der Tabelle der [Anbieterprofile](openai-chat-providers.md#anbieterprofile), und die anderen Anbieter stehen unter [Chat-Completions-Anbieter](openai-chat-providers.md).

Ihre Fixtures liegen in `fixtures/openai_chat/providers/<id>/` ([Fixtures](fixtures.md)).

## OpenRouter

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

## Vercel

Quellen: Vercels Seiten [Chat Completions](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions), [reasoning](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/reasoning) und [advanced configuration](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/advanced), gelesen am 2026-10-29, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/vercel/`. Chat Completions ist Vercels `generation`-Dialekt, den jedes Modell spricht, das Vercel listet.

| Vercel-Feld | KPP | Hinweise |
|---|---|---|
| `providerOptions` (Routing unter `gateway`, etwa `order`, `only`, `sort`, `models`, `byok`, `tags`, `user`), `models`, `provider`, `reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt (`requests/gateway-options`). |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils, wie bei OpenRouter. |
| `message.reasoning_details`, `delta.reasoning_details` | Extras | Dasselbe Reasoning in OpenRouters strukturierter Form, mit `format: "unknown"` für Modelle, die weder von OpenAI noch von Anthropic sind. |
| `provider_metadata` an der Nachricht und am Delta des letzten Chunks (`gateway.routing`, `cost`, `marketCost`, `gatewayCost`, `generationId`), `generationId` auf oberster Ebene | Extras | Gegenüber anderen Dialekten weggelassen. Die `clientSessionId` der Aufzeichnungen, ein Fingerabdruck des Clients, ist durch Nullen ersetzt. |
| `usage.cost`, `market_cost`, `gateway_cost`, `surcharge_cost`, `reporting_tag_write_cost`, `is_byok`, `cost_details`, `cache_creation_input_tokens`, die zusätzlichen Token-Angaben | `raw` der Nutzung | Kaeso berechnet Kosten mit den Preisen des Katalogs. `usage.cost` ist nur die Inferenz: Die Tags in `requests/gateway-options` kosteten einen Aufschlag von 0,00015 $, den nur `gateway_cost` enthält. |
| der letzte Stream-Chunk, mit `finish_reason`, `provider_metadata` und `usage` zusammen | `Usage` | Gelesen wie ein Nutzungs-Chunk mit `choices: []`. |
| Fehler in OpenAIs Hülle mit Vercels eigenem `type` (`model_not_found`, `no_providers_available`, `authentication_error`), einem `param`, das ein Objekt ist, und `providerMetadata` neben `error` | `KppError` | Die Art folgt dem Status: 401 für einen falschen Schlüssel, 403 für ein Modell, das das Konto nicht aufrufen darf (`errors/recorded-restricted-model`, ein Konto im kostenlosen Tarif und Claude), 404 für ein unbekanntes Modell. Chat-Apps erhalten den Body so, wie Vercel ihn geschickt hat. |
| `GET /v1/models`: OpenAIs Form, jedes Modell mit Vercels eigenen Feldern (`name`, `type`, `context_window`, `max_tokens`, `pricing`, `tags`, `supported_parameters`) | `ModelList`, Extras | Die Modellliste beantwortet einen falschen Schlüssel mit 401 (`errors/list-invalid-api-key`), aber `GET /v1/models/{model}`, das eine Modellprüfung zuerst fragt, antwortet, ohne den Schlüssel zu prüfen. |

## Standard Compute

Quellen: die Seiten [Chat Completions](https://docs.standardcompute.com/api/chat-completions), [Modelle und Routing](https://docs.standardcompute.com/models), [Fehler](https://docs.standardcompute.com/api/errors) und [Modellliste](https://docs.standardcompute.com/api/models) von Standard Compute, gelesen am 2026-11-12, und die Live-Aufnahmen in `fixtures/openai_chat/providers/standardcompute/`, die meisten über Smart Routing (`standardcompute`).

| Feld von Standard Compute | KPP | Hinweise |
|---|---|---|
| `model: "StandardCompute"` in einer Antwort von Smart Routing | `Response.model` | Die Antwort nennt nie das Modell, das geantwortet hat; die Anfrage-Logs von Standard Compute tun es. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Über `reasoning_fields` des Profils, wie bei OpenRouter. |
| `message.reasoning_details`, `delta.reasoning_details` | Extras | Dasselbe Reasoning in der strukturierten Form von OpenRouter, auch die Gedankensignatur eines Gemini-Modells als Item `reasoning.encrypted`; Chat-Apps erhalten es unverändert zurück und senden es in späteren Runden zurück. |
| `native_finish_reason`, `refusal: null`, `system_fingerprint: null`, `service_tier` | Extras | Gegenüber anderen Dialekten weggelassen. |
| `cache_write_tokens`, `audio_tokens`, `video_tokens`, `image_tokens` in `usage` | `raw`-Nutzung | |
| der letzte Stream-Chunk, der die Abschlusswahl neben `usage` wiederholt | `Usage` | Gelesen wie ein Nutzungs-Chunk mit `choices: []`. |
| ein `error` mit Status 401 ohne `code`; ein Fehler mit numerischem `code` vom Anbieter des Modells | `KppError` | Die Art folgt dem Status; 402 `insufficient_quota` mit `code: "budget_exhausted"` ist `Billing`. |
| 400 `model_not_found` für ein unbekanntes Modell | `KppError` der Art `InvalidRequest` | Nie 404, daher markiert es nie ein Katalogmodell als nicht verfügbar. |
| `GET /v1/models`: `StandardCompute`, `context_length`, `architecture`, `supported_parameters`, `anthropic_family_tier`, `is_family_default` | `ModelList`, Extras | Ohne Schlüssel abrufbar. Die Liste eines Schlüssels kann Modelle auslassen, die der Schlüssel trotzdem aufrufen darf, und nennt den Router `StandardCompute`, schreibt `anthropic/claude-haiku-5-5` für das `anthropic/claude-haiku-5.5`, das seine Antworten nennen, daher vergleicht eine Modellprüfung IDs ohne Groß- und Kleinschreibung und ohne `.` von `-` zu unterscheiden und nennt ein Modell, das sie nicht findet, unklar. `GET /v1/models/{model}` antwortet für jedes Modell mit 404. |

## Cloudflare AI Gateway

Quellen: die Seiten [REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/), [Unified Billing](https://developers.cloudflare.com/ai-gateway/features/unified-billing/) und [Modellkatalog](https://developers.cloudflare.com/ai/models/) von Cloudflare, gelesen am 2026-11-12, und die Live-Aufnahmen in `fixtures/openai_chat/providers/cloudflare-ai-gateway/` von Workers-AI-Modellen (Modelle anderer Anbieter antworteten ohne Guthaben mit 402).

| Feld von Cloudflare | KPP | Hinweise |
|---|---|---|
| `/client/v4/accounts/{account_id}/ai/v1/…` | `EgressCtx::base_path` | Das Konto des Schlüssels tritt für Chat Completions, Responses und Messages an die Stelle von `/v1`; das Gateway eines Schlüssels steht in `cf-aig-gateway-id`. |
| `message.reasoning` und `reasoning_content`, `delta.reasoning` und `reasoning_content` | `Block::Reasoning` | Derselbe Text in beiden Feldern; das Profil liest `reasoning`, und Chat-Apps erhalten `reasoning_content` unverändert zurück. |
| `usage` in jedem Stream-Chunk, dann ein letzter Chunk mit der ganzen Nutzung | `Usage` | Jeder Chunk zählt nur seine eigenen Tokens; der letzte, nach `finish_reason`, enthält die Summen. |
| `usage.neurons` | `raw`-Nutzung | Die eigene Einheit von Workers AI; Kaeso bepreist Tokens. |
| `refusal`, `annotations`, `audio`, `function_call`, `tool_calls: []`, `token_ids` | Extras | Round-Trip für Chat-Apps. |
| `{"errors": [{"code", "message"}], "success": false}` | `KppError` | Die eigene Hülle von Cloudflare für ein abgelehntes Token (401, Code 10000), ein leeres Guthaben (402, Code 2021), ein unbekanntes (404, Code 7003) oder eingestelltes Modell (410, Code 7010), einen Bild-Link (400, Code 6004) und eine abgelehnte Reasoning-Stufe (400, Code 8001). Die Art folgt dem Status; die Meldung ist die von Cloudflare. |
| Messages-Fehler `{"type": "error", "error": {"type": "invalid_request_error"}}` mit 402 | `KppError` der Art `Billing` | Ein 402 ist ein Abrechnungsfehler, unabhängig von seinem Typ. |
| Responses-Streams von Workers-AI-Modellen | nicht genutzt | `$error: Error: Network connection lost.` oder die eigenen `{"response"}`-Events von Workers AI statt Responses-Events (live), daher streamt Kaeso Workers-AI-Modelle über Chat Completions. |
