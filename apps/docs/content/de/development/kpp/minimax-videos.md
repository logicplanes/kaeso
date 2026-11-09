---
title: MiniMax Videos
description: Wie die Videojobs von KPP auf die Video-APIs Hailuo und H3 von MiniMax abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie ein Videojob MiniMax erreicht: an welche API ein Modell geht, welches Feld wohin gehört und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::minimax_videos::MinimaxVideos` (`crates/kaeso-codecs/src/minimax_videos/`)
- **Pfade:** ab der Wurzel des Hosts von MiniMax, mit `Authorization: Bearer`: `POST /v1/video_generation`, `GET /v1/query/video_generation`, `GET /v1/files/retrieve`; `POST /v2/video_generation`, `GET /v2/query/video_generation/{task_id}`, `DELETE /v2/video_generation/{task_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_videos/`, aus den OpenAPI-Dokumenten von MiniMax gebaut, keine aufgezeichnet

**Quellen.** Die Referenzen von MiniMax zu [Text zu Video](https://platform.minimax.io/docs/api-reference/video-generation-t2v), [Bild zu Video](https://platform.minimax.io/docs/api-reference/video-generation-i2v), [erstem und letztem Bild](https://platform.minimax.io/docs/api-reference/video-generation-fl2v), [Abfrage](https://platform.minimax.io/docs/api-reference/video-generation-query), [Datei abrufen](https://platform.minimax.io/docs/api-reference/file-management-retrieve) und zur [H3-API](https://platform.minimax.io/docs/api-reference/video-generation-v2-create), gelesen am 2026-11-09.

## Entscheidungen

- **Das Modell wählt die API.** `MiniMax-H3` und seine Varianten gehen an die H3-API (`/v2`); die Hailuo-Modelle an `/v1`.
- **Eine Hailuo-Datei ist eine Datei-ID.** Ein fertiger Hailuo-Job nennt eine `file_id`; `encode_video_file` fragt bei jedem Abruf der Datei `GET /v1/files/retrieve` nach ihrer `download_url`, die eine Stunde gilt.
- **Fehler können mit HTTP 200 kommen.** Ein scheiternder `base_resp` ist ein Fehler, gleich welcher Status, nach seinem Code abgebildet wie bei den anderen APIs von MiniMax.
- **H3 braucht Länge und Auflösung.** MiniMax verlangt beides, deshalb wird eine Anfrage ohne `seconds` oder ohne Größe vor dem Senden abgelehnt (Parameter `seconds` und `size`).

## Anfrage

| KPP | Hailuo (`/v1`) | H3 (`/v2`) |
| --- | --- | --- |
| `model` | `model` | `model` |
| `prompt` | `prompt` | `content[]` `{type: "text"}` |
| `seconds` | `duration` | `duration` |
| `resolution` | `resolution` (`512P`, `720P`, `768P`, `1080P`) | `resolution` (`480P`, `768P`, `2K`) |
| `size` | abgelehnt | `resolution` und `ratio`, passend zu einem gelisteten Verhältnis auf 1 % genau |
| `aspect_ratio` | abgelehnt | `ratio` |
| Bilder `FirstFrame`, `LastFrame` | `first_frame_image`, `last_frame_image` | `content[]` `image_url` mit `role` |
| Bilder `Reference` | abgelehnt | `content[]` `image_url`, `role: "reference_image"` |
| `prompt_optimizer`, `fast_pretreatment`, `extra`, `callback_url` | Extras | Extras |

Referenzvideos und Referenzaudio (Einträge `video_url`, `audio_url`) werden mit Parameter `content` abgelehnt.

## Antwort

| Hailuo | H3 | KPP |
| --- | --- | --- |
| `task_id` | `task_id` | `VideoJob.id` |
| `Preparing`, `Queueing` | `queued` | `queued` |
| `Processing` | `running` | `in_progress` |
| `Success` | `succeeded` | `completed` |
| `Fail` | `failed`, `cancelled` | `failed` |
| `file_id` | `content.url` | eine Ausgabe als Datei-ID oder Link |
| `video_width`, `video_height` | `resolution`, `duration`, `ratio` | `size`, oder `resolution`, `seconds`, `aspect_ratio` |
| | `error` `{code, message}` | `error` |
