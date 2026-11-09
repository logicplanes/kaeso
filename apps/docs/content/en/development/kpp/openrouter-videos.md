---
title: OpenRouter Videos
description: How KPP's video jobs map to OpenRouter's video generation API.
order: 8
---

Look up how a video job reaches OpenRouter here: which field goes where, where its file is fetched, and the source of each mapping.

- **Codec:** `kaeso_codecs::openrouter_videos::OpenRouterVideos` (`crates/kaeso-codecs/src/openrouter_videos/`)
- **Paths:** after OpenRouter's base URL (`https://openrouter.ai/api`): `POST /v1/videos`, `GET /v1/videos/{jobId}` and `GET /v1/videos/{jobId}/content`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openrouter_videos/`, built from OpenRouter's OpenAPI document, none recorded

**Sources.** OpenRouter's [submit](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request), [poll](https://openrouter.ai/docs/api/api-reference/video-generation/poll-video-generation-status) and [content](https://openrouter.ai/docs/api/api-reference/video-generation/download-generated-video-content) references, read 2026-11-09.

## Decisions

- **No remix.** `previous_job_id` edits or extends a finished job, and the reference doesn't say which, so a remix is refused (param `video_id`).
- **Where the file is.** A finished job's signed `urls` are fetched first, then `unsigned_urls` with the key, then the content path.

## Request

| KPP | OpenRouter |
| --- | --- |
| `model`, `prompt`, `user` | `model`, `prompt`, `user` |
| `seconds` | `duration` |
| `size`, `resolution`, `aspect_ratio` | `size`, `resolution`, `aspect_ratio`, each from OpenRouter's lists |
| `seed`, `audio` | `seed`, `generate_audio` |
| `FirstFrame`, `LastFrame` images | `frame_images[]` with `frame_type` |
| `Reference` images | `input_references[]` `image_url` items |
| `provider`, `callback_url`, `session_id`, `trace`; audio or video references | extras |

## Reply

| OpenRouter | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `pending`, `in_progress`, `completed` | `queued`, `in_progress`, `completed` |
| `status` `failed`, `cancelled`, `expired` | `failed`, with the status as the code |
| `error` (a string) | `error.message` |
| `expires_at` | `expires_at` |
| `urls`, `unsigned_urls` | the output link, and extras |
| `usage`, `polling_url`, `generation_id` | extras |
