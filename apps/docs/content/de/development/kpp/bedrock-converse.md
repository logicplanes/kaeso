---
title: Amazon Bedrock Converse
description: Wie Generierungsanfragen von KPP auf die Converse-API von Amazon Bedrock abgebildet werden, über die Kaeso Nova und Llama aufruft.
order: 9
---

Schlagen Sie hier nach, wie eine Generierungsanfrage die Converse-API von Amazon Bedrock erreicht: welches Feld wohin gehört, wie der binäre Stream gelesen wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::bedrock_converse::BedrockConverse` (`crates/kaeso-codecs/src/bedrock_converse/`)
- **Pfade:** `POST /model/{modelId}/converse` und `/model/{modelId}/converse-stream`, die Modell-ID oder das Inferenzprofil prozentkodiert, auf dem Runtime-Host der Region des Schlüssels
- **Fixtures:** `crates/kaeso-codecs/fixtures/bedrock_converse/`, aus den unten genannten Referenzen gebaut, keine aufgezeichnet

**Quellen.** [Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html), [ConverseStream](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html), [ContentBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlock.html), [Tool-Nutzung mit Amazon Nova](https://docs.aws.amazon.com/nova/latest/userguide/tool-use-invocation.html) und die [Event-Stream-Kodierung](https://smithy.io/2.0/aws/amazon-eventstream.html), gelesen am 2026-11-16.

## Entscheidungen

- **Nur ausgehend.** Keine App schickt Converse an Kaeso; der Codec dekodiert keine Anfrage und schreibt keine Antwort darin.
- **Welche Modelle.** KPMR nennt `bedrock_converse` als ausgehenden Dialekt der Chat-Modelle von Bedrock, die weder Chat Completions noch Messages beantworten, etwa Amazon Nova, Meta Llama und Mistral Pixtral. Claude-Modelle, die Messages bedient, bleiben bei Messages.
- **Schlüssel.** Converse nimmt einen Bedrock-API-Schlüssel als Bearer-Token an, und AWS-Anmeldedaten, mit Signature Version 4 signiert.
- **Was eine Modellfamilie eigens braucht, wird noch nicht geschrieben.** Reasoning-Einstellungen, `top_k`, JSON-Ausgabeformate und andere Felder, die in `additionalModelRequestFields` gehören, sind `Unsupported` und werden nie verworfen.

## Anfrage

| KPP | Converse |
| --- | --- |
| Textblöcke in `system` | `system[]` `{text}` |
| `messages[]` | `messages[]` `{role, content[]}`; aufeinanderfolgende Nachrichten einer Rolle werden zusammengefasst, da Converse abwechselnde Rollen verlangt |
| Text, Ablehnung | `{text}` |
| Bild, als Bytes oder `data:`-URL | `{image: {format, source: {bytes}}}`, Format `png`, `jpeg`, `gif` oder `webp` |
| Dokument | `{document: {format, name, source}}`: Bytes oder `text` für ein Textdokument; `name` aus dem Titel oder `Document N` |
| Tool-Aufruf | `{toolUse: {toolUseId, name, input}}` |
| Tool-Ergebnis | `{toolResult: {toolUseId, content[], status}}`, der Inhalt als `text`, `json`, `image` oder `document`, `status: "error"` bei einem Fehler |
| Eigenes Reasoning von Converse | `{reasoningContent: {reasoningText: {text, signature}}}` oder `{redactedContent}` |
| Cache-Hinweis | ein `{cachePoint: {type: "default", ttl}}`-Block nach dem markierten Block oder Tool, `ttl` `5m` oder `1h` |
| `max_output_tokens`, `temperature`, `top_p`, `stop` | `inferenceConfig.maxTokens`, `temperature`, `topP`, `stopSequences` |
| Funktions-Tools | `toolConfig.tools[].toolSpec` `{name, description, inputSchema: {json}, strict}` |
| `tool_choice` auto, required, ein Tool | `toolConfig.toolChoice` `{auto}`, `{any}`, `{tool: {name}}` |

Tool-Namen und Aufruf-IDs, die die Muster von Converse ablehnen (Namen `[a-zA-Z0-9_-]`, IDs `[a-zA-Z0-9_.:-]`, höchstens 64 Zeichen), werden beim Senden umgeschrieben und in der Antwort wiederhergestellt.

Mit `Unsupported` abgelehnt: Bilder und Dokumente per Weblink oder Datei-ID, Audio- und Videoblöcke, Dokumentzitate, gehostete Tools, `tool_choice: none`, `parallel_tool_calls: false`, Reasoning-Einstellungen, strukturierte Ausgabe, `top_k`, `seed` und die Penalties. Reasoning eines anderen Formats wird nicht gesendet.

## Antwort

| Converse | KPP |
| --- | --- |
| `output.message.content[]` `text` | Textblock |
| `toolUse` | Tool-Aufruf, ID und Name so, wie die Anfrage sie gesendet hat |
| `reasoningContent` | Reasoning-Block, die Signatur oder der geschwärzte Inhalt als opake Daten |
| andere Blöcke | unbekannte Blöcke |
| `stopReason` `end_turn`, `tool_use`, `max_tokens`, `stop_sequence` | derselbe Stoppgrund |
| `guardrail_intervened`, `content_filtered` | `content_filter` |
| `model_context_window_exceeded` | `max_tokens` |
| `usage.inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheWriteInputTokens` | Eingabe-, Ausgabe-, Cache-Lese- und Cache-Schreib-Tokens |
| Header `x-amzn-requestid` | die ID der Antwort |

## Streams

`converse-stream` antwortet in der binären Event-Stream-Kodierung von AWS (`application/vnd.amazon.eventstream`), nicht mit Server-Sent Events. `bedrock_converse::eventstream::Framer` liest ihre Nachrichten: ein 12-Byte-Präludium mit Gesamtlänge, Länge der Header und deren CRC32, die Header, die Nutzdaten und das CRC32 der Nachricht. Eine Prüfsumme, die nicht passt, beendet den Stream mit einem Fehler.

| Event | KPP |
| --- | --- |
| `messageStart` | Beginn der Antwort |
| `contentBlockStart` mit `toolUse` | ein Tool-Aufruf-Block beginnt |
| `contentBlockDelta` `text`, `toolUse.input`, `reasoningContent` | Text-, Tool-Argument-, Reasoning-Text- oder opake Deltas; ein Text- oder Reasoning-Block beginnt mit seinem ersten Delta |
| `contentBlockStop` | der Block endet |
| `messageStop` | der Stoppgrund, bis zur Nutzung zurückgehalten |
| `metadata` | die Nutzung, danach endet die Antwort |
| eine Exception-Nachricht, etwa `throttlingException` | ein Fehler-Event im Format der App |

## Fehler

Bedrock nennt den Fehler im Header `x-amzn-errortype` oder im `__type` des Bodys, die Meldung in `message`:

| Fehler | KPP |
| --- | --- |
| `ValidationException` | `invalid_request`, oder `context_too_long`, wenn die Meldung sagt, dass die Eingabe zu lang ist |
| `ThrottlingException`, `ServiceQuotaExceededException` | `rate_limited` |
| `AccessDeniedException` | `permission_denied`, ein abgelehnter Schlüssel |
| `UnrecognizedClientException`, `InvalidSignatureException`, `ExpiredTokenException` | `authentication` |
| `ModelNotReadyException`, `ServiceUnavailableException` | `overloaded` |
| `ModelTimeoutException` | `timeout` |
| `ResourceNotFoundException` | `not_found` |
| `ModelErrorException`, `InternalServerException` | `provider_error` |
