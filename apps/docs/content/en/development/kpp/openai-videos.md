---
title: OpenAI Videos
description: How OpenAI's Videos API maps to KPP's video jobs, and how Together AI takes it.
order: 8
---

Look up any field of OpenAI's Videos API here: where it goes in KPP, how a video job is asked about later, and the source of each mapping. Check it before you change how a video codec reads or writes a field.

- **Codec:** `kaeso_codecs::openai_videos::OpenAiVideos` (`crates/kaeso-codecs/src/openai_videos/`), with the helpers every video dialect shares in `crates/kaeso-codecs/src/video_jobs.rs`
- **Endpoints:** `POST /v1/videos` (JSON or `multipart/form-data`), `POST /v1/videos/{video_id}/remix`, `GET /v1/videos`, `GET /v1/videos/{video_id}`, `GET /v1/videos/{video_id}/content` and `DELETE /v1/videos/{video_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_videos/`, built from the specification and Together AI's reference, none recorded

**Sources.** The OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateVideoJsonBody`, `CreateVideoMultipartBody`, `ImageRefParam-2`, `CreateVideoRemixBody`, `VideoResource`, `VideoListResource`, `DeletedVideoResource`, `VideoContentVariant`, `Error-2`, and the field names openai-python 3.41.0 writes. Together AI: [create video](https://docs.together.ai/reference/create-videos) and [fetch video metadata](https://docs.together.ai/reference/get-videos-id), read 2026-11-09.

## Decisions

The maintainer delegated these to the media work; each one is recorded here.

- **A video is a job.** `Request::Video(VideoRequest)` starts one, with `VideoOperation` `Generate` or `Remix`; the reply is `Reply::Video(VideoJob)`, the job as the provider reports it. Asking about a job later is not a KPP request: the `Codec` trait's `encode_video_poll` and `decode_video_poll` build and read that call, `encode_video_delete` deletes a job where the provider can, and `encode_video_file` and `decode_video_file` turn a file id into a link.
- **Images say what they are for.** `VideoImage` carries `VideoImageRole` `FirstFrame`, `LastFrame` or `Reference`. OpenAI's `input_reference` was the first frame of a Sora video, so it decodes as `FirstFrame`.
- **Outputs name where the file is.** `VideoJob.outputs` holds each file as a link, inline bytes or a file id, with its `VideoVariant` (`video`, `thumbnail`, `spritesheet`). A link that starts with `/` is a path at the provider.
- **No provider of this form.** OpenAI shut its video models down on 2026-09-24. Apps still speak the API, so it is the form Kaeso answers in; `VideosProfile::OPENAI` is kept for round trips, and only Together AI receives jobs in a form of it.
- **Sizes cross only where the provider lists them.** A `size` becomes another provider's resolution and aspect ratio only when the provider documents both, as 1280x720 is 720p at 16:9. Kaeso doesn't resize or convert.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `openai_videos`; other dialects refuse them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker.

## Request

| Wire field | KPP | Class |
| --- | --- | --- |
| `model` | `VideoRequest.model` | typed |
| `prompt` | `prompt` | typed |
| `input_reference` upload | `images[]` `FirstFrame`, inline bytes with the file name | typed, form `body: multipart` |
| `input_reference.image_url` | `images[]` `FirstFrame`, a link or the bytes of a data URL | typed |
| `input_reference.file_id` | `Source::FileId`, origin `openai_videos` | typed |
| `seconds`, a string of digits | `seconds` | typed |
| `seconds`, a number | `seconds`, with the marker `seconds_number` | typed, form |
| `size` | `size`, `WIDTHxHEIGHT` | typed |
| any other field | the field | extras |

A remix's body holds only `prompt`; the path names the video. `openai_videos::decode_remix` decodes it with an empty model, which the caller fills in with the video's model and its provider's job id (`remix_of`).

## Reply

| `VideoResource` | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` (`queued`, `in_progress`, `completed`, `failed`) | `status` |
| `progress` | `progress`; written as 100 for a completed job that names none, otherwise 0 |
| `created_at`, `completed_at`, `expires_at` | the same, Unix seconds |
| `prompt`, `model`, `remixed_from_video_id` | `prompt`, `model`, `remixed_from` |
| `seconds` | `seconds`, written as a string |
| `size` | `size`, or a size made from `resolution` and `aspect_ratio`, or `""` since the field is required |
| `error` `{code, message, param, type}` | `error` |

`openai_videos::encode_list` writes `VideoListResource` with `first_id`, `last_id` and `has_more`; `encode_deleted` writes `DeletedVideoResource`. `decode_list_query` reads `after`, `limit` (0 to 100, 20 by default) and `order`; `decode_content_query` reads `variant`. Errors are OpenAI's `ErrorResponse`, as for the other OpenAI dialects.

## Together AI

`VideosProfile::TOGETHER` sends `POST /v2/videos` and asks `GET /v2/videos/{id}`; Together documents no delete.

| KPP | Together AI |
| --- | --- |
| `size` | `width`, `height` |
| `resolution`, `aspect_ratio` | `resolution`, `ratio` |
| `seconds` | `seconds`, a string |
| `seed`, `negative_prompt`, `audio` | `seed`, `negative_prompt`, `generate_audio` |
| `FirstFrame`, `LastFrame` images | `media.frame_images[]` `{input_image, frame}` |
| `Reference` images | `media.reference_images[]` |
| reply `status` `in_progress`, `completed`, `failed` | `status`; Together documents no `queued` |
| reply `outputs.video_url` | an output link; `outputs.cost` stays in extras |

Together takes images only as links (`input_image` is "URL path to hosted image"), so an upload is refused (param `input_reference`), as is a remix (param `video_id`). The app's other fields, kept as extras, reach Together as they are, such as `steps`; a field of `input_reference` other than `image_url` is refused.
