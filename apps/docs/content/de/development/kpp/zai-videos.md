---
title: Z.ai Videos
description: Wie die Videojobs von KPP auf die CogVideoX-API von Z.ai abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob Z.ai erreicht: welches Feld wohin gehört, wie nach dem Job gefragt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::zai_videos::ZaiVideos` (`crates/kaeso-codecs/src/zai_videos/`)
- **Pfade:** ab der Wurzel des Hosts von Z.ai, mit `Authorization: Bearer`: `POST /api/paas/v4/videos/generations` und `GET /api/paas/v4/async-result/{id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/zai_videos/`, aus den OpenAPI-Dokumenten von Z.ai gebaut, keine aufgezeichnet

**Quellen.** Die Referenzen von Z.ai zu [Video erzeugen](https://docs.z.ai/api-reference/video/generate-video) und [Ergebnis abrufen](https://docs.z.ai/api-reference/video/get-video-status), gelesen am 2026-11-09.

## Anfrage

| KPP | Z.ai |
| --- | --- |
| `model`, `prompt` | `model`, `prompt` |
| `seconds` (5 oder 10) | `duration` |
| `size`, oder `resolution` mit `aspect_ratio` | `size`, eine der Größen, die Z.ai listet |
| `audio` | `with_audio` |
| `user` | `user_id` |
| Bilder `FirstFrame`, `LastFrame` | `image_url`, ein Link oder eine Liste von zwei |
| `quality`, `fps`, `request_id` | Extras |

Z.ai nimmt keinen Seed, keinen Negativ-Prompt, keine Referenzbilder und keinen Remix: Jedes davon wird vor dem Senden abgelehnt.

## Antwort

| Z.ai | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `task_status` `PROCESSING`, `SUCCESS`, `FAIL` | `in_progress`, `completed`, `failed` |
| `video_result[].url` | der Link des Videos |
| `video_result[].cover_image_url` | der Link für `thumbnail` |
