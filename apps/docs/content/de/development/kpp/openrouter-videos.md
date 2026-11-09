---
title: OpenRouter Videos
description: Wie die Videojobs von KPP auf die Videoerzeugungs-API von OpenRouter abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob OpenRouter erreicht: welches Feld wohin gehört, wo seine Datei abgerufen wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::openrouter_videos::OpenRouterVideos` (`crates/kaeso-codecs/src/openrouter_videos/`)
- **Pfade:** nach der Basis-URL von OpenRouter (`https://openrouter.ai/api`): `POST /v1/videos`, `GET /v1/videos/{jobId}` und `GET /v1/videos/{jobId}/content`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openrouter_videos/`, aus dem OpenAPI-Dokument von OpenRouter gebaut, keine aufgezeichnet

**Quellen.** Die Referenzen von OpenRouter zu [Absenden](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request), [Abfragen](https://openrouter.ai/docs/api/api-reference/video-generation/poll-video-generation-status) und [Inhalt](https://openrouter.ai/docs/api/api-reference/video-generation/download-generated-video-content), gelesen am 2026-11-09.

## Entscheidungen

- **Kein Remix.** `previous_job_id` bearbeitet oder verlängert einen fertigen Job, und die Referenz sagt nicht, welches von beiden; deshalb wird ein Remix abgelehnt (Parameter `video_id`).
- **Wo die Datei liegt.** Zuerst werden die signierten `urls` eines fertigen Jobs abgerufen, dann `unsigned_urls` mit dem Schlüssel, dann der Inhaltspfad.

## Anfrage

| KPP | OpenRouter |
| --- | --- |
| `model`, `prompt`, `user` | `model`, `prompt`, `user` |
| `seconds` | `duration` |
| `size`, `resolution`, `aspect_ratio` | `size`, `resolution`, `aspect_ratio`, jeweils aus den Listen von OpenRouter |
| `seed`, `audio` | `seed`, `generate_audio` |
| Bilder `FirstFrame`, `LastFrame` | `frame_images[]` mit `frame_type` |
| Bilder `Reference` | Einträge `input_references[]` `image_url` |
| `provider`, `callback_url`, `session_id`, `trace`; Audio- oder Videoreferenzen | Extras |

## Antwort

| OpenRouter | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `pending`, `in_progress`, `completed` | `queued`, `in_progress`, `completed` |
| `status` `failed`, `cancelled`, `expired` | `failed`, mit dem Status als Code |
| `error` (ein String) | `error.message` |
| `expires_at` | `expires_at` |
| `urls`, `unsigned_urls` | der Ausgabelink, und Extras |
| `usage`, `polling_url`, `generation_id` | Extras |
