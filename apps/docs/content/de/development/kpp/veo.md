---
title: Veo
description: Wie die Videojobs von KPP auf die lang laufenden Vorgänge von Veo auf Gemini Enterprise Agent Platform abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob Veo auf Gemini Enterprise Agent Platform erreicht: welches Feld wohin gehört, wie nach dem Vorgang gefragt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::veo::Veo` (`crates/kaeso-codecs/src/veo/`)
- **Pfade:** `{models}/{model}:predictLongRunning` und `{models}/{model}:fetchPredictOperation`, wobei `{models}` der Publisher-Pfad der Plattform ist (`EgressCtx::gemini_profile`)
- **Fixtures:** `crates/kaeso-codecs/fixtures/veo/`, aus der Referenz der Plattform gebaut, keine aufgezeichnet; Kaeso ruft die Plattform nie live auf

**Quellen.** Die Seiten der Agent Platform zu [Videos aus Text erzeugen](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text) und [Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate), `VideoGenerationModelInstance` und `VideoGenerationModelParams`, gelesen am 2026-11-09.

## Entscheidungen

- **Die Job-ID ist der Name des Vorgangs.** Danach gefragt wird mit `POST …:fetchPredictOperation` und `{"operationName": …}`, beim Modell, das ihn gestartet hat.
- **Videos kommen in der Antwort.** Kaeso nennt keine `storageUri`, deshalb gibt die Plattform jedes Video als `bytesBase64Encoded` in `response.videos` des Vorgangs zurück. Eine `gcsUri` bleibt als Link erhalten, den Kaeso nicht abrufen kann.
- **Nur mit einem Dienstkonto.** Der Express-Modus listet keine Veo-Modelle ([Express-Modus](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/express-mode/overview)), deshalb lehnt das Gateway einen Express-Schlüssel für sie ab.
- **Ein gefilterter Vorgang scheitert.** Ein fertiger Vorgang ohne Videos und mit `raiMediaFilteredCount` ist `failed`, Code `moderation_blocked`.

## Anfrage

| KPP | Veo |
| --- | --- |
| `prompt` | `instances[0].prompt` |
| Bild `FirstFrame` | `instances[0].image` `{bytesBase64Encoded, mimeType}` oder `{gcsUri}` |
| Bild `LastFrame` | `instances[0].lastFrame` |
| Bilder `Reference` | `instances[0].referenceImages[]` mit `referenceType: "asset"` |
| `seconds` (4, 6 oder 8) | `parameters.durationSeconds` |
| `size`, oder `resolution` und `aspect_ratio` | `parameters.resolution` (`720p`, `1080p`, `4k`), `parameters.aspectRatio` (`16:9`, `9:16`) |
| `seed`, `negative_prompt`, `audio`, `count` (1 bis 4) | `seed`, `negativePrompt`, `generateAudio`, `sampleCount` |
| andere Felder der Instanz und der Parameter | Extras |

Bilder per Weblink werden abgelehnt: Die Plattform nimmt Bytes oder Links `gs://`.

## Antwort

| Veo | KPP |
| --- | --- |
| `name` | `VideoJob.id` |
| `done: false` oder fehlend | `in_progress` |
| `response.videos[]` | `completed`, eine Ausgabe pro Video |
| `error` `{code, message}` | `failed` mit diesem Fehler |
