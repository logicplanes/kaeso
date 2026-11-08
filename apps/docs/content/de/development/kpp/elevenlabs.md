---
title: ElevenLabs
description: Wie die API von ElevenLabs auf KPP abgebildet wird, für Apps im Format von ElevenLabs und für ElevenLabs als Anbieter.
order: 9
---

Schlagen Sie nach, wie die Endpunkte von ElevenLabs zum Erzeugen, Transkribieren und Auflisten auf KPP abgebildet werden, wie Kaeso sie von Apps im Format von ElevenLabs liest und an ElevenLabs schreibt. Prüfen Sie es, bevor Sie den ElevenLabs-Codec ändern.

- **Codec:** `kaeso_codecs::elevenlabs::ElevenLabs` (`crates/kaeso-codecs/src/elevenlabs/`)
- **Fixtures:** `crates/kaeso-codecs/fixtures/elevenlabs/`, live im kostenlosen Tarif aufgezeichnet, einmal pro Endpunkt, insgesamt etwa 40 Credits

**Quellen.** Das OpenAPI-Dokument von ElevenLabs, [api.elevenlabs.io/openapi.json](https://api.elevenlabs.io/openapi.json) (gelesen am 2026-11-08), seine [Fehler](https://elevenlabs.io/docs/eleven-api/resources/errors) und [Kosten-Header](https://elevenlabs.io/docs/api-reference/introduction) sowie die Pfade des offiziellen Python-SDKs (das `x-fern-sdk-method-name` jeder Operation).

## Entscheidungen

- **Der Pfad nennt den Aufruf.** `ElevenLabsCall::from_path` liest die Operation, die Stimmen-ID und die Formen `/stream` und `/with-timestamps`; Anfragen von Apps werden mit `dispatch::decode_elevenlabs_request` dekodiert. Erzeugen wird `Request::Speech` (`Speech`, `Dialogue`, `SoundEffect`, `Music`, `VoiceChange`), Speech to Text `Request::Transcription`.
- **Stimmen-IDs, nie geraten.** ElevenLabs nimmt Stimmen per ID. Eine `Voice::Id` oder ein Name aus zwanzig Buchstaben und Ziffern, wie die IDs von ElevenLabs es sind, geht als ID; jeder andere Name, etwa `alloy` von OpenAI, ist `Unsupported` mit einem Hinweis auf `GET /v1/voices`.
- **Audio fließt, während es entsteht.** Für eine App eines anderen Formats ruft Kaeso den Pfad `/stream` auf, der das Audio sendet, während es entsteht, außer bei WAV, das nur der Pfad ohne Streaming nimmt. `/with-timestamps` antwortet mit JSON, ganz gelesen (`EgressCtx::speech_reply_json`); `/stream/with-timestamps` antwortet mit einer JSON-Zeile pro Stück, gelesen als `SpeechEvent`s mit ihrer Ausrichtung.
- **Zeichen aus dem Header.** ElevenLabs rechnet Sprache pro Zeichen ab und nennt die Zeichen in seinem Header `character-cost`; `elevenlabs::billed_characters` liest ihn für die Kosten.
- **Standardmodelle.** Eine Anfrage ohne `model_id` bekommt den dokumentierten Standard von ElevenLabs für die Operation (`eleven_multilingual_v2`, `eleven_v3` für Dialoge, `eleven_text_to_sound_v2`, `music_v1`, `eleven_english_sts_v2`), damit Kaeso sie auflösen kann; die Markierung `default_model` hält ihn aus der Anfrage heraus, die ElevenLabs bekommt.
- **Query-Parameter gehen nur an ElevenLabs.** `output_format` ist typisiert; `enable_logging` und `optimize_streaming_latency` sind Extras, damit ein anderer Anbieter sie ablehnt, statt eine Einstellung ohne Datenspeicherung zu verlieren.

## Anfragen

| ElevenLabs | KPP | Klasse |
| --- | --- | --- |
| Pfad `voice_id` | `voice: Voice::Id` | typisiert |
| Query `output_format`, etwa `mp3_44100_128`, `pcm_24000`, `opus_48000_64`, `wav_16000`, `ulaw_8000` | `format`, `sample_rate`, `bitrate`; `pcm_24000` ist `Pcm16`; der Wert wie geschrieben | typisiert, Form |
| andere Query-Parameter | das Feld `query` | Extras |
| `text` (Sprache, Soundeffekte), `prompt` (Musik) | `input` | typisiert |
| `inputs[]` `{text, voice_id}` | Zeilen in `dialogue` | typisiert |
| `model_id` | `model` | typisiert |
| `language_code` | `language` | typisiert |
| `seed` | `seed` | typisiert |
| `voice_settings.speed` | `speed` | typisiert |
| `duration_seconds`, `music_length_ms` | `duration` in Sekunden | typisiert |
| `/with-timestamps` | `timestamps` | typisiert |
| `/stream` | die Markierung `stream_path`; mit Zeitmarken `stream: On` | Form, typisiert |
| der Rest von `voice_settings`, `pronunciation_dictionary_locators`, `previous_text`, `apply_text_normalization`, `audio_effects`, … | das Feld | Extras |
| Upload des Voice Changers `audio`, `seed`; Felder | `audio`, `seed`; der Rest Extras | typisiert, Extras |
| Upload für Speech to Text `file`, oder `source_url` / `cloud_storage_url` | `audio` | typisiert |
| `language_code`, `diarize`, `num_speakers`, `temperature` | `language`, `diarize`, `speakers`, `temperature` | typisiert |
| `timestamps_granularity` `word` (der Standard) oder `none` | `format` `verbose_json` oder `json`; `word` auch `timestamps`; `character` die Markierung `timestamps_granularity` | typisiert, Form |
| `tag_audio_events`, `additional_formats`, `keyterms`, `entity_detection`, `webhook`, … | das Feld | Extras |

Aus einem anderen Format werden `mp3`, `opus`, `wav`, `pcm16`, μ-law und A-law ein `output_format`, das ElevenLabs listet, mit den Raten des kostenlosen Tarifs, wenn die Anfrage keine nennt: 44,1 kHz bei 128 kbit/s für MP3, 48 kHz bei 64 kbit/s für Opus, 24 kHz für WAV und PCM. `aac`, `flac` und Raten, die ElevenLabs nicht listet, sind `Unsupported`. `speed` geht an `voice_settings.speed`; `instructions` ist `Unsupported`.

## Antworten

| ElevenLabs | KPP |
| --- | --- |
| Audio-Bytes mit `content-type` und `character-cost` | `SpeechResponse::audio`, `characters`; Stücke als `SpeechEvent`s |
| `audio_base64`, `alignment {characters, character_start_times_seconds, character_end_times_seconds}`, `normalized_alignment` | `audio`, `alignment: SpeechAlignment`; `normalized_alignment` Extras |
| eine Zeile von `/stream/with-timestamps` | ein `SpeechEvent` mit seiner Ausrichtung |
| `text`, `language_code`, `words[]` `{text, start, end, type, speaker_id, logprob}`, `audio_duration_secs` | `text`, `language`, `words` mit `speaker` (`type`, `logprob` und `characters` Extras), `duration` und `usage_seconds`; der Rest Extras |
| ein Transkript pro Kanal (`transcripts`) | abgelehnt: Ein Transkript hat keinen Platz für mehrere Kanäle |

Eine OpenAI-App bekommt die Wörter von ElevenLabs ohne deren Einträge `spacing` und `audio_event`, die keine Wörter sind. Eine ElevenLabs-App bekommt das Transkript eines anderen Anbieters mit `language_probability` 1 und jedem Wort als `type: word`.

## Fehler

ElevenLabs antwortet mit `{"detail": {"type", "code", "message", "status", "request_id", "param"}}`; ältere Endpunkte mit `{"detail": {"status", "message"}}`, die Validierung mit `{"detail": [{"loc", "msg"}]}`. Kaeso liest alle drei, bildet `type` und `code` auf die Fehlerart ab (`insufficient_credits` und `payment_required` auf Abrechnung, `rate_limit_error` auf Ratenlimit) und schreibt seine eigenen Fehler an ElevenLabs-Apps in der dokumentierten Form. Die eigenen Fehler von ElevenLabs gehen weiter, wie sie kamen, mit ihrem Status.

## Listen

`GET /v1/models` listet die ElevenLabs-Modelle des Projekts in der Form `ModelResponseModel` von ElevenLabs, `model_id` und `name`. Die Stimmenlisten sind die eigenen des Kontos bei ElevenLabs: `voice_listing_request` baut `GET /v1/voices`, `/v2/voices` oder `/v1/voices/{voice_id}` mit der Query der App, und `voices_reply` prüft, dass die Antwort eine Stimmenliste oder eine Stimme ist, bevor sie weitergeht.
