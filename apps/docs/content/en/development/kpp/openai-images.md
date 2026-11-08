---
title: OpenAI Images
description: How every field of the OpenAI Images API maps to KPP, and how other image formats take it.
order: 8
---

Look up any field of OpenAI's Images API here: where it goes in KPP, which providers take it in which form, and the source of each mapping. Check it before you change how an image codec reads or writes a field.

- **Codecs:** `kaeso_codecs::openai_images::OpenAiImages` (`crates/kaeso-codecs/src/openai_images/`), `kaeso_codecs::minimax_images::MinimaxImages` (`crates/kaeso-codecs/src/minimax_images/`), and image requests in the Gemini codec (`crates/kaeso-codecs/src/gemini/images.rs`)
- **Endpoints:** `POST /v1/images/generations`, `POST /v1/images/edits` (JSON or `multipart/form-data`) and `POST /v1/images/variations`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_images/` and `fixtures/minimax_images/`, all built from the specification and the providers' documentation, none recorded

**Sources.** The OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateImageRequest`, `CreateImageEditRequest`, `EditImageBodyJsonParam`, `ImageRefParam`, `CreateImageVariationRequest`, `ImagesResponse`, `Image`, `ImageGenUsage`, `ImagesUsage`, `ImageGenPartialImageEvent`, `ImageGenCompletedEvent` and their `image_edit.*` twins. The image schemas changed after the spec commit the other OpenAI codecs pin (`e95c0fe`), so this dialect pins the newer one. RFC 7578 for `multipart/form-data`. The provider pages are named in [providers](#providers-and-their-forms).

## Decisions

The maintainer delegated these to the media work; each one is recorded here.

- **One KPP request for all three operations.** `Request::Image(ImageRequest)` with `ImageOperation` `Generate`, `Edit` or `Variation`, and `Reply::Image(ImageResponse)`. The path names the operation, so apps' requests are decoded with `dispatch::decode_image_request`, as Gemini requests are with `decode_gemini_request`.
- **One dialect per wire format.** `openai_images` serves OpenAI and every provider that speaks a form of its API, each described by an `ImagesProfile`. `minimax_images` is MiniMax's own format. Gemini image models take image requests through the `gemini` dialect, as `generateContent` calls.
- **Variations are refused everywhere.** OpenAI retired its variations endpoint (410 in the spec), and no other provider documents one. The request is decoded, so the app gets a clear error in its own format: `Unsupported` (`image variations`, param `image`), naming `/v1/images/edits`.
- **No image processing.** Kaeso doesn't convert, resize or fetch images. A size a provider can't produce, a link where bytes are needed, or bytes where a provider only answers with links, is refused before anything is sent.
- **Streams from providers that don't stream.** With `partial_images` 0 or unset, OpenAI sends the finished image as one `*.completed` event. Kaeso asks such providers for the whole reply and writes the same event, one per image, with the usage on the last. Asking those providers for partial images is `Unsupported` (param `partial_images`).

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `openai_images`. Every `openai_images` provider receives them unless its profile refuses the field; other dialects refuse them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker.

## Request

The JSON generation request, the JSON edit request and the multipart upload share every field but the images.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `ImageRequest.model` typed | Required by `validate()`. |
| `prompt` | `prompt` typed | Required for generations and edits. |
| `images[]` (JSON edit) `{image_url}` | `images[]` `Source::Url`, or `Source::Base64` for a canonical data URL | Form `body: "json"`. |
| `images[]` `{file_id}` | `Source::FileId`, origin `openai_images` | Sent only to providers whose profile takes file ids. |
| `image`, `image[]` (upload) | `images[]` `Source::Base64` with the part's `Content-Type`, and `ImageInput.filename` | Form `body: "multipart"`; the first field name in form `image_field`. A part without a file name is `InvalidRequest`. |
| `mask` | `mask` typed | An upload or an `ImageRefParam`. |
| `markup` | extras `markup` as `{image_url}` | An upload becomes a data URL, and is written back as a file part. |
| `n` | `count` typed | |
| `size` | `size` (`ImageSize`) typed | `auto` or `WIDTHxHEIGHT`, other values as written. |
| `quality` | `quality` (`ImageQuality`) typed | `auto`, `low`, `medium`, `high`, `xhigh`, `max`, `standard`, `hd`; others as written. |
| `output_format` | `format` (`ImageFormat`) typed | `png`, `jpeg`, `webp`, `gif`, `svg`; others as written. |
| `output_compression` | `compression` typed | 0 to 100, checked by `validate()`. |
| `background` | `background` typed | |
| `response_format` `url`, `b64_json` | `delivery` `Url`, `Base64` typed | Other values: `InvalidRequest`. |
| `stream` `true` | `stream` on | `false`: form `stream_off`. Upload values `true`/`false` only. |
| `partial_images` | `partial_images` typed | |
| `safety_identifier` | `user` typed | `user` beside it: extras `user`. |
| `user` alone | `user` typed | Form `user: "user"`, so it is written back as `user`. |
| `aspect_ratio`, `resolution`, `seed`, `negative_prompt` | typed | Not OpenAI's fields; SpaceX, OpenRouter, Together AI, QwenCloud, MiniMax and Gemini take some of them. OpenAI's profile refuses them. |
| `moderation`, `style`, `input_fidelity`, `complexity`, `frame_count`, `frame_duration_ms`, `svg`, unknown keys | extras | Upload fields with brackets nest: `svg[model]` becomes `svg: {model}`. `frame_count` and `frame_duration_ms` uploads become numbers. |
| header `openai-beta` | extras `header:openai-beta` | |

The multipart reader (`kaeso_codecs::multipart`) is sans-IO and bounded: at most 64 parts (`MAX_FORM_PARTS`), 50 MB per part (`MAX_IMAGE_BYTES`, OpenAI's limit per image) and 8 KiB of headers per part. It reads CRLF framing only, as RFC 7578 requires, and refuses a part without `Content-Disposition: form-data` or a field name. The writer picks a boundary that occurs in no part, derived from the content so that encoding stays deterministic.

`validate()` refuses a generation with images, an edit or variation without one, a variation with more than one image or a mask, `n` 0 and `output_compression` above 100.

## Response

| Wire field | KPP | Notes |
|---|---|---|
| `created` | `ImageResponse.created` | Written as Kaeso's request time when the provider sent none, because `ImagesResponse` requires it. |
| `data[].b64_json` | `GeneratedImage.source` `Source::Base64` | The media type comes from `data[].mime_type` (SpaceX), `data[].media_type` (OpenRouter), the reply's `output_format`, or `image/png`, OpenAI's default. |
| `data[].url` | `Source::Url` | Both `b64_json` and `url`: the link in extras `url`. |
| `data[].revised_prompt` | `revised_prompt` typed | |
| `background`, `output_format`, `size`, `quality` | `settings` typed | |
| `usage.input_tokens` (or OpenRouter's `prompt_tokens`) | `usage.input_tokens`, without `input_tokens_details.cached_tokens` | The usage object is kept in `usage.raw`. |
| `usage.output_tokens` (or `completion_tokens`) | `usage.output_tokens` | |
| `usage.input_tokens_details.image_tokens` | `usage.input_image_tokens` | |
| `usage.output_tokens_details.image_tokens` (or `completion_tokens_details`) | `usage.output_image_tokens` | |
| no `usage` | form `no_usage` | |
| `svg_usage`, `safety_identifier`, unknown keys | extras | |

A reply from another format gets OpenAI's usage shape: `input_tokens`, `output_tokens`, `total_tokens`, `input_tokens_details` and, when image output tokens are known, `output_tokens_details`.

## Streaming

`image_generation.partial_image` and `image_edit.partial_image` become `StreamEvent::Image` with `ImageEvent.partial` set to `partial_image_index`; `*.completed` events become finished images with their usage. `created_at`, `size`, `quality`, `background` and `output_format` go to `created` and `settings`. Event names follow the app's endpoint: an edit stream says `image_edit.*`.

The spec requires every field but `usage` in these events. Events from another format get `auto` for unknown settings, `png` for an unknown format and `0` for an unknown time. A stream error is written as `event: error` with `{"type": "error", "error": {type, code, message, param}}`, the form OpenRouter documents, since OpenAI's spec defines no error event. Other events, such as OpenRouter's `image_generation.text_chunk`, stay `StreamEvent::Extra`. OpenRouter's closing `data: [DONE]` is not written back.

## Errors

The OpenAI error shape, shared with [OpenAI Responses](openai-responses.md#errors-errorresponse); errors carry the origin `openai_images`. `moderation_blocked` and OpenAI's other codes keep their kind, and raw OpenAI errors reach OpenAI apps unchanged.

## Providers and their forms

Each provider's `ImagesProfile` says where it serves generations and edits, how it spells sizes, images and delivery, and which OpenAI fields it lacks. A field a provider lacks is `Unsupported` before sending, with the field in `param`.

| Profile | Generations | Edits | Differences | Source |
|---|---|---|---|---|
| `OPENAI` | `/v1/images/generations` | `/v1/images/edits`, multipart for uploads, JSON when an image is a link or file id | Streams partial images. | spec `926a7b8` |
| `VERCEL` | same | same, without file ids | No streams; `user` instead of `safety_identifier`; no `n` limit. | [Vercel image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| `XAI` (SpaceX) | same | `/v1/images/edits`, JSON only: `image` or `images[]` of `{type: "image_url", url}`, at most 5, no mask | `aspect_ratio` and `resolution` instead of `size`; `quality` `low`, `medium`, `auto`; no output format, background, compression or moderation. | [xAI images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| `TOGETHER` | same | on generations: one link in `image_url`; uploads refused | `width` and `height`; `response_format: "base64"`; `n` up to 4; `jpeg` or `png`; `aspect_ratio`, `seed`, `negative_prompt`. | [Together AI create image](https://docs.together.ai/reference/post-images-generations) |
| `QWENCLOUD` | same, under `/compatible-mode` | on generations: `image`, one data URL or link, or a list of up to 10 | Links only, valid 24 hours: `b64_json` refused; `n` up to 6; `seed`, `negative_prompt`. | [QwenCloud OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai) |
| `ZAI` | `/api/paas/v4/images/generations` | none | One image per request; `quality` `hd` or `standard`; `user_id`; links only, valid 30 days. | [Z.ai generate image](https://docs.z.ai/api-reference/image/generate-image) |
| `OPENROUTER` | `/v1/images` (under `/api`) | on generations: `input_references` of `{type: "image_url", image_url: {url}}`, up to 16 | Bytes only: `url` refused; `aspect_ratio`, `resolution`, `seed`; data-only streams ending in `[DONE]`. | [OpenRouter image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |

`ImagesProfile::compatible(name)` is OpenAI's form under another name, for custom providers.

## Gemini image models

The Gemini codec writes an image request as `generateContent` with the prompt and the edit's images as `inlineData` parts, and `generationConfig.responseModalities: ["IMAGE"]`, as Google's [image generation guide](https://aistudio.google.com/docs/image-generation) does. Sources: that guide, discovery revision `20261106`, and the Agent Platform's `GoogleCloudAiplatformV1ImageConfig`.

| KPP | Gemini | Notes |
|---|---|---|
| `size` | `imageConfig.aspectRatio` and `imageSize` | Only the sizes in the guide's tables: one table for the Gemini 3 image models, one for Gemini 2.5 Flash Image. `1024x1024` is `1:1` at `1K`; `1536x1024` is not produced and is `Unsupported`. |
| `aspect_ratio`, `resolution` | `imageConfig.aspectRatio`, `imageSize` | Not together with `size`. |
| `seed` | `generationConfig.seed` | |
| `format`, `compression` | `imageConfig.imageOutputOptions` `mimeType`, `compressionQuality` | Agent Platform only (`GeminiProfile::image_output_options`); `png` or `jpeg`. |
| `count` above 1, `quality`, `background`, `negative_prompt`, `mask`, images by link or file id, `delivery` `Url` | `Unsupported` | Google warns the model doesn't always follow the number of images; Gemini answers with bytes only. |
| `user` | left out (`kaeso-left-out: user`) | Gemini has no end-user field. |

The reply's image parts become the images, and `usageMetadata.candidatesTokensDetails` with modality `IMAGE` becomes `usage.output_image_tokens`, for every Gemini reply. Text the model writes beside the image has no place in an image reply and is left out. A reply without an image is an error: `ContentFiltered` when Google's filters stopped it, `ProviderError` otherwise. Imagen is not mapped: AI Studio shut it down, and the Agent Platform lists its endpoints as discontinued.

## MiniMax Images

`minimax_images` reads and writes `POST /v1/image_generation`, served from the root of MiniMax's host ([MiniMax text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i)).

| KPP | MiniMax | Notes |
|---|---|---|
| `prompt`, `model`, `seed`, `count` | `prompt`, `model`, `seed`, `n` | `n` at most 9. |
| `size` `WIDTHxHEIGHT` | `width`, `height` | MiniMax checks its range (512 to 2,048, divisible by 8). |
| `aspect_ratio` | `aspect_ratio` | |
| `delivery` | `response_format` `url`, `base64` | |
| `prompt_optimizer`, unknown keys | extras | |
| edits, variations, `quality`, `output_format`, `output_compression`, `background`, `resolution`, `negative_prompt` | `Unsupported` | MiniMax's image-to-image API takes one character reference, which is not an edit. |

The reply's `data.image_urls` become links and `data.image_base64` bytes, whose media type is read from their first bytes, since MiniMax doesn't name the format. MiniMax answers HTTP 200 with failures in `base_resp`: `1002` and `1039` are `RateLimited`, `1004` and `2049` `Authentication`, `1008` `Billing`, `1026` and `1027` `ContentFiltered`, `2013` `InvalidRequest`, others `ProviderError`. The endpoint doesn't stream.
