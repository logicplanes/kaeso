---
title: Anthropic Messages
description: Wie jedes Feld des Formats Anthropic Messages auf KPP abgebildet wird und zurück.
order: 3
---

- **Codec:** `kaeso_codecs::anthropic_messages::AnthropicMessages` (`crates/kaeso-codecs/src/anthropic_messages/`)
- **Endpunkte:** `POST /v1/messages` (JSON und SSE), `POST /v1/messages/count_tokens` und die [Modell-Endpunkte](#modelle)
- **Fixtures:** `crates/kaeso-codecs/fixtures/anthropic_messages/`

**Quellen.** Die OpenAPI-Spezifikation von Anthropic `anthropic-465bff21a179090915396565d1ae8f705cf8596e2ec920eb121072f25b8a7d68.yml` (die letzte `openapi_spec_url` in `anthropic-sdk-python/.stats.yml`, SDK-Commit `41aa7674`), die SDK-Typen in `anthropic-sdk-python@b4b7916d` (1.13.0) und die [Claude-Dokumentation](https://platform.claude.com/docs/en/) (`api/messages/create`, `api/messages/count_tokens`, `api/errors`, `api/versioning`, `build-with-claude/streaming`, `…/thinking`, `…/extended-thinking`, `…/effort`, `…/prompt-caching`, `…/structured-outputs`, `…/citations`, `…/pdf-support`, `…/vision`, `agents-and-tools/tool-use/*`, `…/handling-stop-reasons`). Wo die SDK-Typen neuer sind als die Spezifikation, haben sie Vorrang, und ein Feld, das nur in den SDK-Typen vorkommt, wird als solches genannt. Schemanamen unten beziehen sich auf die Spezifikation.

## Lesart der Tabellen

- **KPP**: das typisierte Feld, auf das der Wire-Wert abgebildet wird.
- **fields**: roh in `Extras::fields` aufbewahrt. Diese Felder sind semantisch: Sie werden nur an Anthropic zurückgeschrieben, für andere Dialekte mit `Unsupported` abgelehnt ([Extras](cross-format.md#extras)), sofern die [Allowlist](cross-format.md#extras) sie nicht weglassen lässt, und in Antworten an andere Dialekte weggelassen ([Antworten](cross-format.md#antworten)). Verschachtelte Objekte verwenden Schlüsselpfade mit Punkten (`metadata.x`).
- **form**: eine codec-interne Darstellungsmarkierung in `Extras::form`, die andere Dialekte ignorieren. `verbatim` steht für den allgemeinen Mechanismus von `wire::Ext` (die Schreibweisen `null`, `false` und `[]`).
- **raw**: vollständig in einem `RawItem`, `Citation` oder `RawUsage` aufbewahrt.
- **Eigene** Daten haben Extras oder eine Herkunft `anthropic_messages`; **fremde** Daten stammen aus einem anderen Dialekt.

**Zahlen.** Typisierte Zahlenfelder werden als `u32` oder `u64` gelesen, `temperature` und `top_p` als JSON-Zahlen, die KPP als `f64` hält. Ein Wert außerhalb ihres Bereichs wird nie verändert: Das Dekodieren schlägt fehl, mit `InvalidRequest` in einer Anfrage (`` The request body is not a valid Anthropic Messages request: `top_k` must be a non-negative integer. ``) und `ProviderError` in einer Antwort des Anbieters.

**Schlüssel, die Kaeso nicht tragen kann.** Jedes Objekt, das Kaeso Feld für Feld liest, bewahrt seine unbekannten Schlüssel als fields auf, außer einem Schlüssel, der mit `header:` beginnt oder `.` enthält: Er würde mit einem weitergegebenen Header oder einem verschachtelten Schlüsselpfad verwechselt, daher wird das Objekt abgelehnt ([Wire-Structs](index.md#wire-structs)). In einer Anfrage ist das `InvalidRequest` (`` The request body is not a valid Anthropic Messages request: the field `a.b` has a name Kaeso cannot carry; remove it. ``), in einer Antwort, einem Stream oder einer Modellliste `ProviderError`. Ein Fehler-Body mit einem solchen Schlüssel auf oberster Ebene gilt als Body, der keine `ErrorResponse` ist ([Fehler](#fehler)).

- **Feld für Feld gelesen:** der Body, Nachrichten, `metadata`, `output_config`, `thinking`, `tool_choice`, Funktions-Tools, die bekannten Blocktypen der Tabelle [Content-Blöcke](#content-blöcke) mit ihren Objekten `source`, `cache_control` und `citations`, Antworten, Token-Zählungen, Stream-Events und ihre Deltas, Modellinfos und Modelllisten.
- **Vollständig aufbewahrt, nicht geprüft:** unbekannte Blöcke und Tools, `input_schema`, die Eingabe eines Tools, Zitate, `usage` und `output_config.format`.

## Anfragen

Schemas `CreateMessageParams` und `CountMessageTokensParams`.

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `model` | `model` | |
| `messages[]` (`InputMessage`) | `messages[]` | Unbekannte Schlüssel → `extras.fields` der Nachricht. |
| `messages[].output_config`, `.clear_at` (Systemnachrichten, Beta; SDK `BetaMessageParam`) | `extras.fields` der Nachricht | Andere Dialekte: Ein `output_config`, das `{}` ist oder das `output_config.effort` der Anfrage wiederholt, wird weggelassen, weil es nichts ändert (Claude Code sendet es an seiner Systemnachricht); jeder andere Wert und `clear_at` → `Unsupported` ([Extras](cross-format.md#extras)). |
| `messages[].role` `user` / `assistant` | `Role::User` / `Role::Assistant` | |
| `messages[].role` `system` | `Role::System` | Eine Systemnachricht mitten in der Unterhaltung (SDK `MessageParam.role`; [Systemnachrichten mitten in der Unterhaltung](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)). Geht unverändert an Anthropic zurück und an OpenAI als spätere `system`- oder `developer`-Nachricht ([Systemnachrichten](cross-format.md#systemnachrichten)). Andere Rollen → `InvalidRequest`. |
| `messages[].content` als String | ein `Block::Text` | form `content: "string"`; fremde Nachrichten werden immer als Arrays gesendet. |
| `messages[].content` als Array | `blocks` | `[]` → form `content: "array"`. |
| `system` als String | `system: [Text]` | form `system: "string"`. |
| `system` als Array (`RequestTextBlock[]`) | `system` | `[]` → form `system: "array"`. |
| `max_tokens` | `sampling.max_output_tokens` | Für die Generierung Pflicht (`CreateMessageParams`): Eine Anfrage ohne das Feld ist schon beim Dekodieren `InvalidRequest` (`` The request body is not a valid Anthropic Messages request: `max_tokens` is required. ``, Parameter `max_tokens`), sodass das Gateway es für eine Anthropic-App nie ergänzt; die Token-Zählung hat kein `max_tokens`. `0` ist gültig, das Minimum der Spezifikation: Es füllt den Prompt-Cache ohne Antwort. Eine Anfrage aus einem anderen Dialekt ohne `sampling.max_output_tokens` ist in `check()` `InvalidRequest` und nennt das Feld der App (`max_completion_tokens` oder `max_output_tokens`, [Formatwechsel](cross-format.md#anfragen)). |
| `temperature`, `top_p`, `top_k` | `sampling.temperature`, `.top_p`, `.top_k` | In der Spezifikation veraltet und in SDK 1.13.0 nicht vorhanden, werden aber weiter abgebildet. `temperature` und `top_p` behalten ihre Schreibweise: Die als Ganzzahl geschriebenen Schlüssel (`1`) stehen in form `integers` und werden wieder als Ganzzahl geschrieben, solange der Wert noch ganzzahlig ist, sodass `temperature: 1` als `1` zurückkommt, nicht als `1.0`; ein geänderter Wert wird als neuer Wert geschrieben. Anfragen aus anderen Dialekten erhalten die Schreibweise von Anthropic (`1.0`) ([Sampling](cross-format.md#sampling)). |
| `stop_sequences` | `sampling.stop` | `[]` → verbatim. |
| `stream: true` | `stream = On { include_usage: true }` | Anthropic meldet die Nutzung immer. `stream: false` → verbatim. |
| `metadata.user_id` | `user` | `null` → verbatim (`verbatim:metadata`); andere Schlüssel → fields `metadata.*`; ein vorhandenes `metadata`-Objekt → form `objects: ["metadata"]`. |
| `thinking.type` `enabled` / `disabled` / `adaptive` | `reasoning.mode` `Enabled` / `Disabled` / `Adaptive` | `between_tools` (nur im SDK) und künftige Typen → `ReasoningMode::Other`. |
| `thinking.budget_tokens` | `reasoning.budget_tokens` | |
| `thinking.display` und andere `thinking`-Schlüssel | `reasoning.extras.fields` | Kein KPP-Feld. `display` steht auf der [Allowlist](cross-format.md#extras) und wird daher für andere Dialekte weggelassen; OpenAI Responses erhält `reasoning.summary: "auto"`, außer der Wert ist `omitted` ([lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)). Andere Schlüssel → dort `Unsupported`. |
| `output_config.effort` | `reasoning.effort` | `low`, `medium`, `high`, `xhigh`, `max`; unbekannte Werte → `ReasoningEffort::Other`. |
| `output_config.format` `{type: json_schema, schema}` | `output = JsonSchema { schema }` | `name`, `description` und `strict` sind `None`. Unbekannte Schlüssel von `format` → die `extras` der Spezifikation. Andere Formattypen und ein `json_schema`-Format, das Kaeso nicht Feld für Feld lesen kann (ohne `schema` oder mit einem Schlüssel, den es nicht tragen kann), → fields `output_config.format`. |
| andere `output_config`-Schlüssel (etwa `task_budget`, Beta) | fields `output_config.*` | Ein vorhandenes `output_config` → form `objects: ["output_config"]`; `null`-Einträge → `verbatim:output_config`. `task_budget` stellt dem Modell einen Countdown voran und ist daher semantisch: andere Dialekte → `Unsupported`, mit dem Hinweis, in Claude Code `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` zu setzen. |
| `context_management` (Beta, `BetaContextManagementConfigParam`) | fields `context_management` | Andere Dialekte: weggelassen, wenn es nur `edits` enthält und jede Änderung `clear_thinking_*` ist, weil sie nur Thinking-Blöcke von Anthropic betrifft, die nie den Dialekt wechseln ([Reasoning](reasoning.md)); Claude Code sendet `clear_thinking_20251015` mit `keep: "all"` bei jeder Anfrage. Andere Änderungen (`clear_tool_uses_*`, `compact_*`) ändern, was das Modell sieht → `Unsupported`, mit dem Hinweis auf `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`. |
| `tool_choice` `auto` / `any` / `none` / `tool{name}` | `tool_choice` `Auto` / `Required` / `None` / `Tool(name)` | Andere Typen, `tool` ohne `name` und `auto`, `any` oder `none` mit einem `name` → `ToolChoice::Unknown` (raw, nur eigen). Unbekannte Schlüssel → fields `tool_choice.*`. |
| `tool_choice.disable_parallel_tool_use` | `parallel_tool_calls = Some(!value)` | |
| `tools[]` mit `type` fehlend, `null` oder `custom` (`Tool`) | `ToolDef::Function` | Die Schreibweise von `type` → `verbatim`. |
| `tools[].name`, `.description`, `.input_schema`, `.strict`, `.cache_control` | `name`, `description`, `parameters` (raw), `strict`, `cache` | |
| `tools[].eager_input_streaming`, `.allowed_callers`, `.defer_loading`, `.input_examples` | `extras.fields` des Tools | Andere Dialekte → `Unsupported`; die Meldung rät Nutzern von Claude Code, `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` zu setzen, womit diese Beta-Felder für Tools wegfallen. |
| `tools[]` jedes anderen Typs (Server-Tools wie `web_search_20250305` oder `advisor_20260301`) | `ToolDef::Unknown` (raw) | Nur im selben Dialekt. Für andere Dialekte nennt die `Unsupported`-Meldung den Typ nach `Input tag` (`Input tag 'advisor_20260301' found using 'type' is not a function tool, …`), im Wortlaut der API selbst, sodass Claude Code es ohne das Advisor-Tool erneut versucht ([Gateway-Protokoll](https://code.claude.com/docs/en/llm-gateway-protocol), „Automatic retry and error forwarding“). |
| `cache_control` (oberste Ebene, automatisches Caching) | fields `cache_control` | Nur ein Hinweis; KPP hat keinen Cache-Hinweis auf Ebene der Anfrage. Steht auf der [Allowlist](cross-format.md#extras) und wird daher für andere Dialekte weggelassen. |
| `container`, `inference_geo`, `service_tier`, SDK `diagnostics`, `user_profile_id`, `workspace_id`, unbekannte Schlüssel | `extras.fields` | |
| Header `anthropic-beta` | `extras`-Header `header:anthropic-beta` | Wird beim Egress nur an Anthropic weitergegeben. |
| Header `anthropic-version` | `extras`-Header, wenn er nicht `2023-06-01` ist | Die festgelegte Version wird immer gesendet, außer eine neuere wurde weitergegeben ([die Anthropic-Version](cross-format.md#die-anthropic-version)). |

### Content-Blöcke

Diese Regeln gelten für Anfragen und Antworten gleichermaßen.

| Wire-Block | KPP | Hinweise |
|---|---|---|
| `text` (`text`, `citations`, `cache_control`) | `Block::Text` | `citations[]` → `Citation { origin, raw }`; `null` und `[]` → verbatim. |
| `image` `source.type: base64` (`media_type`, `data`) | `Image { source: Base64 }` | |
| `image` `source.type: url` | `Image { source: Url }` | |
| `image` `source.type: file` (`file_id`) | `Image { source: FileId { origin } }` | Nur mit Anthropic nutzbar (`cross::file_id_usable`, [Dateien](cross-format.md#dateien)). |
| `image.transformations`, unbekannte Schlüssel | `extras.fields` des Blocks | Unbekannte Schlüssel der Quelle → `source.*`. Ein Bild, dessen `source.type` keiner dieser Typen ist → `Block::Unknown`. |
| `document` `source.type: base64` (PDF), `url` oder `file` | `Document { source: Base64 / Url / FileId }` | |
| `document` `source.type: text` (`text/plain`) | `Document { source: Text }` | |
| `document` `source.type: content` (eigener Inhalt) | `Block::Unknown` | Nur im selben Dialekt. |
| `document.title`, `.context` | `title`, `context` | `null` → verbatim. |
| `document.citations.enabled` | `citations` | `{enabled: false}` → form `citations: "disabled"`; `{}` → `"empty"`; `null` → verbatim. |
| `cache_control` an jedem Block (`CacheControlEphemeral`) | `cache = CacheHint { ttl }` | `ttl` `5m` → `FiveMinutes`, `1h` → `OneHour`, andere Werte → `Other`; fehlend → `None`. Ein anderer `type` als `ephemeral` → `extras.fields.type` des Hinweises. |
| `tool_use` (`id`, `name`, `input`, `cache_control`) | `Block::ToolCall` (`arguments` raw) | `caller` und `toolset_name` → `extras.fields` des Blocks, außer `caller: {type: "direct"}`, ein gewöhnlicher Aufruf: Er bleibt verbatim, damit der Aufruf zu anderen Dialekten wechseln kann. |
| `tool_result` (`tool_use_id`, `content`, `is_error`, `cache_control`) | `Block::ToolResult` | `content` als String → form `content: "string"`; `[]` → `"array"`; `is_error: false` → verbatim. `toolset_name` → fields. Ein `tool_result`, das mehr als 4 Ebenen tief in einem anderen steckt, wird abgelehnt: `InvalidRequest` in einer Anfrage, `ProviderError` in einer Antwort oder einem Stream. |
| `thinking` (`thinking`, `signature`) | `Block::Reasoning { text: [thinking], opaque: [AnthropicThinkingSignature] }` | `thinking: ""` (Anzeige ausgelassen) bleibt `[""]`. `signature: ""` sind keine opaken Daten: Daraus wird form `empty_signature`, sodass der Block nie als signiertes Thinking gilt, und er wird mit `""` zurückgeschrieben; eine fehlende `signature` bleibt weg. Eine `signature`, die ein Reasoning-Blob ist (`kpp:reasoning:v1:…`, Reasoning eines anderen Dialekts, das Kaeso der App gegeben hat, siehe [Reasoning-Blobs](reasoning.md#reasoning-blobs)), wird zum ursprünglichen Block darin dekodiert, samt Herkunft; der `thinking`-Text wird dann ignoriert. |
| `redacted_thinking` (`data`) | `Block::Reasoning { opaque: [AnthropicRedactedThinking] }` | |
| jeder andere Typ (`server_tool_use`, `web_search_tool_result`, `web_fetch_tool_result`, `code_execution_tool_result`, `search_result`, `container_upload`, `tool_reference`, künftige Typen) | `Block::Unknown` (raw) | Nur im selben Dialekt. Wird eine `tool_reference` (MCP-Tool-Suche) für einen anderen Dialekt abgelehnt, rät die Meldung, `ENABLE_TOOL_SEARCH` in Claude Code zu entfernen; dann ist die Tool-Suche hinter einem Gateway standardmäßig aus. |

### Anfragen aus anderen Dialekten

Was `encode_request` für eine KPP-Anfrage schreibt, die nicht aus Anthropic Messages stammt.

| KPP | Wire | Hinweise |
|---|---|---|
| `system` | `system` als Array von Textblöcken | Andere Blöcke im System-Prompt → `Unsupported`. |
| `messages[].blocks` | `content` als Array | Leere Textblöcke entfallen, weil Anthropic sie ablehnt. Nachrichten, die nach dem Weglassen von Reasoning oder leerem Text leer sind, entfallen. |
| `Block::Reasoning`, das nicht von Anthropic oder von einem anderen Anthropic-Anbieter stammt | weggelassen | [Reasoning](reasoning.md). Das gilt auch für Reasoning eines anderen Dialekts, das eine App in einem Reasoning-Blob zurückgesendet hat: Es geht nur an seinen eigenen Anbieter. |
| `thinking` ohne nicht leere Signatur | eigene Blöcke (aus Anthropic Messages gelesen): so geschrieben, wie sie ankamen, eine fehlende `signature` bleibt weg und `""` (form `empty_signature`) bleibt `""`; andere weggelassen | Ein Assistenten-Zug, der nur einen solchen eigenen Block enthält, bleibt erhalten. Unsigniertes Anthropic-Reasoning, das nicht aus Anthropic Messages gelesen wurde (per Code gebaut, ohne Anthropic-Extras), wird weggelassen, weil Anthropic unsignierte Thinking-Blöcke ablehnt. `redacted_thinking` wird wiedergegeben. |
| `Source::Url` mit einer base64-`data:`-URL (aus einem anderen Dialekt) | `source: {type: base64, media_type, data}` | Eine `data:`-URL, die nicht base64 ist → `Unsupported`; Medientypen werden wie bei base64 geprüft. |
| Inhalt eines `ToolResult` außer Text, Bild oder Dokument (und eigenen unbekannten Blöcken) | `Unsupported` | `RequestToolResultBlock.content`. |
| `Block::Refusal` | `Unsupported` (`refusal block`) | |
| `Text.cache`, jeder `CacheHint` | `cache_control {type: ephemeral, ttl?}` | Fremde Extras des Hinweises werden weggelassen. |
| `ToolCall.id` oder `ToolResult.call_id`, die nicht auf `^[a-zA-Z0-9_-]+$` passen | umgeschrieben (andere Zeichen werden zu `_`, dann werden `_` und ein 16-stelliger hexadezimaler FNV-1a-Hash angehängt) und in `EgressCtx::tool_ids` festgehalten | In Antworten und Streams zurückübersetzt. |
| Tool-Namen, die nicht auf `^[a-zA-Z0-9_-]{1,128}$` passen | ebenso umgeschrieben, höchstens 128 Zeichen einschließlich Hash, und in `EgressCtx::tool_names` festgehalten | Gilt für `tools[].name`, `tool_choice.name` und `tool_use.name`. |
| `FunctionTool.parameters: None` | `input_schema: {"type":"object","properties":{}}` | Bei OpenAI bedeuten fehlende Parameter eine leere Parameterliste. |
| `parallel_tool_calls: Some(false)` ohne `tool_choice` | `tool_choice: {type: auto, disable_parallel_tool_use: true}` | Nicht bei `none`, dessen Schema keinen solchen Schlüssel hat. |
| `reasoning.effort` `low` bis `max` ohne Modus oder Budget | `thinking: {type: adaptive}` plus `output_config.effort`; bei Modellen ohne adaptives Thinking (`EgressCtx::adaptive_thinking = Some(false)`) `thinking: {type: enabled, budget_tokens}` nach der Tabelle unter [Aufwand und Budget](reasoning.md#aufwand-und-budget) (`low` 2.048, `medium` 8.192, `high`, `xhigh` und `max` 24.576, angepasst auf mindestens 1.024 und weniger als `max_tokens`; ist `max_tokens` 1.024 oder kleiner, wird es um 1.024 erhöht und das Budget ist 1.024) | [Reasoning](reasoning.md). |
| `reasoning.summary` ohne Aufwand, Modus oder Budget, mit `EgressCtx::reasoning = Some(true)` | `thinking: {type: adaptive}` ohne Aufwand (die Standardtiefe des Modells); bei Modellen ohne adaptives Thinking das Budget für `medium` | Reasoning-Modelle von OpenAI denken standardmäßig nach, also erwartet eine App, die Zusammenfassungen anfordert, Reasoning ([lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)). Unbekannte Modelle oder Modelle ohne Reasoning: Es wird nichts gesendet. |
| `reasoning.summary` (jeder Wert) bei eingeschaltetem Thinking | `thinking.display: "summarized"` | Claude Opus 4.7 und später verwenden standardmäßig `omitted` (leerer Thinking-Text). Nicht bei der Token-Zählung. Bei Modellen, die zwischen Tool-Aufrufen kurze Fortschrittsnotizen schreiben, kommen diese mit den Zusammenfassungen gemischt zurück; `display: "updates"` wird nie gesendet. |
| `sampling.temperature`, `.top_p`, `.top_k`, wenn Thinking für einen anderen Dialekt eingeschaltet ist | weggelassen | Anthropic lehnt sie zusammen mit Thinking ab oder schränkt `top_p` ein ([Thinking und Sampling](reasoning.md#thinking-und-sampling)). Ohne Reasoning-Einstellungen werden sie so gesendet, wie sie ankamen, auch an Modelle, die standardmäßig nachdenken; das Gateway lehnt eine nicht standardmäßige `temperature` für Modelle ab, deren Katalogeintrag `temperature: false` sagt. |
| `tool_choice` `Required` oder `Tool` aus einem anderen Dialekt, mit `enabled`-Thinking | ohne `thinking` gesendet (und ohne dessen erhöhtes `max_tokens`) | Anthropic lehnt die Kombination ab, und ein erzwungener Aufruf enthält ohnehin kein Thinking. Adaptives Thinking behält das erzwungene Tool. |
| `reasoning.effort` `none` | kein `thinking`, kein Aufwand | Der Standard von Anthropic. Modelle, die ohne `thinking` nachdenken (Claude Opus 5 und 5.5, Sonnet 5 und 5.5, Haiku 5.5 und Fable), denken daher weiter mit ihrem Standardaufwand nach. `none` bei eingeschaltetem Thinking, `minimal` und unbekannte Aufwände → `Unsupported`. |
| `reasoning.budget_tokens` (ohne Modus) | `thinking: {type: enabled, budget_tokens}` | |
| `reasoning.mode = Enabled` ohne Budget; ein Budget mit `Adaptive` oder `Disabled` | `Unsupported` | Hier gilt keine Umrechnung zwischen Aufwand und Budget. |
| `reasoning.include_encrypted` aus einem anderen Dialekt | weggelassen | Es sagt nur, welches Reasoning zurückkommen soll (Codex sendet es immer); Anthropic gibt immer signiertes Thinking zurück. |
| `thinking` `enabled`, wenn die letzte Nachricht (abgesehen von Systemnachrichten danach) Tool-Ergebnisse zurückgibt und der letzte Assistenten-Zug Tool-Aufrufe machte, ohne mit einem wiedergebbaren Thinking-Block zu beginnen (signiertes `thinking` oder `redacted_thinking`; etwa weil dieser Zug von einem anderen Anbieter kam) | die Anfrage wird ohne `thinking` und `output_config.effort` gesendet | Anthropic lehnt eine solche Anfrage ab („a final assistant message must start with a thinking block“); siehe [Tool-Runden ohne Thinking](reasoning.md#tool-runden-ohne-thinking). `adaptive`-Thinking bleibt: Laut der Dokumentation zu adaptivem Thinking braucht es den führenden Block nicht. Sonst wird nichts festgehalten. Das ist selten, weil Thinking von Claude aus Responses- und Anthropic-Apps in [Reasoning-Blobs](reasoning.md#reasoning-blobs) zurückkommt und aus Chat-Apps über den [Speicher des Gateways](reasoning.md#vom-gateway-aufbewahrtes-reasoning), sodass der Zug mit wiedergebbarem Thinking beginnt. Die Regel gilt auch für Anfragen von Anthropic-Apps. |
| `output = JsonObject` | `Unsupported` (`json_object output`) | [Strukturierte Ausgabe](cross-format.md#strukturierte-ausgabe). |
| `output = JsonSchema` | `output_config.format` | `name` wird weggelassen; `description` und `strict: false` → `Unsupported`. |
| `sampling.seed`, `.frequency_penalty`, `.presence_penalty` | `Unsupported` | [Sampling](cross-format.md#sampling); nicht bei der Token-Zählung, bei der Sampling entfällt. |
| `sampling.temperature` außerhalb von 0 bis 1 | `Unsupported` | Der Bereich von Anthropic ist 0 bis 1 (OpenAI erlaubt bis 2); der Wert wird nicht umskaliert. Nicht bei der Token-Zählung. |
| `user` | `metadata.user_id` | |
| `Role::System` | eine `role: "system"`-Nachricht für Anfragen, die aus Anthropic Messages dekodiert wurden; aus anderen Dialekten `Unsupported` (`system message`) | [Systemnachrichten](cross-format.md#systemnachrichten). |
| base64-Bilder außer JPEG, PNG, GIF oder WebP; Bildquellen aus Text | `Unsupported` | `Base64ImageSource`. |
| base64-Dokumente außer `application/pdf`; Textdokumente außer `text/plain` | `Unsupported` | `Base64PDFSource`, `PlainTextSource`. |
| Argumente eines Tool-Aufrufs, die kein Objekt sind; Tool-Parameter, die kein Objekt sind | `Unsupported` | `input` und `input_schema` sind Objekte. |
| `Request::CountTokens` aus einem anderen Dialekt | `/v1/messages/count_tokens` ohne `max_tokens`, Sampling, `metadata` und `stream` | Sie ändern die Zahl der Eingabe-Token nicht, und der Endpunkt lehnt sie ab. Die eigene Zählanfrage einer Anthropic-App wird so gesendet, wie sie ankam. |
| Ausgabe- und Reasoning-Einstellungen eines `Request::CountTokens`, die `check()` für die Generierung ablehnen würde (JSON-Modus, eine Schemabeschreibung, `strict: false`, Aufwand `minimal`, …) | weggelassen, nicht abgelehnt | Abgelehnt wird nur, was die gezählte Eingabe ändert. |
| `Request::Embed` | `Unsupported` | |
| fremde Extras, unbekannte Blöcke, Tools und Tool-Auswahlen, Zitate, fremde Datei-IDs | `Unsupported` | `cross::check_foreign` ([Anfragen](cross-format.md#anfragen)). |

## Antworten

Schema `Message`.

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `id`, `model` | `id`, `model` | Aus einem anderen Format die ID des Anbieters, unverändert ([IDs von Antworten](cross-format.md#ids-von-antworten)). |
| `content[]` | `blocks` | Wie bei den [Content-Blöcken](#content-blöcke); Tool-IDs und -Namen werden über `EgressCtx` zurückübersetzt. |
| `stop_reason` `end_turn` / `max_tokens` / `stop_sequence` / `tool_use` / `pause_turn` / `refusal` | `stop` `EndTurn` / `MaxTokens` / `StopSequence(s)` / `ToolUse` / `PauseTurn` / `Refusal` | |
| `stop_reason` `model_context_window_exceeded` | `MaxTokens` plus form `spelling:stop_reason` | Die Ausgabe wurde vom Kontextfenster abgeschnitten; Anthropic-Apps erhalten den genauen Wert zurück. |
| unbekannte Werte von `stop_reason` | `StopReason::Other(value)` | |
| `stop_reason: null` oder fehlend | `EndTurn` plus form `spelling:stop_reason` | |
| `stop_sequence` | in `StopSequence(s)` | Jede andere Schreibweise (fehlend oder ein Wert bei einem anderen Stoppgrund) → form `spelling:stop_sequence`. |
| `stop_details` (`RefusalStopDetails`), `container`, `diagnostics`, `type`, `role`, unbekannte Schlüssel | `extras.fields` | |
| `usage.input_tokens` | `usage.input_tokens` | Enthält Cache-Lese- und -Schreibvorgänge bereits nicht ([Nutzung](cross-format.md#nutzung)). |
| `usage.cache_read_input_tokens` | `usage.cache_read_tokens` | `null` → 0. |
| `usage.cache_creation_input_tokens` | `usage.cache_write_tokens` | `null` → 0. |
| `usage.cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` | `usage.cache_write_breakdown` | |
| `usage.output_tokens` | `usage.output_tokens` | Enthält das Thinking. |
| `usage.output_tokens_details.thinking_tokens` | `usage.reasoning_tokens` | |
| das ganze `usage` (mit `server_tool_use`, `service_tier`, `inference_geo`, `iterations` und unbekannten Schlüsseln) | `usage.raw` (an seine Herkunft gebunden) | Wird für Anthropic-Antworten mit denselben Schlüsseln und Werten wieder geschrieben. |

Antworten aus anderen Dialekten (`encode_response`) erhalten `type: "message"`, `role: "assistant"`, `stop_sequence: null` (oder die Sequenz) und `stop_details: null`. Die Nutzung wird aus den typisierten Feldern gebildet: `cache_creation` aus der Aufschlüsselung oder `null`, und `output_tokens_details`, wenn `reasoning_tokens` bekannt ist.

- **Stoppgründe.** `ContentFilter` wird zu `refusal`, weil Anthropic keinen Stoppgrund für Inhaltsfilter hat. `Other(s)` wird zu `s`, wenn es ein dokumentierter Wert von Anthropic ist; sonst werden `length`, `max_output_tokens` und `max_completion_tokens` zu `max_tokens`, `function_call` und `tool_calls` zu `tool_use`, `content_filter` zu `refusal` und alles andere zu `end_turn`. Eigene `Other`-Werte werden so geschrieben, wie sie ankamen.
- **Verweigerungen.** `Refusal`-Blöcke werden zu `text`-Blöcken.
- **Verlustbehaftete Tool-Argumente.** KPP bewahrt Argumente von Tool-Aufrufen, die kein gültiges JSON sind, etwa weil ein Ausgabelimit sie abgeschnitten hat, als JSON-String auf. Ein solcher Aufruf wird mit `input: {}` geschrieben, weil `tool_use.input` ein Objekt sein muss; der Teiltext geht für die App also verloren, steht aber weiter im Log des Gateways mit der KPP-Antwort. Anfragen mit solchen Argumenten lehnt `check()` ab.
- **Reasoning** aus anderen Dialekten wird zu einem `thinking`-Block ([Reasoning-Blobs](reasoning.md#reasoning-blobs)): seine Zusammenfassungen, dann sein Reasoning-Text, als `thinking` (Absätze durch eine Leerzeile getrennt), und ein Reasoning-Blob des ursprünglichen Blocks als `signature`. Anthropic sieht diesen Block nie: Der Anfrage-Decoder stellt das Original wieder her, das nur an seinen eigenen Anbieter zurückgeht.
- **Weggelassen** ([Antworten](cross-format.md#antworten)): Anthropic-Reasoning ohne Signatur, das nicht aus Anthropic Messages gelesen wurde (eigenes unsigniertes Thinking wird so geschrieben, wie es ankam); Bild-, Dokument- und Tool-Ergebnis-Blöcke, die nicht zur Content-Union von `Message` gehören; sowie fremde opake Daten, Zitate, Extras und unbekannte Blöcke.

## Token-Zählung

Schema `CountMessageTokensResponse`.

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `input_tokens` | `CountTokensResponse::input_tokens` | Alle Eingabe-Token; der Endpunkt meldet eine Gesamtzahl. |
| unbekannte Schlüssel | `extras.fields` | |

## Streams

Schema `MessageStreamEvent`.

| SSE-Event | KPP | Hinweise |
|---|---|---|
| `message_start` | `ResponseStart { id, model, usage }` | Andere Schlüssel von `message` (`type`, `role`, `content: []`, `stop_reason`, `stop_sequence`, `stop_details`, …) → fields `message.*`. |
| `content_block_start` `text` | `BlockStart(Text)` | Anfänglicher Text und Zitate → `BlockDelta`s mit form `in_start`. |
| `content_block_start` `tool_use` | `BlockStart(ToolCall { id, name })` | IDs und Namen werden zurückübersetzt; ein nicht leeres anfängliches `input` → ein `ToolArguments`-Delta mit `in_start`. |
| `content_block_start` `thinking` | `BlockStart(Reasoning { origin })` | Anfängliches Thinking und Signatur → Deltas mit `in_start`. |
| `content_block_start` `redacted_thinking` | `BlockStart(Reasoning { opaque: [AnthropicRedactedThinking] })` | |
| `content_block_start` anderer Typen | `BlockStart(Unknown)` (raw) | |
| ein Startblock, der vom Standard abweicht (`{type:text,text:""}`, `{type:tool_use,id,name,input:{}}`, `{type:thinking,thinking:"",signature:""}`, `{type:redacted_thinking,data}`) | form `content_block` (der genaue Block) | |
| `content_block_start.index`, der vom KPP-Index abweicht | form `index` | Anthropic nummeriert Blöcke 0, 1, …, das kommt also nur in ungewöhnlichen Streams vor. |
| `text_delta` | `Delta::Text` | |
| `input_json_delta` (`partial_json`) | `Delta::ToolArguments(partial_json)` | Die `partial_json`-Fragmente eines Blocks ergeben aneinandergehängt die Argumente. An `Unknown`-Blöcken (etwa `server_tool_use`): `Delta::Unknown` (raw). |
| `thinking_delta` | `Delta::ReasoningText` | |
| `signature_delta` | `Delta::Opaque(AnthropicThinkingSignature)` | |
| `citations_delta` | `Delta::Citation` | |
| unbekannte Delta-Typen | `Delta::Unknown` (raw) | |
| `content_block_stop` | `BlockStop` | |
| `message_delta.delta.stop_reason` / `.stop_sequence` | `ResponseDelta.stop` | Ungewöhnliche Schreibweisen → form `delta:spelling:*`. |
| `message_delta.delta.stop_details`, `.container`, unbekannte Schlüssel | fields `delta.*` | |
| `message_delta.usage` (`MessageDeltaUsage`) | `ResponseDelta.usage` (kumulativ) | Zahlen, die es auslässt, und die Aufschlüsselung der Cache-Schreibvorgänge werden aus `message_start` ergänzt; `raw` ist das eigene Objekt des Deltas. |
| `message_stop` | `ResponseStop` | |
| `error` | `StreamEvent::Error` (Art laut [Fehlertabelle](#fehler), ohne Status) | Der Decoder hört auf. |
| `ping`, unbekannte Events | `StreamEvent::Extra { event, data }` | Nur im selben Dialekt. Daten, die kein JSON sind, bleiben als Text erhalten. `id`, `retry` und andere SSE-Felder des Frames bleiben im Extra. |
| ein Frame ohne `data:`-Zeile (etwa `retry: 3000` oder `event: ping` allein), gleich mit welchem Event-Namen | `StreamEvent::Extra` ohne `data`-Schlüssel | Nach der SSE-Spezifikation kein Event, also nie ein Fehler oder ein Lebenszyklus-Event. Wird ohne `data:`-Zeile zurückgeschrieben. |
| SSE-Kommentare | `StreamEvent::Extra { comment }` | |
| ein `event:`-Name, der vom `type` der Daten abweicht; `id:`, `retry:` und andere SSE-Felder | form `event` / `sse` | `sse` ist `{"id", "retry", "other": [[name, value], …]}` und wird am selben Frame zurückgeschrieben. Eine `id` mit NUL und ein `retry`, das nicht aus ASCII-Ziffern besteht, die in einen `u64` passen, sind nach der Spezifikation kein `id` und kein `retry`; sie bleiben bei den anderen Feldern erhalten ([Server-sent Events](index.md#server-sent-events)). |
| Verstöße gegen die Reihenfolge: Block- oder Nachrichten-Events vor `message_start`, ein zweites `message_start`, Lebenszyklus-Events nach `message_stop`, `message_stop` bei offenem Block, ein zweimal gestarteter Blockindex, ein Delta oder Stopp für einen Block, der nie begann oder schon endete, ein bekannter Delta-Typ an der falschen Art von Block (etwa `text_delta` an `tool_use`; jedes Delta an einem unbekannten Block ist erlaubt), ein bekanntes Event, dessen Daten keinen `type` haben | ein abschließendes `StreamEvent::Error` (`ProviderError`); frühere Events bleiben erhalten | `ping`, Kommentare und unbekannte Events dürfen auf `message_stop` folgen. |
| ein Stream, der ohne `message_stop` oder mitten in einem Block endet; ein bekanntes Event, dessen Daten kein JSON sind | ein abschließendes `StreamEvent::Error` (`ProviderError`) | Lücken in den Indizes von Anthropic sind keine Fehler: Blöcke erhalten KPP-Indizes in Startreihenfolge (form `index`). |

Kodieren von Events aus anderen Dialekten:

- **Ein Block nach dem anderen.** Verschachtelte Blöcke werden in Startreihenfolge gepuffert, und `ResponseDelta` oder `ResponseStop` schließt offene Blöcke. Ein OpenAI-Stream, der seinen Textblock bis zum Ende offen hält, liefert seine Tool-Aufrufe daher erst am Ende. Mehrere fremde `ResponseDelta`s werden zu einem `message_delta` zusammengeführt.
- **Ein fehlendes `ResponseStart`** wird zu einem `message_start` mit leerer ID und dem Modell der Anfrage.
- **Verweigerungen** werden zu Textblöcken.
- **Argumente von Tool-Aufrufen** aus anderen Dialekten werden gesammelt und beim Ende des Blocks als ein `input_json_delta` geschrieben, aber nur, wenn sie ein JSON-Objekt bilden; sonst wird keines geschrieben und die Eingabe bleibt `{}`. Das ist verlustbehaftet, wie bei Antworten. Die eigenen Streams von Anthropic werden so wiedergegeben, wie sie ankamen.
- **Reasoning-Blöcke** aus anderen Dialekten werden zu `thinking`-Blöcken ([Reasoning-Blobs](reasoning.md#reasoning-blobs)): Zusammenfassungen und Reasoning-Text als `thinking_delta`s mit einer Leerzeile zwischen den Teilen, opake Deltas gesammelt, und beim Ende des Blocks ein `signature_delta` mit dem Reasoning-Blob des ganzen Blocks.
- **Weggelassen** ([Antworten](cross-format.md#antworten)): fremde Zitat- und unbekannte Deltas, unbekannte Blöcke und `Extra`-Events, wie bei vollständigen Antworten. Während sie eintreffen, schreibt der Encoder dosierte `event: ping`-Frames (`{"type": "ping"}`, so wie Anthropic sie sendet): einen pro fremdem Extra-Event und einen pro 1.024 Byte verborgenem Text ([Keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gibt dem Gateway denselben Frame.
- **Stoppgründe:** `StopReason::Other` wird wie bei Antworten abgebildet.
- **Fehler:** Ein KPP-`Error` wird zu `event: error` mit seinem Body wie unter [Fehler](#fehler), ohne `request_id`: von Anthropic dekodierte Fehler so, wie sie ankamen, andere als `{"type":"error","error":{type,message}}`.

## Fehler

Schema `ErrorResponse`. Dekodierfehler, bei Anfragen wie bei Antworten von Anbietern, nennen das Feld und was daran falsch ist, nie eine Position des Parsers.

| HTTP | `error.type` | `ErrorKind` (dekodieren) | Kodieren aus `ErrorKind` |
|---|---|---|---|
| 400 | `invalid_request_error` | `InvalidRequest`; `ContextTooLong`, wenn die Meldung mit `prompt is too long` beginnt | `InvalidRequest`, `ContextTooLong`, `ContentFiltered`, `Unsupported` |
| 401 | `authentication_error` | `Authentication` | `Authentication` |
| 402 | `billing_error` | `Billing` | `Billing` |
| 403 | `permission_error` | `PermissionDenied` | `PermissionDenied` |
| 404 | `not_found_error` | `NotFound` | `NotFound` |
| 409 | `conflict_error` | `InvalidRequest` (Status 409 bleibt erhalten) | keine |
| 413 | `request_too_large` | `RequestTooLarge` | `RequestTooLarge` |
| 429 | `rate_limit_error` | `RateLimited` | `RateLimited` |
| 500 | `api_error` | `ProviderError` | `ProviderError` (ein 5xx-Status bleibt erhalten), unbekannte Arten |
| 504 | `timeout_error` | `Timeout` | `Timeout` |
| 529 | `overloaded_error` | `Overloaded` | `Overloaded` |

- `error.message` → `message`; der ganze Body (mit `request_id` und `error.details`) → `raw`; der HTTP-Status → `status`; `retry-after-ms` oder `retry-after` → `retry_after`, das als `retry-after` zurückgeschrieben wird, auf ganze Sekunden aufgerundet.
- Ein zu langer Prompt ist ein 400 `invalid_request_error` wie jeder andere, nur die Meldung unterscheidet ihn. Ändert Anthropic den Wortlaut, wird der Fehler als `InvalidRequest` dekodiert.
- Ein unbekannter `error.type` oder ein Body, der keine `ErrorResponse` ist (etwa das HTML-413 von Cloudflare, JSON, dessen `type` nicht `error` ist, oder eines mit einem Schlüssel auf oberster Ebene, den Kaeso nicht tragen kann), erhält seine Art aus dem Status: 400, 409 und 422 `InvalidRequest`, 401 `Authentication`, 402 `Billing`, 403 `PermissionDenied`, 404 `NotFound`, 408 und 504 `Timeout`, 413 `RequestTooLarge`, 429 `RateLimited`, 503 und 529 `Overloaded` und `ProviderError` für jeden anderen Status und für Stream-Fehler, die keinen haben. Ein dokumentierter `error.type` hat immer Vorrang vor dem Status. Status und roher Body (als String, wenn er kein JSON ist) bleiben erhalten, und solche Bodys werden wieder in der Fehlerform von Anthropic kodiert.
- Von Anthropic dekodierte Fehler werden aus `raw` mit ihrem Status wieder kodiert, sodass Anthropic-Apps sie unverändert sehen; andere werden als `{"type":"error","error":{"type","message"},"request_id":null}` gebildet.
- `request_id` ist in `ErrorResponse` Pflicht, ein String oder `null`. `encode_error_for_request` (`dispatch::encode_error_for_request`) schreibt dort die übergebene ID hinein, auch in einen aus `raw` wieder kodierten Body, wo sie die ID von Anthropic ersetzt; das Gateway übergibt die ID, die es in `kaeso-request-id` sendet, und behält den Header `request-id` von Anthropic in seinem Anfrageprotokoll. `encode_error` schreibt `null` in die Bodys, die es bildet.
- Andere Antwort-Header (etwa `anthropic-ratelimit-*` und `request-id`) gehören nicht zu `KppError`.

## Modelle

`GET /v1/models` und `GET /v1/models/{model_id}`. Code: `anthropic_messages/models.rs`; Fixtures: `crates/kaeso-codecs/fixtures/anthropic_messages/models/`. Quellen: die Seiten [list models](https://platform.claude.com/docs/en/api/models/list.md) und [get a model](https://platform.claude.com/docs/en/api/models/retrieve.md), die SDK-Dateien `types/model_info.py`, `model_list_params.py`, `model_capabilities.py` und `model_line.py` sowie `ModelInfo` und `ListResponse_ModelInfo_` der Spezifikation. Die Formen der Spezifikation haben weder `lifecycle`, `deprecated_at`, `retires_at` noch `line` und keinen Query-Parameter `lifecycle`; Dokumentation und SDK haben Vorrang, und jedes Feld, das der Encoder sonst mit einem Standardwert füllen würde, wird als optional dekodiert, wobei sein Fehlen festgehalten wird (form `none`), sodass beide Formen den Round Trip überstehen. Die Anfrage einer App gilt als Anthropic, wenn sie `anthropic-version` trägt (`dispatch::models_dialect`, siehe [Modelllisten](index.md#modelllisten)).

### Query-Parameter

`decode_models_query` macht aus dem Query-String ein `ModelsQuery`.

| Parameter | KPP | Hinweise |
|---|---|---|
| `limit` | `limit` | Nur Ziffern (ohne Vorzeichen), 1 bis 1000, einmal; alles andere → `InvalidRequest` (Parameter `limit`). Fehlt er, übergibt das Gateway den dokumentierten Standard von 20 an `ModelList::page`. |
| `after_id`, `before_id` | `after_id`, `before_id` | Nicht leer, jeweils einmal; beide zusammen → `InvalidRequest` (Parameter `before_id`). Eine ID, die nicht in der gefilterten, deduplizierten Liste steht → `InvalidRequest` von `ModelList::page` (Parameter `after_id` oder `before_id`), statt einer leeren Seite, die das Blättern einer App stillschweigend beenden würde; ein Cursor, den `lifecycle` herausfiltert, gilt als unbekannt. |
| `lifecycle[]` (was das SDK sendet: `Querystring(array_format="brackets")`), `lifecycle` (wiederholt) | `lifecycle` | `active`, `deprecated`, `retired`; bis zu 3 Werte (`maxItems: 3`). Jeder andere Wert, auch ein durch Kommas getrenntes `active,deprecated` → `InvalidRequest` (Parameter `lifecycle`). Eine leere Liste → der Standard von `ModelList::page` (`active` und `deprecated`). |
| jeder andere Parameter (etwa `beta=true` von `client.beta.models`) | Extras `query.<name>` | Ein String oder, wenn wiederholt, ein Array von Strings. |

Namen und Werte werden prozentdekodiert (`+` ist ein Leerzeichen); ein fehlerhaftes Escape oder ein Ergebnis, das kein UTF-8 ist → `InvalidRequest`. Das Pfadsegment `{model_id}` kommt ebenfalls prozentkodiert an; der Router des Gateways dekodiert es, weil die Codecs nie Pfade sehen. Header (`anthropic-beta`, `anthropic-workspace-id`) gehören nicht zur Query.

### Modellinfo

`ModelInfo` auf dem Wire und `kaeso_kpp::ModelInfo` in KPP.

| Wire-Feld | KPP | Kodiert, wenn KPP keinen Wert hat |
|---|---|---|
| `id` | `id` | (Pflicht) |
| `type` (`"model"`) | form `type`, nur wenn er abweicht | `"model"` |
| `display_name` | `display_name` | die `id` |
| `created_at` (RFC 3339) | `created_at` (Unix-Sekunden) | `"1970-01-01T00:00:00Z"`: Laut Dokumentation kann `created_at` auf einen Epoch-Wert gesetzt sein, wenn das Veröffentlichungsdatum unbekannt ist |
| `lifecycle` | `lifecycle` (`Other(s)` für undokumentierte Werte) | `"active"` (der dokumentierte Standard) |
| `deprecated_at`, `retires_at` (RFC 3339 oder `null`) | `deprecated_at`, `retires_at` | `null` |
| `max_input_tokens`, `max_tokens` (oder `null`) | `context_window`, `max_output_tokens` | `null` |
| `capabilities` (`ModelCapabilities` oder `null`) | Extras-Feld `capabilities` | `null` |
| `line` (`ModelLine` oder `null`) | Extras-Feld `line` | `null` |
| unbekannte Schlüssel | Extras-Felder | |

`capabilities` und `line` sind Extras, weil `ModelCapabilities` ein großer, schnell wachsender Baum von Funktionsnamen bei Anthropic ist (Batches, Zitate, Aufwandsstufen, Thinking-Typen, Server-Tools, Strategien für das Kontextmanagement) ohne Gegenstück in einem anderen Dialekt, und `line` eine Produktfamilie von Anthropic ist (`opus`, `sonnet`, …), die laut Dokumentation nicht aus der `id` abgeleitet werden soll. KPP würde sie nur für Anthropic-Apps durchreichen, und das tun Extras bereits Byte für Byte.

**Datumswerte.** Ein `date-time` nach RFC 3339 wird zu Unix-Sekunden (Offsets angewendet, Sekundenbruchteile abgeschnitten, `T` und `Z` in beliebiger Schreibung) und als `YYYY-MM-DDTHH:MM:SSZ` zurückgeschrieben. Eine empfangene Schreibweise, die so nicht wiederhergestellt würde (`.117Z`, `-07:00`, `t`), wird in form `spelling` aufbewahrt und wieder geschrieben, solange sie noch dieselbe Sekunde bedeutet. Datumswerte, die sich nicht parsen lassen, und Datumswerte vor 1970 → `ProviderError`. Die Sekunde `:60` wird in jeder Minute akzeptiert; sie zählt als erste Sekunde der nächsten Minute, und ihre Schreibweise bleibt erhalten. Eine KPP-Zeit nach `9999-12-31T23:59:59Z` lässt sich in RFC 3339 mit seinen vierstelligen Jahren nicht schreiben; das passiert bei einem OpenAI-kompatiblen `created` in Millisekunden. `created_at` erhält dann die Epoche, den dokumentierten Wert für unbekannt, und `deprecated_at` und `retires_at` erhalten `null`.

**Fehlend oder `null`.** Ein Feld, das fehlend ankam oder als `null`, wo der Standard oben ein Wert ist, wird in form `none` festgehalten und genauso geschrieben, sodass die ältere Form der Spezifikation ohne `lifecycle` den Round Trip übersteht. `capabilities` und `line`, die bei einem Anthropic-Modell fehlen, bleiben weg.

### Modelllisten

`ListResponse[ModelInfo]` auf dem Wire und `ModelList` in KPP.

| Wire-Feld | KPP | Kodiert |
|---|---|---|
| `data` | `models` | |
| `has_more` | `has_more` | `false`, wenn nicht gesetzt (fehlend oder `null` wird in form `none` festgehalten) |
| `first_id`, `last_id` | aus `models` abgeleitet | Die erste und letzte Modell-ID, `null` bei einer leeren Seite. Ein empfangener Wert, der vom abgeleiteten abweicht (das Beispiel der Dokumentation sendet die Platzhalter `"first_id"` und `"last_id"`), wird zusammen mit der ID, für die er empfangen wurde, in form `first_id` oder `last_id` aufbewahrt und nur wieder geschrieben, solange die Seite noch mit diesem Modell beginnt oder endet. |
| unbekannte Schlüssel | Extras-Felder | |

Verlustbehaftet gegenüber Anthropic-Apps: `owned_by` (OpenAI) hat hier kein Feld, und Extras aus anderen Dialekten werden weggelassen ([Antworten](cross-format.md#antworten)). Ein Modell aus einem anderen Dialekt erhält die Standardwerte oben (`display_name` ist die ID, `lifecycle: "active"`, `capabilities: null`, `line`, `max_*` und Datumswerte `null`).

Auf der Egress-Seite baut `encode_model_request` `GET /v1/models/{model_id}` mit `anthropic-version: 2023-06-01` und ohne Body, was auch einen Alias wie `claude-sonnet-4-5` auflöst (die Seite [get a model](https://platform.claude.com/docs/en/api/models/retrieve.md)), `encode_models_request` baut `GET /v1/models` mit demselben Header und `limit`, `after_id`, `before_id` und `lifecycle[]` der Query, jeder Wert prozentkodiert (`Unsupported` für jeden anderen Parameter), und `decode_models` und `decode_model` nehmen die Antwort eines Anbieters entgegen; ein Status außer 2xx geht an `decode_error`. Dekodierte Extras tragen die Herkunft `anthropic_messages` ohne Anbieter, weil diese Funktionen keinen `EgressCtx` erhalten.

## Round Trips

Es gibt keine Ausnahmen: Jede Fixture für Anfragen, Zählanfragen, Antworten, Zählantworten, Modelllisten, Streams und Fehler übersteht den Round Trip (`tests/`, `models_tests.rs`), und jede Stream-Fixture ergibt nach Dekodieren und Kodieren aufsummiert auch ihre zugehörige Antwort.
