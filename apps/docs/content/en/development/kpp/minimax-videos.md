---
title: MiniMax Videos
description: How KPP's video jobs map to MiniMax's Hailuo and H3 video APIs.
order: 8
---

Look up how a video job reaches MiniMax here: which API a model goes to, which field goes where, and the source of each mapping.

- **Codec:** `kaeso_codecs::minimax_videos::MinimaxVideos` (`crates/kaeso-codecs/src/minimax_videos/`)
- **Paths:** from the root of MiniMax's host, with `Authorization: Bearer`: `POST /v1/video_generation`, `GET /v1/query/video_generation`, `GET /v1/files/retrieve`; `POST /v2/video_generation`, `GET /v2/query/video_generation/{task_id}`, `DELETE /v2/video_generation/{task_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_videos/`, built from MiniMax's OpenAPI documents, none recorded

**Sources.** MiniMax's [text to video](https://platform.minimax.io/docs/api-reference/video-generation-t2v), [image to video](https://platform.minimax.io/docs/api-reference/video-generation-i2v), [first and last frame](https://platform.minimax.io/docs/api-reference/video-generation-fl2v), [query](https://platform.minimax.io/docs/api-reference/video-generation-query), [retrieve file](https://platform.minimax.io/docs/api-reference/file-management-retrieve) and the [H3 API](https://platform.minimax.io/docs/api-reference/video-generation-v2-create), read 2026-11-09.

## Decisions

- **The model picks the API.** `MiniMax-H3` and its variants go to the H3 API (`/v2`); the Hailuo models go to `/v1`.
- **A Hailuo file is a file id.** A finished Hailuo job names a `file_id`; `encode_video_file` asks `GET /v1/files/retrieve` for its `download_url`, which is valid for an hour, each time the file is fetched.
- **Errors can come with HTTP 200.** A failing `base_resp` is an error whatever the status, mapped by its code as for MiniMax's other APIs.
- **H3 needs its length and resolution.** Both are required by MiniMax, so a request without `seconds` or a size is refused before sending (params `seconds` and `size`).

## Request

| KPP | Hailuo (`/v1`) | H3 (`/v2`) |
| --- | --- | --- |
| `model` | `model` | `model` |
| `prompt` | `prompt` | `content[]` `{type: "text"}` |
| `seconds` | `duration` | `duration` |
| `resolution` | `resolution` (`512P`, `720P`, `768P`, `1080P`) | `resolution` (`480P`, `768P`, `2K`) |
| `size` | refused | `resolution` and `ratio`, matching a listed ratio within 1% |
| `aspect_ratio` | refused | `ratio` |
| `FirstFrame`, `LastFrame` images | `first_frame_image`, `last_frame_image` | `content[]` `image_url` with `role` |
| `Reference` images | refused | `content[]` `image_url`, `role: "reference_image"` |
| `prompt_optimizer`, `fast_pretreatment`, `extra`, `callback_url` | extras | extras |

Reference video and audio (`video_url`, `audio_url` items) are refused with param `content`.

## Reply

| Hailuo | H3 | KPP |
| --- | --- | --- |
| `task_id` | `task_id` | `VideoJob.id` |
| `Preparing`, `Queueing` | `queued` | `queued` |
| `Processing` | `running` | `in_progress` |
| `Success` | `succeeded` | `completed` |
| `Fail` | `failed`, `cancelled` | `failed` |
| `file_id` | `content.url` | an output file id or link |
| `video_width`, `video_height` | `resolution`, `duration`, `ratio` | `size`, or `resolution`, `seconds`, `aspect_ratio` |
| | `error` `{code, message}` | `error` |
