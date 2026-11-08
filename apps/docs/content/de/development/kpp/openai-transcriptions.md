---
title: OpenAI Transcriptions
description: Wie die API von OpenAI für Transkription und Übersetzung auf KPP abgebildet wird und wie andere Transkriptionsformate sie übernehmen.
order: 8
---

Schlagen Sie hier jedes Feld der API von OpenAI für Transkription und Übersetzung nach: wohin es in KPP gehört, welche Anbieter es in welcher Form nehmen und woher jede Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie ein Transkriptions-Codec ein Feld liest oder schreibt.

- **Codecs:** `kaeso_codecs::openai_transcriptions::OpenAiTranscriptions` (`crates/kaeso-codecs/src/openai_transcriptions/`), Transkriptionen im Gemini-Codec (`gemini/audio.rs`) und im Chat-Completions-Codec für QwenCloud (`openai_chat/transcription.rs`)
- **Endpunkte:** `POST /v1/audio/transcriptions` und `POST /v1/audio/translations`, Uploads als `multipart/form-data`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_transcriptions/`, aus der Spezifikation und der Dokumentation der Anbieter gebaut; `fixtures/gemini/` enthält ein aufgezeichnetes Transkript von Gemini

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateTranscriptionRequest`, `CreateTranslationRequest`, `AudioResponseFormat`, `CreateTranscriptionResponseJson`, `CreateTranscriptionResponseVerboseJson`, `CreateTranscriptionResponseDiarizedJson`, `CreateTranslationResponseJson`, `CreateTranslationResponseVerboseJson`, `TranscriptionSegment`, `TranscriptionWord`, `TranscriptTextUsageTokens`, `TranscriptTextUsageDuration`, `TranscriptTextDeltaEvent`, `TranscriptTextSegmentEvent`, `TranscriptTextDoneEvent`. openai-python 3.41.0 für die Namen der Multipart-Felder. RFC 7578.

## Entscheidungen

- **Eine KPP-Anfrage für beide Endpunkte.** `Request::Transcription(TranscriptionRequest)` mit `TranscriptionOperation` `Transcribe` oder `Translate`; der Pfad nennt den Vorgang, deshalb werden Uploads von Apps mit `dispatch::decode_transcription_request` dekodiert. `Reply::Transcription(TranscriptionResponse)` enthält den Text, die Form, in der er kam (`TranscriptFormat`), die Sprache, die Dauer, zeitlich markierte Segmente und Wörter mit Sprechern, die Nutzung und die abgerechneten Sekunden. `StreamEvent::Transcript(TranscriptEvent)` trägt Deltas, fertige Segmente und das Ende.
- **Untertitel aus Zeitangaben.** Hat der Anbieter die Form der App nicht, verlangt Kaeso `verbose_json` mit Zeitmarken für Segmente (oder die zeitlich markierten Wörter eines Anbieters) und schreibt `srt`, `vtt` oder Segmente selbst (`openai_transcriptions::subtitles`): nummerierte Cues und Zeiten `HH:MM:SS,mmm` für SubRip, `WEBVTT` und `HH:MM:SS.mmm` für WebVTT. Eine Antwort mit Wörtern und ohne Segmente wird ein Cue von ihrem ersten bis zu ihrem letzten Wort. Eine Form, die keine Zeitangabe ergeben kann, etwa `diarized_json` ohne Sprecher, ist `Unsupported`, bevor etwas gesendet wird.
- **Keine Umwandlung.** Kaeso dekodiert und wandelt kein Audio um; der Upload geht, wie er kam.

## Anfrage

| Feld von `CreateTranscriptionRequest` | KPP | Klasse |
| --- | --- | --- |
| `file` | `audio: AudioInput` mit den Bytes, dem Medientyp und dem Dateinamen des Uploads | typisiert |
| `model` | `model` | typisiert |
| `language` | `language` | typisiert |
| `prompt` | `prompt` | typisiert |
| `response_format` | `format: TranscriptFormat` | typisiert |
| `temperature` | `temperature`; eine ganze Zahl bleibt eine | typisiert, Form |
| `timestamp_granularities[]` | `timestamps`; die Schreibweise des Felds mit oder ohne `[]` | typisiert, Form |
| `stream` | `stream`; ein ausdrückliches `false` bleibt | typisiert, Form |
| `diarize`, ein Link in `url` oder `file_url` (Formen anderer Anbieter) | `diarize`, `audio` als Link | typisiert, Form |
| `include[]`, `chunking_strategy`, `known_speaker_names[]`, `known_speaker_references[]`, `languages[]`, `keywords[]` und jedes andere Feld | das Feld, Listen als Arrays | Extras |

## Antwort und Stream

| Wire | KPP |
| --- | --- |
| `json`: `text`, `usage`, `logprobs`, `languages` | `text`, `usage`; der Rest Extras |
| `verbose_json`: `language`, `duration`, `segments`, `words`, `usage` | `language`, `duration`, `segments` (`id`, `start`, `end`, `text`; `seek`, `tokens` und die Scores als Extras; eine numerische `id` markiert mit `numeric_id`), `words` |
| `diarized_json`: `segments` mit `speaker` | `segments` mit `speaker` |
| `text`, `srt`, `vtt` | `text` wie geschrieben, mit seinem `format` |
| Nutzung `{"type": "tokens", …}` | `Usage`, der Audioanteil als `input_audio_tokens` |
| Nutzung `{"type": "duration", "seconds"}` | `usage_seconds` |
| `transcript.text.delta` `delta`, `segment_id`, `logprobs` | `TranscriptEvent::delta`, `segment_id`; `logprobs` Extras |
| `transcript.text.segment` | `TranscriptEvent::segment` |
| `transcript.text.done` | `TranscriptEvent::done` mit dem ganzen Transkript |

Kaeso schreibt ein Transkript in der Form der App mit `Codec::response_media_type`: `application/json`, `text/plain; charset=utf-8` für `text`, `srt` und `vtt`, oder Ereignisse. Felder, die die Form eines anderen Anbieters hinzugefügt hat, etwa `x_groq` von Groq oder `finish_reason` von Mistral, gehören diesem Anbieter und fehlen in der Antwort an eine OpenAI-App, ebenso die Form der Nutzung eines Anbieters, die Kaeso in der Form von OpenAI schreibt.

## Anbieter und ihre Formen

`TranscriptionsProfile` beschreibt jeden Anbieter, der eine Form der API spricht: Pfade, Body (`TranscriptionBody`), Formen, ob er `response_format` überhaupt nimmt, wo er die Sprache nimmt (`LanguageField`), wie er Zeitangaben nimmt (`TimestampsField`), Links, Streams und die Felder über die von OpenAI hinaus.

| Profil | Form | Quelle |
| --- | --- | --- |
| `OPENAI` | die Spezifikation | die Spezifikation oben |
| `GROQ` | `json`, `text`, `verbose_json`; Links in `url`; Übersetzungen; keine Streams | [speech to text](https://console.groq.com/docs/speech-to-text) |
| `TOGETHER` | `json`, `verbose_json`; `timestamp_granularities`; Links in `file`; `diarize`; Übersetzungen | [transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| `MISTRAL` | eine JSON-Form mit `segments`, wenn `timestamp_granularities=segment` gesendet wird, keine Zeitangaben für Wörter; `file_url`, `file_id`; `diarize`; Ereignisse `transcription.*`; kein `prompt`, keine Übersetzungen | [transcribe files](https://docs.mistral.ai/studio/audio/speech-to-text/transcribe-files) |
| `ZAI` | `/api/paas/v4/audio/transcriptions` von der Wurzel des Hosts, `Authorization: Bearer`; JSON-Text; kein `language`, kein `temperature`; Ereignisse, deren Ende den Text fehlen lassen kann, den Kaeso aus den Deltas ergänzt | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| `XAI` | `/v1/stt`; zeitlich markierte Wörter in jeder Antwort (`text`, ein nummerierter `speaker`); Links in `url`; `diarize`; die Datei zuletzt | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `OPENROUTER` | `json`, `verbose_json`; Links in `source_url`; Nutzung mit `seconds` | [STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| `COHERE` | `/v2/audio/transcriptions` von der Wurzel des Hosts; `language` verlangt; nur der Text | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| `MINIMAX` | `/v1/speech_to_text` von der Wurzel des Hosts, `Authorization: Bearer`; `json`, `verbose_json`, `srt`, `vtt`; die Sprache in einem Header `language`; `timestamp_level` `sentence` oder `word`; Ereignisse `{index, delta, finish, duration}` | [speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| `VERCEL` | `/v4/ai/transcription-model`; JSON mit base64 `audio` und `mediaType`, das Modell in `ai-model-id`; eine Antwort mit `durationInSeconds` | [speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |

## Transkription bei Gemini

Eine Transkription wird ein Aufruf von `generateContent` an ein Transkriptionsmodell mit dem Audio als `inlineData` und `generationConfig.audioTranscriptionConfig` ([audio transcription](https://aistudio.google.com/docs/transcribe), Discovery `AudioTranscriptionConfig` in Revision 20261106): `language` als `languageCodes`, `diarize` oder `diarized_json` als `diarization: true` und `wordTimestamp: true` für jede Form mit Zeiten und für Zeitmarken von Wörtern. Die Textteile der Antwort sind der Text; jeder Teil `audioTranscription` ist ein Segment mit seinem `speakerLabel` und seinen Wörtern, Offsets wie `"0.400s"` als Sekunden gelesen. Übersetzungen, Links, `prompt`, `temperature` und eine Zahl von Sprechern sind `Unsupported`. `fixtures/gemini/requests/transcribe-diarized.json` und seine aufgezeichnete Antwort zeigen eine MP3 von einer Sekunde, transkribiert mit einem Sprecher und zwei zeitlich markierten Wörtern.

## Transkription bei QwenCloud

QwenCloud bedient `qwen3-asr-flash` über sein OpenAI-kompatibles `chat/completions` ([Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference)), deshalb geht eine Transkription an QwenCloud durch den Codec `openai_chat`: eine Nutzernachricht mit einem Teil `input_audio`, dessen `data` das Audio als Data-URL ist (oder der Link der App), `asr_options.language` und `stream: false`. Der Inhalt der Nachricht in der Antwort ist der Text, ihre Annotation `audio_info` die Sprache und `usage.seconds` die abgerechneten Sekunden. Nur `json` und `text` sind möglich: Das Modell liefert auf dieser API keine Zeitmarken.
