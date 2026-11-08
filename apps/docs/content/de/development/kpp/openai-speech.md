---
title: OpenAI Speech
description: Wie die Speech API von OpenAI auf KPP abgebildet wird und wie die Sprachformate von Gemini, MiniMax und anderen Anbietern sie übernehmen.
order: 8
---

Schlagen Sie hier jedes Feld der Speech API von OpenAI nach: wohin es in KPP gehört, welche Anbieter es in welcher Form nehmen und woher jede Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie ein Sprach-Codec ein Feld liest oder schreibt.

- **Codecs:** `kaeso_codecs::openai_speech::OpenAiSpeech` (`crates/kaeso-codecs/src/openai_speech/`), `kaeso_codecs::minimax_speech::MinimaxSpeech` (`crates/kaeso-codecs/src/minimax_speech/`) und Sprachanfragen im Gemini-Codec (`crates/kaeso-codecs/src/gemini/audio.rs`)
- **Endpunkt:** `POST /v1/audio/speech`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_speech/` und `fixtures/minimax_speech/`, aus der Spezifikation und der Dokumentation der Anbieter gebaut; `fixtures/gemini/` enthält aufgezeichnete Sprache von Gemini

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateSpeechRequest`, `CreateSpeechResponseStreamEvent`, `SpeechAudioDeltaEvent`, `SpeechAudioDoneEvent`. Die Seiten der Anbieter stehen unter [Anbieter](#anbieter-und-ihre-formen).

## Entscheidungen

- **Eine KPP-Anfrage für erzeugtes Audio.** `Request::Speech(SpeechRequest)` mit `SpeechOperation` `Speech`, `Dialogue`, `SoundEffect`, `Music` oder `VoiceChange`, `Reply::Speech(SpeechResponse)` mit dem Audio base64-kodiert und seinem Medientyp, und `StreamEvent::Speech(SpeechEvent)` für Audiostücke und das Ende mit seiner Nutzung. Die Speech API von OpenAI erzeugt nur `Speech`; die anderen Operationen sind für Anbieter, die sie anbieten.
- **Stimmen sind Namen oder IDs.** `Voice::Named` für eine Stimme, die ein Anbieter benennt, `Voice::Id` für eine Stimme per ID (`{"id": …}` von OpenAI). Ein Name geht nur an einen Anbieter, der ihn listet.
- **Audio fließt, während es entsteht.** Eine Antwort in rohem Audio wird mit dem Stream-Decoder gelesen, ob die App streamt oder nicht (`Codec::reads_reply_as_stream`), damit die ersten Bytes der App nicht auf die letzten warten. `Codec::response_media_type` und `stream_media_type` nennen, was der Codec geschrieben hat: den Medientyp des Audios oder `text/event-stream` für Ereignisse.
- **Keine Umwandlung.** Ein Format, eine Stimme oder eine Abtastrate, die der Anbieter nicht hat, ist `Unsupported`, bevor etwas gesendet wird.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_speech`; Anbieter des Dialekts bekommen sie, andere Dialekte lehnen sie ab.
- **Form**: eine codec-eigene Markierung in `Extras::form`.

## Anfrage

| `CreateSpeechRequest` | KPP | Klasse |
| --- | --- | --- |
| `model` | `SpeechRequest::model` | typisiert |
| `input` | `input` | typisiert |
| `voice`, ein String | `voice: Voice::Named` | typisiert |
| `voice`, `{"id": …}` | `voice: Voice::Id` | typisiert |
| `instructions` | `instructions` | typisiert |
| `response_format` | `format`: `mp3`, `opus`, `aac`, `flac`, `wav` als `AudioFormat`, `pcm` als `Pcm16` (24 kHz, 16 Bit, mono) | typisiert |
| kein `response_format` | `format: Mp3`, mit der Markierung `default_format` | typisiert, Form |
| `speed` | `speed`; eine ganze Zahl bleibt eine (`integers`) | typisiert, Form |
| `stream_format: "sse"` | `stream: On` | typisiert |
| `stream_format: "audio"` | `stream: Off` mit der Markierung `stream_format` | Form |
| `sample_rate`, `language`, `seed` (Formen anderer Anbieter) | `sample_rate`, `language`, `seed` | typisiert |
| jedes andere Feld | das Feld | Extras |

## Antwort und Stream

| Wire | KPP |
| --- | --- |
| die Audio-Bytes, mit ihrem `content-type` | `SpeechResponse::audio` als Base64 mit diesem Medientyp, oder Stücke in `SpeechEvent::audio`, wenn es beim Eintreffen gelesen wird |
| `speech.audio.delta` `audio` | `SpeechEvent::audio` |
| `speech.audio.done` `usage` (`input_tokens`, `output_tokens`) | `SpeechEvent::done` mit `Usage`, die Ausgabe als `output_audio_tokens` |
| Ereignis `error`, Fehler-Body | `KppError` über den Fehler-Codec von OpenAI |

An eine App schreibt Kaeso die Audio-Bytes mit dem Medientyp, mit dem OpenAI das verlangte Format beantwortet (`audio/mpeg`, `audio/opus`, `audio/aac`, `audio/flac`, `audio/wav`, `audio/pcm`), oder bei `stream_format: "sse"` ein `speech.audio.delta` pro Stück und `speech.audio.done`. Eine ganze Antwort eines Anbieters ohne Ereignisse wird ein Delta und das Ende.

**Die Nutzung bei OpenAI.** OpenAI meldet die Nutzung nur in seinen Ereignissen. Kaeso verlangt von OpenAI Ereignisse (`SpeechProfile::usage_in_events`), außer das Modell hat keine (`sse_refused`: `tts-1`, `tts-1-hd`), und eine App, die das Audio will, bekommt trotzdem Bytes. Der Round-Trip-Test entfernt dieses `stream_format` vor dem Vergleich.

## Anbieter und ihre Formen

`SpeechProfile` beschreibt jeden Anbieter, der eine Form der Speech API von OpenAI spricht: Pfad und Body (`SpeechBody`), Antwort (`SpeechReply`), Stimmen, Formate, Standardformat, Ereignisse (`SpeechEvents`) und die Felder, die er über die von OpenAI hinaus nimmt. Eine App, die kein Format nennt, bekommt das mp3 von OpenAI, deshalb nennt Kaeso einem Anbieter mit anderem Standard ausdrücklich mp3.

| Profil | Form | Quelle |
| --- | --- | --- |
| `OPENAI` | die Spezifikation | die Spezifikation oben |
| `GROQ` | nur `wav` (der Orpheus-Leitfaden; seine API-Referenz nennt mehr), `sample_rate`, keine Ereignisse, kein `instructions` | [Orpheus](https://console.groq.com/docs/text-to-speech/orpheus) |
| `TOGETHER` | `mp3` oder `wav`, `sample_rate`, `language`, kein `speed`, keine Ereignisse | [`AudioSpeechRequest`](https://docs.together.ai/reference/audio-speech) |
| `MISTRAL` | `voice_id`, eine JSON-Antwort mit base64 `audio_data`, Ereignisse mit `stream: true`, die `audio_data` und `UsageInfo` tragen | [generate speech](https://docs.mistral.ai/studio/audio/text-to-speech/generate-speech), [stream speech](https://docs.mistral.ai/studio/audio/text-to-speech/stream-speech) |
| `OPENROUTER` | `mp3` oder `pcm`, standardmäßig `pcm`, keine Ereignisse | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts) |
| `XAI` | `POST /v1/tts`: `text`, `voice_id`, ein verlangtes `language` (`auto`, wenn die App keines nennt), `output_format {codec, sample_rate}`, kein Modell | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `VERCEL` | `POST /v4/ai/speech-model`, das Modell in `ai-model-id`, `text`, `voice`, `outputFormat` (nur `mp3` dokumentiert), eine JSON-Antwort mit base64 `audio` | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech) |

Die Bodys von SpaceX und Vercel nehmen keine Felder anderer Dialekte; ein Feld in den Extras wird abgelehnt.

## Sprache bei Gemini

Eine Sprachanfrage wird ein Aufruf von `generateContent` (`gemini/audio.rs`) mit dem Text als Teil des Nutzers, `responseModalities: ["AUDIO"]`, `speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName` für eine der 30 Stimmen von Gemini, `speechConfig.languageCode` und `seed`. `wav`, `mp3` und `pcm` werden `responseFormat.audio.mimeType` `AUDIO_WAV`, `AUDIO_MP3` und `AUDIO_L16`, auf der Agent Platform eine Liste mit einem Eintrag (`GeminiProfile::response_format_list`). Gemini streamt nur 16-Bit-PCM, deshalb geht `pcm` an `streamGenerateContent`, und die anderen Formate kommen ganz. `instructions` (schreiben Sie den Stil in `input`, wie der Leitfaden von Google es tut), `speed`, `sample_rate`, Zeitmarken und andere Vorgänge sind `Unsupported`. Eine Antwort ohne Audio ist ein Fehler, `ContentFiltered`, wenn die Filter von Google sie gestoppt haben. Die aufgezeichneten Fixtures `gemini/requests/speech-mp3.json` und ihre Antwort zeigen, dass `AUDIO_MP3` als `audio/mp3; rate=44100; channels=1` beantwortet wird.

## Sprache bei MiniMax

`minimax_speech` liest und schreibt `POST /v1/t2a_v2` ([T2A](https://platform.minimax.io/docs/api-reference/speech-t2a-http)), bedient von der Wurzel des Hosts von MiniMax mit `Authorization: Bearer`. Der Text ist `text`; die Stimme ist `voice_setting.voice_id`, und `speed` ist `voice_setting.speed`; Format, Abtastrate und Bitrate sind `audio_setting.format`, `sample_rate` und `bitrate` (Bit pro Sekunde; KPP zählt Kilobit). `pcm16` von KPP hat 24 kHz, deshalb geht `pcm` mit `sample_rate: 24000`. Kaeso verlangt die ganze Antwort mit `output_format: hex`: Die Stream-Rahmung von MiniMax ist nicht dokumentiert. Das Hex-Audio wird Base64, `extra_info.audio_format` nennt seinen Medientyp und `extra_info.usage_characters` die abgerechneten Zeichen (`SpeechResponse::characters`). Ein fehlschlagendes `base_resp`, das MiniMax mit HTTP 200 sendet, ist ein Fehler, abgebildet nach seinem Code. `instructions`, `language`, `seed`, Zeitmarken und andere Vorgänge sind `Unsupported`.
