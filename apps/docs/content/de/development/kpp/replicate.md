---
title: Replicate
description: Wie die Bildanfragen und Videojobs von KPP auf die Predictions von Replicate abgebildet werden.
order: 8
---

Hier sehen Sie, wie eine Bildanfrage oder ein Videojob Replicate erreicht: welches Feld bei welchem Modell wohin geht, wie Kaeso einer Prediction folgt und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::replicate::Replicate` (`crates/kaeso-codecs/src/replicate/`)
- **Pfade:** mit `Authorization: Bearer r8_…`: `POST /v1/models/{owner}/{name}/predictions` mit `{"input": …}`, danach `GET /v1/predictions/{id}`; `GET /v1/collections/official` für die Modellprüfung
- **Fixtures:** `crates/kaeso-codecs/fixtures/replicate/`, gebaut aus der Dokumentation von Replicate und dem Schema jedes Modells, mit Predictions, die Replicate auf seinen Modellseiten veröffentlicht hat, und live aufgezeichneten abgelehnten Tokens

**Quellen.** Die [HTTP-API](https://replicate.com/docs/reference/http) von Replicate und ihr OpenAPI-Dokument, [Prediction erstellen](https://replicate.com/docs/topics/predictions/create-a-prediction), [Ausgabedateien](https://replicate.com/docs/topics/predictions/output-files), [Datenaufbewahrung](https://replicate.com/docs/topics/predictions/data-retention), [Ratenlimits](https://replicate.com/docs/topics/predictions/rate-limits), [offizielle Modelle](https://replicate.com/docs/topics/models/official-models) und das Schema jedes Modells (`https://replicate.com/{owner}/{name}/api/schema`), gelesen am 14.11.2026.

## Predictions

Eine Bildanfrage fragt mit `Prefer: wait=60`, daher antwortet Replicate, sobald die Bilder fertig sind, bei den meisten Modellen innerhalb dieser Anfrage. Eine Prediction, die nach der Wartezeit noch `starting` oder `processing` ist, wird über ihre ID erneut gefragt, bis sie endet, innerhalb des Zeitlimits der Anfrage. Ein Videojob startet ohne zu warten; seine ID ist die der Prediction, und jeder Abruf fragt `GET /v1/predictions/{id}`. Kaeso fragt nur `/v1/predictions/{id}` mit einer ID aus einfachen Zeichen, nie die `urls`, die eine Prediction nennt.

Replicate löscht Eingabe, Ausgabe und Dateien einer Prediction eine Stunde nach ihrem Ende. Die `url` eines Bildes gilt diese Stunde lang, wie die Bildlinks von OpenAI selbst; die Datei eines Videos lässt sich bis dahin herunterladen, und `expires_at` seines Jobs sagt, bis wann.

## Modelle

Jedes Modell hat eine eigene Eingabe, daher bildet Kaeso nur diese offiziellen Modelle ab und lehnt jedes andere mit der Liste ab:

| Modell | Art | Nimmt |
| --- | --- | --- |
| `black-forest-labs/flux-schnell`, `black-forest-labs/flux-dev` | Bild | `n` 1 bis 4, `aspect_ratio` (`1:1`, `16:9`, `21:9`, `3:2`, `2:3`, `4:5`, `5:4`, `3:4`, `4:3`, `9:16`, `9:21`), `seed`, `output_format` `webp`, `jpeg` oder `png`, `output_compression`; keine Pixelgröße |
| `black-forest-labs/flux-1.1-pro` | Bild | Ein Bild; `size` mit 256 bis 1440 je Seite in Schritten von 32 oder `aspect_ratio`; `seed`, `output_format`, `output_compression` |
| `bytedance/seedream-4.5` | Bild und Bearbeitung | `n` 1 bis 15 (das Modell kann weniger erzeugen); `size` mit 1024 bis 4096 je Seite oder `aspect_ratio` und `resolution` `2K` oder `4K`; bis zu 14 Bilder; kein `seed` und kein `output_format` |
| `google/nano-banana-pro` | Bild und Bearbeitung | Ein Bild; `aspect_ratio` und `resolution` `1K`, `2K` oder `4K`; bis zu 14 Bilder; `output_format` `jpeg` oder `png` |
| `google/veo-3.1` | Video | `seconds` 4, 6 oder 8; `size` in 720p oder 1080p, 16:9 oder 9:16; `audio`, `seed`, `negative_prompt`; ein erstes und ein letztes Bild oder 1 bis 3 Referenzbilder für 8 Sekunden in 16:9 |
| `kwaivgi/kling-v2.5-turbo-pro` | Video | `seconds` 5 oder 10; `aspect_ratio` `16:9`, `9:16` oder `1:1`, außer ein erstes Bild legt die Form fest; `negative_prompt`; ein erstes und ein letztes Bild |

## Bildanfrage

| KPP | `input` von Replicate |
| --- | --- |
| `prompt` | `prompt` |
| `n` | `num_outputs` bei FLUX.1; `max_images` mit `sequential_image_generation: "auto"` bei Seedream |
| `size` `BREITExHÖHE` | `aspect_ratio: "custom"` mit `width` und `height` bei FLUX1.1 [pro], `size: "custom"` mit `width` und `height` bei Seedream; `auto` sendet nichts |
| `aspect_ratio` | `aspect_ratio` |
| `resolution` | `size` bei Seedream, `resolution` bei Nano Banana Pro |
| `output_format` | `output_format`, `jpeg` als `jpg` |
| `output_compression` | `output_quality` |
| `seed` | `seed` |
| zu bearbeitende Bilder | `image_input`: Links wie gekommen, Uploads als Data-URIs |
| `response_format: b64_json` | nichts: Kaeso lädt die Links von `replicate.delivery` herunter |

Die Schemas der Modelle sagen nicht, ob Replicate Felder ablehnt, die ihnen fehlen, daher lehnt Kaeso vor dem Senden `quality` und `background` außer `auto`, `negative_prompt`, `user`, eine Maske, Streaming und Variationen ab, und `n` über 1 bei den Modellen, die ein Bild erzeugen. Die eigenen Felder einer Anfrage im Format von Replicate, etwa `guidance` oder `safety_tolerance`, werden wie geschrieben gesendet.

## Bildergebnis

| Replicate | KPP |
| --- | --- |
| `output`: ein Link oder eine Liste von Links | die Links der Bilder |
| `input.width`, `input.height` bei eigener Größe | die Größe der Antwort |
| `input.resolution` oder `input.size` von Seedream | die Auflösung, zu der die Bilder abgerechnet werden |
| `status` `failed`, `canceled` oder `aborted` | ein Anbieterfehler mit `error` als Meldung |
| `id`, `version`, `input`, `logs`, `metrics`, `urls` und die Zeiten | Extras |

## Videojob

| KPP | `input` von Replicate |
| --- | --- |
| `prompt` | `prompt` |
| `seconds` | `duration` |
| `size` | `resolution` aus der kurzen Seite und `aspect_ratio`, nur Veo |
| `aspect_ratio`, `resolution` | `aspect_ratio`, `resolution` |
| `audio` | `generate_audio`, nur Veo; Veo erzeugt Ton, außer er wird abgeschaltet, zum doppelten Preis |
| `seed`, `negative_prompt` | `seed` (nur Veo), `negative_prompt` |
| erstes und letztes Bild | `image` und `last_frame` bei Veo, `start_image` und `end_image` bei Kling |
| Referenzbilder | `reference_images`, nur Veo |

Ein Videojob braucht `seconds`, da Replicate jede Sekunde abrechnet. Kaeso lehnt `n` über 1, `user` und Remixe ab, und bei Kling `size`, `seed` und `audio`, die sein Schema behält, aber laut eigener Beschreibung nicht verwendet.

## Status

| Replicate | KPP |
| --- | --- |
| `starting` | `queued` |
| `processing` | `in_progress` |
| `succeeded` | `completed`, mit `output` als Link des Videos; `expires_at` liegt eine Stunde nach `completed_at` |
| `failed`, `canceled`, `aborted` | `failed`, mit dem Status als Code und `error` als Meldung |

## Fehler

| Replicate | KPP |
| --- | --- |
| `{"title", "detail", "status"}` (Problem Details) | `title: detail` als Meldung; die Art aus dem Status: 401 `authentication`, 402 `billing`, 404 `not_found`, 422 `invalid_request`, 429 `rate_limited` |
| `{"detail": "…"}` | `detail` als Meldung, die Art aus dem Status |
| `Retry-After` | die Wartezeit bis zum nächsten Versuch |

## Modellprüfung

**Verfügbarkeit prüfen** liest `GET /v1/collections/official`, die offiziellen Modelle von Replicate, und vergleicht das `owner/name` jedes Modells. Ein Modell, das die Sammlung auslässt, ist unklar, nicht nicht verfügbar.
