---
title: SpaceX Videos
description: Wie die Videojobs von KPP auf die Grok-Imagine-Video-API von SpaceX abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob SpaceX erreicht: welches Feld wohin gehört, wie nach dem Job gefragt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::xai_videos::XaiVideos` (`crates/kaeso-codecs/src/xai_videos/`)
- **Pfade:** `POST /v1/videos/generations`, `POST /v1/videos/edits` und `GET /v1/videos/{request_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/xai_videos/`, aus der Referenz von SpaceX gebaut, keine aufgezeichnet

**Quellen.** Die [Video-Referenz](https://docs.x.ai/developers/rest-api-reference/inference/videos) und der [Leitfaden zur Videoerzeugung](https://docs.x.ai/developers/model-capabilities/video/generation) von SpaceX, gelesen am 2026-11-09.

## Entscheidungen

- **Ein Remix ist eine Bearbeitung.** `POST /v1/videos/edits` ändert ein fertiges Video mit einem neuen Prompt, und genau das tut der Remix von OpenAI. Es nimmt das frühere Video per Link, deshalb fragt das Gateway SpaceX zuerst nach diesem Link und sendet ihn als `video.url` (`VideoRequest.source_video`).
- **Die Job-ID ist `request_id`.** Der Start eines Jobs antwortet nur mit `request_id`; Status, Link und Länge des Jobs kommen von `GET /v1/videos/{request_id}`.
- **Ein zurückgehaltenes Video scheitert.** `status: "done"` mit `respect_moderation: false` trägt keinen Link; KPP meldet es als `failed` mit dem Code `moderation_blocked`. `expired` ist `failed` mit dem Code `expired`.

## Anfrage

| KPP | SpaceX |
| --- | --- |
| `model`, `prompt`, `user` | `model`, `prompt`, `user` |
| `seconds` (1 bis 15) | `duration` |
| `aspect_ratio`, `resolution` (`480p`, `720p`, `1080p`) | `aspect_ratio`, `resolution` |
| `size` | `resolution` und `aspect_ratio`, wo SpaceX beides listet |
| Bild `FirstFrame` | `image` `{url}` oder `{file_id}` |
| Bilder `Reference` | `reference_images[]` |
| `source_video` eines Remix | `video.url` auf dem Pfad für Bearbeitungen |
| `keyframes`, `reference_audios`, `output`, `storage_options` | Extras |

SpaceX nimmt kein letztes Bild, keinen Seed, keinen Negativ-Prompt und keine Anzahl: Jedes davon wird vor dem Senden abgelehnt, mit seinem Feld in `param`.

## Antwort

| SpaceX | KPP |
| --- | --- |
| `request_id` | `VideoJob.id` |
| `status` `pending` | `queued`, solange `progress` 0 ist, danach `in_progress` |
| `status` `done`, `failed`, `expired` | `completed`, `failed`, `failed` |
| `progress`, `model` | `progress`, `model` |
| `video.url`, `video.duration` | ein Ausgabelink, `seconds` |
| `error` `{code, message}` | `error` |
| `usage`, `video.file_output` | Extras |
