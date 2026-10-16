---
title: OpenAI Chat Completions
description: Wie jedes Feld des Formats OpenAI Chat Completions auf KPP abgebildet wird und zurück.
order: 5
---

OpenAI Chat Completions, `POST /v1/chat/completions`, einschließlich Streaming. Codec: `kaeso_codecs::openai_chat::OpenAiChat`.

Quellen: die OpenAPI-Spezifikation von OpenAI im Commit `e95c0fe615f45a19e8878af923a729948445a6bd` (die Schemanamen unten), die [Referenz zum Erstellen](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create), die [Referenz der Streaming-Events](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events) und der [Leitfaden zu Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes).

Die Tabellen verwenden diese Begriffe:

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields` des genannten Elements, gebunden an `openai_chat`. Sie bleiben in diesem Dialekt; ein anderer Dialekt lehnt sie mit `Unsupported` ab ([Extras](cross-format.md#extras)). Verschachtelte Schlüssel verwenden Schlüsselpfade (`image_url.detail`).
- **Form**: eine Codec-eigene Markierung in `Extras::form`. Sie betrifft nur die Darstellung, und andere Dialekte ignorieren sie. `verbatim`-Markierungen bewahren die Schreibweisen `null` und `[]` deklarierter Schlüssel sowie nicht deklarierter Schlüssel mit dem Wert `null`.
- **Unsupported**: Das Dekodieren schlägt mit `ErrorKind::Unsupported { feature }` fehl.

Typisierte Zahlenfelder laufen über `u32`, `u64` oder `f64`. Ein Wert außerhalb ihres Bereichs lässt das Dekodieren fehlschlagen, als `InvalidRequest` in einer Anfrage und als `ProviderError` in einer Antwort, statt verändert zu werden. Unbekannte Felder bleiben Byte für Byte erhalten.

## Anfrage: `CreateChatCompletionRequest`

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `model` | `GenerateRequest.model` typisiert | |
| `messages` | `system` + `messages` typisiert | Siehe [Nachrichten](#nachrichten-chatcompletionrequestmessage). |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typisiert | |
| `max_completion_tokens` | `sampling.max_output_tokens` typisiert | Die Schreibweise, die für Anfragen aus jedem Quelldialekt geschrieben wird. |
| `max_tokens` (veraltet) | `sampling.max_output_tokens` typisiert | Form `max_tokens: "max_tokens"`. Beide mit demselben Wert vorhanden: Form `"both"`. Mit verschiedenen Werten gewinnt `max_completion_tokens`, und `max_tokens` wird zum Extra `max_tokens (differs from max_completion_tokens)`, das als `max_tokens` zurückgesendet wird; der Schlüssel benennt den Konflikt, wenn ein anderer Dialekt ihn ablehnt. |
| `stop` | `sampling.stop` typisiert | Schreibweise als String: Form `stop: "string"`; `[]`: Form `"array"`. Mehr als vier Sequenzen aus einem anderen Dialekt: `Unsupported` (`stop`). |
| `seed` | `sampling.seed` typisiert | |
| `frequency_penalty`, `presence_penalty` | `sampling.*` typisiert | |
| `n` | Form `n` bei `1` | `n > 1`: `Unsupported` (`n`, siehe [Sampling](cross-format.md#sampling)); `0`: `InvalidRequest`. |
| `reasoning_effort` | `reasoning.effort` typisiert | Aus anderen Dialekten ([Aufwand und Budget](cross-format.md#aufwand-und-budget)): eine ausdrückliche Aufwandsstufe unverändert; Anthropic `thinking.disabled` wird zu `none`; ein Budget wird zu `low` (unter 4.096), `medium` (4.096 bis 16.383) oder `high` (16.384 und mehr). Werte `none` bis `max`; unbekannte Werte bleiben als `ReasoningEffort::Other` erhalten und gehen für Chat-Apps unverändert weiter, während `check()` für Anfragen aus anderen Dialekten Werte außerhalb des Enums der Spezifikation ablehnt (`reasoning.effort`). |
| `response_format` `{type: text}` | `output = Text` typisiert | Form `response_format_text`; unbekannte Schlüssel sind Extras `response_format.<key>`. |
| `response_format` `{type: json_object}` | `output = JsonObject` typisiert | |
| `response_format` `{type: json_schema, json_schema: {name, description, schema, strict}}` | `output = JsonSchema` typisiert | Unbekannte Schlüssel sind Extras (`json_schema.<key>` für verschachtelte). Fehlendes `schema`: `{}` + Form `schema_absent`. Ein Schema ohne Namen aus einem anderen Dialekt wird als `name: "response"` gesendet ([strukturierte Ausgabe](cross-format.md#strukturierte-ausgabe)). |
| `response_format` andere Typen | Extras `response_format` | Nur im selben Dialekt. |
| `tools[]` `{type: function, function: FunctionObject}` | `ToolDef::Function` typisiert (`name`, `description`, `parameters` roh, `strict`) | Unbekannte Schlüssel sind Extras (verschachtelt `function.<key>`). Namen außerhalb von `[a-zA-Z0-9_-]{1,64}` aus anderen Dialekten werden deterministisch umgeschrieben (`EgressCtx::tool_names`). |
| `tools[]` andere Typen (`custom`) | `ToolDef::Unknown` | Nur im selben Dialekt. |
| `tool_choice` `"auto"`, `"none"`, `"required"` | `ToolChoice::Auto`, `None`, `Required` typisiert | |
| `tool_choice` `{type: function, function: {name}}` | `ToolChoice::Tool(name)` typisiert | Mit unbekannten Schlüsseln: `ToolChoice::Unknown`. |
| `tool_choice` `allowed_tools`, `custom`, andere Strings | `ToolChoice::Unknown` | Nur im selben Dialekt. |
| `parallel_tool_calls` | `parallel_tool_calls` typisiert | |
| `safety_identifier` | `user` typisiert | KPP `user` ist die ID des Endbenutzers für die Missbrauchserkennung, und genau das ist `safety_identifier`. |
| `user` (veraltet) | `user` typisiert, wenn `safety_identifier` fehlt (Form `user`); sonst das Extra `user (sent alongside safety_identifier)`, das als `user` zurückgesendet wird | KPP `user` aus anderen Dialekten wird als `safety_identifier` gesendet. |
| `stream` | `stream` typisiert | `false`: Form `stream_false`. |
| `stream_options.include_usage` | `StreamMode::On { include_usage }` typisiert | `stream_options` fehlt: Form `stream_options: "absent"`; ohne `include_usage`: Form `"no_include_usage"`; andere Schlüssel (`include_obfuscation`) sind Extras `stream_options.<key>`; bei ausgeschaltetem `stream`: das ganze Objekt als Form `stream_options_off`. Anfragen aus anderen Dialekten verlangen immer `include_usage: true`. Das Gateway braucht die Nutzung, setzt deshalb bei jeder Streaming-Anfrage `include_usage: true` und behält die ursprüngliche Anfrage der App in `EncodeCtx`; der Stream-Encoder der App blendet den Nutzungs-Chunk dann für Apps aus, die nicht danach gefragt haben. |
| `audio`, `functions` und `function_call` (veraltet, oberste Ebene), `logit_bias`, `logprobs`, `metadata`, `modalities`, `moderation`, `prediction`, `prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `service_tier`, `store`, `top_logprobs`, `verbosity`, `web_search_options` | Extras | Keine Entsprechung in KPP; über Dialektgrenzen abgelehnt ([Extras](cross-format.md#extras), [Sampling](cross-format.md#sampling)). `null`-Werte sind Form. |
| unbekannte Schlüssel | Extras | |
| Header `openai-beta` | Extras `header:openai-beta` | Wird nur an Chat weitergeleitet. |

### Nachrichten (`ChatCompletionRequestMessage`)

| Wire | KPP | Hinweise |
|---|---|---|
| führende `system`- und `developer`-Nachrichten | Blöcke in `GenerateRequest.system` | Der erste Block jeder Nachricht trägt die Form `message: {role, content}`, die Nachrichtengrenzen, die Rolle und die Schreibweise als String oder Array bewahrt; `name` und unbekannte Schlüssel der Nachricht sind Extras `message.<key>` an diesem Block. Eine führende Nachricht, deren erster Teil unbekannt ist, bleibt eine Nachricht mit `Role::System` und beendet die führende Folge. Aus anderen Dialekten: eine `system`-Nachricht. |
| spätere `system`- und `developer`-Nachrichten | `Message { role: System }` ([Systemnachrichten](cross-format.md#systemnachrichten)) | `developer`: Form `role: "developer"`. |
| `user` | `Message { role: User }` | `name` und unbekannte Schlüssel: Extras der Nachricht. |
| `assistant` | `Message { role: Assistant }`: Inhaltsteile, dann `refusal`, dann `tool_calls` | `content` fehlt: Form `content: "absent"`; `null`: verbatim; `name`, `audio`: Extras. `annotations` steht nicht im Anfrageschema, aber die SDKs senden es mit jeder Antwort zurück: `[]` ist verbatim (Form), damit die Unterhaltung zu einem anderen Anbieter wechseln kann; ein nicht leerer Wert wird zu `TextBlock.citations` des ersten Textblocks, gebunden an Chat (ohne Text: Extras `annotations`), was andere Dialekte ablehnen (`citations`) und Chat als `annotations` zurücksendet. Aus anderen Dialekten gehen Text- und Verweigerungsblöcke in `content` und Tool-Aufrufe in `tool_calls`, weil Chat sie nicht verschränken kann; Reasoning wird verworfen ([Reasoning](cross-format.md#reasoning)); ein Zug, in dem nichts bleibt, was Chat zeigen kann, etwa nur Reasoning, wird als `content: ""` gesendet, damit die Unterhaltung ihre Züge behält und die Regel von Chat erfüllt ist, dass eine Assistentennachricht ohne `tool_calls` Inhalt braucht. |
| Assistent-`refusal` | `Block::Refusal` | |
| Assistent-`function_call` (veraltet) | `Unsupported` (`function_call`) | `null` bleibt verbatim erhalten. |
| `tool`-Nachrichten | `Block::ToolResult` in einer Nachricht mit `Role::User`; aufeinanderfolgende Tool-Nachrichten teilen sich eine KPP-Nachricht | `tool_call_id` wird zu `call_id`; Inhaltsteile werden zu verschachtelten Textblöcken; unbekannte Schlüssel sind Extras des Tool-Ergebnisses. `is_error` aus anderen Dialekten wird verworfen, weil Chat Fehler im Inhalt meldet ([Tool-Fehler](cross-format.md#tool-fehler)). Aus anderen Dialekten werden die Tool-Ergebnisse einer Benutzernachricht zu `tool`-Nachrichten, gefolgt von einer `user`-Nachricht mit den übrigen Blöcken. |
| `function`-Nachrichten (veraltet) | `Unsupported` (`function role message`) | |
| `content` als String oder Array | Form `content: "array"` (Nachricht oder Tool-Ergebnis) | Ein String ist der Standard für einen einzelnen reinen Textblock. Leere Arrays sind ungültig (`minItems: 1`). |

Das alte Function Calling hat keine IDs für Tool-Aufrufe, deshalb können `function`-Nachrichten und ein Assistent-`function_call`, der nicht `null` ist, keine KPP-Tool-Aufrufe und -Ergebnisse werden. Die Parameter `functions` und `function_call` auf oberster Ebene reisen als Extras innerhalb dieses Dialekts. Die Fixture ist `fixtures/openai_chat/requests/legacy-functions.json`.

### Inhaltsteile

| Wire | KPP | Hinweise |
|---|---|---|
| `text` | `Block::Text` | Unbekannte Schlüssel: Extras des Blocks. |
| `prompt_cache_breakpoint` (Text-, Bild- und Dateiteile) | `cache: CacheHint { ttl: None }`, gebunden an Chat | Beide markieren das Ende eines wiederverwendbaren Präfixes; die TTL kommt aus `prompt_cache_options`. Nur Hinweise aus Chat werden zurückgesendet; Hinweise aus anderen Dialekten werden verworfen ([Cache-Hinweise](cross-format.md#cache-hinweise)). `mode` außer `explicit`: Form `mode`. |
| `image_url.url` (`https:`) | `Block::Image { source: Url }` | |
| `image_url.url` (`data:<type>[;params];base64,<data>`, RFC 2397, Schema und `base64` in beliebiger Schreibung) | `Block::Image { source: Base64 }` | Ein anderer Kopf als `data:<type>;base64`, etwa mit Parametern (`;name=a.png`) oder in Großbuchstaben, bleibt in der Form `data_url` exakt erhalten, auch für `file.file_data`. Data-URLs, die nicht base64-kodiert sind oder keinen Medientyp haben, bleiben `Url`. |
| `image_url.detail` | Extras `image_url.detail` | Steht auf der [Extras-Allowlist](extras-allowlist.md): wird für andere Dialekte stillschweigend verworfen. |
| `file.file_data` (`data:`-URL) | `Block::Document { source: Base64 }` | Reines Base64 gilt als Medientyp `application/pdf`, der einzige Typ im Leitfaden von OpenAI zu Dateieingaben, mit Form `bare_file_data`, und wird wieder ohne Kopf geschrieben. |
| `file.file_id` | `Block::Document { source: FileId }`, gebunden an den Dateispeicher von OpenAI ([Dateien](cross-format.md#dateien)) | Zusammen mit `file_data`: Extras `file.file_id`. |
| `file.filename` | Extras `file.filename` | Kein Titel (siehe die Dokumentation von KPP `DocumentBlock::title`). Steht auf der [Extras-Allowlist](extras-allowlist.md). |
| `input_audio` | `Unsupported` (`input_audio`) | Audioeingaben sind nicht Teil von KPP. |
| `refusal` (Assistent) | `Block::Refusal` + Form `part` | |
| andere Typen | `Block::Unknown` | Nur im selben Dialekt. |

Für KPP aus anderen Dialekten lehnt `check()` ab: Textdokumente, Dokument-URLs, `title`, `context` und `citations` von Dokumenten, Bild-Datei-IDs, Bilder und Dokumente außerhalb von Benutzernachrichten, Verweigerungen in Benutzernachrichten, Zitate außerhalb von Assistentennachrichten (und über `check_foreign` die Zitate anderer Dialekte), `top_k` ([Sampling](cross-format.md#sampling)), unbekannte Reasoning-Modi und Aufwandsstufen außerhalb des Enums der Spezifikation. Budgets und `disabled` werden umgerechnet ([Aufwand und Budget](cross-format.md#aufwand-und-budget)). Diese Hinweise werden stattdessen verworfen: Reasoning-Blöcke ([Reasoning](cross-format.md#reasoning)), Cache-Hinweise anderer Dialekte ([Cache-Hinweise](cross-format.md#cache-hinweise)), `ToolResult::is_error`, dessen Information der Fehlerinhalt trägt ([Tool-Fehler](cross-format.md#tool-fehler)), sowie `reasoning.summary` und `reasoning.include_encrypted` anderer Dialekte, weil Chat kein Reasoning zum Anzeigen liefert ([lesbares Reasoning und Zusammenfassungen](cross-format.md#lesbares-reasoning-und-zusammenfassungen)).

Ein Body, den der Codec nicht lesen kann, ist `InvalidRequest` und nennt das Feld und was es sein muss (`` `max_completion_tokens` must be a non-negative integer ``, mit gesetztem `param`), nie eine Parserposition.

### Tool-Aufrufe (`ChatCompletionMessageToolCall`)

| Wire | KPP | Hinweise |
|---|---|---|
| `id`, `function.name` | `ToolCallBlock.id`, `.name` | Umgeschriebene Namen werden beim Dekodieren zurückübersetzt. |
| `function.arguments` | `ToolCallBlock.arguments` roh | Der JSON-Text im String, Byte für Byte. Text, der kein gültiges JSON ist oder Leerraum drumherum hat, behält sein Literal in der Form `arguments`; KPP enthält dann `{}` für leeren Text und sonst den Text als JSON-String. Nach der Konvention von KPP bedeutet ein JSON-String in `ToolCallBlock.arguments`, dass der Anbieter Text gesendet hat, der kein JSON ist; für Aufrufe aus anderen Dialekten wird deshalb der Inhalt des Strings als `arguments` gesendet, Objekte, Arrays und andere JSON-Werte als ihr JSON-Text. |
| unbekannte Schlüssel | Extras (verschachtelt `function.<key>`) | |
| `type: custom` | `Block::Unknown` | Wird in `tool_calls` zurückgesendet. |

## Antwort: `CreateChatCompletionResponse`

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `id`, `model` | `Response.id`, `.model` | |
| `object` | keins | Immer `chat.completion`; andere Werte: Form `object`. |
| `created`, `service_tier`, `system_fingerprint`, `metadata`, `moderation` | Extras | Eine Antwort aus einem anderen Dialekt bekommt `created` aus `EncodeCtx::created_at`. |
| `choices` | `choices[0]` | Mehr als eine Auswahl: `Unsupported` (`n`); keine: `ProviderError`. |
| `choices[0].logprobs` | Extras `choice.logprobs` | `null`: verbatim. |
| `choices[0].finish_reason` | `Response.stop` | `stop` ⇄ `EndTurn`, `length` ⇄ `MaxTokens`, `tool_calls` ⇄ `ToolUse`, `content_filter` ⇄ `ContentFilter`, andere Werte (etwa `function_call`) ⇄ `Other`. Aus anderen Dialekten: `StopSequence` und `PauseTurn` werden zu `stop`; `Refusal` wird zu `content_filter`, weil die Antwort zurückgehalten wurde. Eine Antwort ohne Streaming und ohne `finish_reason`, das die Spezifikation verlangt: `ProviderError`. |
| `message.content` | `Block::Text` | Aus anderen Dialekten werden alle Textblöcke ohne Trennzeichen verbunden, weil Chat in Antworten keine Inhaltsteile hat. Ohne Trennzeichen deshalb, weil Anthropic eine Antwort rund um Zitate in mehrere Textblöcke teilt und die gestreamten Deltas, die ebenfalls kein Trennzeichen tragen, denselben `content` ergeben müssen. |
| `message.annotations` | `TextBlock.citations` (roh, gebunden an Chat) | Ohne Inhalt: Extras `choice.message.annotations`. Zitate anderer Dialekte werden weggelassen ([Antworten](cross-format.md#antworten)). |
| `message.refusal` | `Block::Refusal` | |
| `message.tool_calls` | `Block::ToolCall` oder `Block::Unknown` | Wie in Anfragen. |
| `message.audio` (`id`, `data`, `expires_at`, `transcript`), `message.function_call`, unbekannte Schlüssel | Extras `choice.message.<key>` | Das Audio-Objekt bleibt mit `transcript` und `expires_at` vollständig erhalten. |
| Reasoning, unbekannte Blöcke anderer Dialekte | keins | Werden in einer Chat-Antwort weggelassen ([Antworten](cross-format.md#antworten)). Das Gateway bewahrt wiederverwendbares Reasoning für die nächste Runde auf, zugeordnet den Tool-Aufrufen, vor denen es stand ([vom Gateway aufbewahrtes Reasoning](cross-format.md#vom-gateway-aufbewahrtes-reasoning), `kaeso_codecs::continuity`). |

### Nutzung (`CompletionUsage`)

Die KPP-Felder folgen der Konvention unter [Nutzung](cross-format.md#nutzung).

| Wire | KPP |
|---|---|
| `prompt_tokens − cached_tokens − cache_write_tokens` | `input_tokens` |
| `prompt_tokens_details.cached_tokens` | `cache_read_tokens` |
| `prompt_tokens_details.cache_write_tokens` | `cache_write_tokens` |
| `completion_tokens` | `output_tokens` (einschließlich Reasoning) |
| `completion_tokens_details.reasoning_tokens` | `reasoning_tokens` |
| `total_tokens` | abgeleitet (`prompt_tokens + completion_tokens`); nicht eigens gespeichert, in `raw` erhalten |
| `prompt_tokens_details.audio_tokens`, `text_tokens`, `image_tokens`, `completion_tokens_details.audio_tokens`, `text_tokens`, `accepted_prediction_tokens`, `rejected_prediction_tokens` | nur `raw` (Aufschlüsselungen der typisierten Summen) |
| das ganze Objekt | `raw`, unverändert zurückgesendet, solange die typisierten Felder noch dazu passen; sonst gewinnen die typisierten Felder, und das rohe Objekt ergänzt die übrigen Schlüssel |

Aus anderen Dialekten: `prompt_tokens = input + cache_read + cache_write`, `prompt_tokens_details.cached_tokens = cache_read`, dazu `cache_write_tokens`, wenn es nicht null ist.

## Streams: `CreateChatCompletionStreamResponse`

Einfache `data:`-Frames mit je einem JSON-Chunk, am Ende `data: [DONE]`.

| Wire | KPP-Event | Hinweise |
|---|---|---|
| erster Chunk | `ResponseStart { id, model }` | `created`, `service_tier`, `system_fingerprint`: Extras; `object` außer `chat.completion.chunk`: Form `object`. |
| `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint` in späteren Chunks | keins | Der Umschlag wiederholt sich; Abweichungen in `created`, `service_tier` und `system_fingerprint` bleiben in `chunk.env` und `chunk.env_absent` erhalten. `id`, `object` und `model` kommen immer aus `ResponseStart`. |
| `choices[0].delta.role` | Form `chunk.role` | Aus anderen Dialekten beginnt der Stream, sobald die Antwort startet, mit einem Rollen-Chunk, `delta: {"role": "assistant", "content": "", "refusal": null}`, dem ersten Chunk, den OpenAI selbst bei einer Textantwort sendet; so hört die App vom Stream, während das Modell nachdenkt ([Keepalives](cross-format.md#keepalives)). |
| `choices[0].delta.content` | einmal `BlockStart(Text)`, dann `BlockDelta(Text)` | Jedes Stück, auch `""`. |
| `choices[0].delta.refusal` | einmal `BlockStart(Refusal)`, dann `BlockDelta(Refusal)` | |
| `choices[0].delta.tool_calls[]` mit neuem `index` | `BlockStart(ToolCall { id, name })` + `BlockDelta(ToolArguments)` für `arguments` | KPP-Blockindizes folgen der Reihenfolge, in der Blöcke beginnen; der Chat-`index` wird auf den KPP-Index abgebildet. Umgeschriebene Namen werden zurückübersetzt. Abweichungen in der Gestalt (fehlendes `id`, `type`, `function`, `name` oder `arguments`, ein unerwarteter `index`): Form `tc`. Ein anderer `type` als `function`: `BlockStart(Unknown)`. |
| `choices[0].delta.tool_calls[]` mit bekanntem `index` | `BlockDelta(ToolArguments)` | Ein wiederholtes `id`, `type` oder `name`: Form `tc`. |
| `choices[0].finish_reason` | `BlockStop` für jeden offenen Block, dann `ResponseDelta { stop }` | Abbildung wie bei Antworten. |
| `usage` (Chunk mit `choices: []`) | `ResponseDelta { usage }` | Nutzung wie unter [Nutzung](#nutzung-completionusage), roh erhalten. |
| `usage: null`, `logprobs: null`, `finish_reason: null`, `content: null`, `refusal: null` | Form `chunk.kv`, `cv`, `dv` | |
| `choices[0].logprobs`, unbekannte Schlüssel der Auswahl | Extras `choice.<key>` am ersten Event des Chunks | |
| `delta.audio`, `delta.function_call`, unbekannte Schlüssel im Delta | Extras `delta.<key>` | Ein Chunk ganz ohne typisierten Inhalt ist ein `Extra` (`{"event": null, "data": <chunk>}`). |
| `obfuscation` | Form `chunk.obf` | Wird nicht an Apps gesendet, wenn die Events aus einem anderen Dialekt kamen. |
| `moderation`, unbekannte Schlüssel im Chunk | Extras am ersten Event des Chunks | |
| `data: [DONE]` | `ResponseStop` (nach den `BlockStop`s noch offener Blöcke) | Ein Stream, der nach `finish_reason` ohne `[DONE]` endet: `ResponseStop` mit Form `no_done`; einer, der früher endet: `Error`. |
| `data: {"error": {…}}` | `Error`, dekodiert wie ein Fehler-Body ohne Status | Der Decoder hört auf. Wird als derselbe Frame kodiert; kein `[DONE]` folgt. |
| `event: error`-Frames, die manche OpenAI-kompatiblen Anbieter senden | `Error` | Dekodiert wie ein Fehler-Body. |
| SSE-Kommentare, andere benannte Events, Frames nach `[DONE]` | `Extra` (`{"comment"}` oder `{"event", "data", "id", "retry", "fields"}`) | Nur im selben Dialekt. |
| mehrere Auswahlen | `Error` mit `Unsupported` (`n`) | |

Gruppierung in Chunks: Das erste Event jedes Chunks trägt die Form-Markierung `chunk` (`n` ist die Zahl der Events aus diesem Chunk, dazu die Gestalt-Markierungen oben), die anderen tragen `in_chunk`, damit der Encoder sie wieder als einen Frame schreibt. Ein offener Chunk wird geschrieben, sobald ein Event ankommt, das nicht dazugehört (ein Event eines anderen Dialekts, `[DONE]`, ein Fehler), sodass ein verworfenes Event nie einen Chunk hinter `[DONE]` schiebt; nach `[DONE]` oder einem Fehler wird nichts mehr geschrieben. Ein Gateway, das `id` oder `model` in `ResponseStart` umschreibt, schreibt jeden Chunk um. Tool-Argumente für einen Block, den Chat nicht zeigen kann, werden weggelassen.

Ein Daten-Chunk mit SSE-Zeilen `id` oder `retry`, die OpenAI nicht sendet, wird wie jeder andere Chunk dekodiert, und die Zeilen werden an Chat-Apps zurückgeschrieben. Benannte Events, unbekannte SSE-Felder und Frames nach `[DONE]` bleiben vollständig erhalten und werden nur an Chat-Apps wiedergegeben.

OpenAI sendet `id`, `type` und `function.name` eines Tool-Aufrufs in dessen erstem Chunk, das Schema erlaubt sie aber auch in einem späteren. Streams, die in diesem Format bleiben, geben sie exakt wieder. Ein Aufruf, der ohne sie beginnt, erreicht Anthropic- und Responses-Apps mit leerer ID und leerem Namen, weil diese Formate beides beim Start des Blocks ankündigen.

Events aus anderen Dialekten werden zu je einem Chunk, nach dem eröffnenden Rollen-Chunk, mit `logprobs: null` und `finish_reason`. Tool-Indizes werden in der Reihenfolge vergeben, in der Tool-Aufrufe beginnen, und Reasoning, Zitate, opake Daten, unbekannte Blöcke und Extras anderer Dialekte werden weggelassen ([Antworten](cross-format.md#antworten)). Während sie ankommen, schreibt der Encoder dosierte SSE-Kommentare (`: keepalive`), die die SSE-Spezifikation und die Parser der SDKs ignorieren: einen für jedes Extra-Event eines anderen Dialekts, etwa Anthropic `ping`, und einen je 1.024 Bytes verborgenen Texts ([Keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gibt dem Gateway denselben Frame für Stille beim Anbieter. Nutzung wird nur geschrieben, wenn die App danach gefragt hat (`EncodeCtx::stream` mit `include_usage`): Jeder Chunk hat dann `usage: null`, und ein letzter Chunk mit `choices: []` trägt die Nutzung. Aus anderen Dialekten ist das die zuletzt gemeldete Nutzung, mit den Eingabezahlen aus `ResponseStart`, wenn die spätere Meldung keine hat.

## Fehler (`ErrorResponse`)

Der Fehler-Body ist `{error: {message, type, param, code}}`.

| Signal | `ErrorKind` | Kodiert für KPP-Fehler von anderswo |
|---|---|---|
| `code: context_length_exceeded` | `ContextTooLong` | 400 `invalid_request_error`, Code `context_length_exceeded` |
| `type` oder `code` `insufficient_quota`, Abrechnungscodes | `Billing` | 429 `insufficient_quota`, Code `insufficient_quota` |
| `code` `server_is_overloaded` | `Overloaded` | siehe 503 |
| `code` `content_policy_violation` oder `content_filter` | `ContentFiltered` | 400 `invalid_request_error`, Code `content_policy_violation` |
| 400, 409, 422 | `InvalidRequest` (und `Unsupported` beim Kodieren) | 400 `invalid_request_error` |
| 401 | `Authentication` | 401 `invalid_request_error` |
| 403 | `PermissionDenied` | 403 `request_forbidden` |
| 404 | `NotFound` | 404 `invalid_request_error` |
| 413 | `RequestTooLarge` | 413 `invalid_request_error` |
| 429 | `RateLimited` | 429 `rate_limit_error`, Code `rate_limit_exceeded` |
| 503 | `Overloaded` | 503 `service_unavailable_error`, Code `server_is_overloaded` |
| 408, 504 | `Timeout` | 504 `server_error`, Code `timeout` |
| andere | `ProviderError` | sein 5xx-Status (außer 503) oder 500, `server_error` |
| `error.misalignment` (`MisalignmentErrorDetailsResource` im Schema `Error`), unbekannte Schlüssel in `error` | in `KppError.raw` erhalten (der ganze Body) | Wird nur erneut gesendet, wenn der Fehler aus Chat kam. |

Die Codecs für Responses und Embeddings verwenden dieselbe Code-Tabelle (`openai_responses::error`), sodass ein Fehler in jeder OpenAI-API dieselbe Art, denselben `type` und denselben `code` hat. Die OpenAI-SDKs wählen ihre Exception-Klasse nach dem Status, `type` ist also nur eine zusätzliche Information.

`retry-after-ms` oder `retry-after` wird zu `retry_after`, das als `retry-after` in ganzen Sekunden geschrieben wird, aufgerundet, dazu `retry-after-ms`, aufgerundet, wenn die Wartezeit einen Sekundenbruchteil hat. `param` wird zu `KppError.param`. Ein aus Chat dekodierter Fehler wird mit seinem ursprünglichen Body und Status wieder kodiert. Bodies, die keine `ErrorResponse` sind, etwa ein HTML-429 von einem Proxy, ein 503 als reiner Text oder `{"error": "rate limited"}`, bekommen ihre Art vom Status (Tabelle oben) und behalten den rohen Body; ein `error` als String wird zur Meldung.

## Modelle

`GET /v1/models` und `GET /v1/models/{model}` sind in `crates/kaeso-codecs/src/openai_models.rs` umgesetzt, das die Codecs für Chat, Responses und Embeddings gemeinsam nutzen, weil alle OpenAI-APIs Modelle in derselben Gestalt auflisten. Die Fixtures liegen in `fixtures/openai_chat/models/`, und alle drei Codecs prüfen an ihnen den Round Trip. Quellen: die Schemas `Model` und `ListModelsResponse` der Spezifikation und die Operationen `listModels` und `retrieveModel`, deren `x-oaiMeta`-Beispiele als Fixtures dienen. Die Anfrage einer App ist OpenAI, wenn sie keinen Header `anthropic-version` trägt (`dispatch::models_dialect`, siehe [Modelllisten](index.md#modelllisten)); das Gateway kodiert dann mit `openai_chat`.

### Query-Parameter

`listModels` dokumentiert keine. Jeder Parameter wird prozentdekodiert als Extra `query.<name>` des dekodierenden Dialekts aufbewahrt: als String oder, wenn er sich wiederholt, als Array. Fehlerhafte Escapes sind `InvalidRequest`. OpenAI blättert nicht, deshalb ruft das Gateway `ModelList::page` ohne Standardlimit auf, und eine OpenAI-App bekommt immer die ganze Liste. Das Pfadsegment `{model}` kommt prozentkodiert an, da IDs feinabgestimmter Modelle `:` enthalten, und der Router des Gateways dekodiert es.

### `Model` ⇄ `kaeso_kpp::ModelInfo`

| Wire-Feld | KPP | Kodiert, wenn KPP keinen Wert hat |
|---|---|---|
| `id` | `id` | (erforderlich) |
| `object` (`"model"`) | Form `object`, nur wenn es abweicht | `"model"` |
| `created` (Unix-Sekunden) | `created_at` | `0` |
| `owned_by` | `owned_by` | `"kaeso"`: Der Eigentümer ist unbekannt, wie bei einem von Anthropic gelisteten Modell, und das Modell wird vom Kaeso-Gateway bereitgestellt. |
| `shutdown_date` (`YYYY-MM-DD` oder `null`) | `retires_at` (dieses Datum um 00:00 UTC) | `null` |
| unbekannte Schlüssel | Extras-Felder | |

Das Gateway listet Modelle aus seinem Katalog: Es setzt `owned_by` auf die ID des Anbieters und `created_at` auf das Veröffentlichungsdatum des Modells, wenn der Katalog eines hat.

Ein Feld, das fehlend ankommt oder als `null`, wo der Standard ein Wert ist, wird in der Form `none` vermerkt und genauso geschrieben; `shutdown_date` ist in der Spezifikation optional. Ein `shutdown_date`, das kein `full-date` ist, ergibt `ProviderError`. Extras aus jedem der drei OpenAI-Dialekte werden von allen drei Encodern berücksichtigt.

### `ListModelsResponse` ⇄ `ModelList`

| Wire-Feld | KPP |
|---|---|
| `object` (`"list"`) | Form `object`, nur wenn es abweicht |
| `data` | `models` |
| unbekannte Schlüssel | Extras-Felder |

Für OpenAI-Apps geht dabei etwas verloren: `display_name`, `lifecycle`, `deprecated_at`, `context_window`, `max_output_tokens`, `has_more` und Extras anderer Dialekte (Anthropic `capabilities`, `line`) haben kein Feld und werden weggelassen ([Antworten](cross-format.md#antworten)). Ein `retires_at`, das nicht auf Mitternacht UTC fällt, wird auf sein UTC-Datum gekürzt, und eines nach dem Jahr 9999 wird als `shutdown_date: null` geschrieben. `created` ist eine Ganzzahl und wird so geschrieben, wie es gespeichert ist; ein Wert in Millisekunden bleibt also einer. In `decode_models` und `decode_model` geht ein Status außerhalb von 2xx an `decode_error`.
