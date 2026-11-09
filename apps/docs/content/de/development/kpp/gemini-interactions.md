---
title: Gemini Interactions
description: Wie die Videojobs von KPP auf Gemini Omni in der Interactions API von Google AI Studio abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob Gemini Omni auf Google AI Studio erreicht: welches Feld wohin gehört, wie nach dem Job gefragt wird und woher jede Abbildung stammt. Abgebildet ist nur der Videoteil der Interactions API.

- **Codec:** `kaeso_codecs::gemini_interactions::GeminiInteractions` (`crates/kaeso-codecs/src/gemini_interactions/`)
- **Pfade:** `POST /v1beta/interactions`, `GET /v1beta/interactions/{id}` und `DELETE /v1beta/interactions/{id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini_interactions/`; `responses/recorded-started.json` und `polls/recorded-failed-quota.json` sind mit einem Schlüssel der kostenlosen Stufe aufgezeichnet, der Rest aus der Referenz gebaut

**Quellen.** Die Referenzen von Google AI Studio zu [Gemini Omni Flash](https://aistudio.google.com/docs/omni) und [Interaktion erstellen](https://aistudio.google.com/api-reference/interactions-api/create-interaction), gelesen am 2026-11-09.

## Entscheidungen

- **Nicht `generateContent`.** Die Modelle listen `generateContent`, aber Google dokumentiert das Video von Omni nur über die Interactions API, deshalb sendet Kaeso Videojobs dorthin.
- **Im Hintergrund.** Jeder Job geht mit `background: true`, das sofort mit der ID der Interaktion antwortet; spätere Fragen danach liefern das Video in der Antwort.
- **Die Aufgabe folgt den Bildern.** Erstes und letztes Bild senden `generation_config.video_config.task: "image_to_video"`, Referenzbilder `"reference_to_video"`; ohne Bilder leitet das Modell die Aufgabe ab.
- **Ein Remix setzt die Interaktion fort.** `previous_interaction_id` verfeinert das frühere Video mit dem neuen Prompt, so wie Googles zustandsbehaftete Bearbeitung.
- **Omni erzeugt immer Ton.** `audio: false` wird abgelehnt (Parameter `audio`); `audio: true` sendet nichts.

## Anfrage

| KPP | Interactions API |
| --- | --- |
| `model` | `model` |
| `prompt` | `input`, ein String, oder ein Eintrag `text` nach den Bildern |
| Bilder | `input[]` `{type: "image", data, mime_type}` oder `{uri}` |
| `seconds` (3 bis 15) | `response_format.duration`, etwa `"8s"` |
| `size`, oder `resolution` und `aspect_ratio` | `response_format.resolution` (`360p`, `720p`, `1080p`, `4k`), `response_format.aspect_ratio` (`16:9`, `9:16`) |
| `remix_of` eines Remix | `previous_interaction_id` |

Omni nimmt keinen Seed, keinen Negativ-Prompt, keine Anzahl und keine Bilder zugleich mit Referenzen.

## Antwort

| Interactions API | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `in_progress` | `in_progress` |
| `status` `completed` mit einem Video | `completed` |
| `status` `failed`, `cancelled`, `incomplete`, oder `completed` ohne Video | `failed`, mit dem ersten Eintrag aus `errors[]` als Fehler |
| Einträge `video` in `steps[]` `model_output` | Ausgaben, Bytes oder Links |
| `usage` `total_input_tokens`, `total_output_tokens`, `total_thought_tokens` | `usage`; Denk-Tokens zählen als Ausgabe und als Reasoning |
| `created`, `updated` | `created_at`, und `completed_at`, sobald er endet |
