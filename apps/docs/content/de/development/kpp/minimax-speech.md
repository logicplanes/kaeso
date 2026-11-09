---
title: MiniMax Speech
description: Wie die Sprachanfragen von KPP auf die synchrone Text-to-Speech-API von MiniMax abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie eine Sprachanfrage MiniMax erreicht: welches Feld wohin gehört, was abgelehnt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::minimax_speech::MinimaxSpeech` (`crates/kaeso-codecs/src/minimax_speech/`)
- **Pfad:** `POST /v1/t2a_v2` (T2A v2), ab der Wurzel des Hosts von MiniMax, mit `Authorization: Bearer`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_speech/`, aus dem OpenAPI-Dokument von MiniMax gebaut, keine aufgezeichnet

**Quellen.** Die Referenz von MiniMax zu [synchronem Text to Speech](https://platform.minimax.io/docs/api-reference/speech-t2a-http), ihr [OpenAPI-Dokument](https://platform.minimax.io/docs/api-reference/speech/t2a/api/openapi.json) und ihre [Fehlercodes](https://platform.minimax.io/docs/api-reference/errorcode), gelesen am 2026-11-08.

## Entscheidungen

- **Ganze Antworten.** Die Stream-Rahmung von MiniMax ist nicht dokumentiert, deshalb sendet Kaeso immer `stream: false` und liest die ganze Antwort. Für eine streamende App wird die Antwort ein Stück Audio und das Ende, mit der Nutzung und den abgerechneten Zeichen.
- **Audio als Hex.** Kaeso fragt `output_format: hex` an, damit das Audio in der Antwort kommt und nicht als Link. In KPP wird das Hex zu Base64.
- **Zeichen aus der Antwort.** `extra_info.usage_characters` ist, was MiniMax abrechnet, und wird für die Kosten zu `SpeechResponse::characters`.
- **Fehler können mit HTTP 200 kommen.** Ein fehlgeschlagenes `base_resp` ist unabhängig vom Status ein Fehler, nach seinem Code abgebildet wie bei [MiniMax Images](minimax-images.md#fehler).

## Anfrage

| KPP | MiniMax | Hinweise |
| --- | --- | --- |
| `model` | `model` | |
| `input` | `text` | |
| `voice` | `voice_setting.voice_id` | Ein Name oder eine ID geht wie geschrieben: Die Stimmen von MiniMax hängen vom Modell ab. |
| `speed` | `voice_setting.speed` | Eine ganze Zahl bleibt eine. |
| `format` | `audio_setting.format`: `mp3`, `wav`, `flac`, `opus` und `pcm` für `Pcm16` | |
| `sample_rate` | `audio_setting.sample_rate` | `Pcm16` ohne Rate erhält `24000`, weil `pcm16` in KPP 24 kHz hat. |
| `bitrate` (Kilobit) | `audio_setting.bitrate` (Bit pro Sekunde) | |
| `stream` | `stream: false` | Kaeso fragt MiniMax nie nach einem Stream. |
| — | `output_format: hex` | Gesendet, sofern eine Anfrage im Format von MiniMax kein anderes nannte. |
| andere Schlüssel von `voice_setting` und `audio_setting`, `pronunciation_dict`, `voice_modify`, `language_boost` und andere Schlüssel | Extras | Nur aus einer Anfrage im Format von MiniMax gesendet; andere Dialekte lehnen sie ab. |

## Abgelehnt

Jedes davon ist `Unsupported`, bevor etwas gesendet wird, mit dem Feld in `param`:

| KPP | `param` | Grund |
| --- | --- | --- |
| eine andere Operation als `Speech`: Dialog, Soundeffekte, Musik, Stimmwechsel | `model` | T2A erzeugt Sprache nur aus Text. |
| Dialogzeilen oder Quellaudio | `input` | |
| ein anderes Format als mp3, pcm, flac, wav oder opus | `response_format` | Kaeso wandelt Audio nie um. |
| `instructions`, `language`, `seed`, Zeitstempel, `duration` | das Feld | T2A v2 hat keine solchen Felder, die Kaeso abbildet. |

## Antwort

| MiniMax | KPP |
| --- | --- |
| `data.audio`, Hex | das Audio als Base64; Hex, das sich nicht dekodieren lässt, ist ein `ProviderError` |
| `extra_info.audio_format` | der Medientyp des Audios (`audio/mpeg`, `audio/wav`, `audio/flac`, `audio/opus`, `audio/pcm`); `mp3`, wenn es fehlt |
| `extra_info.usage_characters` | `characters` |
| der Rest von `data`, `extra_info`, `base_resp` | Extras |

Einer App im Format von MiniMax schreibt der Codec das Audio als Hex in `data.audio`, mit `base_resp` `0`.

## Fixtures

| Datei | Deckt ab | Gebaut aus |
| --- | --- | --- |
| `requests/speech.json` | Stimm- und Audioeinstellungen, `pronunciation_dict`, `voice_modify`, `language_boost`, Ausgabe als Hex | Dem Beispiel "Request(Non-streaming)" des OpenAPI-Dokuments |
| `responses/speech.json` | Audio als Hex, `extra_info` mit `usage_characters`, `base_resp` | Dem Antwortbeispiel desselben Dokuments; das Hex sind zehn erfundene MP3-Bytes |
| `errors/invalid-params-200.json` | Ein fehlgeschlagenes `base_resp` mit HTTP 200 | Dem Schema `T2aV2Resp.base_resp` und den Fehlercodes; die Meldung ist erfunden |

Es gab keinen MiniMax-Schlüssel, deshalb ist nichts aufgezeichnet. Die `README.md` des Ordners nennt die Herkunft.
