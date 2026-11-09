---
title: DashScope
description: How KPP's video jobs map to QwenCloud's DashScope video synthesis for the Wan models.
order: 8
---

Look up how a video job reaches QwenCloud's Wan models here: which body a model takes, which field goes where, how the task is asked about, and the source of each mapping.

- **Codec:** `kaeso_codecs::dashscope::DashScope` (`crates/kaeso-codecs/src/dashscope/`)
- **Paths:** from the root of QwenCloud's host, with `Authorization: Bearer` and `X-DashScope-Async: enable`: `POST /api/v1/services/aigc/video-generation/video-synthesis`, `POST /api/v1/services/aigc/image2video/video-synthesis` (Wan 2.2 and 2.1 first and last frames) and `GET /api/v1/tasks/{task_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/dashscope/`, built from QwenCloud's OpenAPI documents, none recorded

**Sources.** QwenCloud's references for [Wan 2.6 and older text to video](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task), [first-frame image to video](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task), [Wan 2.7 text to video](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task) and [its query result](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result), [Wan 2.7 image to video](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task), [Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task) and [asynchronous tasks](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks), read 2026-11-09.

## Decisions

- **The model picks the body.** `dashscope::family` reads the model name: `wan3.0-video*`, `wan2.7-t2v*`, `wan2.7-i2v*`, `*-kf2v-*` (first and last frames), `*-t2v*` and `*-i2v*` (Wan 2.6 and older). Models that edit or reference video, such as `wan2.7-r2v`, `wan2.7-videoedit`, `wan2.1-vace-plus` and `wan2.2-animate-*`, are refused with `param` `model`.
- **Sizes only where QwenCloud lists them.** Wan 2.6 and older take a pixel `size` as `width*height`. Wan 2.7's text to video takes a tier and a ratio, so a `size` crosses only when it is one of the ten sizes its reference lists, as `1104x832` is `720P` at `4:3`. Image to video takes a tier only, since the image sets the aspect ratio, so a `size` is refused there.
- **No remix, no delete.** DashScope cancels only tasks that haven't started, which is not a delete, so Kaeso forgets a deleted video.
- **Times are Beijing time.** `submit_time` and `end_time` are UTC+8 without an offset; `dashscope::unix_of_beijing` reads them. The video's link is valid for 24 hours, so a finished job expires 24 hours after `end_time`.

## Request

| KPP | Wan 2.6 and older | Wan 2.7 | Wan 3.0 |
| --- | --- | --- | --- |
| `model`, `prompt` | `model`, `input.prompt` | the same | the same |
| `negative_prompt` | `input.negative_prompt` | `input.negative_prompt` | refused |
| `FirstFrame` image | `input.img_url`; Wan 2.2 and 2.1 first and last frames: `input.first_frame_url` | `input.media[]` `{type: "first_frame", url}` | the same |
| `LastFrame` image | `input.last_frame_url`, web links only | `input.media[]` `{type: "last_frame", url}` | the same |
| `Reference` images | refused | refused | `input.media[]` `{type: "reference_image", url}`, never with frames |
| `size` | `parameters.size`, `1280*720`, also made from a resolution and an aspect ratio (text to video) | `parameters.resolution` and `ratio`, from the listed sizes (text to video) | refused |
| `resolution` | `parameters.resolution` (image to video), uppercased | `parameters.resolution` | `parameters.resolution` |
| `aspect_ratio` | refused | `parameters.ratio` (text to video) | `parameters.ratio` |
| `seconds`, `seed` | `parameters.duration`, `parameters.seed` | the same | the same |
| `audio` | `parameters.audio`, `wan2.6-i2v-flash` only | refused | `parameters.audio` |
| other `input` and `parameters` fields, such as `audio_url`, `prompt_extend`, `watermark` | extras | extras | extras |

Images go as links or data URLs, except Wan 2.2 and 2.1 first and last frames, which QwenCloud takes as public links only. Media of other types, such as `driving_audio` or `first_clip`, are refused when a request is decoded.

## Reply

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`, `RUNNING`, `SUCCEEDED` | `queued`, `in_progress`, `completed` |
| `output.task_status` `FAILED`, `CANCELED`, `UNKNOWN` | `failed`, with `output.code` and `output.message`, or the status as the code |
| `output.video_url` | the video's link, `video/mp4` |
| `output.submit_time`, `output.end_time` | `created_at`, `completed_at` |
| `usage.output_video_duration`, `usage.SR`, `usage.ratio` | `seconds`, `resolution`, `aspect_ratio` |
| `request_id`, other `output` and `usage` fields | extras |

Errors are `{"request_id", "code", "message"}`: `InvalidApiKey` is an authentication error, `Throttling…` a rate limit and `DataInspection…` a content filter; other codes follow the HTTP status.
