---
title: Medien über Formate hinweg
nav: Medien
description: Wie die Codecs Audio, Video und Bilder zwischen den API-Formaten tragen, in Anfragen, Antworten und Tool-Ergebnissen.
order: 3
---

Audio, Video und Bilder erreichen ein Modell in jedem API-Format in anderer Form, und manche Formate tragen keines davon. Diese Seite listet, was übergeht, was abgelehnt wird und wie Medien in Tool-Ergebnissen ein Chat-Completions-Modell erreichen. Die übrigen Regeln für den Formatwechsel stehen unter [Formate wechseln](cross-format.md).

## Audio und Video

`Block::Audio` und `Block::Video` halten eine `Source`, einen Cache-Hinweis und Extras, wie Bilder. Jedes Format nimmt sie so, wie seine Dokumentation es beschreibt, und lehnt den Rest vor dem Senden ab:

| Ziel | Audio | Video |
| --- | --- | --- |
| Gemini | `inlineData` oder `fileData`, mit dem Medientyp wie gesendet | `inlineData`, `fileData` oder eine YouTube-URL ohne MIME-Typ |
| Chat Completions | `input_audio` in Benutzernachrichten, nur base64, in den Formaten des Anbieters: `wav` und `mp3` für OpenAI, jedes für OpenRouter | `video_url` für Anbieter, die es dokumentieren (OpenRouter, Moonshot); sonst `Unsupported` |
| Anthropic Messages | `Unsupported`: Senden Sie ein Transkript als Text | MiniMax' `video`-Block für MiniMax; sonst `Unsupported`: Senden Sie Einzelbilder als Bilder |
| Responses | `Unsupported` | `Unsupported` |

Eine Datei-ID eines Anbieters funktioniert nur bei dem Anbieter, der sie ausgestellt hat ([Dateien](cross-format.md#dateien)). Geminis `videoMetadata` (Ausschnitt und Bildrate) und `mediaResolution` bleiben Extras ihres Parts: Nur Gemini hat sie, und ein Ausschnitt ändert, was das Modell sieht, darum lehnen andere Formate sie ab. Nichts wird umkodiert: Ein Medientyp, für den das Ziel keinen Namen hat, etwa `audio/webm` für Chat Completions, ist `Unsupported`.

Das Gateway lehnt Audio und Video auch für ein Modell ab, dessen Katalogeintrag `audio` oder `video` nicht unter seinen Eingaben führt, wie bei Bildern und PDFs: `` The model `gpt-4o` does not accept audio. ``

## Audio- und Bildantworten

`GenerateRequest::output_modalities` nennt, was die Antwort enthalten soll (`Text`, `Image`, `Audio`), und `audio_output` Stimme und Format von Audio. Eine Audioantwort ist ein `Block::Audio` mit base64-Daten, seinem Medientyp und, wo das Format eines hat, einem `transcript`; Streams tragen sie als `BlockStart(Audio)` und `Audio`-Deltas mit Daten und Transkripttext.

| Format der App | Verlangt mit | Bekommt |
| --- | --- | --- |
| Chat Completions | `modalities: ["text", "audio"]` mit `audio.voice` und `audio.format` | `message.audio` und `delta.audio`, von OpenAI oder von einem Gemini-Sprachmodell |
| Gemini | `responseModalities: ["AUDIO"]` oder `["IMAGE"]` mit `speechConfig` und `responseFormat` | `inlineData`-Audio und -Bilder, von Gemini oder von einem OpenAI-Audiomodell |
| Anthropic Messages, Responses | nichts: Ihre Formate haben keine Einstellung dafür | nur Text; Audio und Bilder, die ein Anbieter ungefragt schickt, werden weggelassen |

- **Stimmen** gehören ihrem Anbieter. Eine Stimme geht nur über, wenn das Ziel sie nennt: OpenAIs eingebaute Stimmen für Chat Completions, Geminis 30 vorgefertigte Stimmen für Gemini; alles andere ist `Unsupported` mit der Stimme in `param`.
- **Formate** werden nie umkodiert. `wav`, `mp3` und `pcm16` gehen zwischen Chat Completions und Gemini über (`AUDIO_WAV`, `AUDIO_MP3`, `AUDIO_L16` bei 24 kHz); `flac`, `opus` und `aac` erreichen nur OpenAI. Ohne Format bekommt ein Chat-Completions-Ziel `wav`, oder `pcm16` für einen Stream.
- **Der Text von Chat.** In `["text", "audio"]` ist der Text das Transkript des Audios, darum bekommt Gemini `["AUDIO"]`, was seine Sprachmodelle brauchen.
- **Wiedergabe.** OpenAI bewahrt eine Audioantwort eine Weile auf und nimmt sie als `audio: {id}` zurück. Diese ID funktioniert nur dort, wie eine Datei-ID; ein anderes Ziel lehnt sie ab, es sei denn, der Audioblock trägt ein Transkript, das Chat Completions dann als Text sendet.
- **Bilder.** Gemini-Bildmodelle liefern Bilder als Parts. Nur Gemini-Apps bekommen sie: Chat Completions und Messages haben kein Bild in einer Antwort, und ein `image_generation_call` von Responses gehört zu OpenAIs Bild-Tool, ein Gemini-Bild läse sich also als Tool-Aufruf, den die App nie gemacht hat. Bildausgabe von diesen Formaten zu verlangen ist `Unsupported`.
- **Katalog.** Das Gateway lehnt Audioausgabe für ein Modell ab, dessen Katalogausgaben kein `audio` führen, und Bildausgabe ohne `image`: `` The model `gemini-3.5-flash` does not produce audio. `` Ein Sprachmodell, das keinen Text schreibt, nimmt `["text", "audio"]` von Chat an.

## Medien in Tool-Ergebnissen

Claudes `tool_result` und das `function_call_output` von Responses können Bilder und Dateien enthalten, etwa den Screenshot eines Tools oder das Bild, das `Read` von Claude Code oder `view_image` von Codex öffnet. Eine `tool`-Nachricht von Chat Completions enthält nur Text (`ChatCompletionToolMessageParam`), und OpenAI und DeepSeek lehnen dort ein Bild ab. Für Chat-Completions-Modelle verschiebt Kaeso sie daher:

- **Die Tool-Nachricht** behält den Text des Ergebnisses, gefolgt von `The rest of this result is in the next message.`
- **Die Benutzernachricht nach den Ergebnissen** beginnt für jedes solche Ergebnis mit `` The rest of the result of tool call `<id>`: `` und den Bildern, Dateien und Audioinhalten des Ergebnisses in ihrer Reihenfolge. Text, den die App nach den Ergebnissen gesendet hat, folgt danach.
- **Der Header.** Jedes Ergebnis wird im Header `kaeso-adapted` der Antwort als `tool result` mit der ID des Aufrufs genannt, etwa `tool result call_abc`.

Anthropic- und Responses-Modelle bekommen die Medien unverändert im Tool-Ergebnis, Gemini-Modelle in `functionResponse.parts` ([Gemini API](gemini.md)). Fixtures: `anthropic_messages/requests/tool-result-multi-turn.json`, `openai_responses/requests/function-call-output-array.json`.
