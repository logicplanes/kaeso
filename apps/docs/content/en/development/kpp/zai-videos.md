---
title: Z.ai Videos
description: How KPP's video jobs map to Z.ai's CogVideoX API.
order: 8
---

Look up how a video job reaches Z.ai here: which field goes where, how the job is asked about, and the source of each mapping.

- **Codec:** `kaeso_codecs::zai_videos::ZaiVideos` (`crates/kaeso-codecs/src/zai_videos/`)
- **Paths:** from the root of Z.ai's host, with `Authorization: Bearer`: `POST /api/paas/v4/videos/generations` and `GET /api/paas/v4/async-result/{id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/zai_videos/`, built from Z.ai's OpenAPI documents, none recorded

**Sources.** Z.ai's [generate video](https://docs.z.ai/api-reference/video/generate-video) and [retrieve result](https://docs.z.ai/api-reference/video/get-video-status), read 2026-11-09.

## Request

| KPP | Z.ai |
| --- | --- |
| `model`, `prompt` | `model`, `prompt` |
| `seconds` (5 or 10) | `duration` |
| `size`, or `resolution` with `aspect_ratio` | `size`, one of the sizes Z.ai lists |
| `audio` | `with_audio` |
| `user` | `user_id` |
| `FirstFrame`, `LastFrame` images | `image_url`, a link or a list of two |
| `quality`, `fps`, `request_id` | extras |

Z.ai takes no seed, negative prompt or reference images, and no remix: each is refused before sending.

## Reply

| Z.ai | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `task_status` `PROCESSING`, `SUCCESS`, `FAIL` | `in_progress`, `completed`, `failed` |
| `video_result[].url` | the video's link |
| `video_result[].cover_image_url` | the `thumbnail` link |
