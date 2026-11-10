---
title: ElevenLabs
description: Wie die API von ElevenLabs auf KPP abgebildet wird, für Apps im Format von ElevenLabs und für ElevenLabs als Anbieter.
order: 9
---

Schlagen Sie nach, wie die Endpunkte von ElevenLabs zum Erzeugen, Transkribieren, für Videos und zum Auflisten auf KPP abgebildet werden, wie Kaeso sie von Apps im Format von ElevenLabs liest und an ElevenLabs schreibt. Prüfen Sie es, bevor Sie den ElevenLabs-Codec ändern.

- **Codec:** `kaeso_codecs::elevenlabs::ElevenLabs` (`crates/kaeso-codecs/src/elevenlabs/`)
- **Fixtures:** `crates/kaeso-codecs/fixtures/elevenlabs/`, live aufgezeichnet, einmal pro Endpunkt

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
| Pfad `voice_id` | `voice: Voice::Id`; eine ID, die nicht nur aus Buchstaben und Ziffern besteht, ist `InvalidRequest` mit `param: voice_id`, bevor ein Schlüssel gesucht wird | typisiert |
| Query `output_format`, etwa `mp3_44100_128`, `pcm_24000`, `opus_48000_64`, `wav_16000`, `ulaw_8000` | `format`, `sample_rate`, `bitrate`; `pcm_24000` ist `Pcm16`; der Wert wie geschrieben | typisiert, Form |
| andere Query-Parameter | das Feld `query` | Extras |
| `text` (Sprache, Soundeffekte), `prompt` (Musik) | `input` | typisiert |
| `inputs[]` `{text, voice_id}` | Zeilen in `dialogue`; eine ungültige `voice_id` ist `InvalidRequest` mit `param: inputs[i].voice_id` | typisiert |
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

Aus einem anderen Format werden `mp3`, `opus`, `wav`, `pcm16`, μ-law und A-law ein `output_format`, das ElevenLabs listet, mit Raten, die jeder Tarif bietet, wenn die Anfrage keine nennt: 44,1 kHz bei 128 kbit/s für MP3, 48 kHz bei 64 kbit/s für Opus, 24 kHz für WAV und PCM. `aac`, `flac` und Raten, die ElevenLabs nicht listet, sind `Unsupported`. `speed` geht an `voice_settings.speed`; `instructions` ist `Unsupported`.

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

## Bilderzeugungen

ElevenLabs erzeugt Bilder mit Modellen von OpenAI, Google und ByteDance über `POST /v1/flows/image`, gemeldet von `GET /v1/flows/image/{id}`; der Body hängt von `model_id` ab (`ImageGenerationRequest` im OpenAPI-Dokument, gelesen am 2026-11-09, und der [Leitfaden zu Bild und Video](https://elevenlabs.io/docs/eleven-api/guides/cookbooks/image-and-video)). Kaeso sendet eine Anfrage der Images API von OpenAI dorthin und fragt dann den Status der Generierung ab, höchstens alle zwei Sekunden, wie ElevenLabs es verlangt, bis sie endet, alles innerhalb der einen Anfrage; `dispatch::pending_task`, `encode_task_poll` und `task_poll_gap` benennen die Schritte. Apps im Format von ElevenLabs senden denselben Body an `POST /v1/flows/image`, der mit der Tabelle unten zurück nach KPP gelesen wird; Felder, die Kaeso nicht typisiert, bleiben ElevenLabs-Extras, und `webhook` ist `Unsupported`, da der Webhook die ID von ElevenLabs statt der von Kaeso trüge. `flows.image` ihres SDK behält die ID der Generierung, daher antwortet das Gateway sofort und führt die Generierung unter einer eigenen ID: `decode_generation` liest die Antworten von ElevenLabs auf den Start- und den Statusaufruf (`MediaGenerationCreateResponse`, `MediaGenerationResponse`) als `Generation`, `encode_generation` und `encode_generation_list` (`MediaGenerationListResponse`) schreiben sie für die App zurück, und `decode_generation_query` liest `cursor`, `page_size` (1 bis 100, standardmäßig 30), `status` und `model_id` von `GET /v1/flows/image` (`list_image_generations`, gelesen am 2026-11-10). Die Fixtures in `fixtures/elevenlabs/image/` sind aus dem OpenAPI-Dokument gebaut; die Bilderzeugung wurde nicht live aufgerufen.

| KPP | ElevenLabs |
| --- | --- |
| `model` | `model_id`: `gpt-image-2.5-sunburst`, `gpt-image-2.5-flare`, `gpt-image-2`, `gpt-image-1.5`, `gpt-image-1`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite-image`, `gemini-3-pro-image`, `gemini-2.5-flash-image`, `bytedance-seedream-5-pro`, `bytedance-seedream-5-lite` |
| `prompt` | `prompt` |
| `images`, `mask` | `images[]`, `mask`: `{type: "inline_base64", content_base64, mime_type}` oder `{type: "asset", asset_id}`, bis zu den 5, 10 oder 14 des Modells; eine Maske nur bei GPT Image |
| `quality` | `quality` aus der Liste des Modells (`low`, `medium`, `high`, dazu `xhigh` und `max` bei GPT Image 2.5); `auto` entfällt |
| `background` | `background`, nur GPT Image 1 und 1.5 |
| `size` | `aspect_ratio`: `1024x1024` als `1:1`, `1536x1024` als `3:2`, `1024x1536` als `2:3`, `auto` als `auto`; andere Größen werden abgelehnt |
| `aspect_ratio`, `resolution` | `aspect_ratio`, `resolution`, jeweils aus den Listen des Modells |
| `seed` | `seed`, nur Seedream |
| Antwort `status` `pending` oder `generating` | die Generierung ist nicht fertig; Kaeso fragt erneut |
| Antwort `content_url`, `content_mime_type` | das Bild als Link, etwa eine Stunde gültig, den Kaeso für eine App herunterlädt, die `b64_json` verlangt |
| Antwort `failure_reason`, `error_message` | der Fehler: `moderated` als gefilterter Inhalt, `invalid_parameters` als ungültige Anfrage, `timeout`, `charging_failed` als Abrechnung, andere als Anbieterfehler |

`n` über 1, Bildlinks, `output_format`, `output_compression`, `negative_prompt`, Teilbilder und Variationen werden vor dem Senden abgelehnt. ElevenLabs rechnet Generierungen in Credits ab, die KPMR nicht listet, deshalb bleiben ihre Kosten unbekannt. Die Modelle von ByteDance sind in einem ElevenLabs-Konto aus, bis ElevenLabs den Zugriff freigibt; seine Ablehnung `model_access_denied` erreicht die App mit dieser Erklärung.

## Videoerzeugungen

ElevenLabs erzeugt Videos mit Veo-3.1- und Seedance-2-Modellen über `POST /v1/flows/video`, gemeldet von `GET /v1/flows/video/{id}`; der Body hängt von `model_id` ab (`VideoGenerationRequest`). Kaeso sendet Videojobs für eine App eines anderen Formats dorthin; Apps im Format von ElevenLabs rufen es nicht über Kaeso auf. Die Fixtures in `fixtures/elevenlabs/video/` sind aus dem OpenAPI-Dokument gebaut.

| KPP | ElevenLabs |
| --- | --- |
| `model` | `model_id`: `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `bytedance-seedance-v2`, `-v2-fast`, `-v2-mini`, `-v2.5` |
| `prompt`, `seed`, `audio` | `prompt`, `seed`, `generate_audio` |
| `negative_prompt` | `negative_prompt`, nur Veo-Modelle |
| `seconds`, `aspect_ratio`, `resolution` | `duration_secs`, `aspect_ratio`, `resolution`, jeweils aus den Listen des Modells |
| Bilder `FirstFrame`, `LastFrame` | `start_frame`, `end_frame`: `{type: "inline_base64", content_base64, mime_type}` oder `{type: "asset", asset_id}` |
| Bilder `Reference` | `images[]` `{image, role}`, nur Veo-Modelle, `role` `subject`, außer die App nannte `style` |
| Antwort `status` `pending`, `generating`, `completed`, `failed` | `queued`, `in_progress`, `completed`, `failed` |
| Antwort `content_url`, `content_mime_type` | der Ausgabelink und sein Medientyp, etwa eine Stunde gültig |
| Antwort `failure_reason`, `error_message` | `error.code`, `error.message` |

Bilder per Weblink, `creatify-aurora` (es braucht Audio als Eingabe) und Remixe werden vor dem Senden abgelehnt.

## Listen

`GET /v1/models` listet die ElevenLabs-Modelle des Projekts in der Form `ModelResponseModel` von ElevenLabs, `model_id` und `name`. Die Stimmenlisten sind die eigenen des Kontos bei ElevenLabs: `voice_listing_request` baut `GET /v1/voices`, `/v2/voices` oder `/v1/voices/{voice_id}` mit der Query der App, und `voices_reply` prüft, dass die Antwort eine Stimmenliste oder eine Stimme ist, bevor sie weitergeht.

## Echtzeit-Sockets

`elevenlabs::realtime` liest die Echtzeit-WebSockets von ElevenLabs ohne I/O, für den Sitzungs-Proxy des Gateways. Diese Nachrichten werden nicht übersetzt: Ein ElevenLabs-Socket erreicht nur ElevenLabs, deshalb steht kein KPP-Typ dazwischen.

| Funktion | Was sie tut |
| --- | --- |
| `RealtimeApi::from_path`, `path` | Benennt den Socket von `/v1/text-to-speech/{voice_id}/stream-input` und `/multi-stream-input`, `/v1/text-to-dialogue/stream-input` und `/multi-stream-input` sowie `/v1/speech-to-text/realtime` und schreibt seinen Pfad bei ElevenLabs, mit der Stimme als einem geprüften Segment |
| `read_client_message` | Nimmt `xi_api_key`, `authorization` und `single_use_token` aus der Nachricht einer App und zählt, was sie trägt: die Zeichen von `text` oder jedes `inputs[].text` oder die Bytes von `audio_base_64` und seine `sample_rate` |
| `bytes_per_second` | Die Bytes, die eine Sekunde Audio in `pcm_*` (16 Bit) oder `ulaw_8000` belegt |
| `error_message` | Die eigene Fehlernachricht des Sockets, `{"message_type", "error"}` bei Speech to Text und sonst `{"message", "error", "code", "param"}`, und der Close-Code: 1008 für alles, was die App beheben kann, sonst 1011 |

Quellen: die [WebSocket-Referenzen](https://elevenlabs.io/docs/websockets) und der [Text-to-Dialogue-WebSocket](https://elevenlabs.io/docs/api-reference/text-to-dialogue/ttd-websocket) von ElevenLabs, gelesen am 2026-11-09, und das offizielle Python-SDK `elevenlabs` 2.77.0 (`realtime_tts.py`, `realtime/`).
