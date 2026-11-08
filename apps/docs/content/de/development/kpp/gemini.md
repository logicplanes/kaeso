---
title: Gemini API
description: Wie jedes Feld der Gemini API auf KPP abgebildet wird und zurück, für Generierung, Token-Zählung, Embeddings und Modelllisten.
order: 9
---

Schlagen Sie hier jedes Feld der Gemini API nach: wohin es in KPP gehört und woher seine Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie der Codec das Feld liest oder schreibt.

- **Codecs:** `kaeso_codecs::gemini::Gemini` (`crates/kaeso-codecs/src/gemini/`) und `kaeso_codecs::gemini_embeddings::GeminiEmbeddings` (`crates/kaeso-codecs/src/gemini_embeddings/`)
- **Endpunkte:** `POST {models}/{model}:generateContent`, `:streamGenerateContent?alt=sse`, `:countTokens`, `:embedContent` und `:batchEmbedContents` sowie die [Modell-Endpunkte](#modelle)
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini/` und `crates/kaeso-codecs/fixtures/gemini_embeddings/`
- **Bildanfragen:** Apps der Images API von OpenAI erreichen Bildmodelle von Gemini über diesen Codec, als Aufrufe von `generateContent` ([Bildmodelle von Gemini](openai-images.md#bildmodelle-von-gemini))

**Quellen.** Die Abbildungen folgen diesen Quellen:

- Das Discovery-Dokument der Gemini API, `https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta`, Revision `20261106`. Die Schemanamen unten beziehen sich darauf.
- Die [Dokumentation der Gemini API](https://aistudio.google.com/docs), gelesen am 2026-11-08: Textgenerierung, Thinking, Thought Signatures, Function Calling, strukturierte Ausgabe, Token-Zählung, Embeddings, Dateieingabe, Audioverständnis, Videoverständnis, Modelle und Fehlerbehebung.
- Das offizielle Python-SDK `google-genai` 2.32.0, das die [SDK-Konformitätssuite](../testing.md#sdk-konformität) gegen die Encoder laufen lässt.

## Lesart der Tabellen

- **KPP**: das typisierte Feld, auf das der Wire-Wert abgebildet wird.
- **Felder**: roh in `Extras::fields` aufbewahrt, gebunden an `gemini` (oder `gemini_embeddings`). Andere Dialekte lehnen sie mit `Unsupported` ab ([Extras](cross-format.md#extras)). Verschachtelte Objekte nutzen Schlüsselpfade mit Punkt, etwa `generationConfig.imageConfig`.
- **Form**: eine Codec-eigene Darstellungsmarkierung in `Extras::form`, die andere Dialekte ignorieren. `verbatim` meint den allgemeinen Mechanismus von `wire::Ext` (Schreibweisen `null`, `false` und `[]`).
- **roh**: ganz in einem `RawItem` oder `RawUsage` aufbewahrt.
- **Eigene** Daten haben Extras oder einen Ursprung `gemini`; **fremde** Daten kommen aus einem anderen Dialekt.

## Pfade

Eine Gemini-Anfrage nennt ihr Modell und ihre Methode im Pfad. Der Codec baut den Pfad daher aus der Anfrage und dekodiert eine Anfrage mit beidem.

| Teil | Regel |
|---|---|
| Modellpfad | `GeminiProfile::models_path` in `EgressCtx::gemini_profile`. `GeminiProfile::GOOGLE_AI_STUDIO` ist `/v1beta/models`; `with_models_path` setzt einen anderen pro Anfrage, etwa `/v1/projects/{project}/locations/{location}/publishers/google/models`. |
| Modell | Ein prozentkodiertes Pfadsegment, `:` eingeschlossen. Ein führendes `models/` ist der Ressourcenname desselben Modells und wird entfernt; `tunedModels/<id>` adressiert ein feinabgestimmtes Modell neben dem Modellpfad. `.` und `..` sind `InvalidRequest`, wie beim [Abrufen eines Modells](codec-design.md#modelllisten). |
| Methode | `GeminiAction`: `generateContent` und `streamGenerateContent` (`RequestKind::Generate`, Streaming entscheidet die Methode), `countTokens` (`CountTokens`), `embedContent` und `batchEmbedContents` (`Embed`, der Dialekt `gemini_embeddings`). `streamGenerateContent` hängt `?alt=sse` an. |
| Dekodieren | `dispatch::decode_gemini_request(model, action, body, headers)` oder `decode_checked_gemini_request` für einen Body, den `limits::Checked` gezählt hat. `gemini::split_model_action` teilt ein Segment wie `gemini-3.5-flash:streamGenerateContent`. `dispatch::decode_request` ohne Pfad dekodiert einen Generierungs-Body mit leerem Modell. Das Gateway bedient Gemini-Apps unter `/v1beta/models/{model}:{method}` mit beiden (`crates/kaeso-server/src/model_api/gemini.rs`). |
| Rolle an jedem Content | `GeminiProfile::role_on_every_content`: Ein Content, den eine App ohne `role` sendet, bekommt `user`. Bei Google AI Studio aus, bei Gemini Enterprise Agent Platform an. |

## Anfragen

Schema `GenerateContentRequest`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `contents[]` (`Content`) | `messages[]` | Eine Nachricht pro Content; aufeinanderfolgende Contents derselben Rolle bleiben wie geschrieben. Unbekannte Schlüssel → Felder der Nachricht. |
| `contents[].role` `user` / `model` | `Role::User` / `Role::Assistant` | Fehlt → `Role::User`, Form `role: "absent"`. Jede andere Rolle → eine Nutzernachricht, die den ganzen Content als einen unbekannten Block hält und über Dialekte hinweg als `` `<role>` message `` abgelehnt wird ([Anfragen](cross-format.md#anfragen)). Gemini beantwortete einen Content mit der Rolle `assistant` (`requests/role-assistant.json`). |
| `systemInstruction` | `system` | Seine Parts als Blöcke; `role` → Form `systemInstruction.role`; unbekannte Schlüssel → Felder `systemInstruction.*`. Die Systemanweisung hält nur Text, daher ist ein fremder Systemblock ohne Text `Unsupported`. |
| `tools[]` nur mit `functionDeclarations` | `tools[]` als `ToolDef::Function` | Der Index des Tools jeder Deklaration → Form `group`, damit eine Kodierung im selben Dialekt die Gruppierung behält. Fremde Tools kommen in ein Tool. |
| `tools[]` mit einem anderen Schlüssel (`googleSearch`, `urlContext`, `codeExecution`, `googleMaps`, `fileSearch`, `computerUse`, `mcpServers`, `googleSearchRetrieval`) oder mit leeren `functionDeclarations` | `ToolDef::Unknown` (roh) | Über Dialekte hinweg abgelehnt. |
| `functionDeclarations[].name`, `.description` | `name`, `description` | |
| `functionDeclarations[].parametersJsonSchema` | `parameters` | Volles JSON Schema. Fremde Tools bekommen immer `parametersJsonSchema`, sodass nichts an die OpenAPI-Teilmenge verloren geht. |
| `functionDeclarations[].parameters` (OpenAPI-`Schema`) | `parameters` | Form `schema: "parameters"`, sodass Gemini es in `parameters` zurückbekommt. Andere Dialekte bekommen es wie geschrieben als JSON Schema; ein Anbieter, der übliche Typnamen verlangt, lehnt großgeschriebene wie `OBJECT` ab. Mit `parametersJsonSchema` daneben → Feld `parameters`. |
| `functionDeclarations[].response`, `.responseJsonSchema`, `.behavior` | Felder des Tools | |
| `toolConfig.functionCallingConfig` `mode: AUTO` / `NONE` / `ANY` | `tool_choice` `Auto` / `None` / `Required` | |
| `mode: ANY` mit einem Eintrag in `allowedFunctionNames` | `ToolChoice::Tool(name)` | |
| `mode: VALIDATED`, `ANY` mit mehreren Namen, jede andere Kombination | `ToolChoice::Unknown` (roh) | Über Dialekte hinweg abgelehnt. |
| `toolConfig.retrievalConfig`, `.includeServerSideToolInvocations` | Felder `toolConfig.*` | |
| `generationConfig.temperature`, `.topP`, `.topK`, `.maxOutputTokens`, `.stopSequences`, `.seed`, `.presencePenalty`, `.frequencyPenalty` | `sampling.*` | Zahlen behalten ihre Schreibweise im Dialekt (Form `integers`). |
| `generationConfig.candidateCount` | Form `candidate_count` | Über 1 → `Unsupported` beim Dekodieren, wie `n` bei Chat ([Sampling](cross-format.md#sampling)). |
| `responseMimeType: application/json` mit `responseJsonSchema` | `output = JsonSchema` | |
| `responseMimeType: application/json` mit `responseSchema` | `output = JsonSchema` | Form `schema: "responseSchema"`; andere Dialekte bekommen das OpenAPI-Schema wie geschrieben. |
| `responseMimeType: application/json` allein | `output = JsonObject` | |
| Jeder andere `responseMimeType` und ein Schema ohne `application/json` | Felder `generationConfig.*` | |
| `generationConfig.thinkingConfig` | `reasoning` | Siehe [Thinking](#thinking). |
| `generationConfig.imageConfig`, `.mediaResolution`, `.responseLogprobs`, `.logprobs`, `.audioTranscriptionConfig`, `.translationConfig`, `.enableAffectiveDialog`, `.enableEnhancedCivicAnswers`, `.responseFormat` | Felder `generationConfig.*` | |
| `generationConfig.responseModalities` | `output_modalities` | `TEXT`, `IMAGE`, `AUDIO` ↔ `Text`, `Image`, `Audio`; andere Werte wie geschrieben; `[]`: Form `modalities_empty`. Aus anderen Dialekten: `["AUDIO"]`, wenn sie Audio verlangen, weil `["text", "audio"]` einer Chat-App Audio mit Transkript bedeutet und Geminis Sprachmodelle nur ein genaues `["AUDIO"]` beantworten. |
| `generationConfig.speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName` oder `.voiceConfig.voice` | `audio_output.voice` | Form `voice` nennt die Schreibweise; `languageCode`, `multiSpeakerVoiceConfig` und andere Schlüssel: Extras von `audio_output`, über Dialekte hinweg abgelehnt. Aus anderen Dialekten: eine vorgefertigte Stimme aus dem Sprach-Leitfaden (Zephyr, Puck, Charon, Kore und 26 weitere), sonst `Unsupported`. |
| `generationConfig.responseFormat` nur mit `audio.mimeType` `AUDIO_WAV`, `AUDIO_MP3` oder `AUDIO_L16` (bei 24 kHz) | `audio_output.format` `Wav`, `Mp3`, `Pcm16` | Jedes andere `responseFormat`, etwa `AUDIO_MULAW` oder ein Bildformat: Feld `generationConfig.responseFormat`. Aus anderen Dialekten: `flac`, `opus` und `aac` sind `Unsupported`; nichts wird umkodiert. |
| `safetySettings`, `cachedContent`, `labels`, `serviceTier`, `store`, `continuationToken` | Felder | |
| `model` (in einem Body) | Form `model` | Der Pfad entscheidet über das Modell. |

### Parts

Schema `Part`. Ein Part trägt genau eines von `text`, `inlineData`, `fileData`, `functionCall` und `functionResponse`; jeder andere Part bleibt ganz erhalten.

| Wire-Part | KPP | Anmerkungen |
|---|---|---|
| `text` | `Block::Text` | `thought: false` → Form `thought`. |
| `text` mit `thought: true` | `Block::Reasoning` | Sein Text und seine `thoughtSignature` als opake Daten. Ein leerer Thought-Part → Form `thought_part`. |
| `thoughtSignature` an jedem anderen Part | `Block::Reasoning` vor dem Block des Parts | Siehe [Thought Signatures](#thought-signatures). |
| `inlineData` `image/*` | `Block::Image`, Base64 | `displayName` → Felder `inlineData.*`. |
| `inlineData` `audio/*` / `video/*` | `Block::Audio` / `Block::Video`, Base64 | Der MIME-Typ wie gesendet (`audio/mp3`, `audio/mpeg`, `video/mp4`, …). |
| `inlineData` eines anderen Typs | `Block::Document`, Base64 | |
| `fileData` mit einer URI der Files API (`files/…` oder `https://generativelanguage.googleapis.com/v1beta/files/…`) | Bild oder Dokument, `Source::FileId` | Nur bei Gemini nutzbar ([Dateien](cross-format.md#dateien)). `mimeType` → Form `mime_type`. |
| `fileData` mit einer anderen URI | Bild, Audio, Video oder Dokument, `Source::Url` | Nach `image/*`, `audio/*` oder `video/*` in `mimeType`. Eine YouTube-URL ohne `mimeType` ist ein Video (`requests/video-youtube.json`). |
| `functionCall` | `Block::ToolCall` | Siehe [Funktionsaufrufe](#funktionsaufrufe). |
| `functionResponse` | `Block::ToolResult` | Siehe [Funktionsaufrufe](#funktionsaufrufe). |
| `executableCode`, `codeExecutionResult`, `toolCall`, `toolResponse`, unbekannte Parts, Parts mit zwei Datenfeldern | `Block::Unknown` (roh) | Der Part behält seine eigene `thoughtSignature`. Über Dialekte hinweg abgelehnt. |
| `videoMetadata`, `mediaResolution`, `partMetadata`, `mediaProcessing` und andere Schlüssel eines Parts | Felder des Blocks | Ausschnitt und Bildrate ändern, was das Modell sieht, darum lehnen andere Dialekte sie ab; kein anderes Format hat sie. |

### Funktionsaufrufe

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `functionCall.id` | `ToolCall.id` | Fehlt → `kaeso_fc_<seed>_<n>`, Form `no_id`. Der Seed ist die `responseId` der Antwort, in einer Anfrage der Index von Content und Part. IDs mit diesem Präfix fallen für Gemini wieder weg. |
| `functionCall.name`, `.args` | `name`, `arguments` (roh) | `args` fehlt → `{}`, Form `no_args`. |
| `functionResponse.id` | `ToolResult.call_id` | Fehlt → der früheste offene Aufruf mit demselben Namen, Form `no_id`. |
| `functionResponse.name` | — | Aus dem Aufruf abgeleitet; ein anderer Name → Form `name`. |
| `functionResponse.response` `{"output": "<text>"}` | ein Textblock | Die dokumentierte Konvention für Ausgaben (`FunctionResponse.response`). |
| `functionResponse.response` `{"error": "<text>"}` | ein Textblock, `is_error` | Die dokumentierte Konvention für Fehler. |
| `functionResponse.response`, jedes andere Objekt | ein Textblock mit dem kompakten JSON | Form `response: "json"`: Gemini bekommt das Objekt zurück; andere Dialekte lesen das JSON als Text. |
| `functionResponse.parts[]` | Bild- und Dokumentblöcke nach dem Text | Audio-, Video- und unbekannte Parts bleiben roh: Function Responses nehmen nur Bilder und Dokumente. |
| `functionResponse.willContinue`, `.scheduling` | Felder `functionResponse.*` | |

### Anfragen aus anderen Dialekten

- **Systemnachrichten** nach der ersten Runde werden Teil der nächsten Nutzer-Runde, wie bei [Anthropic](cross-format.md#systemnachrichten); führende werden Teil von `systemInstruction`.
- **Tool-Ergebnisse** aufeinanderfolgender Nutzernachrichten, wie Chat Completions sie sendet, kommen in einen Content. So werden parallele Aufrufe als `FC1, FC2, FR1, FR2` beantwortet; die Anleitung zu Thought Signatures beantwortet verschachtelte Aufrufe und Ergebnisse mit einem 400.
- **Function Responses** nehmen ihren Namen vom Aufruf mit ihrer ID. Ein Ergebnis, dessen Aufruf nicht in der Anfrage steht, ist `InvalidRequest`. Text wird zu `{"output": …}` und ein Fehler zu `{"error": …}`, ihre Textblöcke aneinandergefügt; Bilder und Dokumente kommen in `functionResponse.parts`, die Gemini-3-Modelle annehmen.
- **IDs** anderer Dialekte werden als `functionCall.id` und `functionResponse.id` gesendet.
- **Argumente**, die kein Objekt sind, werden zu einem, wie bei Anthropic ([Argumente von Tool-Aufrufen](cross-format.md#argumente-von-tool-aufrufen)).
- **Bild-URLs** bekommen `fileData.mimeType` aus der Dateiendung der URL (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.heic`, `.heif`; Dokumente auch `.pdf`, `.txt`), wie es die Anleitung zur Dateieingabe verlangt; eine Bild-URL ohne solche Endung ist `Unsupported`. Textdokumente werden als Base64-`inlineData` gesendet.
- **Audio- und Video-URLs** bekommen ihn genauso, aus den unterstützten Formaten der Audio- und Video-Leitfäden (`.wav`, `.mp3`, `.aiff`, `.aac`, `.ogg`, `.flac`, `.m4a`, `.opus`, `.webm`; `.mp4`, `.mpeg`, `.mpg`, `.mov`, `.avi`, `.flv`, `.webm`, `.wmv`, `.3gp`). Eine YouTube-URL geht ohne; eine andere URL ohne bekannte Endung ist `Unsupported`. Base64-Audio und -Video gehen als `inlineData` mit ihrem Medientyp. Audio und Video in einem Tool-Ergebnis sind `Unsupported`.
- **Abgelehnt** mit `Unsupported`: Refusal-Blöcke, für die Gemini keinen Part hat (wie bei Anthropic), Titel, Kontext oder Zitate eines Dokuments, mehr als fünf Stoppsequenzen (`GenerationConfig.stopSequences`), `parallel_tool_calls: false`, ein Ausgabeschema mit Beschreibung oder `strict: false` sowie ein Aufwand oder Thinking-Modus, für den Gemini keine Einstellung hat.
- **Strikte Tools.** Ein Function-Tool mit `strict: true` und automatischem `tool_choice` sendet `mode: VALIDATED`, das Aufrufe gegen ihre Schemas prüft.
- **Gehostete Websuche** wird [weggelassen](cross-format.md#weggelassene-tools).
- **Cache-Hinweise** entfallen: Gemini cacht implizit.
- **Audioantworten** anderer Dialekte erreichen Gemini-Apps als `inlineData` mit ihrem Medientyp; ein Transkript hat dort keinen Platz und wird weggelassen.
- **`user`** entfällt: Gemini hat kein Feld für Endnutzer.

### Anfragen von google-genai

`google-genai` 2.32.0 schreibt einige Schlüssel in Snake Case, und die API nimmt sie an, weil Proto-JSON den ursprünglichen Namen jedes Felds akzeptiert: `mime_type` und `file_uri` in `inlineData` und `fileData`, `include_thoughts`, `thinking_budget` und `thinking_level` in `thinkingConfig` sowie `parameters_json_schema` und `response_json_schema` in `functionDeclarations`. `gemini::sdk` liest sie unter ihren Namen in Camel Case und setzt die Form `sdk_names`. Eine Anfrage, die an Gemini geht, bekommt sie so zurück, wie die App sie geschrieben hat.

Das SDK sendet `topK` außerdem als Gleitkommazahl, etwa `20.0`. KPP liest eine ganzzahlige Gleitkommazahl als ganze Zahl, mit Form `top_k_float`, und schreibt sie für Gemini wieder als Gleitkommazahl; ein Bruch ist `InvalidRequest`. Die Fixture `requests/sdk-tools-snake-case.json` stammt von `google-genai` 2.32.0.

### Thinking

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `thinkingLevel` `MINIMAL` / `LOW` / `MEDIUM` / `HIGH` | `reasoning.effort` | Andere Schreibweisen → Form `level`; unbekannte Werte → `ReasoningEffort::Other`. |
| `thinkingBudget: 0` | `reasoning.mode = Disabled` | |
| `thinkingBudget: -1` | `reasoning.mode = Adaptive` | Dynamisches Thinking. |
| `thinkingBudget` über 0 | `mode = Enabled`, `budget_tokens` | Andere negative Werte → Reasoning-Feld `thinkingBudget`. |
| `includeThoughts: true` | `reasoning.summary = Auto` | `false` → Form `include_thoughts`. |

Aus anderen Dialekten werden die Aufwände `minimal`, `low`, `medium` und `high` zu `thinkingLevel`, `xhigh` und `max` zu `HIGH`. Ein Budget wird zu `thinkingBudget`, das Gemini 3 auf die nächstliegende Stufe abbildet. Der Aufwand `none` und ausgeschaltetes Thinking werden zu `thinkingBudget: 0`, adaptives oder eingeschaltetes Thinking ohne beides zu `-1`. `includeThoughts: true` geht mit, wenn die App eine Reasoning-Zusammenfassung verlangt hat oder Anthropic-Thinking, das nicht `omitted` ist. [Reasoning](reasoning.md#aufwand-und-budget) hat die Regeln.

### Thought Signatures

Gemini signiert sein Thinking und setzt die Signatur auf einen Part: auf einen Funktionsaufruf, auf Text, auf ein Bild oder am Ende eines Streams auf einen leeren Text-Part ([Thought Signatures](https://aistudio.google.com/docs/thought-signatures)).

- **Dekodieren.** Eine `thoughtSignature` wird zu `Opaque { kind: GeminiThoughtSignature }`. An einem Part mit `thought: true` bleibt sie im Reasoning-Block dieses Parts; an jedem anderen bekannten Part wird sie zu einem eigenen Reasoning-Block mit dem Ursprung `gemini`, ohne Text und nur mit der Signatur, direkt vor dem Block des Parts.
- **Kodieren.** Ein solcher Block nur mit Signatur setzt seine Signatur auf den nächsten Part, den Kaeso schreibt, oder auf einen leeren Text-Part, wenn keiner folgt. Signaturen gehen nur an Gemini; Reasoning anderer Dialekte entfällt ([Reasoning](reasoning.md)).
- **Über Apps hinweg.** Apps von Anthropic und Responses bekommen den Block in einem [Reasoning-Blob](reasoning.md#reasoning-blobs). Apps von Chat Completions bekommen nichts, und das Gateway bewahrt die Signatur pro Tool-Aufruf über `continuity::reasoning_for_tool_calls` und `ReasoningCollector` auf, wie es Anthropic-Signaturen aufbewahrt ([vom Gateway aufbewahrtes Reasoning](reasoning.md#vom-gateway-aufbewahrtes-reasoning)).
- **Platzhalter-Signatur.** Ein fremder Funktionsaufruf, der der erste Aufruf eines Model-Contents in der aktuellen Runde ist, also nach dem letzten Nutzer-Content mit mehr als Function Responses, bekommt die dokumentierte `"skip_thought_signature_validator"`, wenn ihm keine Signatur vorausgeht. `EgressCtx::adapted` nennt sie als `thought signature of tool call <id>`. Geminis eigene Aufrufe werden wie empfangen gesendet.

## Antworten

Schema `GenerateContentResponse`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `responseId` | `id` | Fehlt → Form `no_response_id`. |
| `modelVersion` | `model` | Fehlt → Form `no_model_version`. |
| `candidates[0].content.parts` | `blocks` | Wie in Anfragen. Bild-Parts werden zu `Block::Image`. |
| Mehr als ein Candidate | `Unsupported` | |
| `candidates[0].finishReason` | `stop` | Siehe [Finish-Gründe](#finish-gründe). |
| Andere Schlüssel von `candidates[0]` (`index`, `finishMessage`, `safetyRatings`, `citationMetadata`, `groundingMetadata`, `urlContextMetadata`, `avgLogprobs`, `logprobsResult`, `tokenCount`) | Felder `candidate.*` | `citationMetadata` indiziert den ganzen Text des Candidates, was KPP-Zitate, die an einen Block gebunden sind, nicht ausdrücken können. |
| `candidates[0].content.role` | — | Ein anderer oder kein Wert → Form `no_role`. |
| Keine `candidates` | keine Blöcke, `stop = ContentFilter` | Form `no_candidates`; `promptFeedback` bleibt ein Feld. |
| `usageMetadata` | `usage` | Siehe [Nutzung](#nutzung). |
| `promptFeedback`, `modelStatus` und andere Schlüssel | Felder | |

Antworten an Gemini-Apps aus anderen Dialekten bekommen `role: "model"`, `index: 0` sowie ID und Modell der Antwort. Reasoning anderer Dialekte entfällt wie bei Apps von Chat Completions ([Antworten](cross-format.md#antworten)): Ein Thought-Part kann keinen [Reasoning-Blob](reasoning.md#reasoning-blobs) tragen, weil das SDK `google-genai` `thoughtSignature` als Base64 liest.

### Finish-Gründe

| `finishReason` | KPP | Zurück zu Gemini |
|---|---|---|
| `STOP` | `EndTurn`, oder `ToolUse`, wenn die Antwort Funktionen aufruft | `STOP`, auch für `StopSequence` |
| `MAX_TOKENS` | `MaxTokens` | `MAX_TOKENS` |
| `SAFETY`, `BLOCKLIST`, `PROHIBITED_CONTENT`, `SPII`, `RECITATION`, `IMAGE_SAFETY`, `IMAGE_PROHIBITED_CONTENT`, `IMAGE_RECITATION` | `ContentFilter`, der Wert in Form `finish` | `SAFETY` für `ContentFilter` und `Refusal` |
| Jeder andere Wert, etwa `MALFORMED_FUNCTION_CALL` | `Other(value)` | der Wert, wenn das Discovery-Dokument ihn führt, sonst `OTHER` |

### Nutzung

`promptTokenCount` enthält die gecachten Prompt-Token und die der Tool-Nutzung. Daher gilt `input_tokens = promptTokenCount + toolUsePromptTokenCount − cachedContentTokenCount`, `cache_read_tokens = cachedContentTokenCount`, `output_tokens = candidatesTokenCount + thoughtsTokenCount` und `reasoning_tokens = thoughtsTokenCount` ([Nutzung](cross-format.md#nutzung)). `Usage::raw` bewahrt das ganze `usageMetadata` auf. Die Nutzung anderer Dialekte wird als `promptTokenCount`, `candidatesTokenCount` und `totalTokenCount` geschrieben, dazu `cachedContentTokenCount` und `thoughtsTokenCount`, wenn sie nicht 0 sind.

## Token-Zählung

Schemas `CountTokensRequest` und `CountTokensResponse`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `contents` | `messages` | |
| `generateContentRequest` | die ganze Anfrage | Form `count: "wrapped"`; sein `model` → Form `model`. Beides zu senden ist `InvalidRequest`. |
| `totalTokens` | `input_tokens` | |
| `cachedContentTokenCount`, `promptTokensDetails`, `cacheTokensDetails` | Felder | |

Eine Zählung aus einem anderen Dialekt mit Systemprompt oder Tools geht als `generateContentRequest` mit `model: "models/<id>"` und ohne `generationConfig`, sonst als `contents`.

## Streams

`streamGenerateContent?alt=sse` sendet jeden Chunk als ganze `GenerateContentResponse` in einer `data:`-Zeile. Funktionsaufrufe kommen ganz an: Die Dokumentation kennt keine gestreamten Argumente.

- **Audio.** Ein Part, der nur `inlineData`-Audio enthält, wie Sprachmodelle 16-Bit-PCM-Stücke streamen (`streams/recorded-stream-speech.sse`), öffnet einen Audioblock, und die nächsten solchen Parts desselben Medientyps setzen ihn als `Audio`-Deltas fort; andere Dialekte bekommen die Stücke der Reihe nach.
- **Blöcke.** Text-Parts setzen den offenen Textblock fort und Thought-Parts den offenen Reasoning-Block. Ein Funktionsaufruf ist ein ganzer Tool-Call-Block: Start, ein Argument-Delta, Stopp. Eine Signatur an einem Thought-Part ist ein `Opaque`-Delta in seinem Block; an einem anderen Part ist sie ein Reasoning-Block nur mit Signatur davor. Andere Parts sind unbekannte Blöcke.
- **Nutzung und Ende.** Jeder Chunk trägt `usageMetadata`; der mit `finishReason` wird zu einem `ResponseDelta` nach dem Stopp des letzten Blocks. Ein Stream, der ohne `finishReason` endet, endet mit einem Fehler.
- **Zurückspielen im selben Dialekt.** Das erste Event jedes Chunks trägt die Form-Markierung `chunk`: den Chunk ohne seine Parts, seinen Finish-Grund und seine letzte Nutzung. Der Encoder setzt sie wieder ein, sodass jeder Chunk wie empfangen herauskommt.
- **Andere Dialekte** bekommen einen Chunk pro Text-Delta, einen pro Funktionsaufruf und einen letzten Chunk mit leerem Text-Part, `finishReason` und `usageMetadata`.
- **Fehler.** Eine `data:`-Zeile mit einem `error`-Objekt ist ein Fehler-Event; der Encoder schreibt Fehler genauso.
- **Keepalives.** Eine Leerzeile ([Keepalives](cross-format.md#keepalives)). Das SDK `google-genai` liest jede Zeile, die nicht mit `data: ` beginnt, als JSON, ein Kommentar würde es also brechen.
- Kommentare, benannte Events und Frames, die keine JSON-Objekte sind, gehen als `Extra` durch.

## Fehler

Der Fehler-Body von Google `{"error": {"code", "message", "status", "details"}}` ([Fehlerbehebung](https://aistudio.google.com/docs/troubleshooting)).

| `status` | KPP | Anmerkungen |
|---|---|---|
| `INVALID_ARGUMENT` | `InvalidRequest` | `Authentication`, wenn `details[].reason` `API_KEY_INVALID` ist (`errors/recorded-error-invalid-key.json`), `ContextTooLong`, wenn die Meldung sagt, dass die Eingabe die Höchstzahl an Token überschreitet. |
| `FAILED_PRECONDITION`, `OUT_OF_RANGE` | `InvalidRequest` | Etwa Bildgenerierung, die im Land des Schlüssels nicht verfügbar ist. |
| `UNAUTHENTICATED` | `Authentication` | |
| `PERMISSION_DENIED` | `PermissionDenied` | |
| `NOT_FOUND` | `NotFound` | |
| `RESOURCE_EXHAUSTED` | `RateLimited` | `RetryInfo.retryDelay`, etwa `"23s"`, → `retry_after`. |
| `UNAVAILABLE` | `Overloaded` | |
| `DEADLINE_EXCEEDED` | `Timeout` | |
| `INTERNAL` und die anderen Codes | `ProviderError` | |
| Kein `status` | nach HTTP-Status | |

`BadRequest.fieldViolations[0].field` → `param`. Ein Gemini-Fehler erreicht Gemini-Apps wie empfangen. Fehler anderer Dialekte werden mit dem Code und Status ihrer Art geschrieben, ein 429 oder 503 mit `RetryInfo` und `retry-after`: `RateLimited` und `Billing` 429 `RESOURCE_EXHAUSTED`, `Overloaded` 503 `UNAVAILABLE`, `Authentication` 401 `UNAUTHENTICATED`.

## Embeddings

Der Codec `gemini_embeddings`. Schemas `EmbedContentRequest`, `BatchEmbedContentsRequest`, `EmbedContentResponse` und `BatchEmbedContentsResponse`.

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `content` mit einem Text-Part | ein `EmbedInput::Text` | Jeder andere Content → Feld `content` (oder `requests[i].content`) und eine leere Eingabe, über Dialekte hinweg abgelehnt. |
| `content.role` `user` | Form `user_role:<i>` | `google-genai` sendet sie an jedem Content (`requests/sdk-batch-user-role.json`). Gemini bekommt sie zurück; andere Dialekte bekommen die Eingabe ohne sie. Jede andere Rolle lässt den Content roh, über Dialekte hinweg abgelehnt. |
| `requests[]` | je eine Eingabe | |
| `outputDimensionality`, `embedContentConfig.outputDimensionality` | `dimensions` | Wenn sich die Anfragen eines Batches unterscheiden → Felder `requests[i].outputDimensionality`. `embedContentConfig` → Form `config_dimensions`. |
| `taskType`, `title`, andere Schlüssel von `embedContentConfig` | Felder | |
| `model` im Body | — | `models/<Modell des Pfads>` wird abgeleitet; eine andere Schreibweise → Form `models`. |
| `embedding.values`, `embeddings[].values` | `data[].vector` (Float) | `shape` → Felder des Embeddings. |
| `usageMetadata.promptTokenCount` | `usage.input_tokens` | `gemini-embedding-2` und `gemini-embedding-001` melden es; `Usage::raw` bewahrt es auf. |

Eine Anfrage von OpenAI Embeddings geht als `batchEmbedContents` mit `model: "models/<id>"` und `outputDimensionality` an jeder Anfrage; Eingaben aus Token-IDs sind `Unsupported`. Eine Antwort geht in der Form der Methode der App zurück, `embedding` für `embedContent` und `embeddings` für `batchEmbedContents`; Base64-Vektoren eines anderen Dialekts werden zu Floats. Embeddings streamen nie.

## Modelle

`GET {models}` und `GET {models}/{model}` (Schemas `ListModelsResponse` und `Model`).

| Wire-Feld | KPP | Anmerkungen |
|---|---|---|
| `name` `models/<id>` | `id` | Ohne Präfix → Form `bare_name`. |
| `displayName` | `display_name` | |
| `inputTokenLimit`, `outputTokenLimit` | `context_window`, `max_output_tokens` | |
| `version`, `baseModelId`, `description`, `supportedGenerationMethods`, `thinking`, `temperature`, `maxTemperature`, `topP`, `topK` und andere Schlüssel | Felder | |
| `nextPageToken` | `has_more`, Feld `nextPageToken` | |
| Query `pageSize` | `limit` | |
| Query `pageToken` | `after_id` | Eine Gemini-App behandelt das Token als opak, daher nutzen Kaesos eigene Listen die letzte ID der vorigen Seite als Token. |

`dispatch::encode_models_request_with` und `encode_model_request_with` nutzen den Modellpfad aus `EgressCtx::gemini_profile`. Eine Listenanfrage sendet `limit` als `pageSize` und `after_id` als `pageToken`; `before_id`, ein Lebenszyklus-Filter und andere Query-Parameter sind `Unsupported`.

`dispatch::next_models_query(dialect, query, page)` liefert die Anfrage für die Seite nach einer dekodierten, oder `None` nach der letzten: bei Gemini das `nextPageToken` der Seite, bei den anderen Dialekten die letzte ID der Seite. Damit blättert das Gateway durch Googles Liste, wenn eine Modellprüfung die ganze Liste braucht.

## Anbieter, die dieses Format nutzen

| Anbieter | Profil | Schlüssel |
|---|---|---|
| Google AI Studio (`google`) | `GeminiProfile::GOOGLE_AI_STUDIO`, das das Gateway dem Encoder mit `EgressCtx::with_gemini_profile` für jede Anfrage, Modellprüfung und Liste übergibt | `x-goog-api-key` (`AuthStyle::XGoogApiKey`), nie in der URL |
| Gemini Enterprise Agent Platform (`gemini-agent-platform`) | `GeminiProfile::AGENT_PLATFORM`: `/v1/publishers/google/models` für einen Schlüssel des Express-Modus und für einen Dienstkonto-Schlüssel pro Anfrage `with_models_path("/v1/projects/{project}/locations/{location}/publishers/google/models")`, aus dem `project_id` des Schlüssels und dem Standort des Zugangs ([REST-Referenz](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest)). `role_on_every_content` ist an, weil die [Referenz zur Token-Zählung](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/models/count-tokens) auch in Anfragen mit einer Runde eine Rolle verlangt, während das Discovery-Dokument sie optional nennt | Ein Schlüssel des Express-Modus als `x-goog-api-key`; das Zugriffstoken eines Dienstkonto-Schlüssels als `Authorization: Bearer` |

## Round Trips

Jede Fixture wird dekodiert und zu einem gleichen JSON-Wert kodiert: Anfragen mit ihrem Pfad, Antworten, Token-Zählungen, Embeddings, Modelllisten und Fehler. Jede Stream-Fixture ergibt bei jeder Stückelung dieselben Events und wird Chunk für Chunk zurückgespielt. Aufzeichnungen haben keine Ausnahmen.

## Nächste Schritte

- [Formatwechsel](cross-format.md): Schlagen Sie nach, was mit Daten geschieht, die von einem API-Format in ein anderes wechseln, und warum.
- [Reasoning über Formate hinweg](reasoning.md): Sehen Sie, wie Thinking-Stufen, Budgets und Thought Signatures Formate wechseln.
- [Fixtures und Aufzeichnungen](fixtures.md): Zeichnen Sie Gemini-Fixtures mit `cargo xtask record gemini` auf.
