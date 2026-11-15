---
title: Cartesia
description: Wie die Sprach- und Transkriptionsanfragen von KPP auf die API von Cartesia abgebildet werden.
order: 8
---

Hier sehen Sie, wie eine Sprach- oder Transkriptionsanfrage Cartesia erreicht: welches Feld wohin geht, was Kaeso ablehnt und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::cartesia::Cartesia` (`crates/kaeso-codecs/src/cartesia/`)
- **Pfade:** mit `X-API-Key` und `Cartesia-Version: 2026-08-14`: `POST /tts/bytes` (JSON hinein, Audio heraus), `POST /stt` (ein Upload hinein, JSON heraus), `GET /voices?limit=1` für die Modellprüfung
- **Fixtures:** `crates/kaeso-codecs/fixtures/cartesia/`, gebaut aus den Referenzseiten, mit live aufgezeichneten abgelehnten Schlüsseln und einer Anfrage ohne Version

**Quellen.** Die [API-Konventionen](https://docs.cartesia.ai/use-the-api/api-conventions) von Cartesia, die Referenzen zu [TTS bytes](https://docs.cartesia.ai/api-reference/tts/bytes) und [STT](https://docs.cartesia.ai/api-reference/stt/transcribe), [Ausgabeformate](https://docs.cartesia.ai/build-with-cartesia/capability-guides/tts-output-audio-format), [Fehler](https://docs.cartesia.ai/use-the-api/api-errors) und die [Anleitung zum Umstieg von Whisper](https://www.cartesia.ai/migrate/openai-whisper), gelesen am 14.11.2026.

## Version

Jede Anfrage trägt `Cartesia-Version: 2026-08-14`, die Version, der diese Abbildungen folgen und die Cartesia als seine neueste nennt. Cartesia hält die Felder einer Version stabil; eine neue Version bedeutet ein neues Release von Kaeso, das die Konstante `kaeso_codecs::cartesia::VERSION` ändert.

## Sprachanfrage

| KPP | `/tts/bytes` von Cartesia |
| --- | --- |
| `model` | `model_id` |
| `input` | `transcript` |
| `voice` | `voice`: die Stimmen-ID als Zeichenkette |
| `response_format` `mp3` | `output_format` `{"container": "mp3", "sample_rate": 44100, "bit_rate": 128000}` |
| `response_format` `wav` | `{"container": "wav", "encoding": "pcm_s16le", "sample_rate": 24000}` |
| `response_format` `pcm` | `{"container": "raw", "encoding": "pcm_s16le", "sample_rate": 24000}`, das 16-Bit-PCM mit 24 kHz von OpenAI |
| `sample_rate`, `bitrate` | `sample_rate` (8000 bis 48000 wie gelistet), `bit_rate` bei mp3 |
| `speed` | `generation_config.speed`, 0,6 bis 1,5 |
| `language` | `language` |

Kaeso lehnt vor dem Senden `opus`, `aac` und `flac` ab, die Cartesia nicht erzeugt, sowie `instructions`, `seed`, Zeitstempel und eine Anfrage ohne Stimme. Das Audio kommt gestreamt zurück, während Cartesia es erzeugt. Eine Stimme, die Cartesia nicht kennt, beantwortet es mit `voice_not_found`, das die App als Nicht-gefunden-Fehler zu `voice` erreicht.

## Transkriptionsanfrage

| KPP | `/stt` von Cartesia |
| --- | --- |
| `file` | `file` |
| `model` | `model` |
| `language` | `language`, erforderlich: die Batch-API nimmt Englisch an und erkennt nichts |
| `verbose_json`, `srt`, `vtt`, Wort-Zeitstempel | `timestamp_granularities[]=word`; Kaeso schreibt Segmente und Untertitel aus den Wörtern |

Kaeso lehnt vor dem Senden `prompt`, `temperature`, Segment-Zeitstempel, Sprecherangaben (`diarized_json`), Übersetzungen, Streaming und Audio per Link ab.

## Transkriptionsergebnis

| Cartesia | KPP |
| --- | --- |
| `text` | der Text |
| `language`, `duration` | die Sprache und die Länge des Audios, nach der abgerechnet wird |
| `words[]` `{word, start, end}` | zeitlich markierte Wörter, ein führendes Leerzeichen bleibt |
| `type`, `request_id` | Extras |

## Fehler

| Cartesia | KPP |
| --- | --- |
| `{"error_code", "title", "message", "request_id"}` | `message`; `voice_not_found` ist nicht gefunden zu `voice`, `model_not_found` und `model_sunsetted` zu `model`, `quota_exceeded` ein Abrechnungsfehler, `concurrency_limited` ein Ratenlimit, `file_too_large` zu groß, sonst die Art aus dem Status |
| reiner Text, etwa bei fehlendem `Cartesia-Version` | der Text, die Art aus dem Status |

## Modellprüfung

Cartesia listet keine Modelle, daher liest **Verfügbarkeit prüfen** `GET /voices?limit=1`, was nur zeigt, dass der Schlüssel funktioniert.
