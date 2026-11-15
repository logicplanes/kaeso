---
title: Deepgram
description: Wie die Transkriptionen und Sprachanfragen von KPP auf die Speech-to-Text- und Aura-Text-to-Speech-APIs von Deepgram abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie eine Transkription oder eine Sprachanfrage Deepgram erreicht: welches Feld wohin gehört, was abgelehnt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::deepgram::Deepgram` (`crates/kaeso-codecs/src/deepgram/`)
- **Pfade:** `POST /v1/listen` (aufgezeichnetes Audio) und `POST /v1/speak` (Aura), jede Option im Query-String, mit `Authorization: Token`
- **Fixtures:** `crates/kaeso-codecs/fixtures/deepgram/`; die Fehler und die Modellliste live aufgezeichnet, die Antworten aus der Dokumentation von Deepgram

**Quellen.** Die Seiten von Deepgram zu [aufgezeichnetem Audio](https://developers.deepgram.com/docs/pre-recorded-audio), [Text to Speech](https://developers.deepgram.com/docs/text-to-speech), [Einstellungen der Medienausgabe](https://developers.deepgram.com/docs/tts-media-output-settings), [Kodierung](https://developers.deepgram.com/docs/tts-encoding), [Spracherkennung](https://developers.deepgram.com/docs/language-detection), [Utterances](https://developers.deepgram.com/docs/utterances) und [Fehlern](https://developers.deepgram.com/docs/errors) und das OpenAPI-Dokument von `deepgram/deepgram-api-specs` beim Commit `0ccc6f4`, gelesen am 2026-11-14.

## Entscheidungen

- **Die Sprache wird erkannt, wenn die App keine nennt.** OpenAI erkennt die Sprache; Deepgram nimmt Englisch an. Kaeso sendet dann `detect_language=true`, das eine Sprache wählt und den Preis für eine Sprache abrechnet, nie `language=multi`.
- **Immer Smart Formatting.** Kaeso sendet `smart_format=true`, das bei Deepgram nichts kostet, damit das Transkript wie bei OpenAI Satzzeichen und Großschreibung hat.
- **Jede Transkriptform kommt aus einer JSON-Antwort.** Deepgram antwortet nur mit JSON. Für `srt`, `vtt`, `verbose_json` und `diarized_json` fragt Kaeso `utterances=true` an und schreibt die Form aus den Utterances: ein Untertitel oder Segment je Utterance, für `diarized_json` mit seinem Sprecher.
- **Die Stimme ist das Modell.** Deepgram nennt jede Aura-Stimme als Modell, `aura-2-thalia-en`. Die Modelle von Kaeso sind die Familien `aura-2` und `aura`, und `voice` nennt die Stimme: `thalia` (Englisch, sofern die Anfrage keine Sprache nennt), `thalia-en` oder die ganze ID.
- **Ganze Transkripte, gestreamtes Audio.** Ein Transkript wird ganz beantwortet; eine streamende App erhält es als ein Stück und das Ende. Sprache kommt zurück, wie Deepgram sie sendet, Stück für Stück.
- **Zeichen aus dem Header.** `dg-char-count` ist, was Deepgram abrechnet, und wird für die Kosten zu den Zeichen der Anfrage. `metadata.duration` eines Transkripts, mal seine Kanäle, ist die abgerechnete Länge.
- **Die Standardwerte von Deepgram bleiben.** Kaeso sendet kein `mip_opt_out`, also gilt die Einstellung des Model Improvement Program, und kein `callback`, `redact`, `keyterm` oder andere bezahlte Zusätze.

## Transkriptionsanfrage

| KPP | Deepgram | Hinweise |
| --- | --- | --- |
| `model` | `?model=` | |
| `audio`, hochgeladen | der Body, mit dem Medientyp der Datei als `content-type` | `application/octet-stream`, wenn der Upload keinen nennt; Deepgram liest das Format aus den Bytes. |
| `audio`, ein Link | `{"url": …}` als JSON | |
| `language` | `?language=` | Ohne sie: `?detect_language=true`. |
| `format`, das Zeiten braucht, `timestamps: [segment]` | `?utterances=true` | Wörter haben immer Zeiten. |
| `diarize` oder `format: diarized_json` | `?diarize_model=latest` | `diarize` (ohne `_model`) ist veraltet. |
| — | `?smart_format=true` | |
| Query-Parameter einer Anfrage im Format von Deepgram | dieselben Parameter, in ihrer Reihenfolge | Andere Dialekte lehnen sie ab. |

## Sprachanfrage

| KPP | Deepgram | Hinweise |
| --- | --- | --- |
| `model` und `voice` | `?model=` | `aura-2` mit `thalia` ist `aura-2-thalia-en`; ein Modell, das seine Stimme nennt, geht unverändert. |
| `input` | `{"text": …}` | Höchstens 2.000 Zeichen. |
| `format` | `?encoding=`: `mp3`, `opus`, `aac`, `flac`; `wav` ist `linear16` mit `container=wav`; `pcm16` ist `linear16` mit `container=none` und `sample_rate=24000` | `linear16` von Deepgram ist 16-Bit-PCM in Little Endian. Ohne Format `mp3`. |
| `sample_rate` | `?sample_rate=` | |
| `bitrate` (Kilobit) | `?bit_rate=` (Bit pro Sekunde) | |
| `speed` | `?speed=` | 0,7 bis 1,5. |

## Abgelehnt

Jedes davon ist `Unsupported` oder eine ungültige Anfrage, bevor etwas gesendet wird, mit dem Feld in `param`:

| KPP | `param` | Grund |
| --- | --- | --- |
| Übersetzungen | `model` | Deepgram transkribiert in der gesprochenen Sprache. |
| `prompt`, `temperature`, `num_speakers` | das Feld | Deepgram hat kein Gegenstück. |
| eine Datei-ID | `file` | |
| eine andere Sprachoperation als `Speech`, Dialog oder Quellaudio | `model`, `input` | |
| `instructions`, `seed`, Zeitstempel, `duration` | das Feld | |
| ein anderes Format als mp3, opus, aac, flac, wav oder pcm | `response_format` | |
| `speed` außerhalb von 0,7 bis 1,5 | `speed` | |
| Text über 2.000 Zeichen | `input` | Deepgram antwortet darüber mit 413. |
| keine Stimme bei `aura-2` oder `aura`, eine Stimme einer anderen Familie, eine Stimme mit anderen Zeichen als Kleinbuchstaben, Ziffern und Bindestrichen | `voice` | |

## Transkriptionsantwort

| Deepgram | KPP |
| --- | --- |
| `results.channels[0].alternatives[0].transcript` | `text` |
| ihre `words[]`: `punctuated_word` oder `word`, `start`, `end`, `speaker` | `words` |
| `results.utterances[]`: `transcript`, `start`, `end`, `speaker`, `id` | `segments` |
| `results.channels[0].detected_language` | `language` |
| `metadata.duration` | `duration`; mal `metadata.channels` für die abgerechnete Länge |
| der Rest von `metadata`, des Kanals, der Alternative, von `results` und den Utterances | Extras |

Eine Antwort mit mehr als einem Kanal oder einer Alternative ist ein `ProviderError`: Kaeso fragt keines von beiden an.

## Sprachantwort

Das Audio selbst, mit seinem `content-type`; `audio/mpeg`, wenn es keinen hat. `dg-char-count` sind die abgerechneten Zeichen.

## Fehler

| Deepgram | KPP |
| --- | --- |
| `INVALID_AUTH`, 401 | `Authentication` |
| `INSUFFICIENT_PERMISSIONS`, 403 | `PermissionDenied` |
| `ASR_PAYMENT_REQUIRED`, 402 | `Billing` |
| `Payload Too Large`, `PAYLOAD_TOO_LARGE`, 413 | `RequestTooLarge` |
| `TOO_MANY_REQUESTS`, `Too Many Requests`, 429 | `RateLimited` |
| `Bad Request` und andere 400er, 415, 422 | `InvalidRequest` |
| 404 | `NotFound` |
| 503 | `Overloaded` |
| 504 | `Timeout` |

Die Meldung ist `err_msg`, oder `message` und `details`, oder ein Body aus reinem Text, wie er ist. Die eigenen Fehler von Deepgram gehen an eine App im Format von Deepgram zurück, wie sie kamen.

## Fixtures

Die `README.md` des Ordners nennt jede Datei, ihre Herkunft und wo Deepgram von den Audio-APIs von OpenAI abweicht. Keine erfolgreiche Antwort ist aufgezeichnet: Es gab keinen Deepgram-Schlüssel.
