---
title: OpenAI Responses
description: Wie jedes Feld des Formats OpenAI Responses auf KPP abgebildet wird und zurück.
order: 7
---

Schlagen Sie hier jedes Feld des Formats OpenAI Responses nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::openai_responses::OpenAiResponses` (`crates/kaeso-codecs/src/openai_responses/`)
- **Endpunkte:** `POST /v1/responses` (JSON und SSE), `POST /v1/responses/input_tokens` und die [Modell-Endpunkte](#modelle), die alle OpenAI-Codecs teilen
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_responses/`

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `e95c0fe615f45a19e8878af923a729948445a6bd` (die Schemanamen unten), die [Referenz zum Erstellen](https://developers.openai.com/api/reference/resources/responses/methods/create), die [Referenz der Streaming-Events](https://developers.openai.com/api/reference/resources/responses/streaming-events), die [Referenz zum Zählen der Eingabe-Token](https://developers.openai.com/api/reference/resources/responses/subresources/input_tokens/methods/count) und der [Leitfaden zu Fehlercodes](https://developers.openai.com/api/docs/guides/error-codes).

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields` des genannten Elements, gebunden an `openai_responses`. Sie gelten nur im selben Dialekt: Ein anderer Dialekt lehnt sie mit `Unsupported` ab ([Extras](cross-format.md#extras)). Verschachtelte Schlüssel verwenden Schlüsselpfade (`text.verbosity`, `summary.0.x`).
- **Form**: eine codec-eigene `Extras::form`-Markierung. Sie betrifft nur die Darstellung, und andere Dialekte ignorieren sie.
- **Unsupported**: Das Dekodieren oder `check()` schlägt mit `ErrorKind::Unsupported { feature }` fehl.

Anders als die übrigen Codecs nutzt dieser kein `wire::Ext`; der nächste Abschnitt erklärt, wie er genaue Schreibweisen stattdessen bewahrt.

## Wie genaue Schreibweisen erhalten bleiben

Jedes Wire-Objekt hat eine *kanonische* Kodierung, die nur aus typisierten KPP-Feldern und den strukturellen Form-Markierungen unter [Form-Markierungen](#form-markierungen) entsteht. Anfragen und Antworten aus anderen Dialekten bekommen genau diese Kodierung. Beim Dekodieren baut der Codec die kanonische Kodierung dessen, was er dekodiert hat, und vergleicht sie mit dem empfangenen JSON. Jeden Unterschied hält er als Override in den Extras des KPP-Elements fest, zu dem das Objekt gehört:

- `fields["<path>"]`: ein Schlüssel, den die kanonische Kodierung nicht schreibt oder mit einem anderen Wert schreibt. Das sind unbekannte Schlüssel und bekannte Schlüssel, die KPP nicht modelliert (`metadata`, `text.verbosity`, `detail: "low"`, …), Byte für Byte erhalten. In den Tabellen unten heißen sie **Extras**.
- `form["set"]["<path>"]`: dasselbe für die Darstellung, in den Tabellen unten **Form** genannt. Es enthält `null`-Werte und ausdrücklich ausgeschriebene Standardwerte (`store: false`, `background: false`, `stream: false`, `tools: []`, `include: []`, `metadata: {}`, `text: {}`, `detail: "auto"`, `truncation: "disabled"`, `service_tier: "auto"`, `text.format: {type: text}`, `text.verbosity: "medium"`, `stream_options: {}`, `stream_options.include_obfuscation`, leere `annotations`/`logprobs`/`summary`/`content`). Außerdem enthält es die Schlüssel `type`, `id`, `item_id`, `status`, `role`, `object`, `filename`, `obfuscation` und `sequence_number`. Auch Zahlen, die sich vom kanonischen Wert nur in der Schreibweise unterscheiden, gehören hierher (`1`, wo die kanonische Kodierung `1.0` schreibt).
- `form["unset"]`: Pfade, die die kanonische Kodierung schreibt, die im Original aber fehlten.

Beim Kodieren werden die Overrides nur angewendet, wenn die Extras aus `openai_responses` stammen. Typisierte Felder haben immer Vorrang. Werte werden genau verglichen, Zahlen eingeschlossen (`1` und `1.0` unterscheiden sich). Für einen Wert, den die kanonische Kodierung reproduziert, wird nie ein Override festgehalten, und ein Test prüft, dass kein typisierter Schlüssel je als Extras-Feld gespeichert wird.

Jeder Override merkt sich außerdem, was die kanonische Kodierung beim Dekodieren an seinem Pfad hatte (`form["canon"]`). Hat sich ein typisiertes Feld nach dem Dekodieren geändert, weicht der kanonische Wert an diesem Pfad ab und der veraltete Override wird übersprungen. Das geschieht etwa, wenn das Gateway `max_output_tokens` setzt, einer Anfrage, die nur aus einem Prompt besteht, Nachrichten hinzufügt oder eine Reasoning-Aufwandsstufe setzt. Eine als `1` geschriebene Zahl kommt daher als `1` zurück, solange der Wert noch 1 ist, und ein geänderter Wert wird als neuer Wert geschrieben.

Ein `type` betrifft nur die Darstellung, wo die kanonische Kodierung an diesem Pfad ein `type` hat (Schreibweisen von Part- und Item-Typen), und beim optionalen `type: "message"` einer Eingabenachricht. Ein Objekt, das die kanonische Kodierung nicht hat und das einen anderen `type` trägt (zum Beispiel `text.format: {type: "grammar", …}`), wird als Ganzes in einem Extras-Feld aufbewahrt, sodass ein anderer Dialekt es ablehnt.

Ein Schlüsselpfad kann keinen Schlüssel aufnehmen, der leer ist, `.` enthält oder mit `header:`, `@` oder `~` beginnt. Diese Schreibweisen gehören zu Schlüsselpfaden, weitergegebenen Headern (`header:openai-beta`) und Overrides mit Geltungsbereich.

Ein Objekt mit einem solchen Schlüssel wird als Ganzes verglichen. Weicht es von der kanonischen Kodierung ab, ist das ganze Objekt ein Extras-Feld an seinem eigenen Pfad. Bei einem Schlüssel auf oberster Ebene ist das der ganze Body, am leeren Pfad. Solche Schlüssel bleiben im Body: Sie werden nie als Header gesendet, und andere Dialekte lehnen sie wie jedes Extras-Feld ab.

Wie jeder Override gilt das Feld nur, solange der kanonische Wert an seinem Pfad unverändert ist. Ändert sich ein typisiertes Feld in diesem Objekt, wird das Objekt kanonisch geschrieben und diese Schlüssel fallen weg. Bei einem Schlüssel auf oberster Ebene ist das jedes typisierte Feld der Anfrage, etwa das `model`.

## Anfrage: `CreateResponse` / `TokenCountsBody`

| Wire-Feld | KPP | Anmerkungen |
| --- | --- | --- |
| `model` | `model` typisiert | Pflicht für `/v1/responses`; optional für `/v1/responses/input_tokens` (fehlt es, ist das KPP-`model` leer und wird wieder weggelassen). |
| `input` (String) | eine `Role::User`-Nachricht mit einem Textblock, typisiert | Form `input: "string"`. Andere Dialekte bekommen immer eine Item-Liste. |
| `input` (Items) | `system` + `messages` typisiert | Siehe [Eingabe-Items](#eingabe-items-inputitem). Fehlendes `input` (mit `prompt` erlaubt): keine Nachrichten; Form `unset input`. |
| `instructions` | Textblock `system[0]` typisiert | Aus anderen Dialekten wird ein einzelner Textblock zu `instructions`; mehrere Blöcke werden zu einem führenden `developer`-Nachrichten-Item mit `input_text`-Parts. Das erhält die Blockgrenzen, aber nicht den Unterschied zwischen `instructions` und einer Systemnachricht (die API behandelt beide als Developer-Anweisungen). `null`: Form. |
| `tools[]` `{type: function, name, description, parameters, strict}` (`FunctionTool`) | `ToolDef::Function` typisiert (`parameters` roh) | Andere Schlüssel (`defer_loading`, `allowed_callers`, `output_schema`, …) sind Extras des Tools. Cache-Hinweise an Tools aus anderen Dialekten werden verworfen ([Cache-Hinweise](cross-format.md#cache-hinweise)). Namen aus anderen Dialekten außerhalb von `^[a-zA-Z0-9_-]{1,64}$` werden deterministisch umgeschrieben (andere Zeichen werden zu `_`, dann folgen `_` und ein FNV-1a-Hash aus 16 hexadezimalen Stellen, insgesamt höchstens 64 Zeichen), in `EgressCtx::tool_names` festgehalten und beim Dekodieren zurückübersetzt. |
| `tools[]` `namespace` | `ToolDef::Unknown` | Nur im selben Dialekt; für Ziele ohne Namespaces werden seine Tools zu Funktions-Tools ([Tool-Namespaces](cross-format.md#tool-namespaces)). |
| `tools[]` `web_search`, `web_search_2025_08_26`, `web_search_preview`, `web_search_preview_2025_03_11` | `ToolDef::Unknown` | Nur im selben Dialekt; für Anthropic Anthropics Websuche-Tool; für SpaceX das von xAI; weggelassen für Chat Completions und wo nur eine Suche im Cache verlangt wird ([gehostete Websuche](cross-format.md#gehostete-websuche)). |
| `max_tool_calls` und `include` `"web_search_call.action.sources"` (Anfrage, aus Anthropic) | geschrieben für Anthropics `web_search_20250305` | `max_uses` wird zu `max_tool_calls`, und die Quellen werden angefordert, damit die Ergebnisse die App erreichen ([gehostete Websuche](cross-format.md#gehostete-websuche)). SpaceX bekommt stattdessen `max_turns`. |
| `tools[]` andere Typen (`file_search`, `mcp`, `custom`, …) | `ToolDef::Unknown` | Nur im selben Dialekt. |
| `tool_choice` `"auto"` / `"none"` / `"required"` | `ToolChoice::Auto` / `None` / `Required` typisiert | |
| `tool_choice` `{type: function, name}` | `ToolChoice::Tool(name)` typisiert | Mit weiteren Schlüsseln: `ToolChoice::Unknown`. |
| `tool_choice` `allowed_tools`, gehostete Tools, `mcp`, `custom`, andere Strings | `ToolChoice::Unknown` | Nur im selben Dialekt. |
| `parallel_tool_calls` | `parallel_tool_calls` typisiert | |
| `text.format` `{type: text}` | `output = Text` | Fehlen ist die kanonische Form; ein ausdrückliches `{type: text}` ist Form. |
| `text.format` `{type: json_object}` | `output = JsonObject` typisiert | |
| `text.format` `{type: json_schema, name, description, schema, strict}` | `output = JsonSchema` typisiert (`schema` roh) | Andere Schlüssel: Extras der `JsonSchemaSpec`. Ein Schema ohne Namen (Anthropic) wird mit `name: "response"` gesendet. |
| `text.format` andere Typen | Extras `text.format` | |
| `text.verbosity` | `"medium"`, der Standardwert, den GPT-5- und Codex-Clients bei jeder Anfrage senden: Form. `low`/`high`: Extras `text.verbosity`, ein Hinweis zur Ausgabelänge: Chat-Completions-Ziele, die `verbosity` annehmen, bekommen ihn, andere Dialekte lassen ihn weg ([bedingte Einträge](cross-format.md#bedingte-einträge)). | `null`: Form. |
| `reasoning.effort` | `reasoning.effort` typisiert | Aus anderen Dialekten ([Reasoning](reasoning.md)): eine ausdrückliche Aufwandsstufe unverändert; Anthropic `thinking.disabled` → `none`; ein Budget → `low` (unter 4.096), `medium` (4.096 bis 16.383), `high` (16.384 und mehr), wie unter [Aufwand und Budget](reasoning.md#aufwand-und-budget). Werte von `none` bis `max`; unbekannte Werte sind `ReasoningEffort::Other`. |
| `reasoning.summary` | `reasoning.summary` typisiert | `auto`/`concise`/`detailed`/andere. Anthropic-Messages-Anfragen mit eingeschaltetem Thinking und einem `display` außer `omitted` bekommen `summary: "auto"`, weil OpenAI lesbares Reasoning nur auf Nachfrage zurückgibt ([Lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)). Nicht für `/v1/responses/input_tokens`. OpenAI verlangt von manchen Organisationen eine Verifizierung, bevor sie Reasoning-Zusammenfassungen bekommen; bei einem solchen Anbieter-Schlüssel erreicht der Fehler von OpenAI die App unverändert. |
| `reasoning.mode` (`standard`/`pro`), `reasoning.context`, `reasoning.generate_summary` (veraltet) | Extras der `ReasoningConfig` | Nicht dasselbe Konzept wie KPP-`ReasoningMode` (Anthropic-Thinking an oder aus). |
| `reasoning` `{}` | `reasoning` = leere Konfiguration | Form `object`. |
| `include[]` `"reasoning.encrypted_content"` | `reasoning.include_encrypted` typisiert | Ohne `reasoning`-Objekt: Form `no_object` an der Konfiguration. Anfragen aus anderen Dialekten fordern es immer an, wenn sie (nach der Umrechnung der Aufwandsstufe) eine Aufwandsstufe außer `none` senden, ebenso Anthropic-Thinking `adaptive` oder `enabled` ohne Aufwandsstufe, wenn `EgressCtx::reasoning = Some(true)` gilt ([Lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)). Mit `store: false` ist das der einzige Weg, das Reasoning in der nächsten Runde zurückzugeben ([Reasoning-Blobs](reasoning.md#reasoning-blobs)). |
| `include[]` andere Werte | Extras `include` (die ganze Liste) | `message.output_text.logprobs`, `web_search_call.action.sources`, … |
| `max_output_tokens` | `sampling.max_output_tokens` typisiert | `CreateResponse` setzt ein Minimum von 16: Ein kleinerer Wert ist `InvalidRequest` (`` The request body is not a valid OpenAI Responses request: `max_output_tokens` must be at least 16. ``, Parameter `max_output_tokens`); `TokenCountsBody` hat kein solches Feld. Eine Anfrage aus einem anderen Dialekt mit kleinerem `sampling.max_output_tokens` ist in `check()` `Unsupported`, wird nie angehoben und nennt das Feld der App, etwa `max_completion_tokens` ([Sampling](cross-format.md#sampling)). |
| `temperature`, `top_p` | `sampling.temperature`, `sampling.top_p` typisiert | Die kanonische Schreibweise ist eine Gleitkommazahl (`1.0`); `1` ist Form `set` mit `canon` ([Wie genaue Schreibweisen erhalten bleiben](#wie-genaue-schreibweisen-erhalten-bleiben)) und kommt daher als `1` zurück, solange der Wert unverändert ist. Andere Dialekte bekommen `1.0`. |
| `safety_identifier` | `user` typisiert | KPP-`user` aus anderen Dialekten wird als `safety_identifier` gesendet. |
| `user` (veraltet) | `user` typisiert, wenn `safety_identifier` fehlt (Form `user_key`); sonst Extras `user` | |
| `stream: true` | `StreamMode::On { include_usage: true }` | Responses-Streams melden die Nutzung immer. `false`/`null`: Form. |
| `stream_options` | `include_obfuscation`: Form; andere Schlüssel: Extras | Obfuscation ist Füllmaterial, kein Inhalt. |
| `store` | `false`/`null`: Form; `true`: **Unsupported** (`store`, [Zustandsbehaftete Responses-Funktionen](cross-format.md#zustandsbehaftete-responses-funktionen)) | Anfragen aus anderen Dialekten werden mit `store: false` gesendet ([Speicherung und IDs bei Responses](cross-format.md#speicherung-und-ids-bei-responses)). Anfragen dieses Dialekts behalten ihre Schreibweise, ein fehlendes `store` bedeutet also weiterhin, dass OpenAI die Antwort speichert. Die Form `native` kennzeichnet eine aus diesem Dialekt dekodierte Anfrage. |
| `previous_response_id`, `conversation` | `null`: Form; jeder Wert: **Unsupported** ([Zustandsbehaftete Responses-Funktionen](cross-format.md#zustandsbehaftete-responses-funktionen)) | |
| `background` | `false`/`null`: Form; `true`: **Unsupported** ([Zustandsbehaftete Responses-Funktionen](cross-format.md#zustandsbehaftete-responses-funktionen)) | |
| `metadata`, `truncation`, `service_tier`, `top_logprobs`, `max_tool_calls`, `prompt_cache_key`, `prompt_cache_retention`, `prompt_cache_options`, `prompt`, `context_management`, `moderation`, `access_programs`, `personality` (nur Zählung), unbekannte Schlüssel | Extras | Ausgeschriebene Standardwerte und `null` sind Form ([Wie genaue Schreibweisen erhalten bleiben](#wie-genaue-schreibweisen-erhalten-bleiben)). |
| Header `openai-beta` | Extras `header:openai-beta` | Wird nur an Responses weitergegeben. |

Ein Body ist `InvalidRequest`, wenn er:

- kein JSON-Objekt ist;
- doppelte Schlüssel hat;
- kein `model` enthält (bei `/v1/responses`);
- ein bekanntes Feld mit falschem Typ hat.

Der Fehler nennt das Feld, nie eine Parser-Position, und setzt `param`: `` The request body is not a valid OpenAI Responses request: `max_output_tokens` must be a non-negative integer. ``

`check()` lehnt außerdem bei Anfragen aus jedem Dialekt ab:

- `sampling.top_k`, `seed`, `frequency_penalty`, `presence_penalty` und Stoppsequenzen, die es in der Responses-API nicht gibt ([Sampling](cross-format.md#sampling));
- unbekannte Reasoning-Modi, benannt nach ihrem Wire-Wert;
- Textdokumente, `title`/`context`/`citations` von Dokumenten und Text-Bildquellen;
- alles, was `cross::check_foreign` ablehnt (fremde Extras, unbekannte Blöcke, Tools und Tool-Auswahlen, fremde Datei-IDs, fremde Zitate; siehe [Anfragen](cross-format.md#anfragen)).

Reasoning-Blöcke, die sich nicht zurückspielen lassen, werden verworfen ([Reasoning](reasoning.md)), ebenso Cache-Hinweise aus anderen Dialekten ([Cache-Hinweise](cross-format.md#cache-hinweise)). Auch `ToolResult::is_error` wird verworfen: Es ist ein Hinweis, und der Inhalt beschreibt den Fehler ([Tool-Fehler](cross-format.md#tool-fehler)).

Eine Reasoning-Konfiguration aus einem anderen Dialekt wird als ihre Aufwandsstufe gesendet: eine ausdrückliche Aufwandsstufe unverändert, ein Budget nach der [Aufwandstabelle](reasoning.md#aufwand-und-budget), `disabled` als `none`. `adaptive` oder `enabled` ohne beides wird ohne Aufwandsstufe gesendet, sodass das Modell seine Standard-Aufwandsstufe nutzt. Embeddings-Anfragen sind `Unsupported`.

### Eingabe-Items (`InputItem`)

| Wire | KPP | Anmerkungen |
| --- | --- | --- |
| führende `system`- / `developer`-Nachrichten-Items, die nur Text und außer `type`, `role` und `id` keine Item-Daten enthalten | Blöcke von `GenerateRequest.system` | Die Form `system_items` (`[{n, role, type, parts, id}]`) und `instructions` an der Anfrage halten die Aufteilung fest; eine `id`, die nicht Kaesos eigene ist, wird zurückgeschrieben. Codex 0.164 sendet seine Anweisungen so. Ein führendes Item mit `status`, einem Bild oder einem unbekannten Schlüssel bleibt eine `Role::System`-Nachricht. |
| `message`-Item, Rolle `user` / `assistant` | Inhaltsblöcke von `Message { role: User / Assistant }` | Gestalten `EasyInputMessage`, `InputMessage` und `OutputMessage`. Item-`type`, `id`, `status` und die Schreibweise von `role`: Form an der Nachricht; `phase` und unbekannte Schlüssel: Extras der Nachricht; `phase` wird für andere Formate weggelassen ([Allowlist](cross-format.md#allowlist)). Eine KPP-Nachricht enthält höchstens ein Nachrichten-Item. |
| `message`-Item, Rolle `system` / `developer` (nicht führend) | `Message { role: System }` ([Systemnachrichten](cross-format.md#systemnachrichten)) | Kanonische Rolle `developer`; `system`: Form. |
| `content` als String oder Parts | Form `parts` an der Nachricht | Der String ist kanonisch für einen einfachen Textblock. Leere Part-Liste: Form `empty_at`. |
| `function_call` (`FunctionToolCall`) | `Block::ToolCall` in der aktuellen Assistentennachricht | `call_id` → `id`, `name`, `arguments` (ein JSON-String) → rohes JSON. Argumente, die kein JSON sind, bleiben als JSON-String mit der Form `args_string` erhalten, nach der KPP-Konvention; ein `arguments`-Wert aus einem anderen Dialekt, der ein JSON-String ist, wird als sein Text gesendet. `id`, `status`: Form; `namespace`, `caller`, unbekannte Schlüssel: Extras des Blocks. Aufruf-IDs aus anderen Dialekten mit mehr als 64 Zeichen werden auf ihre ersten 55 Zeichen, `_` und 8 hexadezimale Stellen eines FNV-1a-Hashs gekürzt, in `EgressCtx::tool_ids` festgehalten und beim Dekodieren zurückübersetzt. |
| `function_call_output` (`FunctionCallOutputItemParam`) | `Block::ToolResult` in der aktuellen Benutzernachricht | `call_id`; ein `output`-String → ein Textblock, Parts → Text-, Bild- und Dokumentblöcke (Form `parts`, wenn ein String genügen würde). `id`, `status`: Form; `name`, `namespace`, unbekannte Schlüssel: Extras. `is_error` aus anderen Dialekten wird verworfen. Leerer Inhalt aus anderen Dialekten: `output: ""`. |
| `reasoning` (`ReasoningItem`) | `Block::Reasoning` (Ursprung `openai_responses`) in der aktuellen Assistentennachricht | Eine von Kaeso erfundene ID (`rs_kaeso_…`) wird nicht behalten (Form `kaeso_id`); ein solches Item ohne `encrypted_content` wird gar nicht an den Anbieter gesendet, weil es weder eine `id` noch Inhalt hätte, mit dem OpenAI etwas anfangen kann. `id` → opake `OpenAiReasoningItemId`, `encrypted_content` → opakes `OpenAiEncryptedReasoning`, `summary[].text` → `summary`, `content[].text` → `text`; `status` und die `type`s der Parts: Form; unbekannte Schlüssel: Extras. Wird nur an `openai_responses` beim selben Anbieter zurückgespielt ([Reasoning](reasoning.md)). Ein Item, dessen `encrypted_content` ein Reasoning-Blob ist (`kpp:reasoning:v1:…`, Reasoning eines anderen Dialekts, das Kaeso der App gegeben hat, siehe [Reasoning-Blobs](reasoning.md#reasoning-blobs)), wird zum ursprünglichen Block darin dekodiert, einschließlich Ursprung (zum Beispiel Claudes signiertes Thinking), egal was der Rest des Items sagt. In Anfragen, die Kaeso mit `store: false` sendet (aus anderen Dialekten, siehe [Speicherung und IDs bei Responses](cross-format.md#speicherung-und-ids-bei-responses)), wird Reasoning ohne verschlüsselten Inhalt nicht gesendet, weil OpenAI die Item-ID nicht auflösen kann. |
| `custom_tool_call`, `custom_tool_call_output`, `local_shell_call`, `local_shell_call_output` | wie unten; für Anthropic und Chat Completions Tool-Aufrufe und -Ergebnisse ([Tool-Verlauf von Codex](cross-format.md#tool-verlauf-von-codex)) | |
| `item_reference` und jeder andere Item-Typ (Aufrufe und Ausgaben von Server-Tools, `compaction`, künftige Typen) | eine Nachricht mit einem `Block::Unknown`, Form `raw_item` | Rolle `Assistant` für Typen, die auf `_call` enden, sowie `mcp_list_tools` und `mcp_approval_request`; sonst `User`. Die Rolle zählt nur für die KPP-Validierung, weil diese Items nie an einen anderen Dialekt gehen. Nur im selben Dialekt. |
| Items, die sich nicht als ihr Typ parsen lassen (zum Beispiel ein `function_call_output` mit `call_id: null`) | `Block::Unknown` (wie oben) | Verlustfrei; der Anbieter beurteilt sie. |

Gruppierung: Aufeinanderfolgende `function_call`- und `reasoning`-Items kommen in die aktuelle Assistentennachricht, aufeinanderfolgende `function_call_output`-Items in die aktuelle Benutzernachricht. Ein Nachrichten-Item kommt in eine Nachricht seiner Rolle, die noch kein Nachrichten-Item enthält; System- und Developer-Items kommen nie in eine bestehende Nachricht.

Beim Kodieren wird jede KPP-Nachricht wieder aufgeteilt: Jede Folge von Inhaltsblöcken wird ein Nachrichten-Item, jeder andere Block ein eigenes Item. Eine Nachricht aus einem anderen Dialekt wie Anthropics `[thinking, text, tool_use, tool_use]` wird zu einem `message`-Item und zwei `function_call`-Items. Das Thinking wird verworfen ([Reasoning](reasoning.md)), außer es ist OpenAI-Reasoning, das eine Anthropic-App in einem [Reasoning-Blob](reasoning.md#reasoning-blobs) zurückgebracht hat.

`id` und `status` von Nachrichten- und Funktionsaufruf-Items (`msg_…`, `fc_…`) betreffen nur die Darstellung, sodass eine Unterhaltung mit zurückgeschickten Ausgabe-Items trotzdem an einen anderen Anbieter gehen kann. IDs von Reasoning-Items sind semantisch (opake `OpenAiReasoningItemId`), weil OpenAI sie auflöst.

### Inhaltsteile

| Wire-Part | KPP | Anmerkungen |
| --- | --- | --- |
| `input_text` (User, System, Developer, Tool-Ausgabe) | `Block::Text` | |
| `output_text` (Assistant) | `Block::Text`, `annotations[]` → `citations` (roh, an den Ursprung gebunden) | Kanonisch in Anfragen: `{type, text, annotations}`; `logprobs` im Eingabeverlauf: Extras. Der Texttyp, der nicht der Standard der Rolle ist: Form. |
| `refusal` | `Block::Refusal` | |
| `input_image` `image_url` (Data-URL `data:<type>;base64,…`) | `Block::Image`, `Source::Base64` | Data-URLs mit weiteren Parametern bleiben `Source::Url`. |
| `input_image` `image_url` (URL) / `file_id` | `Source::Url` / `Source::FileId` (an den Ursprung gebunden) | Beide oder keins gesetzt: `Block::Unknown`. Eine Bild-Datei-ID funktioniert nur mit OpenAI Responses desselben Anbieters, weil OpenAI Chat Completions keine Bild-Datei-IDs annimmt. |
| `input_image.detail`, `input_file.detail` | `"auto"`: Form; andere Werte: Extras `detail` | Bei Bildern ist `detail` ein Auflösungshinweis auf der [Extras-Allowlist](cross-format.md#extras) und wird für andere Dialekte verworfen. Bei Dateien steuert es, wie PDFs gerendert werden, und andere Dialekte lehnen es ab. |
| `input_file` `file_data` / `file_url` / `file_id` | `Block::Document`, `Source::Base64` / `Url` / `FileId` | Genau eine Quelle, und `file_data` muss eine Data-URL sein; sonst `Block::Unknown`. |
| `input_file.filename` | Form | KPP hat kein Feld für einen Dateinamen (`DocumentBlock::title` ist ein Titel), so können PDFs den Dialekt wechseln. OpenAI verlangt bei `file_data` einen Dateinamen: Dokumente aus anderen Dialekten, die als `file_data` gesendet werden, bekommen `filename: "document.<ext>"`, mit der Endung `pdf`, `txt`, `md`, `csv` oder `json` je nach Medientyp und sonst `bin`. |
| `prompt_cache_breakpoint` (`PromptCacheBreakpointConfig`) an einem Text-, Bild- oder Dateiteil | `cache: CacheHint` des Blocks (Form `breakpoint`) | Wird nur an diesen Dialekt wieder ausgegeben; Hinweise aus anderen Dialekten werden verworfen ([Cache-Hinweise](cross-format.md#cache-hinweise)). |
| andere Part-Typen | `Block::Unknown` | |

## Antwort: `Response`

| Wire-Feld | KPP | Anmerkungen |
| --- | --- | --- |
| `id`, `model` | `Response.id`, `.model` typisiert | Aus einem anderen Format die ID des Anbieters, unverändert ([IDs von Antworten](cross-format.md#ids-von-antworten)). |
| `status` `completed` | `stop = EndTurn`, oder `ToolUse`, wenn `output` einen `function_call` enthält | |
| `status` `incomplete` + `incomplete_details.reason` `max_output_tokens` / `content_filter` / anderer | `stop = MaxTokens` / `ContentFilter` / `Other(reason)` | Andere Status (`in_progress`, `cancelled`, `queued`): `Other(status)`; das genaue `status` und `incomplete_details` kommen aus den Overrides zurück. |
| `status` `failed` + `error {code, message}` | `decode_response` gibt den Fehler zurück | Art allein aus `error.code` (`ResponseErrorCode`), nach jedem Code der [Fehlertabelle](#fehler-errorresponse) und den darunter aufgeführten Codes, zum Beispiel `server_error` → `ProviderError`, `rate_limit_exceeded` → `RateLimited`, `vector_store_timeout` → `Timeout`, `invalid_prompt` → `InvalidRequest`, `cyber_policy` → `ContentFiltered`; andere oder fehlende Codes → `ProviderError`. Ohne Meldung: `The provider reported that the response failed.` Das ganze Objekt bleibt in `raw`. Es ist kein Fehler-Body, daher bekommt eine Responses-App eine `ErrorResponse` mit der Meldung sowie Status, `type` und `code` der Art. |
| `output[]` `message` (Rolle `assistant`) | Inhaltsblöcke (`output_text` → `Text` mit Zitaten, `refusal` → `Refusal`, andere Parts → `Unknown`) | Item-Grenzen: Form `messages` = `[[erster Block, Blockanzahl], …]` und `blocks` (Blockanzahl) an der Antwort, dazu Form `item` (Ausgabeindex) an jedem dekodierten Block. Die Bereiche werden nur genutzt, solange die Blockliste unverändert ist. Hat das Gateway Blöcke entfernt oder hinzugefügt, werden die Blöcke nach ihren `item`-Markierungen gruppiert, sodass IDs und Item-Overrides (Geltungsbereich `@out<i>`) bei ihrem Item bleiben und kein falsches Item entsteht. `logprobs` eines Parts: Extras des Blocks. |
| `output[]` `function_call` | `Block::ToolCall` | `id` (`fc_…`): Form am Block; auf dem Hinweg umgeschriebene Namen und IDs werden zurückübersetzt. |
| `output[]` `reasoning` | `Block::Reasoning` | Wie bei Eingabe-Items. |
| `output[]` andere Typen (`web_search_call`, …) | `Block::Unknown` | |
| `usage` (`ResponseUsage`) | `usage` typisiert | Wie unter [Nutzung](cross-format.md#nutzung) beschrieben: `input_tokens` = Wire-`input_tokens` − `cached_tokens` − `cache_write_tokens`, weil das Wire-`input_tokens` Cache-Lesezugriffe und Cache-Schreibzugriffe mitzählt; `cache_read_tokens` = `cached_tokens`; `cache_write_tokens`; `output_tokens` (einschließlich Reasoning); `reasoning_tokens`; das Objekt in `raw`. Beim Kodieren werden die typisierten Felder geschrieben und `raw` aus diesem Dialekt darübergelegt. `null`: Form. Ein Nutzungsobjekt, das Kaeso nicht lesen kann, etwa eine Zahl, die keine nicht negative ganze Zahl ist, ist `ProviderError` (im Stream ein `Error`-Event) statt einer Nutzung von null, damit eine bezahlte Antwort nie als kostenlos protokolliert wird. |
| `object`, `created_at`, `completed_at`, `error: null`, zurückgegebene Anfrageparameter (`instructions`, `tools`, `tool_choice`, `text`, `reasoning`, `temperature`, `top_p`, `store`, `truncation`, `metadata`, `service_tier`, …), unbekannte Schlüssel | Extras der Antwort (Overrides) | Die Form `native` kennzeichnet eine aus diesem Dialekt dekodierte Antwort. |

Eine Antwort aus einem anderen Dialekt wird als vollständiges `Response`-Objekt geschrieben:

- `object: "response"`, `created_at` (`EncodeCtx::created_at`), `background: false`, `completed_at` (derselbe Zeitpunkt, wenn abgeschlossen, sonst `null`) und `error: null`.
- Die zurückgegebenen Anfrageparameter stammen aus der Anfrage der App (`EncodeCtx::request`, so kodiert, wie dieser Codec sie sendet), mit den Standardwerten von OpenAI, wo nichts gesetzt ist: `instructions`, `max_output_tokens`, `max_tool_calls`, `prompt_cache_key`, `safety_identifier` und `user` sind `null`, `parallel_tool_calls: true`, `reasoning: {effort, summary}` (`null`, wo nicht gesetzt), `temperature: 1.0`, `top_p: 1.0`, `top_logprobs: 0`, `text: {format: {type: text}}`, `tool_choice: "auto"`, `tools: []`, `truncation: "disabled"`, `metadata: {}`. `previous_response_id: null`, `service_tier: "default"` und `store: false` (Kaeso speichert nichts) werden immer geschrieben.
- Stoppgründe ohne Entsprechung (`StopSequence`, `Refusal`, `PauseTurn`, `Other`) werden als `completed` gemeldet. Dabei geht Information verloren: Ein Anthropic-`pause_turn` fordert die App zum Fortsetzen auf, und eine Responses-App kann das nicht erkennen. Nachrichten-Items sind `incomplete`, wenn die Antwort es ist.
- Reasoning eines anderen Dialekts wird zu einem Reasoning-Item mit der ID `rs_kaeso_…` ([Reasoning-Blobs](reasoning.md#reasoning-blobs)). Sein Reasoning-Text ist ein `summary_text`, gefolgt von seinen Zusammenfassungen, und `encrypted_content` ist ein Reasoning-Blob des ursprünglichen Blocks. Der Blob wird gesendet, egal ob die App ihn in `include` angefordert hat. Kaeso speichert nichts, und die Agent-Schleifen der SDKs schicken Ausgabe-Items so zurück, wie sie sie bekommen haben.
- Antwortdaten, die dieser Dialekt nicht darstellen kann, werden weggelassen ([Antworten](cross-format.md#antworten)): fremde `Unknown`-Blöcke, fremde Zitate und Extras, Bilder, Dokumente und Tool-Ergebnisse.
- Websuchen von Anthropic werden zu `web_search_call`-Items ([gehostete Websuche](cross-format.md#gehostete-websuche)), und Aufrufe umbenannter Namespace-Tools bekommen ihren `namespace` zurück ([Tool-Namespaces](cross-format.md#tool-namespaces)).
- Nachrichten-, Funktionsaufruf- und Websuche-Items bekommen die IDs `msg_kaeso_…`, `fc_kaeso_…` und `ws_kaeso_…`. Anfragen erkennen diese von Kaeso erfundenen IDs und senden sie nie an einen Anbieter.

## Zählung der Eingabe-Token: `TokenCountsResource`

| Wire | KPP | Anmerkungen |
| --- | --- | --- |
| `{object: "response.input_tokens", input_tokens}` | `CountTokensResponse.input_tokens` typisiert | Andere Schlüssel: Extras. |

Die Anfrage ist `TokenCountsBody` und wird wie `CreateResponse` in `Request::CountTokens` dekodiert; der Pfad ist `/v1/responses/input_tokens`. `include`, `max_output_tokens`, `temperature`, `top_p`, `user`, `safety_identifier`, `stream` und `store` gehören nicht zu `TokenCountsBody`. Sendet eine App sie, bleiben sie Extras der Anfrage (nur im selben Dialekt); ausgeschriebene Standardwerte und `null` sind Form, und `store: true` wird wie bei `/v1/responses` abgelehnt. KPP-Zählanfragen aus anderen Dialekten werden ohne sie kodiert, da sie die Zahl der Eingabe-Token nicht ändern.

## Streams: `ResponseStreamEvent`

Es gibt einen KPP-Block pro Inhaltsteil einer Nachricht, Funktionsaufruf, Reasoning-Item und sonstigem Item. Abweichungen bei `sequence_number` und `obfuscation` sind Form.

Ein `event:`-Name, der nicht dem `type` der Daten entspricht, ist der Form-Override `~event`. Die SSE-Felder `id`, `retry` und unbekannte Felder eines dekodierten Frames sind der Form-Override `~sse` (`{"id", "retry", "other": [[name, value], …]}`). Beide haben den Geltungsbereich des Events und werden zurückgeschrieben. Alles andere, was von den kanonischen Events abweicht, ist ein Extras-Override, dessen Geltungsbereich das angehängte Event angibt (`@open`, `@close`, `@done`, `@in_progress`, `@sdone`, `@spdone`, `@rdone`, `@rpdone`, `@text`, `@e<index>`, `@a<index>-…`).

| SSE-Event | KPP |
| --- | --- |
| `response.created` | `ResponseStart` (`id`, `model`, `usage`, wenn nicht null; Form `native`) |
| `response.in_progress` | an `ResponseStart` angehängt |
| `response.output_item.added` (`message`) | an den `BlockStart` des ersten Parts angehängt (`@open`) |
| `response.content_part.added` (Nachricht: `output_text` / `refusal` / anderer) | `BlockStart` `Text` / `Refusal` / `Unknown` (Form `part`; Form `new_item`, wenn es ein Nachrichten-Item öffnet, während ein anderes offen ist) |
| `response.output_text.delta` | `BlockDelta` `Text`; `logprobs` Extras |
| `response.output_text.annotation.added` | `BlockDelta` `Citation` |
| `response.refusal.delta` | `BlockDelta` `Refusal` |
| `response.output_text.done`, `response.refusal.done` | an den `BlockStop` des Parts angehängt (`@done`); hatte der Part keine Deltas, wird sein `text` oder `refusal` zum `BlockDelta` des Parts, damit andere Dialekte ihn sehen; Responses-Apps bekommen das Event so zurück, wie es kam. `response.function_call_arguments.done` ohne Deltas wird ebenso zu den `ToolArguments` des Aufrufs. |
| `response.content_part.done` (Nachricht) | `BlockStop` |
| `response.output_item.done` (`message`) | an das nächste Lebenszyklus-Event angehängt (`@close`); bei einer Nachricht, deren Parts nie gestreamt wurden (kein `content_part.added`), wird jeder `output_text`- oder `refusal`-Part des Items zu `BlockStart`, `BlockDelta` und `BlockStop`, die beim Zurückschreiben an Responses-Apps übersprungen werden (Form `skip`). |
| `response.output_item.added` (`function_call`) | `BlockStart` `ToolCall` (`call_id`, `name`) |
| `response.function_call_arguments.delta` | `BlockDelta` `ToolArguments` |
| `response.function_call_arguments.done` | an den `BlockStop` angehängt (`@done`) |
| `response.output_item.done` (`function_call`) | `BlockStop` |
| `response.output_item.added` (`reasoning`) | `BlockStart` `Reasoning` (Item-ID als opake `OpenAiReasoningItemId`) |
| `response.reasoning_summary_part.added` | `BlockDelta` `ReasoningSummary` mit leerem Text (öffnet die Zusammenfassung `summary_index`) |
| `response.reasoning_summary_text.delta` | `BlockDelta` `ReasoningSummary` |
| `response.reasoning_summary_text.done`, `response.reasoning_summary_part.done` | an den nächsten Zusammenfassungs-Part oder den `BlockStop` angehängt (`@sdone`, `@spdone`) |
| `response.content_part.added` (Reasoning, `reasoning_text`) | `BlockDelta` `ReasoningText` mit leerem Text, Form `new_part` |
| `response.reasoning_text.delta` | `BlockDelta` `ReasoningText` |
| `response.reasoning_text.done`, `response.content_part.done` (Reasoning) | an den nächsten Part oder den `BlockStop` angehängt (`@rdone`, `@rpdone`) |
| `response.output_item.done` (`reasoning`) | `BlockDelta` `Opaque` (`encrypted_content`, falls vorhanden) + `BlockStop` |
| `response.output_item.added` / `.done` (andere Item-Typen) | `BlockStart` `Unknown` / `BlockDelta` `Unknown` (das fertige Item) + `BlockStop` |
| `response.completed`, `response.incomplete` | `ResponseDelta` (`stop`, `usage`) + `ResponseStop` (Overrides des fertigen Antwortobjekts, zum Beispiel `completed_at`). `stop` ist `ToolUse`, wenn der Stream ein `function_call`-Item angekündigt hat, auch wenn das fertige `output` es nicht aufführt: Die Responses-Route eines ChatGPT-Abos endet mit `output: []` |
| `response.failed` | `StreamEvent::Error` (Art aus `error.code`; das Event in `raw`, unverändert zurückgespielt) |
| `error` (`ResponseErrorEvent`, flaches `code`/`message`/`param`) | `StreamEvent::Error` (das Event in `raw`, unverändert zurückgespielt). `code`, `message`, `param` und `type` werden aus einem verschachtelten `error`-Objekt gelesen, wenn es eines gibt, sonst aus dem Event. Die Art ergibt sich aus `code` und `type` (`type: "error"` wird ignoriert) wie in der [Fehlertabelle](#fehler-errorresponse) ohne Status, sodass jeder aufgeführte Code entscheidet; sonst `ProviderError`. Ohne Meldung: `The provider reported an error in the middle of the stream.` |
| kein `response.created` vor dem Inhalt | ein erzeugter `ResponseStart` (leere ID und leeres Modell, Form `skip`, damit er nie zurückgeschrieben wird; das fertige Antwortobjekt trägt weiterhin die echte ID) |
| ein bekanntes Event für ein Ausgabe-Item oder einen Part, den der Stream nie angekündigt hat, oder ohne seine Pflichtfelder | `StreamEvent::Error` (`ProviderError`, ``The provider sent a `<type>` event that refers to an output item or part it never announced, or lacks required fields.``); danach wird nichts mehr dekodiert |
| `error`, unmittelbar gefolgt von `response.failed` | ein `StreamEvent::Error` (beide Events in `raw`, beide zurückgespielt) |
| alles nach `response.completed`/`.incomplete`, `response.failed` oder `error` | wird nicht dekodiert |
| alles andere (`response.queued`, `response.metadata` (die Moderations-Metadaten eines ChatGPT-Abos), `response.web_search_call.*`, `response.file_search_call.*`, `response.mcp_*`, `response.image_generation_call.*`, `response.audio.*`, künftige Events, ein `response.created` nach dem Start oder ohne lesbares `response`), SSE-Kommentare, Frames ohne `data:`-Zeile (etwa `retry: 3000` allein), leere Daten oder JSON-Daten ohne `type` | `StreamEvent::Extra` (`{event, data}` / `{comment}`), nur im selben Dialekt. Ein Frame ohne Daten hat keinen `data`-Schlüssel und wird ohne `data:`-Zeile zurückgeschrieben; leere Daten bleiben als Text erhalten (`text: true`); `id`, `retry` und unbekannte SSE-Felder des Frames bleiben als `id`, `retry` und `other` erhalten. |
| Daten, die kein JSON sind | `StreamEvent::Error` (`ProviderError`, `The provider sent a stream event that is not valid JSON: …`); danach wird nichts mehr dekodiert |
| ein begonnener Stream, der vor `response.completed`/`.incomplete`, `response.failed` oder `error` endet; ein Stream, der mitten in einem Event endet; eine Zeile, die kein UTF-8 ist; ein Frame über 16 MiB | `StreamEvent::Error` (`ProviderError`: `The provider's stream ended before the response was complete.`, `the provider's event stream ended in the middle of an event`, `the provider's event stream contains a line that is not valid UTF-8`, `the provider sent a streamed event larger than Kaeso's limit of 16777216 bytes`) |
| redundante Events an ungewöhnlicher Stelle | `StreamEvent::Extra`, und das Lebenszyklus-Event, das sie erzeugt hätte, bekommt die Form `skip` |

Das Kodieren von KPP-Events aus einem anderen Dialekt erzeugt die vollständige kanonische Folge:

- `response.created` und `response.in_progress`, jeweils mit dem Antwortobjekt samt zurückgegebenen Feldern und `usage: null`;
- `output_item.added`, wenn der erste Part eines Nachrichten-Items beginnt, und `content_part.added` beim `BlockStart` jedes Parts;
- die `.done`-Events und `content_part.done` bei jedem `BlockStop`;
- `output_item.done` für eine Nachricht, wenn der nächste Block, der kein Text ist, beginnt oder die Antwort endet;
- `reasoning_summary_*`-Part-Events um Zusammenfassungen herum, und `content_part.*` und `reasoning_text.*` um Reasoning-Text herum;
- dann `response.completed`, oder `response.incomplete` bei `MaxTokens` und `ContentFilter`, mit allen Ausgabe-Items und der Nutzung.

`sequence_number` zählt ab 0. Item-IDs werden erzeugt (`msg_kaeso_…`, `fc_kaeso_…`, `rs_kaeso_…`, `ws_kaeso_…` aus der Antwort-ID und dem Ausgabeindex). `obfuscation` wird nie erzeugt. Ein Stream ohne `ResponseStart` bekommt einen, mit der ID `resp_kaeso_<created_at>` aus `EncodeCtx::created_at`.

Blöcke, die bei `ResponseStop` noch offen sind, werden zuerst geschlossen. Nach `ResponseStop` oder `Error` lehnt der Encoder weitere Lebenszyklus-Events ab. Fremde Fehler werden zu einem `error`-Event, mit dem `code` aus der Fehlerart (siehe [Fehler](#fehler-errorresponse)).

Reasoning-Blöcke anderer Dialekte werden zu Reasoning-Items mit einer `rs_kaeso_…`-ID ([Reasoning-Blobs](reasoning.md#reasoning-blobs)). Der Reasoning-Text wird als `reasoning_summary_text` der Zusammenfassung 0 gestreamt, und Zusammenfassungen behalten ihren Index. `output_item.done` trägt den Reasoning-Blob des ganzen Blocks in `encrypted_content`.

Was ein anderer Dialekt sendet und dieser Dialekt nicht darstellen kann, etwa zusätzliche Events wie Anthropic-`ping` oder fremde unbekannte Blöcke, löst getaktete SSE-Kommentare aus: `: keepalive` ([Keepalives](cross-format.md#keepalives)). Der Encoder schreibt einen pro zusätzlichem Event und einen pro 1.024 Byte verborgenem Text. `StreamEncoder::keepalive` gibt dem Gateway denselben Frame. Es gibt kein Endsignal für den Stream.

## Schema-Eigenschaften ohne eigene Zeile

Alle übrigen Eigenschaften der abgedeckten Schemas, nach Namen:

| Eigenschaft | KPP-Ziel |
| --- | --- |
| `FunctionTool.async` | Extras des `FunctionTool` (`async`) |
| `FunctionTool.defer_loading`, `.allowed_callers`, `.output_schema` | Extras des `FunctionTool` |
| `FunctionToolCall.async`, `.namespace`, `.caller` (Eingabe- und Ausgabe-Items) | Extras des `ToolCallBlock` (`async`, …) |
| `FunctionToolCallOutputResource.created_by` (und `id`, `status`) | `created_by`: Extras des `ToolResultBlock`; `id`, `status`: Form |
| `Response.prompt_cache_diagnostics` | Extras der `Response` (`prompt_cache_diagnostics`, das ganze Objekt) |
| `Response.prompt_cache_options`, `.moderation`, `.conversation`, `.output_text` | Extras der `Response` |
| `ResponseUsage.input_tokens` | `usage.input_tokens` + `usage.cache_read_tokens` + `usage.cache_write_tokens` ([Nutzung](cross-format.md#nutzung)) |
| `ResponseUsage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens` |
| `ResponseUsage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens` |
| `ResponseUsage.output_tokens` | `usage.output_tokens` |
| `ResponseUsage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens` |
| `ResponseUsage.total_tokens` | abgeleitet (Eingabe + Ausgabe); das Originalobjekt liegt in `usage.raw` und hat für diesen Dialekt Vorrang |
| `output_index` der Stream-Events | der KPP-Block des Items (Zustand des Decoders; Nachrichten-Parts über `output_index` + `content_index`); geschrieben aus dem eigenen Zähler des Encoders, Abweichungen sind `fields`-Overrides des Events |
| `content_index` der Stream-Events | der KPP-Block des Inhaltsteils (Nachrichten-Parts) oder der Reasoning-Text-Part (`new_part`-Deltas); geschrieben aus dem Zähler des Encoders |
| `item_id` der Stream-Events | die Item-ID (Form-Overrides für IDs dieses Dialekts, sonst erzeugte IDs) |
| `sequence_number` der Stream-Events | Form-Overrides, wenn sie von 0, 1, 2, … abweichen |
| `obfuscation` der Stream-Events | Form-Override am Delta |
| `logprobs` der Stream-Events (`output_text.delta`/`.done`) | `fields`-Override am Delta / Stop-Event |
| `prompt.id`, `prompt.version`, `prompt.variables` (`Prompt`) | Extras `prompt` (das ganze Objekt; wiederverwendbare Prompts liegen auf dem Server) |
| `prompt_cache_options.ttl`, `prompt_cache_options.mode` | Extras `prompt_cache_options` (das ganze Objekt) |
| `context_management[].type`, `context_management[].compact_threshold` | Extras `context_management` (die ganze Liste) |
| `Error.misalignment` (`MisalignmentErrorDetailsResource`: `review_target`, `error_type`, `detailed_explanation`, `steer`) | `KppError.raw` (der ganze Body), zurückgespielt an Responses- und Embeddings-Apps |
| `ResponseError.misalignment` (fehlgeschlagene Antworten, `response.failed`) | `KppError.raw` (die ganze Antwort oder das ganze Event); ein `response.failed`-Event wird in Responses-Streams zurückgespielt |
| `ResponseError.code` / `.message` | `KppError.kind` (über `ResponseErrorCode`) / `.message` |
| `Error.type`, `.code`, `.param`, `.message` | `KppError.kind` (mit dem Status), `.param`, `.message`; der Body in `raw` |

## Fehler: `ErrorResponse`

`{"error": {"message", "type", "param", "code"}}` ⇄ `KppError` (`message`, `param`, `status`, `retry_after` aus `retry-after-ms` / `retry-after`, der Body in `raw`).

| Dekodieren: Status / `type` / `code` | `ErrorKind` |
| --- | --- |
| `code` `context_length_exceeded` | `ContextTooLong` |
| `type`/`code` `insufficient_quota`, `credit_balance_exhausted`, `organization_spend_limit_exceeded`, `project_spend_limit_exceeded`, `organization_usage_limit_exceeded`, `billing_hard_limit_reached`, `billing_not_active`, `subscription_sharing_usage_limit_exceeded` | `Billing` |
| 503, `service_unavailable_error`, `server_is_overloaded` | `Overloaded` |
| `content_policy_violation`, `content_filter`, `image_content_policy_violation`, `bio_policy`, `cyber_policy`, `misalignment_policy_violation` | `ContentFiltered` |
| 400, 409, 422 | `InvalidRequest` |
| 401 | `Authentication` |
| 402 | `Billing` |
| 403 | `PermissionDenied` |
| 404 | `NotFound` |
| 413 | `RequestTooLarge` |
| 429 | `RateLimited` |
| 408, 504, `vector_store_timeout` | `Timeout` |
| andere Status | `ProviderError` |
| ein Body, der kein OpenAI-Fehler ist, bei jedem Status (etwa ein HTML-429 eines Proxys) | die Art des Status nach den Zeilen oben (ein 429 eines Proxys ist `RateLimited`, ein 502 `ProviderError`), mit der Meldung ``The provider returned HTTP <status> with a body Kaeso could not read: <the body, cut to 300 bytes>``, der Body ohne Leerraum am Anfang und Ende; Status und roher Body bleiben erhalten |
| `{"error": "<message>"}`, wie SpaceX es sendet (mit `"code": "invalid-argument"` bei einem 400) | die Art des Status, mit dem String als Meldung; die Antwort von SpaceX auf einen falschen Schlüssel (`Incorrect API key provided`, in seinen `refused_key_messages`) macht das Gateway zu `Authentication`. Ein unveränderter Fehler wird in der OpenAI-Form mit seinem eigenen Status kodiert |

Die Zeilen mit `type` und `code` haben Vorrang vor dem Status. Andere Codes entscheiden hier nicht, sondern der Status: `rate_limit_exceeded`, `rate_limit_error` und `slow_down` (`RateLimited`), `server_error` (`ProviderError`), `invalid_prompt`, `invalid_request_error` und die Bild-Codes (beginnend mit `invalid_image` oder `image_` oder endend auf `_image`; `InvalidRequest`). Sie entscheiden nur, wo es keinen Status gibt: in einem `error`-Event eines Streams und im `error.code` einer fehlgeschlagenen Antwort. Ein Fehlerobjekt ohne Meldung bekommt `The provider returned HTTP <status> without a message.`

Beim Kodieren werden Body und Status von Fehlern, die aus einem OpenAI-Dialekt (Chat, Responses, Embeddings) stammen, zurückgespielt, solange sie unverändert sind. Unverändert ist ein Fehler, solange die aus seinem Body erneut dekodierte Art der Art des Fehlers entspricht und die Meldung dieselbe ist. Andere Fehler bekommen:

| `ErrorKind` | Status | `type` | `code` |
| --- | --- | --- | --- |
| `InvalidRequest`, `Unsupported` | 400 | `invalid_request_error` | `null` |
| `ContextTooLong` | 400 | `invalid_request_error` | `context_length_exceeded` |
| `ContentFiltered` | 400 | `invalid_request_error` | `content_policy_violation` |
| `Authentication` | 401 | `invalid_request_error` | `null` |
| `PermissionDenied` | 403 | `request_forbidden` | `null` |
| `NotFound` | 404 | `invalid_request_error` | `null` |
| `RequestTooLarge` | 413 | `invalid_request_error` | `null` |
| `RateLimited` | 429 | `rate_limit_error` | `rate_limit_exceeded` |
| `Billing` | 429 | `insufficient_quota` | `insufficient_quota` |
| `Overloaded` | 503 | `service_unavailable_error` | `server_is_overloaded` |
| `Timeout` | 504 | `server_error` | `timeout` |
| `ProviderError` | sein 5xx-Status außer 503, sonst 500 | `server_error` | `null` |

`retry_after` wird als `retry-after` geschrieben (ganze Sekunden, aufgerundet), dazu `retry-after-ms`, wenn es keine ganze Zahl von Sekunden ist. In Streams ist ein fremder Fehler `{"type": "error", "code": <code, or type when null>, "message", "param"}`; `code` ist dort der Code oder, wenn er `null` ist, der `type`.

## Form-Markierungen

| Markierung | An | Bedeutung |
| --- | --- | --- |
| `set`, `unset` | jedem Element | Overrides der Darstellung ([Wie genaue Schreibweisen erhalten bleiben](#wie-genaue-schreibweisen-erhalten-bleiben)). |
| `input` | Anfrage | `input` war ein String. |
| `system_items`, `instructions` | Anfrage | Wie sich `system` auf `instructions` und führende System- und Developer-Items aufteilt. |
| `user_key` | Anfrage | Die ID des Endbenutzers kam aus `user`. |
| `object`, `no_object` | Reasoning-Konfiguration | Das `reasoning`-Objekt war leer / fehlte. |
| `parts` | Nachricht, Tool-Ergebnis | Der Inhalt war eine Part-Liste, obwohl ein String genügt hätte. |
| `empty_at` | Nachricht | Blockindex eines leeren Nachrichten-Items. |
| `raw_item` | Nachricht | Die Nachricht ist ein einzelnes unbekanntes Item. |
| `args_string` | Tool-Aufruf | `arguments` war kein JSON; der rohe Wert ist ein JSON-String. |
| `breakpoint` | Cache-Hinweis | Es war ein ausdrücklicher `prompt_cache_breakpoint`. |
| `native` | Anfrage | Aus diesem Dialekt dekodiert (es wird kein `store: false` hinzugefügt). |
| `canon` | jedem Element | Kanonische Werte an den Override-Pfaden beim Dekodieren, für die Prüfung auf veraltete Overrides. |
| `native`, `messages`, `blocks` | Antwort | Aus diesem Dialekt dekodiert; Blockbereiche der Nachrichten-Items; Blockanzahl. |
| `item` | Block einer Antwort | Index des Ausgabe-Items, aus dem der Block stammt. |
| `kaeso_id` | Reasoning-Block | Seine von Kaeso erfundene Item-ID wurde verworfen. |
| `new_item`, `part`, `new_part`, `skip`, `~event`, `~sse` | Stream-Events | Siehe [Streams](#streams-responsestreamevent). |

## Modelle

`GET /v1/models` und `GET /v1/models/{model}` haben in jeder OpenAI-API dieselbe Gestalt. Dieser Codec kodiert und dekodiert sie mit der gemeinsamen Implementierung, die auf der Seite [OpenAI Chat Completions](openai-chat.md) beschrieben ist (Fixtures `fixtures/openai_chat/models/`). Dekodierte Extras tragen den Ursprung `openai_responses`.

## Nutzung eines ChatGPT-Abos

`openai_responses::for_chatgpt_plan` schreibt einen `GenerateRequest` für die Responses-Route um, die über Sign in with ChatGPT ein ChatGPT-Abo bedient ([Einschränkungen der Vorschau](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)). Das Gateway ruft es für einen Anbieter-Schlüssel auf, der ein ChatGPT-Konto ist.

- Es lässt `background`, `conversation`, `max_output_tokens`, `max_tool_calls`, `metadata`, `moderation`, `multi_agent`, `prompt`, `prompt_cache_retention`, `safety_identifier`, `temperature`, `top_logprobs`, `top_p`, `truncation`, `user` und `previous_response_id` weg. Das betrifft die typisierten Felder, die Extras-Felder und die ausdrücklichen Standardwerte und Nullwerte in den Formen `set`, `unset` und `canon`.
- Es lässt das `max_uses` eines Websuche-Tools von Anthropic weg, das zu `max_tool_calls` würde ([gehostete Websuche](cross-format.md#gehostete-websuche)).
- Es setzt `stream: true` und `store: false`.
- Es entfernt die Form `input`, sodass `input` ein Array ist. Hat ein führendes Item die Rolle `system`, entfernt es auch die Formen `system_items` und `instructions`, sodass führender Systemtext `instructions` wird; führende `developer`-Items, wie Codex sie sendet, behalten ihre Gestalt und IDs. Jede andere Systemnachricht verliert ihre `role`-Überschreibung und wird als `developer`-Nachricht geschrieben, weil die Route `system`-Items ablehnt.

Weil die Route nur streamt, fasst das Gateway ihren Stream zu einer Antwort zusammen, wenn die Anfrage der App nicht streamt. Ein Item, das KPP nicht abbildet, etwa `web_search_call`, ist dann das Item, das `response.output_item.done` gesendet hat (`dispatch::finished_item`). Die `response.*`-Events, die über ihre `item_id` den Fortschritt eines Items melden, etwa `response.web_search_call.searching`, bleiben aus der Antwort heraus, weil das fertige Item enthält, was sie melden (`dispatch::reports_item_progress`).

## Anbieter mit diesem Format

Andere Anbieter als OpenAI, die Responses bereitstellen, zeichnen in `fixtures/openai_responses/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln oben; dieser Abschnitt nennt, was jeder Anbieter hinzufügt.

Jeder Anbieter in `PROVIDERS` nennt in seinem Feld `responses` ein `kaeso_codecs::openai_responses::ResponsesProfile`, und das Gateway übergibt es mit `EgressCtx::with_responses_profile` dem Encoder. `ResponsesProfile::OPENAI`, der Standard, sendet Anfragen so, wie die App sie geschrieben hat. `ResponsesProfile::compatible(name)` unterscheidet sich nur in `tool_namespaces: false` ([Tool-Namespaces](cross-format.md#tool-namespaces)); eigene Anbieter bekommen `compatible`, da nur OpenAI Namespaces dokumentiert.

| Einstellung | Was der Encoder tut |
|---|---|
| `xai_web_search()` | Sendet die gehostete Suche als `web_search`-Tool von xAI ([gehostete Websuche](cross-format.md#gehostete-websuche)). |
| `without_null_reasoning_content()` | Lässt ein ausdrückliches `content: null` eines Reasoning-Items weg, das die App zurücksendet. |

### SpaceX

Quellen: die [Responses-Referenz](https://docs.x.ai/developers/rest-api-reference/inference/responses) und der [Vergleich mit Chat Completions](https://docs.x.ai/developers/model-capabilities/text/comparison) von xAI, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_responses/providers/xai/`. Responses ist der `generation`-Dialekt von SpaceX.

| SpaceX-Feld | KPP | Hinweise |
|---|---|---|
| `reasoning`-Items mit `summary` und `encrypted_content`, das `grok-4.3` immer sendet | wie bei OpenAI | Ein `encrypted_content`, das ein anderer Anbieter geschrieben hat, lehnt SpaceX mit einem 400 `invalid-argument` ab. |
| `web_search_call`-Items, `url_citation`-Annotationen | wie bei OpenAI | |
| `namespace`-Tools (Anfrage) | zu Funktions-Tools umbenannt | SpaceX lehnt sie mit 422 ab (`unknown variant namespace`); sein Profil ist `compatible` ([Tool-Namespaces](cross-format.md#tool-namespaces)). |
| `web_search`-Tool (Anfrage) | das Tool von xAI (`xai_web_search()`) | SpaceX lehnt `external_web_access` ab (`Argument not supported: external_web_access`) und sucht immer live ([gehostete Websuche](cross-format.md#gehostete-websuche)). |
| `content: null` an einem zurückgesendeten `reasoning`-Item (Anfrage) | weggelassen (`without_null_reasoning_content()`) | Codex sendet seine früheren Reasoning-Items so zurück. Mit dem `null` kann SpaceX den `encrypted_content` des Items nicht lesen (`Could not decode the compaction blob`). |
| `usage.num_sources_used`, `num_server_side_tools_used`, `server_side_tool_usage_details`, `cost_in_usd_ticks`, `context_details` | `raw` der Nutzung | Responses-Apps erhalten sie zurück; die Token-Zahlen sind wie oben typisiert. |
| `presence_penalty` (Anfrage) | Extras | SpaceX ignoriert es und gibt `0.0` zurück. |
| `max_output_tokens` (Anfrage) | `sampling.max_output_tokens` | SpaceX wendet es nur auf die sichtbare Ausgabe an, nicht auf Reasoning-Token (Responses-Referenz), wie bei [Chat Completions](openai-chat-providers.md#spacex). |
| `{"error": "<message>"}` | `KppError` | Siehe die Fehlertabelle oben. |
