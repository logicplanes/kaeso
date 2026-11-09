---
title: MiniMax Images
description: How KPP's image requests map to MiniMax's text-to-image API.
order: 8
---

Look up how an image request reaches MiniMax here: which field goes where, what is refused, and the source of each mapping.

- **Codec:** `kaeso_codecs::minimax_images::MinimaxImages` (`crates/kaeso-codecs/src/minimax_images/`)
- **Path:** `POST /v1/image_generation`, from the root of MiniMax's host, with `Authorization: Bearer`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_images/`, built from MiniMax's reference, none recorded

**Sources.** MiniMax's [text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i) reference and its [error codes](https://platform.minimax.io/docs/api-reference/errorcode), read 2026-11-08.

## Decisions

- **Generations only.** MiniMax's image-to-image API takes one character reference, not an image to change, so an edit is refused. Variations are refused, as everywhere ([OpenAI Images](openai-images.md#decisions)).
- **Errors can come with HTTP 200.** A failing `base_resp` is an error whatever the status, mapped by its code.
- **No streams.** MiniMax answers with the finished images. Kaeso asks for the whole reply, and the gateway writes it as one `image_generation.completed` event per image for an app that streams.
- **Media types from the bytes.** MiniMax doesn't name the format of a base64 image, so the codec reads it from the first bytes.

## Request

| KPP | MiniMax | Notes |
| --- | --- | --- |
| `model`, `prompt` | `model`, `prompt` | |
| `size` `WIDTHxHEIGHT` | `width`, `height` | `auto` leaves both out; MiniMax checks the range (512 to 2,048, divisible by 8). A MiniMax body with `width` and no `height` is invalid (`param` `width`). |
| `aspect_ratio` | `aspect_ratio` | |
| `count` | `n` | At most 9. |
| `seed` | `seed` | |
| `delivery` `Url`, `Base64` | `response_format` `url`, `base64` | Any other value is invalid, with `param` `response_format`. |
| `user` | left out, named in `kaeso-left-out` | MiniMax has no end-user field. |
| `prompt_optimizer` and other keys | extras | Sent only from a MiniMax-format request; other dialects refuse them. |

## Refused

Each of these is `Unsupported` before anything is sent, with the field in `param`:

| KPP | `param` | Why |
| --- | --- | --- |
| an edit (`ImageOperation::Edit`) | `image` | The image-to-image API takes a character reference, not an image to change. |
| a variation | `image` | No provider offers variations. |
| a `size` that isn't pixels or `auto` | `size` | Send `WIDTHxHEIGHT` or `aspect_ratio`. |
| `count` above 9 | `n` | MiniMax's limit. |
| `quality`, `format`, `compression`, `background`, `resolution`, `negative_prompt` | `quality`, `output_format`, `output_compression`, `background`, `resolution`, `negative_prompt` | MiniMax's reference has no such fields. |
| `partial_images` above 0 on a stream | `partial_images` | MiniMax doesn't stream. |

## Reply

| MiniMax | KPP |
| --- | --- |
| `data.image_urls[]` | images as `Source::Url` |
| `data.image_base64[]` | images as `Source::Base64`, the media type read from the first bytes, or `application/octet-stream` |
| `base_resp` with `status_code` 0 | extras `base_resp` |
| other keys of `data` | extras `data.<key>` |
| `metadata` and other top-level keys | extras |

## Errors

A `base_resp` whose `status_code` isn't 0 is an error, even with HTTP 200:

| `status_code` | KPP |
| --- | --- |
| `1002`, `1039` | `RateLimited` |
| `1004`, `2049` | `Authentication` |
| `1008` | `Billing` |
| `1026`, `1027` | `ContentFiltered` |
| `2013` | `InvalidRequest` |
| any other | `ProviderError` |

A body without `base_resp` is read as an OpenAI-style error. To a MiniMax-format app, the codec writes `base_resp` with the code for the error's kind: `1002` for a rate limit, `1004` for authentication, `1008` for billing, `1026` for a content filter, `2013` for an invalid or refused request, and `1000` for the rest. [MiniMax Speech](minimax-speech.md) reads errors the same way.

## Fixtures

| File | Covers | Built from |
| --- | --- | --- |
| `requests/generate.json` | `aspect_ratio`, `n`, `prompt_optimizer` | The reference's request example |
| `requests/generate-size-seed.json` | `width`, `height`, `seed`, `response_format: base64` | The schema `ImageGenerationReq` |
| `responses/generate-urls.json` | Three links, `metadata`, `base_resp` | The reference's reply example, links filled in |
| `responses/generate-base64.json` | One base64 image | The schema's `data.image_base64` |
| `errors/sensitive-prompt-200.json` | `1026` with HTTP 200 | The error code reference |
| `errors/invalid-key-200.json` | `2049` with HTTP 200 | The error code reference |

No MiniMax key with image access was available, so nothing is recorded. The folder's `README.md` lists the provenance, and `*.meta.json` the HTTP status.
