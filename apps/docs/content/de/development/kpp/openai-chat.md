---
title: OpenAI Chat Completions
description: Wie jedes Feld des Formats OpenAI Chat Completions auf KPP abgebildet wird und zurück.
order: 5
nav: OpenAI Chat
---

Schlagen Sie hier jedes Feld des Formats OpenAI Chat Completions nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::openai_chat::OpenAiChat` (`crates/kaeso-codecs/src/openai_chat/`)
- **Endpunkte:** `POST /v1/chat/completions` (JSON und SSE) und die [Modell-Endpunkte](#modelle), die alle OpenAI-Codecs teilen
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_chat/`

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `e95c0fe615f45a19e8878af923a729948445a6bd` (die Schemanamen unten), die [Referenz zum Erstellen](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create), die [Referenz der Streaming-Events](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events) und der [Leitfaden zu Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes).

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields` des genannten Elements, gebunden an `openai_chat`. Sie bleiben in diesem Dialekt; ein anderer Dialekt lehnt sie mit `Unsupported` ab ([Extras](cross-format.md#extras)). Verschachtelte Schlüssel verwenden Schlüsselpfade (`image_url.detail`).
- **Form**: eine Codec-eigene Markierung in `Extras::form`. Sie betrifft nur die Darstellung, und andere Dialekte ignorieren sie. `verbatim`-Markierungen bewahren die Schreibweisen `null` und `[]` deklarierter Schlüssel sowie nicht deklarierter Schlüssel mit dem Wert `null`.
- **Unsupported**: Das Dekodieren oder `check()` schlägt mit `ErrorKind::Unsupported { feature }` fehl; das Feature steht in Klammern.

**Zahlen.** Typisierte Zahlenfelder laufen über `u32`, `u64`, `i64` (`seed`) oder `f64`. Ein Wert außerhalb ihres Bereichs lässt das Dekodieren fehlschlagen, als `InvalidRequest` in einer Anfrage und als `ProviderError` in einer Antwort, statt verändert zu werden. Unbekannte Felder bleiben Byte für Byte erhalten, außer Schlüsseln mit Namen, die Kaeso nicht tragen kann ([unten](#schlüssel-die-kaeso-nicht-tragen-kann)).

## Anfrage: `CreateChatCompletionRequest`

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `model` | `GenerateRequest.model` typisiert | |
| `messages` | `system` + `messages` typisiert | Siehe [Nachrichten](#nachrichten-chatcompletionrequestmessage). |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typisiert | Als Ganzzahl geschrieben (`1`): Form `integers`, die Liste der so geschriebenen Schlüssel. Der Encoder schreibt einen solchen Schlüssel als Ganzzahl, solange sein Wert noch ganzzahlig ist, sodass `temperature: 1` als `1` zurückkommt, nicht als `1.0`; ein geänderter Wert wird als der neue Wert geschrieben. Aus anderen Dialekten: die `f64`-Schreibweise (`1.0`). |
| `max_completion_tokens` | `sampling.max_output_tokens` typisiert | Die Schreibweise, die für Anfragen aus jedem Quelldialekt geschrieben wird, außer das Profil des Anbieters verlangt `max_tokens` ([Anbieterprofile](#anbieterprofile)). `CreateChatCompletionRequest` setzt dafür und für `max_tokens` kein Minimum, daher bleibt jede nicht negative Ganzzahl erhalten, auch `0`; eine negative ist `InvalidRequest`. |
| `max_tokens` (veraltet) | `sampling.max_output_tokens` typisiert | Form `max_tokens: "max_tokens"`. Beide mit demselben Wert vorhanden: Form `"both"`. Mit verschiedenen Werten gewinnt `max_completion_tokens`, und `max_tokens` wird zum Extra `max_tokens (differs from max_completion_tokens)`, das als `max_tokens` zurückgesendet wird; der Schlüssel benennt den Konflikt, wenn ein anderer Dialekt ihn ablehnt. |
| `stop` | `sampling.stop` typisiert | Schreibweise als String: Form `stop: "string"`; `[]`: Form `"array"`. Mehr als vier Sequenzen: `Unsupported` (`stop`). |
| `seed` | `sampling.seed` typisiert | Wird als `random_seed` gesendet, wenn das Profil des Anbieters es so festlegt ([Anbieterprofile](#anbieterprofile)). |
| `frequency_penalty`, `presence_penalty` | `sampling.*` typisiert | Ganzzahlige Schreibweise wie bei `temperature` (Form `integers`). |
| `n` | Form `n` bei `1` | `n > 1`: `Unsupported` (`n`, siehe [Sampling](cross-format.md#sampling)); `0`: `InvalidRequest`. |
| `reasoning_effort` | `reasoning.effort` typisiert | Aus anderen Dialekten ([Aufwand und Budget](reasoning.md#aufwand-und-budget)): eine ausdrückliche Aufwandsstufe unverändert; Anthropic `thinking.disabled` wird zu `none`; ein Budget wird zu `low` (unter 4.096), `medium` (4.096 bis 16.383) oder `high` (16.384 und mehr). Werte `none` bis `max`; unbekannte Werte bleiben als `ReasoningEffort::Other` erhalten und gehen für Chat-Apps unverändert weiter, während `check()` für Anfragen aus anderen Dialekten Werte außerhalb des Enums der Spezifikation ablehnt (`reasoning.effort`). |
| `response_format` `{type: text}` | `output = Text` typisiert | Form `response_format_text`; unbekannte Schlüssel sind Extras `response_format.<key>`. |
| `response_format` `{type: json_object}` | `output = JsonObject` typisiert | Unbekannte Schlüssel sind Extras `response_format.<key>`. |
| `response_format` `{type: json_schema, json_schema: {name, description, schema, strict}}` | `output = JsonSchema` typisiert | Unbekannte Schlüssel sind Extras (`json_schema.<key>` für verschachtelte). Fehlendes `schema`: `{}` + Form `schema_absent`; fehlendes `json_schema`-Objekt: `InvalidRequest`. Ein Schema ohne Namen aus einem anderen Dialekt wird als `name: "response"` gesendet ([strukturierte Ausgabe](cross-format.md#strukturierte-ausgabe)). |
| `response_format` andere Typen | Extras `response_format` | Nur im selben Dialekt. |
| `tools[]` `{type: function, function: FunctionObject}` | `ToolDef::Function` typisiert (`name`, `description`, `parameters` roh, `strict`) | Unbekannte Schlüssel sind Extras (verschachtelt `function.<key>`). Namen außerhalb von `[a-zA-Z0-9_-]{1,64}` aus anderen Dialekten werden deterministisch umgeschrieben (`EgressCtx::tool_names`). |
| `tools[]` andere Typen (`custom`) | `ToolDef::Unknown` | Nur im selben Dialekt. |
| `tool_choice` `"auto"`, `"none"`, `"required"` | `ToolChoice::Auto`, `None`, `Required` typisiert | |
| `tool_choice` `{type: function, function: {name}}` | `ToolChoice::Tool(name)` typisiert | Mit unbekannten Schlüsseln: `ToolChoice::Unknown`. |
| `tool_choice` `allowed_tools`, `custom`, andere Strings | `ToolChoice::Unknown` | Nur im selben Dialekt. |
| `parallel_tool_calls` | `parallel_tool_calls` typisiert | |
| `safety_identifier` | `user` typisiert | KPP `user` ist die ID des Endbenutzers für die Missbrauchserkennung, und genau das ist `safety_identifier`. |
| `user` (veraltet) | `user` typisiert, wenn `safety_identifier` fehlt (Form `user`); sonst das Extra `user (sent alongside safety_identifier)`, das als `user` zurückgesendet wird | KPP `user` aus anderen Dialekten wird als `safety_identifier` gesendet. Ein Anbieter, dessen Profil kein Feld für den Endbenutzer hat, erhält keines von beiden ([Anbieterprofile](#anbieterprofile)). |
| `stream` | `stream` typisiert | `false`: Form `stream_false`. |
| `stream_options.include_usage` | `StreamMode::On { include_usage }` typisiert | `stream_options` fehlt: Form `stream_options: "absent"`; ohne `include_usage`: Form `"no_include_usage"`; andere Schlüssel (`include_obfuscation`) sind Extras `stream_options.<key>`; bei ausgeschaltetem `stream`: das ganze Objekt als Form `stream_options_off`. Anfragen aus anderen Dialekten verlangen immer `include_usage: true`. Das Gateway braucht die Nutzung, setzt deshalb bei jeder Streaming-Anfrage `include_usage: true` und behält die ursprüngliche Anfrage der App in `EncodeCtx`; der Stream-Encoder der App blendet den Nutzungs-Chunk dann für Apps aus, die nicht danach gefragt haben. |
| `audio`, `functions` und `function_call` (veraltet, oberste Ebene), `logit_bias`, `logprobs`, `metadata`, `modalities`, `moderation`, `prediction`, `prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `service_tier`, `store`, `top_logprobs`, `verbosity`, `web_search_options` | Extras | Keine Entsprechung in KPP; über Dialektgrenzen abgelehnt ([Extras](cross-format.md#extras), [Sampling](cross-format.md#sampling)), außer `prompt_cache_key`, das auf der [Extras-Allowlist](cross-format.md#extras) steht und weggelassen wird. `null`-Werte sind Form. Gegenüber Anthropic etwa schlägt `functions` fehl mit `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. `` |
| unbekannte Schlüssel | Extras | Ein Schlüssel, der mit `header:` beginnt oder `.` enthält, wird abgelehnt ([Schlüssel, die Kaeso nicht tragen kann](#schlüssel-die-kaeso-nicht-tragen-kann)). |
| Header `openai-beta` | Extras `header:openai-beta` | Wird nur weitergeleitet, wenn die Anfrage an Chat geht; steht auf der [Extras-Allowlist](cross-format.md#extras), andere Dialekte verwerfen ihn also. |

### Nachrichten (`ChatCompletionRequestMessage`)

| Wire | KPP | Anmerkungen |
|---|---|---|
| führende `system`- und `developer`-Nachrichten | Blöcke in `GenerateRequest.system` | Der erste Block jeder Nachricht trägt die Form `message: {role, content}`, die Nachrichtengrenzen, die Rolle und die Schreibweise als String oder Array bewahrt; `name` und unbekannte Schlüssel der Nachricht sind Extras `message.<key>` an diesem Block. Eine führende Nachricht, deren erster Teil unbekannt ist, bleibt eine Nachricht mit `Role::System` und beendet die führende Folge. Aus anderen Dialekten: eine `system`-Nachricht. |
| spätere `system`- und `developer`-Nachrichten | `Message { role: System }` ([Systemnachrichten](cross-format.md#systemnachrichten)) | `developer`: Form `role: "developer"`. |
| `user` | `Message { role: User }` | `name` und unbekannte Schlüssel: Extras der Nachricht. |
| `assistant` | `Message { role: Assistant }`: Inhaltsteile, dann `refusal`, dann `tool_calls` | `content` fehlt: Form `content: "absent"`; `null`: verbatim; `name`, `audio`, `function_call`: Extras. `annotations` steht nicht im Anfrageschema, aber die SDKs senden es mit jeder Antwort zurück: `[]` ist verbatim (Form), damit die Unterhaltung zu einem anderen Anbieter wechseln kann; ein nicht leerer Wert wird zu `TextBlock.citations` des ersten Textblocks, gebunden an Chat (ohne Text: Extras `annotations`), was andere Dialekte ablehnen (`citations`) und Chat als `annotations` zurücksendet. Aus anderen Dialekten gehen Textblöcke in `content`, die erste Verweigerung in `refusal` und Tool-Aufrufe in `tool_calls`, weil Chat sie nicht abwechselnd anordnen kann; Reasoning wird verworfen ([Reasoning](reasoning.md)); eine Runde, in der nichts bleibt, was Chat zeigen kann, etwa nur Reasoning, wird als `content: ""` gesendet, damit die Unterhaltung ihre Runden behält und die Regel von Chat erfüllt ist, dass eine Assistentennachricht ohne `tool_calls` Inhalt braucht. |
| `refusal` des Assistenten | `Block::Refusal` | |
| `function_call` des Assistenten (veraltet) | Extras `function_call` der Nachricht | `null` bleibt verbatim erhalten. Wird so zurückgeschrieben, wie es kam; andere Dialekte lehnen es ab (`messages[i].function_call`). |
| `tool`-Nachrichten | `Block::ToolResult` in einer Nachricht mit `Role::User`; aufeinanderfolgende Tool-Nachrichten teilen sich eine KPP-Nachricht | `tool_call_id` wird zu `call_id`; Inhaltsteile werden zu verschachtelten Textblöcken; unbekannte Schlüssel sind Extras des Tool-Ergebnisses. `is_error` aus anderen Dialekten wird verworfen, weil Chat Fehler im Inhalt meldet ([Tool-Fehler](cross-format.md#tool-fehler)). Aus anderen Dialekten werden die Tool-Ergebnisse einer Benutzernachricht zu `tool`-Nachrichten, gefolgt von einer `user`-Nachricht mit den übrigen Blöcken. |
| `function`-Nachrichten (veraltet) | `Message { role: User }` mit einem `Block::Unknown`, der die ganze Nachricht enthält, dazu die Form `role: "function"` der Nachricht | Wird so zurückgeschrieben, wie sie kam; andere Dialekte lehnen sie ab (`` `function` message ``). |
| `content` als String oder Array | Form `content: "array"` (Nachricht oder Tool-Ergebnis) | Ein String ist der Standard für einen einzelnen reinen Textblock. Leere Arrays sind ungültig (`minItems: 1`). |
| eine Nachricht ohne `role` oder mit einer anderen Rolle | `InvalidRequest` (`param` `messages[i]`) | |

Das alte Function Calling hat keine IDs für Tool-Aufrufe, deshalb werden seine Runden keine KPP-Tool-Aufrufe und -Ergebnisse; innerhalb dieses Dialekts bestehen sie stattdessen den Round Trip, mit den Parametern `functions` und `function_call` auf oberster Ebene als Extras. Die Fixture ist `fixtures/openai_chat/requests/legacy-functions.json`. Andere Dialekte lehnen all das mit `Unsupported` ab: die Parameter auf oberster Ebene als `functions` und `function_call`, das `function_call` eines Assistenten als `messages[i].function_call` und eine `function`-Nachricht als `` `function` message ``, mit dem Hinweis `Remove the message, or send the request to a model served through its original format.`

### Inhaltsteile

| Wire | KPP | Anmerkungen |
|---|---|---|
| `text` | `Block::Text` | Unbekannte Schlüssel: Extras des Blocks. |
| `prompt_cache_breakpoint` (Text-, Bild- und Dateiteile) | `cache: CacheHint { ttl: None }`, gebunden an Chat | Beide markieren das Ende eines wiederverwendbaren Präfixes; die TTL kommt aus `prompt_cache_options`. Nur Hinweise aus Chat werden zurückgesendet; Hinweise aus anderen Dialekten werden verworfen ([Cache-Hinweise](cross-format.md#cache-hinweise)). `mode` außer `explicit`: Form `mode`. |
| `image_url.url` (`https:`) | `Block::Image { source: Url }` | |
| `image_url.url` (`data:<type>[;params];base64,<data>`, RFC 2397, Schema und `base64` in beliebiger Schreibung) | `Block::Image { source: Base64 }` | Ein anderer Kopf als `data:<type>;base64`, etwa mit Parametern (`;name=a.png`) oder in Großbuchstaben, bleibt in der Form `data_url` exakt erhalten, auch für `file.file_data`. Data-URLs, die nicht base64-kodiert sind oder keinen Medientyp haben, bleiben `Url`. |
| `image_url.detail` | Extras `image_url.detail` | Steht auf der [Extras-Allowlist](cross-format.md#extras): wird für andere Dialekte stillschweigend verworfen. |
| `file.file_data` (`data:`-URL) | `Block::Document { source: Base64 }` | Reines Base64 gilt als Medientyp `application/pdf`, der einzige Typ im Leitfaden von OpenAI zu Dateieingaben, mit Form `bare_file_data`, und wird wieder ohne Kopf geschrieben. |
| `file.file_id` | `Block::Document { source: FileId }`, gebunden an den Dateispeicher von OpenAI ([Dateien](cross-format.md#dateien)) | Zusammen mit `file_data`: Extras `file.file_id`. |
| `file.filename` | Extras `file.filename` | Der Dateiname, den OpenAI neben eingebettetem `file_data` verlangt, kein Titel, den das Modell sieht; daher wird er nie zu `DocumentBlock::title`. Steht auf der [Extras-Allowlist](cross-format.md#extras). Eingebettete Dokumente aus anderen Dialekten bekommen `document.<subtype>`, etwa `document.pdf`, weil jedes Beispiel in OpenAIs [Leitfaden zu Datei-Eingaben](https://developers.openai.com/api/docs/guides/file-inputs) einen mit `file_data` sendet. |
| `input_audio` | `Unsupported` (`input_audio`) | Audioeingaben sind nicht Teil von KPP. |
| `refusal` (Assistent) | `Block::Refusal` + Form `part` | |
| andere Typen sowie `image_url`- und `file`-Teile außerhalb von Benutzernachrichten und `refusal`-Teile außerhalb von Assistentennachrichten | `Block::Unknown` | Nur im selben Dialekt. |

Für KPP aus anderen Dialekten lehnt `check()` ab: Textdokumente, Dokument-URLs, `title`, `context` und `citations` von Dokumenten, Bild-Datei-IDs, Bilder und Dokumente außerhalb von Benutzernachrichten, Verweigerungen außerhalb von Assistentennachrichten, Zitate außerhalb von Assistentennachrichten (und über `check_foreign` die Zitate anderer Dialekte), `top_k` ([Sampling](cross-format.md#sampling)), unbekannte Reasoning-Modi und Aufwandsstufen außerhalb des Enums der Spezifikation. Budgets und `disabled` werden umgerechnet ([Aufwand und Budget](reasoning.md#aufwand-und-budget)). Diese Hinweise werden stattdessen verworfen: Reasoning-Blöcke ([Reasoning](reasoning.md)), Cache-Hinweise anderer Dialekte ([Cache-Hinweise](cross-format.md#cache-hinweise)), `ToolResult::is_error`, dessen Information der Fehlerinhalt trägt ([Tool-Fehler](cross-format.md#tool-fehler)), sowie `reasoning.summary` und `reasoning.include_encrypted` anderer Dialekte, weil Chat kein Reasoning zum Anzeigen liefert ([lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)).

Ein Body, den der Codec nicht lesen kann, ist `InvalidRequest` und nennt das Feld und was es sein muss, mit gesetztem `param`, nie eine Parserposition: `` The request body is not a valid OpenAI Chat Completions request: `max_completion_tokens` must be an integer from 0 to 4294967295. ``

### Schlüssel, die Kaeso nicht tragen kann

Ein unbekannter Schlüssel, dessen Name mit `header:` beginnt oder `.` enthält, kann nicht getragen werden, weil KPP Header und Schlüsselpfade so schreibt. Er wird überall abgelehnt, wo der Codec ein Objekt Schlüssel für Schlüssel liest:

- im Body, in `stream_options`, in einem `response_format` vom Typ `text`, `json_object` oder `json_schema` und dessen `json_schema`;
- in Funktions-Tools und ihrem `function` sowie in einer benannten `tool_choice` und ihrem `function`;
- in `system`-, `developer`-, `user`-, `assistant`- und `tool`-Nachrichten;
- in `text`-, `refusal`-, `image_url`- und `file`-Teilen mit ihren Objekten `image_url`, `file` und `prompt_cache_breakpoint`;
- in Funktions-Tool-Aufrufen und ihrem `function`.

Die Anfrage ist dann `InvalidRequest`, etwa `` The request body is not a valid OpenAI Chat Completions request: the field `header:authorization` has a name Kaeso cannot carry; remove it. `` oder, bei einer Nachricht, `` The request contains an invalid OpenAI Chat Completions user message: the field `a.b` has a name Kaeso cannot carry; remove it. `` Ein Name über 64 Bytes wird gekürzt und endet auf `…`. In ganz aufbewahrte Objekte, etwa unbekannte Teile, Tools und Tool-Auswahlen, Custom-Tool-Aufrufe, `function`-Nachrichten und andere `response_format`-Typen, wird nicht hineingesehen. In Antworten und Stream-Chunks ergibt dieselbe Regel einen `ProviderError` (in einem Stream ein `Error`-Event).

### Tool-Aufrufe (`ChatCompletionMessageToolCall`)

| Wire | KPP | Anmerkungen |
|---|---|---|
| `id`, `function.name` | `ToolCallBlock.id`, `.name` | Von Kaeso umgeschriebene Namen werden beim Dekodieren einer Antwort zurückübersetzt. Ein Aufruf in einer Antwort ohne `id` oder mit `""` bekommt `call_kaeso_<Antwort-ID>_<n>` (die Buchstaben und Ziffern der ID, `n` seine Position), damit Apps in anderen Formaten ihn beantworten können; Form `id` hält die empfangene Schreibweise (`null` für fehlend) für Chat-Apps fest. Streams machen es genauso, mit dem KPP-Blockindex als `n` und Form `tc`. |
| `function.arguments` | `ToolCallBlock.arguments` roh | Der JSON-Text im String, Byte für Byte. Text, der kein gültiges JSON ist oder umgebenden Leerraum hat, behält sein Literal in der Form `arguments`; KPP enthält dann `{}` für leeren Text und sonst den Text als JSON-String. Nach der Konvention von KPP bedeutet ein JSON-String in `ToolCallBlock.arguments`, dass der Anbieter Text gesendet hat, der kein JSON ist; für Aufrufe aus anderen Dialekten wird deshalb der Inhalt des Strings als `arguments` gesendet, Objekte, Arrays und andere JSON-Werte als ihr JSON-Text. |
| unbekannte Schlüssel | Extras (verschachtelt `function.<key>`) | |
| `type: custom` | `Block::Unknown` | Wird in `tool_calls` zurückgesendet. |
| `type` fehlt, mit einem `function`-Objekt | `Block::ToolCall`, Form `no_type` | `ChatCompletionMessageToolCall` verlangt `type`, manche OpenAI-kompatiblen Anbieter lassen es aber weg; nur Funktionsaufrufe haben ein `function`-Objekt, daher erreicht der Aufruf jede App. Wird ohne `type` zurückgeschrieben. |

## Anbieterprofile

Andere Anbieter sprechen dieses Format mit eigenen Abweichungen. Jeder Anbieter in `PROVIDERS` (`crates/kaeso-core/src/provider.rs`) nennt im Feld `chat` ein `kaeso_codecs::openai_chat::ChatProfile`, und `EgressCtx::with_chat_profile` übergibt es dem Encoder, der es auf jede Anfrage anwendet, gleich in welchem Format die App sie gesendet hat. `ChatProfile::OPENAI`, der Standard, ändert nichts, ebenso `ChatProfile::compatible(name)`, solange keine Einstellung hinzukommt.

| Einstellung | Was der Encoder tut | Warum das erlaubt ist |
|---|---|---|
| `max_tokens()` | Sendet das Ausgabelimit als `max_tokens` und ein `max_completion_tokens: null` als `max_tokens: null`. Eine Chat-Anfrage mit beiden Feldern und verschiedenen Werten ist `Unsupported` (`max_tokens`). | Nur die Schreibweise ändert sich; der Wert bleibt gleich. |
| `system_role()` | Sendet `developer`-Nachrichten als `system`-Nachrichten. | Die Schreibweise der Rolle ist Form; beide tragen die Anweisungen. |
| `random_seed()` | Sendet `seed` als `random_seed`, auch ein `seed: null`. Ein negativer Seed oder eine Anfrage mit `seed` und `random_seed` (beide auch als `null`) ist `Unsupported` (`seed`). Die Regel folgt Mistrals `random_seed`, dem sein OpenAPI-Dokument `minimum: 0` gibt. | `seed` bleibt semantisch: Es wird umbenannt oder abgelehnt, nie verworfen. |
| `without_end_user()` | Lässt KPP `user` weg, statt es als `user` oder `safety_identifier` zu senden, zusammen mit einem zweiten `user`, das eine Chat-App neben `safety_identifier` gesendet hat, und ihren `null`-Schreibweisen. | `user` ist ein Hinweis ([Feldklassen](index.md#feldklassen)). |
| `refusing(fields)` | Lehnt eine Anfrage ab, deren Body einen dieser Schlüssel auf oberster Ebene enthielte, etwa `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | Extras sind semantisch, daher werden sie abgelehnt, nicht verworfen. |
| `refusing_in_messages(fields)` | Dasselbe für Schlüssel von System-, Benutzer- und Assistentennachrichten. Tool-Nachrichten werden nicht geprüft. | Wie oben. |

`refusing` und `refusing_in_messages` nehmen jeweils die ganze Liste; ein zweiter Aufruf ersetzt den ersten. Eine Ablehnung nennt das Feld so, wie die App es geschrieben hat: den Schlüssel einer Chat-App mit eigenem Namen und Pfad (`logit_bias`, `messages[1].name`) und einen Ablehnungsblock aus einem anderen Format mit seinem KPP-Pfad (`messages[1].blocks[0]`). Die Ablehnung geschieht beim Kodieren der Anfrage, bevor etwas gesendet wird, und erreicht die App in ihrem eigenen Format.

Jeder eingebaute Anbieter, dessen Profil von dem von OpenAI abweicht, steht unten mit der Quelle jeder Einstellung: der Dokumentation des Anbieters, seinem OpenAPI-Dokument oder einer Live-Aufnahme in `fixtures/openai_chat/providers/<id>/`. Noch hat kein eingebauter Anbieter ein solches Profil.

## Antwort: `CreateChatCompletionResponse`

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `id`, `model` | `Response.id`, `.model` | Aus einem anderen Format die ID des Anbieters, unverändert ([IDs von Antworten](cross-format.md#ids-von-antworten)). |
| `object` | keins | Immer `chat.completion`; ein anderer Wert oder keiner: Form `object`. |
| `created`, `service_tier`, `system_fingerprint`, `metadata`, `moderation` | Extras | Eine Antwort ohne `created` in ihren Extras, etwa eine aus einem anderen Dialekt, bekommt `EncodeCtx::created_at` oder `0`. |
| `choices` | `choices[0]` | Mehr als eine Auswahl: `Unsupported` (`n`); keine: `ProviderError`; ein anderer `index` als 0: Extras `choice.index`. Ein 200-Body mit einem `error`-Objekt und ohne `choices`, den OpenRouter sendet, wenn der Anbieter nach dem 200 scheitert ([Errors and debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging)), wird wie ein Stream-Fehler als dieser Fehler dekodiert. |
| `choices[0].logprobs` | Extras `choice.logprobs` | `null`: verbatim. |
| `choices[0].finish_reason` | `Response.stop` | `stop` ⇄ `EndTurn`, `length` ⇄ `MaxTokens`, `tool_calls` ⇄ `ToolUse`, `content_filter` ⇄ `ContentFilter`, andere Werte (etwa `function_call`) ⇄ `Other`. Aus anderen Dialekten: `StopSequence` und `PauseTurn` werden zu `stop`; `Refusal` wird zu `content_filter`, weil die Antwort zurückgehalten wurde. Eigene Werte anderer Dialekte werden zum nächstliegenden Chat-Wert: `max_tokens`, `max_output_tokens` und `model_context_window_exceeded` werden zu `length`, `tool_use` zu `tool_calls`, `refusal` zu `content_filter` und alles andere außerhalb des Enums der Spezifikation zu `stop`. Eine Antwort ohne Streaming und ohne `finish_reason`, das die Spezifikation verlangt: `ProviderError`. |
| `message.content` | `Block::Text` | Aus anderen Dialekten werden alle Textblöcke ohne Trennzeichen verbunden, weil Chat in Antworten keine Inhaltsteile hat. Ohne Trennzeichen deshalb, weil Anthropic eine Antwort rund um Zitate in mehrere Textblöcke teilt und die gestreamten Deltas, die ebenfalls kein Trennzeichen tragen, denselben `content` ergeben müssen. |
| `message.annotations` | `TextBlock.citations` (roh, gebunden an Chat) | Ohne Inhalt: Extras `choice.message.annotations`. Zitate anderer Dialekte werden weggelassen ([Antworten](cross-format.md#antworten)). |
| `message.refusal` | `Block::Refusal` | |
| `message.tool_calls` | `Block::ToolCall` oder `Block::Unknown` | Wie in Anfragen. |
| `message.audio` (`id`, `data`, `expires_at`, `transcript`), `message.function_call`, unbekannte Schlüssel | Extras `choice.message.<key>` | Das Audio-Objekt bleibt mit `transcript` und `expires_at` vollständig erhalten. |
| Reasoning, unbekannte Blöcke anderer Dialekte | keins | Werden in einer Chat-Antwort weggelassen ([Antworten](cross-format.md#antworten)). Das Gateway bewahrt zurückspielbares Reasoning für die nächste Runde auf, zugeordnet den Tool-Aufrufen, vor denen es stand ([vom Gateway aufbewahrtes Reasoning](reasoning.md#vom-gateway-aufbewahrtes-reasoning), `kaeso_codecs::continuity`). |

Eine Antwort aus einem anderen Dialekt schreibt leere Werte so wie OpenAI: `content: null`, `refusal: null`, `annotations: []` und `logprobs: null`.

### Nutzung (`CompletionUsage`)

Die KPP-Felder folgen der Konvention unter [Nutzung](cross-format.md#nutzung).

| Wire | KPP |
|---|---|
| `prompt_tokens − cached_tokens − cache_write_tokens` | `input_tokens` |
| `prompt_tokens_details.cached_tokens` | `cache_read_tokens`, höchstens `prompt_tokens`; `cache_write_tokens` höchstens der Rest |
| `prompt_tokens_details.cache_write_tokens` | `cache_write_tokens` |
| `completion_tokens` | `output_tokens` (einschließlich Reasoning; ist `total_tokens` gleich `prompt_tokens + completion_tokens + reasoning_tokens`, wie SpaceX zählt, kommen die Reasoning-Token hinzu); `null` zählt, wie bei `prompt_tokens` und `total_tokens`, als fehlend (0) und bleibt in `raw` |
| `completion_tokens_details.reasoning_tokens` | `reasoning_tokens` |
| `total_tokens` | abgeleitet (`prompt_tokens + completion_tokens`); nicht eigens gespeichert, in `raw` erhalten |
| `prompt_tokens_details.audio_tokens`, `text_tokens`, `image_tokens`, `completion_tokens_details.audio_tokens`, `text_tokens`, `accepted_prediction_tokens`, `rejected_prediction_tokens` | nur `raw` (Aufschlüsselungen der typisierten Summen) |
| das ganze Objekt | `raw`, unverändert zurückgesendet, solange die typisierten Felder noch dazu passen; sonst gewinnen die typisierten Felder, und das rohe Objekt ergänzt die übrigen Schlüssel |

Aus anderen Dialekten: `prompt_tokens = input + cache_read + cache_write`, `prompt_tokens_details.cached_tokens = cache_read`, dazu `cache_write_tokens`, wenn es nicht null ist, und `completion_tokens_details.reasoning_tokens`, wenn KPP eine Reasoning-Zahl hat.

## Streams: `CreateChatCompletionStreamResponse`

Einfache `data:`-Frames mit je einem JSON-Chunk, am Ende `data: [DONE]`.

| Wire | KPP-Event | Anmerkungen |
|---|---|---|
| erster Chunk | `ResponseStart { id, model }` | `created`, `service_tier`, `system_fingerprint`: Extras; `object` außer `chat.completion.chunk` oder keins: Form `object`. Hat der erste Chunk keinen Inhalt und ein leeres oder fehlendes `id` oder `model`, werden der Start und die Chunks ohne Inhalt danach zurückgehalten, bis beide Werte bekannt sind oder ein Chunk Inhalt trägt (ein Block- oder Antwort-Delta); dann geht der Start mit den bis dahin bekannten Werten hinaus, jeder aus dem ersten Chunk, der ihn nicht leer hatte. |
| `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint` in jedem Chunk | keins | Der Envelope wiederholt sich. Wo der Envelope eines Chunks von dem abweicht, den der Encoder aus `ResponseStart` neu aufbaut, bewahrt die Form `chunk.env` die Werte des Chunks und `chunk.env_absent` die Schlüssel, die ihm fehlen, sodass jeder Chunk, auch der erste, so zurückgeschrieben wird, wie er kam. Übereinstimmende Chunks folgen `ResponseStart`; ein Gateway, das dort `id` oder `model` umschreibt, schreibt sie also um. |
| `choices[0].delta.role` | Form `chunk.role` | Aus anderen Dialekten beginnt der Stream, sobald die Antwort startet, mit einem Rollen-Chunk, `delta: {"role": "assistant", "content": "", "refusal": null}`, dem ersten Chunk, den OpenAI selbst bei einer Textantwort sendet; so hört die App vom Stream, während das Modell nachdenkt ([Keepalives](cross-format.md#keepalives)). |
| `choices[0].delta.content` | einmal `BlockStart(Text)`, dann `BlockDelta(Text)` | Jedes Stück, auch `""`. |
| `choices[0].delta.refusal` | einmal `BlockStart(Refusal)`, dann `BlockDelta(Refusal)` | |
| `choices[0].delta.tool_calls[]` mit neuem `index` | `BlockStart(ToolCall { id, name })` + `BlockDelta(ToolArguments)` für `arguments` | KPP-Blockindizes folgen der Reihenfolge, in der Blöcke beginnen; der Chat-`index` wird auf den KPP-Index abgebildet. Umgeschriebene Namen werden zurückübersetzt. Abweichungen in der Gestalt (fehlendes `id`, `type`, `function`, `name` oder `arguments`, ein unerwarteter `index`): Form `tc`. Unbekannte Schlüssel: Extras `tool_call.<key>` und `tool_call.function.<key>`. Ein anderer `type` als `function`: `BlockStart(Unknown)` und für seine späteren Einträge `BlockDelta(Unknown)`. Ein Eintrag ohne nicht leeres `id` oder `name`, die manche OpenAI-kompatiblen Anbieter in einem späteren Chunk senden: Der `BlockStart` und jedes Event danach werden zurückgehalten, bis beide angekommen sind, der Block endet oder der Stream endet oder abbricht; der Header trägt dann die späten Werte `id` und `name`, Namen zurückübersetzt. Die Form `tc` bewahrt ein `id` oder `name`, das der erste Eintrag als `""` hatte. |
| `choices[0].delta.tool_calls[]` mit bekanntem `index` | `BlockDelta(ToolArguments)` | Ein wiederholtes oder spätes `id`, `type` oder `name`: Form `tc`. Eine andere, nicht leere `id`, wo der Aufruf schon eine hat, ist ein neuer Aufruf mit demselben Index: Der frühere Block endet, und ein neues `BlockStart` beginnt, mit Form `tc.index`. |
| `choices[0].finish_reason` | `BlockStop` für jeden offenen Block, dann `ResponseDelta { stop }` | Abbildung wie bei Antworten. `""`, das kein Stoppgrund ist, zählt wie `null`: Form `chunk.cv`. |
| `usage` (Chunk mit `choices: []`) | `ResponseDelta { usage }` | Nutzung wie unter [Nutzung](#nutzung-completionusage), roh erhalten. `choices` leer oder fehlend: Form `chunk.ch`. |
| `usage` auf einem Chunk vor `finish_reason` | zurückgehalten, Form `chunk.u` | Laufende Nutzung, die Anbieter auf Basis von vLLM mit `stream_options.continuous_usage_stats` in jedem Chunk senden. KPP meldet Nutzung erst, nachdem der letzte Block endet. Der Decoder behält daher die jeweils neueste und sendet sie mit dem `ResponseDelta` des Finish-Chunks, wenn dieser keine eigene hat, oder als `ResponseDelta { usage }` vor `ResponseStop`, wenn nach dem Finish keine Nutzung mehr kommt. Form `chunk.u` hält das Objekt jedes Chunks fest, sodass eine Chat-App, die Nutzung angefordert hat, jeden Chunk so zurückbekommt, wie er kam; eine App, die sie nicht angefordert hat, bekommt keine Nutzung, und ein Chunk, der nur Nutzung enthielt, fällt für sie weg. Die Fixture ist `fixtures/openai_chat/streams/usage-every-chunk.sse`. |
| `usage: null`, `logprobs: null`, `finish_reason: null`, `content: null`, `refusal: null` | Form `chunk.kv`, `cv`, `dv` | |
| `choices[0].logprobs`, unbekannte Schlüssel der Auswahl | Extras `choice.<key>` am ersten Event des Chunks | |
| `delta.audio`, `delta.function_call`, unbekannte Schlüssel im Delta | Extras `delta.<key>` | Ein Chunk nach dem ersten ganz ohne typisierten Inhalt ist ein `Extra` (`{"event": null, "data": <chunk>}`). |
| `obfuscation` | Form `chunk.obf` | Wird nicht an Apps gesendet, wenn die Events aus einem anderen Dialekt kamen. |
| `moderation`, unbekannte Schlüssel im Chunk | Extras am ersten Event des Chunks | |
| `data: [DONE]` | `ResponseStop` (nach den `BlockStop`s noch offener Blöcke) | Ein Stream, der nach `finish_reason` ohne `[DONE]` endet: `ResponseStop` mit Form `no_done`; einer, der früher endet, mit oder ohne `[DONE]`: `Error`, weil die Antwort abgeschnitten wurde. Ein `[DONE]`-Frame mit SSE-Feldern ist ein `Extra`, und der Stream endet dann wie einer ohne `[DONE]`. |
| `data: {"error": {…}}` | `Error`, dekodiert wie ein Fehler-Body ohne Status | Der Decoder hört auf. Wird als derselbe Frame kodiert; kein `[DONE]` folgt. Auch dann, wenn der Chunk zusätzlich `choices` trägt, wie OpenRouter Fehler mitten im Stream sendet ([Errors and debugging](https://openrouter.ai/docs/api/reference/errors-and-debugging)); Fixture `fixtures/openai_chat/streams/error-in-chunk.sse`. |
| `choices[0].finish_reason: "error"` ohne `error`-Objekt | `Error` (`ProviderError`) | Keine fertige Antwort. Der Chunk bleibt in `KppError.raw` und geht so an Chat-Apps zurück, wie er kam. |
| `event: error`-Frames mit Daten, die manche OpenAI-kompatiblen Anbieter senden | `Error` | Dekodiert wie ein Fehler-Body ohne Status; als `data:`-Frame kodiert. Ohne Daten ist ein solcher Frame ein `Extra`. |
| ein `data:`-Frame, der kein JSON ist, ein Chunk, den Kaeso nicht lesen kann | `Error` (`ProviderError`) | Der Decoder hört auf. |
| SSE-Kommentare, Frames ohne Daten oder mit leeren Daten, andere benannte Events, Frames nach `[DONE]` | `Extra` (`{"comment"}` oder `{"event", "data", "text", "id", "retry", "other"}`, ohne `data` bei einem Frame, der keine hatte, und mit `text: true`, wenn die Daten kein JSON sind) | Nur im selben Dialekt; werden so zurückgeschrieben, wie sie kamen. |
| mehrere Auswahlen, eine Auswahl mit anderem `index` als 0 | `Error` mit `Unsupported` (`n`) | |

Gruppierung in Chunks: Das erste Event jedes Chunks trägt die Form-Markierung `chunk` (`n` ist die Zahl der Events aus diesem Chunk, dazu die Gestalt-Markierungen oben), die anderen tragen `in_chunk`, damit der Encoder sie wieder als einen Frame schreibt. Ein offener Chunk wird geschrieben, sobald ein Event ankommt, das nicht dazugehört (ein Event eines anderen Dialekts, `[DONE]`, ein Fehler), sodass ein verworfenes Event nie einen Chunk hinter `[DONE]` schiebt; nach `[DONE]` oder einem Fehler wird kein Chunk mehr geschrieben. Tool-Argumente für einen Block, den Chat nicht zeigen kann, werden weggelassen.

Das SSE-Framing folgt der Spezifikation ([Server-sent Events](codec-design.md#server-sent-events)): Ein `id`, das NUL enthält, oder ein `retry`, das nicht aus ASCII-Ziffern besteht, die in ein `u64` passen, gilt als unbekanntes Feld. SSE-`id`, `retry` und unbekannte SSE-Felder eines Chunks, die OpenAI nicht sendet, bleiben in der Form `chunk.sse` als `{"id", "retry", "other": [[name, value], …]}` erhalten; der Chunk wird wie jeder andere dekodiert, und die Felder werden an Chat-Apps zurückgeschrieben. Ein Frame ohne `data:`-Zeile, etwa `retry: 3000` oder `event: ping` allein, ist nach der Spezifikation kein Event: Er wird zu einem `Extra`, dessen rohes JSON keinen Schlüssel `data` hat, und wird ohne Datenzeile zurückgeschrieben. Benannte Events und Frames nach `[DONE]` bleiben vollständig erhalten und werden nur an Chat-Apps zurückgespielt.

OpenAI sendet `id`, `type` und `function.name` eines Tool-Aufrufs in dessen erstem Chunk, das Schema erlaubt sie aber auch in einem späteren, und manche OpenAI-kompatiblen Anbieter senden `id` und `name` dort, nach einem ersten Chunk mit `choices: []` und leerem `id` und `model`. Anthropic und Responses kündigen beides beim Start eines Blocks an, deshalb hält der Decoder den Start und den Aufruf wie oben zurück, bis er sie kennt; die Reihenfolge der Events ändert sich nie. Streams, die in diesem Format bleiben, werden Chunk für Chunk exakt zurückgespielt. Die Fixture ist `fixtures/openai_chat/streams/late-envelope-and-tool-header.sse`.

Events aus anderen Dialekten werden zu je einem Chunk, nach dem eröffnenden Rollen-Chunk, mit `logprobs: null` und `finish_reason`. Tool-Indizes werden in der Reihenfolge vergeben, in der Tool-Aufrufe beginnen, und Reasoning, Zitate, opake Daten, unbekannte Blöcke und Extras anderer Dialekte werden weggelassen ([Antworten](cross-format.md#antworten)). Während sie ankommen, schreibt der Encoder getaktete SSE-Kommentare (`: keepalive`), die die SSE-Spezifikation und die Parser der SDKs ignorieren: einen für jedes Extra-Event eines anderen Dialekts, etwa Anthropic `ping`, und einen je 1.024 Bytes verborgenen Texts ([Keepalives](cross-format.md#keepalives)). `StreamEncoder::keepalive` gibt dem Gateway denselben Frame für Stille beim Anbieter. Nutzung wird nur geschrieben, wenn die App danach gefragt hat (`EncodeCtx::stream` mit `include_usage`): Jeder Chunk hat dann `usage: null`, und ein letzter Chunk mit `choices: []` trägt die Nutzung. Aus anderen Dialekten ist das die zuletzt gemeldete Nutzung, mit den Eingabezahlen aus `ResponseStart`, wenn die spätere Meldung keine hat.

## Fehler (`ErrorResponse`)

Der Fehler-Body ist `{error: {message, type, param, code}}`.

| Signal | `ErrorKind` | Kodiert für KPP-Fehler von anderswo |
|---|---|---|
| `code: context_length_exceeded` | `ContextTooLong` | 400 `invalid_request_error`, Code `context_length_exceeded` |
| `type` oder `code` `insufficient_quota`, Abrechnungscodes | `Billing` | 429 `insufficient_quota`, Code `insufficient_quota` |
| `code` `server_is_overloaded` oder `service_unavailable_error` | `Overloaded` | siehe 503 |
| `code` `content_policy_violation`, `content_filter`, `bio_policy`, `cyber_policy`, `misalignment_policy_violation` oder `image_content_policy_violation` | `ContentFiltered` | 400 `invalid_request_error`, Code `content_policy_violation` |
| 400, 409, 422 | `InvalidRequest` (und `Unsupported` beim Kodieren) | 400 `invalid_request_error` |
| 401 | `Authentication` | 401 `invalid_request_error` |
| 402 | `Billing` | 429 `insufficient_quota`, code `insufficient_quota` |
| 403 | `PermissionDenied` | 403 `request_forbidden` |
| 404 | `NotFound` | 404 `invalid_request_error` |
| 413 | `RequestTooLarge` | 413 `invalid_request_error` |
| 429 | `RateLimited` | 429 `rate_limit_error`, Code `rate_limit_exceeded` |
| 503 | `Overloaded` | 503 `service_unavailable_error`, Code `server_is_overloaded` |
| 408, 504, `code` `vector_store_timeout` | `Timeout` | 504 `server_error`, Code `timeout` |
| andere | `ProviderError` | sein 5xx-Status (außer 503) oder 500, `server_error` |
| `error.misalignment` (`MisalignmentErrorDetailsResource` im Schema `Error`), unbekannte Schlüssel in `error` | in `KppError.raw` erhalten (der ganze Body) | Wird nur erneut gesendet, wenn der Fehler aus Chat kam. |

Ein in der Tabelle genannter Code, in `code` oder `type`, hat Vorrang vor dem Status. Codes der Arten, die sich nach dem Status richten (`rate_limit_exceeded`, `rate_limit_error`, `slow_down`, `invalid_request_error`, `invalid_prompt`, die Bild-Codes, `server_error`), entscheiden nur bei einem Fehler ohne HTTP-Status: einem Stream-Fehler oder einem Fehler im Body einer Antwort mit 200. Bei einem solchen Fehler bestimmt ein numerischer `code` zwischen 400 und 599, den OpenRouter als HTTP-Status sendet, die Art wie ein Status; einen Status hat der Fehler trotzdem nicht.

Mit HTTP 402 melden OpenAI-kompatible Anbieter wie DeepSeek ein aufgebrauchtes Guthaben ([Fehlercodes von DeepSeek](https://api-docs.deepseek.com/quick_start/error_codes)), deshalb ist es `Billing`, was auch immer sein `code` sagt.

Die Codecs für Responses und Embeddings verwenden dieselbe Code-Tabelle (`openai_responses::error`), sodass ein Fehler in der OpenAI-Form in jeder OpenAI-API dieselbe Art, denselben `type` und denselben `code` hat. Die OpenAI-SDKs wählen ihre Exception-Klasse nach dem Status, `type` ist also nur eine zusätzliche Information.

`retry-after-ms` oder `retry-after` wird zu `retry_after`, das als `retry-after` in ganzen Sekunden geschrieben wird, aufgerundet, dazu `retry-after-ms`, aufgerundet, wenn die Wartezeit einen Sekundenbruchteil hat. `param` wird zu `KppError.param`, wenn es ein String ist. Ein aus Chat dekodierter Fehler wird mit seinem ursprünglichen Status wieder kodiert, und mit seinem ursprünglichen Body, wenn dieser ein `error`-Objekt hat. Bodies, die Kaeso nicht als `ErrorResponse` lesen kann, etwa ein HTML-429 von einem Proxy, ein 503 als reiner Text, `{"error": "rate limited"}` oder ein Body mit einem Schlüssel, den Kaeso nicht tragen kann, bekommen ihre Art allein vom HTTP-Status (Tabelle oben; ohne Status, wie in einem Stream, `ProviderError`) und behalten den rohen Body. Ein `error` als String wird zur Meldung. Der Codec liest aus dem Text keine Bedeutung: Ein Anbieter, dessen Antwort auf einen falschen Schlüssel keinen Authentifizierungsstatus hat, nennt diese Meldung in `refused_key_messages` in `crates/kaeso-core/src/provider.rs`, und das Gateway meldet einen solchen Fehler als `Authentication` (bei SpaceX `Incorrect API key provided` mit HTTP 400). Sonst lautet die Meldung `` The provider returned an error (HTTP 503) that is not in the OpenAI error format: <body> ``, mit nach 500 Bytes gekürztem Body und in einem Stream ohne den HTTP-Teil. Ein lesbarer Body ohne Meldung bekommt `The provider returned an error without a message.`

## Modelle

`GET /v1/models` und `GET /v1/models/{model}` sind in `crates/kaeso-codecs/src/openai_models.rs` umgesetzt, das die Codecs für Chat, Responses und Embeddings gemeinsam nutzen, weil alle OpenAI-APIs Modelle in derselben Gestalt auflisten. Die Fixtures liegen in `fixtures/openai_chat/models/`, und alle drei Codecs prüfen an ihnen den Round Trip. Quellen: die Schemas `Model` und `ListModelsResponse` der Spezifikation und die Operationen `listModels` und `retrieveModel`, deren `x-oaiMeta`-Beispiele als Fixtures dienen. Die Anfrage einer App ist OpenAI, wenn sie keinen Header `anthropic-version` trägt (`dispatch::models_dialect`, siehe [Modelllisten](codec-design.md#modelllisten)); das Gateway kodiert dann mit `openai_chat`.

### Query-Parameter

`listModels` dokumentiert keine. Jeder Parameter wird prozentdekodiert als Extra `query.<name>` des dekodierenden Dialekts aufbewahrt: als String oder, wenn er sich wiederholt, als Array. Fehlerhafte Escapes sind `InvalidRequest`. OpenAI blättert nicht, deshalb ruft das Gateway `ModelList::page` ohne Standardlimit auf, und eine OpenAI-App bekommt immer die ganze Liste. Das Pfadsegment `{model}` kommt prozentkodiert an, da IDs feinabgestimmter Modelle `:` enthalten, und der Router des Gateways dekodiert es.

### `Model` ⇄ `kaeso_kpp::ModelInfo`

| Wire-Feld | KPP | Kodiert, wenn KPP keinen Wert hat |
|---|---|---|
| `id` | `id` | (erforderlich) |
| `object` (`"model"`, optional) | Form `object`, nur wenn es abweicht | `"model"` |
| `created` (Unix-Sekunden) | `created_at` | `0` |
| `owned_by` | `owned_by` | `"kaeso"`: Der Eigentümer ist unbekannt, wie bei einem von Anthropic gelisteten Modell, und das Modell wird vom Kaeso-Gateway bereitgestellt. |
| `shutdown_date` (`YYYY-MM-DD` oder `null`) | `retires_at` (dieses Datum um 00:00 UTC) | `null` |
| unbekannte Schlüssel | Extras-Felder | |

Das Gateway listet Modelle aus seinem Katalog: Es setzt `owned_by` auf die ID des Anbieters und `created_at` auf das Veröffentlichungsdatum des Modells, wenn der Katalog eines hat.

Ein Feld, das fehlend ankommt oder als `null`, wo der Standard ein Wert ist, wird in der Form `none` vermerkt und genauso geschrieben; `shutdown_date` ist in der Spezifikation optional. `object` ist in der Spezifikation Pflicht, aber OpenRouters Liste ([`GET /api/v1/models`](https://openrouter.ai/docs/guides/overview/models)) lässt es in der Liste und bei jedem Modell weg, daher wird es genauso gelesen (Fixture `providers/openrouter/models/list-recorded.json`). Ein `shutdown_date`, das kein `full-date` ist, ergibt `ProviderError`. Extras aus jedem der drei OpenAI-Dialekte werden von allen drei Encodern berücksichtigt.

### `ListModelsResponse` ⇄ `ModelList`

| Wire-Feld | KPP |
|---|---|
| `object` (`"list"`, optional) | Form `object`, nur wenn es abweicht; fehlend oder `null` in der Form `none` |
| `data` | `models` |
| unbekannte Schlüssel | Extras-Felder |
| ein bloßes Array von Modellen statt des Objekts, wie Together AI es sendet | `models`, Form `bare_list` |

Für OpenAI-Apps geht dabei etwas verloren: `display_name`, `lifecycle`, `deprecated_at`, `context_window`, `max_output_tokens`, `has_more` und Extras anderer Dialekte (Anthropic `capabilities`, `line`) haben kein Feld und werden weggelassen ([Antworten](cross-format.md#antworten)). Ein `retires_at`, das nicht auf Mitternacht UTC fällt, wird auf sein UTC-Datum gekürzt, und eines nach dem Jahr 9999 wird als `shutdown_date: null` geschrieben. `created` ist eine Ganzzahl und wird so geschrieben, wie es gespeichert ist; ein Wert in Millisekunden bleibt also einer. `encode_model_request` baut `GET /v1/models/{model}` ohne Header und Body (`retrieveModel` der Spezifikation); den Schlüssel fügt das Gateway hinzu. `encode_models_request` baut `GET /v1/models` (`listModels` der Spezifikation, das keine Query-Parameter kennt; eine Query mit `limit`, `after_id`, `before_id`, `lifecycle` oder anderen Parametern ist daher `Unsupported`). In `decode_models` und `decode_model` geht ein Status außerhalb von 2xx an `decode_error`.

## Anbieter mit diesem Format

Andere Anbieter als OpenAI, die Chat Completions bereitstellen, zeichnen in `fixtures/openai_chat/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln oben; dieser Abschnitt nennt, was jeder Anbieter hinzufügt.

### DeepSeek

Quellen: die [API-Referenz](https://api-docs.deepseek.com/api/create-chat-completion), der [Leitfaden zum Thinking-Modus](https://api-docs.deepseek.com/guides/thinking_mode/) und die [Fehlercodes](https://api-docs.deepseek.com/quick_start/error_codes) von DeepSeek, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/deepseek/`.

| DeepSeek-Feld | KPP | Hinweise |
|---|---|---|
| `thinking` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning_content`, `delta.reasoning_content` | Extras | Chat-Apps erhalten sie unverändert zurück, und ein Assistant-Turn, der eines trägt, schickt es an DeepSeek zurück. Anthropic Messages und Responses zeigen sie noch nicht als Reasoning ([Reasoning](reasoning.md)). |
| `usage.prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` | `raw` der Nutzung | Die Treffer entsprechen `prompt_tokens_details.cached_tokens`, das als Cache-Lesezugriff typisiert ist. |
| `: keep-alive`-Zeilen und leere Zeilen, solange der Server ausgelastet ist | `Extra` | Nur im selben Dialekt, wie andere SSE-Kommentare. |
| HTTP 402 | `Billing` | Ein aufgebrauchtes Guthaben, was auch immer `code` sagt. |
| HTTP 422 für eine `developer`-Nachricht | `InvalidRequest` | DeepSeek kennt die Rolle nicht; die App erhält den Fehler von DeepSeek. |
| zusätzliche Modellfelder (`name`, `context_window`, `max_output_tokens`, `input_modalities`, `output_modalities`, `effort`, `api_capabilities`) | Modell-Extras | Die Modellliste von DeepSeek hat kein `created`. |

### SpaceX

Quellen: die [Chat-Completions-Referenz](https://docs.x.ai/developers/rest-api-reference/inference/chat-completions) und die [Modellreferenz](https://docs.x.ai/developers/rest-api-reference/inference/models) von xAI, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/xai/`. Das eigene Format von SpaceX ist [OpenAI Responses](openai-responses.md#anbieter-mit-diesem-format); ein Modell erreicht Chat Completions über sein `egress_dialect`.

| SpaceX-Feld | KPP | Hinweise |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | Extras | Chat-Apps erhalten sie unverändert zurück. Anthropic Messages und Responses zeigen sie noch nicht als Reasoning ([Reasoning](reasoning.md)). |
| `citations`, `system_fingerprint`, `service_tier` | Extras | Gegenüber anderen Dialekten weggelassen. |
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
| `message.reasoning_content` oder `message.reasoning`, je nach Modell, und dasselbe in Deltas | Extras | Chat-Apps erhalten sie unverändert zurück. Anthropic Messages und Responses zeigen sie noch nicht als Reasoning ([Reasoning](reasoning.md)). |
| `matched_stop` und `seed` an einer Choice, `text` an einer Stream-Choice, `token_id` in einem Delta, `prompt`, `metadata` und `warnings` auf oberster Ebene | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.reasoning_tokens` neben `completion_tokens_details.reasoning_tokens` | `raw` der Nutzung | Die Token-Zahlen sind wie oben typisiert. |
| Fehler in der OpenAI-Hülle mit eigenen `type`- und `code`-Werten von Together AI (`start_generation_failed`, `model_not_available`), manchmal mit einer `id` auf oberster Ebene | `KppError` | Die Art folgt dem Status; ein falscher Schlüssel ist ein 401 `invalid_api_key`, also `Authentication`. |
| `GET /v1/models`: ein bloßes Array von Modellen mit `type`, `display_name`, `organization`, `context_length`, `config` und `pricing` | `ModelList`, Modell-Extras | `GET /v1/models/{model}` antwortet für IDs mit `/` mit 404, daher stützt sich eine Modellprüfung auf die Modellliste. |

Andere Anbieter als OpenAI, die Chat Completions anbieten, zeichnen in `fixtures/openai_chat/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln oben; dieser Abschnitt nennt, was jeder Anbieter hinzufügt.

### Groq

Quellen: die [API-Referenz](https://console.groq.com/docs/api-reference) und der [Reasoning-Leitfaden](https://console.groq.com/docs/reasoning) von Groq, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/groq/`.

| Groq-Feld | KPP | Hinweise |
|---|---|---|
| `reasoning_format`, `include_reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning`, `delta.reasoning`, `delta.channel` | Extras | Chat-Apps erhalten sie unverändert zurück. Anthropic Messages und Responses zeigen sie noch nicht als Reasoning ([Reasoning](reasoning.md)); ein Chunk, der nur Reasoning enthält, wird dort zu einem Keepalive. |
| `x_groq`, `usage_breakdown`, `service_tier` (`on_demand`) | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time` | `raw` der Nutzung | Die Token-Zahlen, `reasoning_tokens` eingeschlossen, sind wie oben typisiert. |
| `usage` im Chunk mit `finish_reason`, auch ohne `stream_options.include_usage` | `Usage` | Jeder Dialekt erhält die Nutzung, ob Groq zusätzlich den Nutzungs-Chunk sendet, den `include_usage` anfordert, oder nicht. |
| zusätzliche Modellfelder (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features` und weitere) | Modell-Extras | |

### OpenRouter

Quellen: die [API-Referenz](https://openrouter.ai/docs/api_reference/overview), [Streaming](https://openrouter.ai/docs/api_reference/streaming), [Fehler und Debugging](https://openrouter.ai/docs/api_reference/errors-and-debugging) und [Reasoning-Token](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens) von OpenRouter, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_chat/providers/openrouter/`, alle mit `:free`-Modellen.

| OpenRouter-Feld | KPP | Hinweise |
|---|---|---|
| `provider`, `models`, `transforms`, `usage`, `reasoning` (Anfrage) | Extras | Abgelehnt, wenn die Anfrage an einen anderen Dialekt geht, wie andere Felder, die nur Chat kennt. |
| `message.reasoning`, `message.reasoning_details` und dasselbe in Deltas | Extras | Chat-Apps erhalten sie unverändert zurück. Anthropic Messages und Responses zeigen sie noch nicht als Reasoning ([Reasoning](reasoning.md)). |
| `provider`, `native_finish_reason` an einer Antwort | Extras | Gegenüber anderen Dialekten weggelassen. |
| `usage.cost`, `is_byok`, `cost_details`, die zusätzlichen Token-Angaben | `raw` der Nutzung | Kaeso berechnet Kosten mit den Preisen des Katalogs, nicht mit `usage.cost`. |
| der letzte Stream-Chunk, der die Choice mit `finish_reason` neben `usage` wiederholt | `Usage` | Gelesen wie ein Nutzungs-Chunk mit `choices: []`. |
| `: OPENROUTER PROCESSING`-Kommentare | `Extra` | Nur im selben Dialekt, wie andere SSE-Kommentare. |
| ein `error`-Objekt nach der 200, im Body oder in einem Chunk neben `choices` | `KppError` | Siehe die [Zeilen oben](#streams-createchatcompletionstreamresponse); ein numerischer `code` bestimmt die Art wie ein Status. |
| `GET /api/v1/models`: kein `object`, Modelle ohne `object` und `owned_by`, mit `total_count` und `links` | `ModelList`, Extras | `GET /api/v1/models/{model}` antwortet für jedes Modell mit 404, daher stützt sich eine Modellprüfung auf die Modellliste, die OpenRouter ohne Blick auf den Schlüssel ausliefert. |
