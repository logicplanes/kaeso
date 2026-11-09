---
title: SpaceX Videos
description: How KPP's video jobs map to SpaceX's Grok Imagine video API.
order: 8
---

Look up how a video job reaches SpaceX here: which field goes where, how the job is asked about, and the source of each mapping.

- **Codec:** `kaeso_codecs::xai_videos::XaiVideos` (`crates/kaeso-codecs/src/xai_videos/`)
- **Paths:** `POST /v1/videos/generations`, `POST /v1/videos/edits` and `GET /v1/videos/{request_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/xai_videos/`, built from SpaceX's reference, none recorded

**Sources.** SpaceX's [videos reference](https://docs.x.ai/developers/rest-api-reference/inference/videos) and [video generation guide](https://docs.x.ai/developers/model-capabilities/video/generation), read 2026-11-09.

## Decisions

- **A remix is an edit.** `POST /v1/videos/edits` changes a finished video with a new prompt, which is what OpenAI's remix does. It takes the earlier video by link, so the gateway asks SpaceX for that link first and sends it as `video.url` (`VideoRequest.source_video`).
- **The job id is `request_id`.** Starting a job answers with `request_id` only; the job's status, link and length come from `GET /v1/videos/{request_id}`.
- **A withheld video fails.** `status: "done"` with `respect_moderation: false` carries no link; KPP reports it as `failed` with the code `moderation_blocked`. `expired` is `failed` with the code `expired`.

## Request

| KPP | SpaceX |
| --- | --- |
| `model`, `prompt`, `user` | `model`, `prompt`, `user` |
| `seconds` (1 to 15) | `duration` |
| `aspect_ratio`, `resolution` (`480p`, `720p`, `1080p`) | `aspect_ratio`, `resolution` |
| `size` | `resolution` and `aspect_ratio`, where SpaceX lists both |
| `FirstFrame` image | `image` `{url}` or `{file_id}` |
| `Reference` images | `reference_images[]` |
| a remix's `source_video` | `video.url` on the edits path |
| `keyframes`, `reference_audios`, `output`, `storage_options` | extras |

SpaceX takes no last frame, seed, negative prompt or count: each is refused before sending, with its field in `param`.

## Reply

| SpaceX | KPP |
| --- | --- |
| `request_id` | `VideoJob.id` |
| `status` `pending` | `queued` while `progress` is 0, then `in_progress` |
| `status` `done`, `failed`, `expired` | `completed`, `failed`, `failed` |
| `progress`, `model` | `progress`, `model` |
| `video.url`, `video.duration` | an output link, `seconds` |
| `error` `{code, message}` | `error` |
| `usage`, `video.file_output` | extras |
