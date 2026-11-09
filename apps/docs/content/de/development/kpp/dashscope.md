---
title: DashScope
description: Wie die Videojobs von KPP auf die Videosynthese von DashScope bei QwenCloud für die Wan-Modelle abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob die Wan-Modelle von QwenCloud erreicht: welchen Body ein Modell nimmt, welches Feld wohin gehört, wie nach der Aufgabe gefragt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::dashscope::DashScope` (`crates/kaeso-codecs/src/dashscope/`)
- **Pfade:** ab der Wurzel des Hosts von QwenCloud, mit `Authorization: Bearer` und `X-DashScope-Async: enable`: `POST /api/v1/services/aigc/video-generation/video-synthesis`, `POST /api/v1/services/aigc/image2video/video-synthesis` (erstes und letztes Bild bei Wan 2.2 und 2.1) und `GET /api/v1/tasks/{task_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/dashscope/`, aus den OpenAPI-Dokumenten von QwenCloud gebaut, keine aufgezeichnet

**Quellen.** Die Referenzen von QwenCloud zu [Text zu Video bis Wan 2.6](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task), [Bild zu Video aus dem ersten Bild](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task), [Text zu Video mit Wan 2.7](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task) und [seinem Abfrageergebnis](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result), [Bild zu Video mit Wan 2.7](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task), [Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task) und [asynchronen Aufgaben](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks), gelesen am 2026-11-09.

## Entscheidungen

- **Das Modell wählt den Body.** `dashscope::family` liest den Modellnamen: `wan3.0-video*`, `wan2.7-t2v*`, `wan2.7-i2v*`, `*-kf2v-*` (erstes und letztes Bild), `*-t2v*` und `*-i2v*` (Wan 2.6 und älter). Modelle, die Video bearbeiten oder referenzieren, etwa `wan2.7-r2v`, `wan2.7-videoedit`, `wan2.1-vace-plus` und `wan2.2-animate-*`, werden mit `param` `model` abgelehnt.
- **Größen nur, wo QwenCloud sie listet.** Wan 2.6 und älter nehmen eine Pixelgröße `size` als `width*height`. Text zu Video mit Wan 2.7 nimmt eine Stufe und ein Verhältnis, deshalb geht eine `size` nur durch, wenn sie eine der zehn Größen seiner Referenz ist, so wie `1104x832` `720P` bei `4:3` ist. Bild zu Video nimmt nur eine Stufe, weil das Bild das Seitenverhältnis bestimmt, deshalb wird eine `size` dort abgelehnt.
- **Kein Remix, kein Löschen.** DashScope bricht nur Aufgaben ab, die noch nicht begonnen haben, und das ist kein Löschen; deshalb vergisst Kaeso ein gelöschtes Video.
- **Zeiten sind Pekinger Zeit.** `submit_time` und `end_time` sind UTC+8 ohne Offset; `dashscope::unix_of_beijing` liest sie. Der Link des Videos gilt 24 Stunden, deshalb läuft ein fertiger Job 24 Stunden nach `end_time` ab.

## Anfrage

| KPP | Wan 2.6 und älter | Wan 2.7 | Wan 3.0 |
| --- | --- | --- | --- |
| `model`, `prompt` | `model`, `input.prompt` | ebenso | ebenso |
| `negative_prompt` | `input.negative_prompt` | `input.negative_prompt` | abgelehnt |
| Bild `FirstFrame` | `input.img_url`; erstes und letztes Bild bei Wan 2.2 und 2.1: `input.first_frame_url` | `input.media[]` `{type: "first_frame", url}` | ebenso |
| Bild `LastFrame` | `input.last_frame_url`, nur Weblinks | `input.media[]` `{type: "last_frame", url}` | ebenso |
| Bilder `Reference` | abgelehnt | abgelehnt | `input.media[]` `{type: "reference_image", url}`, nie zusammen mit Bildern für Anfang oder Ende |
| `size` | `parameters.size`, `1280*720`, auch aus einer Auflösung und einem Seitenverhältnis (Text zu Video) | `parameters.resolution` und `ratio`, aus den gelisteten Größen (Text zu Video) | abgelehnt |
| `resolution` | `parameters.resolution` (Bild zu Video), in Großbuchstaben | `parameters.resolution` | `parameters.resolution` |
| `aspect_ratio` | abgelehnt | `parameters.ratio` (Text zu Video) | `parameters.ratio` |
| `seconds`, `seed` | `parameters.duration`, `parameters.seed` | ebenso | ebenso |
| `audio` | `parameters.audio`, nur `wan2.6-i2v-flash` | abgelehnt | `parameters.audio` |
| andere Felder von `input` und `parameters`, etwa `audio_url`, `prompt_extend`, `watermark` | Extras | Extras | Extras |

Bilder gehen als Links oder Data-URLs, außer erstem und letztem Bild bei Wan 2.2 und 2.1, die QwenCloud nur als öffentliche Links nimmt. Medien anderer Typen, etwa `driving_audio` oder `first_clip`, werden beim Dekodieren einer Anfrage abgelehnt.

## Antwort

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`, `RUNNING`, `SUCCEEDED` | `queued`, `in_progress`, `completed` |
| `output.task_status` `FAILED`, `CANCELED`, `UNKNOWN` | `failed`, mit `output.code` und `output.message`, oder dem Status als Code |
| `output.video_url` | der Link des Videos, `video/mp4` |
| `output.submit_time`, `output.end_time` | `created_at`, `completed_at` |
| `usage.output_video_duration`, `usage.SR`, `usage.ratio` | `seconds`, `resolution`, `aspect_ratio` |
| `request_id`, andere Felder von `output` und `usage` | Extras |

Fehler sind `{"request_id", "code", "message"}`: `InvalidApiKey` ist ein Authentifizierungsfehler, `Throttling…` ein Ratenlimit und `DataInspection…` ein Inhaltsfilter; andere Codes folgen dem HTTP-Status.
