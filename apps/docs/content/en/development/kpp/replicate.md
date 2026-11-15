---
title: Replicate
description: How KPP's image requests and video jobs map to Replicate's predictions.
order: 8
---

Look up how an image request or a video job reaches Replicate here: which field goes where for each model, how Kaeso follows a prediction, and the source of each mapping.

- **Codec:** `kaeso_codecs::replicate::Replicate` (`crates/kaeso-codecs/src/replicate/`)
- **Paths:** with `Authorization: Bearer r8_…`: `POST /v1/models/{owner}/{name}/predictions` with `{"input": …}`, then `GET /v1/predictions/{id}`; `GET /v1/collections/official` for the model check
- **Fixtures:** `crates/kaeso-codecs/fixtures/replicate/`, built from Replicate's documentation and each model's schema, with predictions Replicate published on its model pages and the refused tokens recorded live

**Sources.** Replicate's [HTTP API](https://replicate.com/docs/reference/http) and its OpenAPI document, [creating a prediction](https://replicate.com/docs/topics/predictions/create-a-prediction), [output files](https://replicate.com/docs/topics/predictions/output-files), [data retention](https://replicate.com/docs/topics/predictions/data-retention), [rate limits](https://replicate.com/docs/topics/predictions/rate-limits), [official models](https://replicate.com/docs/topics/models/official-models) and each model's schema (`https://replicate.com/{owner}/{name}/api/schema`), read 2026-11-14.

## Predictions

An image request asks with `Prefer: wait=60`, so Replicate answers once the images are made, for most models within that request. A prediction still `starting` or `processing` after the wait is asked again by its id until it ends, within the request's time limit. A video job starts without waiting; its id is the prediction's, and each retrieve asks `GET /v1/predictions/{id}`. Kaeso asks only `/v1/predictions/{id}` with an id of plain characters, never the `urls` a prediction names.

Replicate deletes a prediction's input, output and files an hour after it ends. An image's `url` lasts that hour, as OpenAI's own image links do; a video's file can be downloaded until then, and its job's `expires_at` says when.

## Models

Each model has its own input, so Kaeso maps only these official models and refuses any other with the list:

| Model | Kind | Takes |
| --- | --- | --- |
| `black-forest-labs/flux-schnell`, `black-forest-labs/flux-dev` | Image | `n` 1 to 4, `aspect_ratio` (`1:1`, `16:9`, `21:9`, `3:2`, `2:3`, `4:5`, `5:4`, `3:4`, `4:3`, `9:16`, `9:21`), `seed`, `output_format` `webp`, `jpeg` or `png`, `output_compression`; no pixel size |
| `black-forest-labs/flux-1.1-pro` | Image | One image; `size` of 256 to 1440 a side in steps of 32, or `aspect_ratio`; `seed`, `output_format`, `output_compression` |
| `bytedance/seedream-4.5` | Image and edit | `n` 1 to 15 (the model may make fewer); `size` of 1024 to 4096 a side, or `aspect_ratio` and `resolution` `2K` or `4K`; up to 14 images; no `seed` or `output_format` |
| `google/nano-banana-pro` | Image and edit | One image; `aspect_ratio` and `resolution` `1K`, `2K` or `4K`; up to 14 images; `output_format` `jpeg` or `png` |
| `google/veo-3.1` | Video | `seconds` 4, 6 or 8; `size` at 720p or 1080p, 16:9 or 9:16; `audio`, `seed`, `negative_prompt`; a first and a last frame, or 1 to 3 reference images for 8 seconds at 16:9 |
| `kwaivgi/kling-v2.5-turbo-pro` | Video | `seconds` 5 or 10; `aspect_ratio` `16:9`, `9:16` or `1:1` unless a first frame sets the shape; `negative_prompt`; a first and a last frame |

## Image request

| KPP | Replicate `input` |
| --- | --- |
| `prompt` | `prompt` |
| `n` | `num_outputs` on FLUX.1; `max_images` with `sequential_image_generation: "auto"` on Seedream |
| `size` `WIDTHxHEIGHT` | `aspect_ratio: "custom"` with `width` and `height` on FLUX1.1 [pro], `size: "custom"` with `width` and `height` on Seedream; `auto` sends none |
| `aspect_ratio` | `aspect_ratio` |
| `resolution` | `size` on Seedream, `resolution` on Nano Banana Pro |
| `output_format` | `output_format`, `jpeg` as `jpg` |
| `output_compression` | `output_quality` |
| `seed` | `seed` |
| images to edit | `image_input`: links as they came, uploads as data URIs |
| `response_format: b64_json` | nothing: Kaeso downloads the links from `replicate.delivery` |

The models' schemas don't say whether Replicate refuses fields they lack, so Kaeso refuses `quality` and `background` other than `auto`, `negative_prompt`, `user`, a mask, streaming and variations before sending, and `n` above 1 on the models that make one image. A Replicate-format request's own fields, such as `guidance` or `safety_tolerance`, are sent as written.

## Image result

| Replicate | KPP |
| --- | --- |
| `output`: a link or a list of links | the images' links |
| `input.width`, `input.height` with a custom size | the reply's size |
| `input.resolution`, or Seedream's `input.size` | the resolution the images are billed at |
| `status` `failed`, `canceled` or `aborted` | a provider error, with `error` as the message |
| `id`, `version`, `input`, `logs`, `metrics`, `urls` and the times | extras |

## Video job

| KPP | Replicate `input` |
| --- | --- |
| `prompt` | `prompt` |
| `seconds` | `duration` |
| `size` | `resolution` from the short side and `aspect_ratio`, Veo only |
| `aspect_ratio`, `resolution` | `aspect_ratio`, `resolution` |
| `audio` | `generate_audio`, Veo only; Veo makes sound unless it is turned off, at twice the price |
| `seed`, `negative_prompt` | `seed` (Veo only), `negative_prompt` |
| first and last frame | `image` and `last_frame` on Veo, `start_image` and `end_image` on Kling |
| reference images | `reference_images`, Veo only |

A video job needs `seconds`, since Replicate bills each second. Kaeso refuses `n` above 1, `user` and remixes, and on Kling `size`, `seed` and `audio`, which its schema keeps but says it doesn't use.

## Status

| Replicate | KPP |
| --- | --- |
| `starting` | `queued` |
| `processing` | `in_progress` |
| `succeeded` | `completed`, with `output` as the video's link; `expires_at` is an hour after `completed_at` |
| `failed`, `canceled`, `aborted` | `failed`, with the status as the code and `error` as the message |

## Errors

| Replicate | KPP |
| --- | --- |
| `{"title", "detail", "status"}` (problem details) | `title: detail` as the message; the kind from the status: 401 `authentication`, 402 `billing`, 404 `not_found`, 422 `invalid_request`, 429 `rate_limited` |
| `{"detail": "…"}` | `detail` as the message, the kind from the status |
| `Retry-After` | the retry delay |

## Model check

**Check availability** reads `GET /v1/collections/official`, Replicate's official models, and matches each model's `owner/name`. A model the collection leaves out is inconclusive, not unavailable.
