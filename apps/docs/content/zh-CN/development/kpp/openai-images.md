---
title: OpenAI Images
description: OpenAI Images API 的每个字段如何映射到 KPP，以及其他图片格式如何接收它。
order: 8
---

在这里查阅 OpenAI Images API 的任意字段：它在 KPP 中的位置、哪些服务商以何种形式接收它，以及每项映射的来源。修改图片编解码器读写某个字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_images::OpenAiImages`（`crates/kaeso-codecs/src/openai_images/`）、`kaeso_codecs::minimax_images::MinimaxImages`（`crates/kaeso-codecs/src/minimax_images/`），以及 Gemini 编解码器中的图片请求（`crates/kaeso-codecs/src/gemini/images.rs`）
- **端点：**`POST /v1/images/generations`、`POST /v1/images/edits`（JSON 或 `multipart/form-data`）和 `POST /v1/images/variations`
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_images/` 和 `fixtures/minimax_images/`，全部依据规范和服务商文档构建，没有录制的

**来源。**OpenAI OpenAPI 规范，提交 `926a7b8133226630bac025350e84b0ca5c31d45a`（2026-11-07）：`CreateImageRequest`、`CreateImageEditRequest`、`EditImageBodyJsonParam`、`ImageRefParam`、`CreateImageVariationRequest`、`ImagesResponse`、`Image`、`ImageGenUsage`、`ImagesUsage`、`ImageGenPartialImageEvent`、`ImageGenCompletedEvent` 以及对应的 `image_edit.*`。图片相关的 schema 在其他 OpenAI 编解码器固定的提交（`e95c0fe`）之后有改动，因此这个方言固定较新的提交。`multipart/form-data` 依据 RFC 7578。各服务商的页面列在[服务商及其形式](#服务商及其形式)中。

## 决定

维护者把以下决定交给了媒体相关工作，每一项都记录在这里。

- **三种操作共用一个 KPP 请求。**`Request::Image(ImageRequest)`，`ImageOperation` 为 `Generate`、`Edit` 或 `Variation`，回复为 `Reply::Image(ImageResponse)`。路径指明操作，因此应用的请求用 `dispatch::decode_image_request` 解码，就像 Gemini 请求用 `decode_gemini_request` 解码一样。
- **每种线上格式一个方言。**`openai_images` 服务 OpenAI 以及所有使用其 API 某种形式的服务商，每个服务商由一个 `ImagesProfile` 描述。`minimax_images` 是 MiniMax 自己的格式。Gemini 图片模型通过 `gemini` 方言以 `generateContent` 调用接收图片请求。
- **所有地方都拒绝变体。**OpenAI 已停用其变体端点（规范中为 410），也没有其他服务商提供文档。请求仍会被解码，以便应用以自己的格式收到明确的错误：`Unsupported`（`image variations`，参数 `image`），并指向 `/v1/images/edits`。
- **不处理图片。**Kaeso 不转换、不缩放、不下载图片。服务商无法生成的尺寸、需要字节却给了链接、或服务商只返回链接却要求字节，都会在发送任何内容之前被拒绝。
- **不支持流式的服务商的流。**`partial_images` 为 0 或未设置时，OpenAI 会把完成的图片作为一个 `*.completed` 事件发送。Kaeso 向这类服务商请求完整回复，并写出相同的事件，每张图片一个，用量放在最后一个事件中。向这类服务商请求部分图片会得到 `Unsupported`（参数 `partial_images`）。

## 表格的读法

- **typed**：有类型的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `openai_images`。每个 `openai_images` 服务商都会收到它们，除非其 profile 拒绝该字段；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **form**：编解码器私有的 `Extras::form` 标记。

## 请求

JSON 生成请求、JSON 编辑请求和 multipart 上传除图片外共用所有字段。

| 线上字段 | KPP | 说明 |
|---|---|---|
| `model` | `ImageRequest.model` typed | `validate()` 要求必填。 |
| `prompt` | `prompt` typed | 生成和编辑时必填。 |
| `images[]`（JSON 编辑）`{image_url}` | `images[]` `Source::Url`，规范的 data URL 为 `Source::Base64` | form `body: "json"`。 |
| `images[]` `{file_id}` | `Source::FileId`，来源 `openai_images` | 只发送给 profile 接受文件 ID 的服务商。 |
| `image`、`image[]`（上传） | `images[]` `Source::Base64`，带该部分的 `Content-Type`，以及 `ImageInput.filename` | form `body: "multipart"`；第一个字段名记在 form `image_field`。没有文件名的部分为 `InvalidRequest`。 |
| `mask` | `mask` typed | 上传文件或 `ImageRefParam`。 |
| `markup` | extras `markup`，形式为 `{image_url}` | 上传文件会变成 data URL，写回时还原为文件部分。 |
| `n` | `count` typed | |
| `size` | `size`（`ImageSize`）typed | `auto` 或 `WIDTHxHEIGHT`，其他值原样保留。 |
| `quality` | `quality`（`ImageQuality`）typed | `auto`、`low`、`medium`、`high`、`xhigh`、`max`、`standard`、`hd`；其他值原样保留。 |
| `output_format` | `format`（`ImageFormat`）typed | `png`、`jpeg`、`webp`、`gif`、`svg`；其他值原样保留。 |
| `output_compression` | `compression` typed | 0 到 100，由 `validate()` 检查。 |
| `background` | `background` typed | |
| `response_format` `url`、`b64_json` | `delivery` `Url`、`Base64` typed | 其他值：`InvalidRequest`。 |
| `stream` `true` | `stream` 开启 | `false`：form `stream_off`。上传中只接受 `true`/`false`。 |
| `partial_images` | `partial_images` typed | |
| `safety_identifier` | `user` typed | 同时有 `user`：extras `user`。 |
| 只有 `user` | `user` typed | form `user: "user"`，以便写回为 `user`。 |
| `aspect_ratio`、`resolution`、`seed`、`negative_prompt` | typed | 不是 OpenAI 的字段；SpaceX、OpenRouter、Together AI、QwenCloud、MiniMax 和 Gemini 接受其中一部分。OpenAI 的 profile 会拒绝它们。 |
| `moderation`、`style`、`input_fidelity`、`complexity`、`frame_count`、`frame_duration_ms`、`svg`、未知键 | extras | 带方括号的上传字段会嵌套：`svg[model]` 变为 `svg: {model}`。上传的 `frame_count` 和 `frame_duration_ms` 会变为数字。 |
| header `openai-beta` | extras `header:openai-beta` | |

multipart 读取器（`kaeso_codecs::multipart`）不含 I/O 且有上限：最多 64 个部分（`MAX_FORM_PARTS`）、每部分 50 MB（`MAX_IMAGE_BYTES`，OpenAI 每张图片的上限）、每部分 8 KiB 的头部。它只读取 RFC 7578 要求的 CRLF 分帧，并拒绝没有 `Content-Disposition: form-data` 或没有字段名的部分。写入器选择一个不出现在任何部分中的分隔符，由内容推导，使编码保持确定性。

`validate()` 会拒绝带图片的生成请求、没有图片的编辑或变体请求、带多张图片或遮罩的变体请求、`n` 为 0，以及大于 100 的 `output_compression`。

## 回复

| 线上字段 | KPP | 说明 |
|---|---|---|
| `created` | `ImageResponse.created` | 服务商未发送时写为 Kaeso 收到请求的时间，因为 `ImagesResponse` 要求该字段。 |
| `data[].b64_json` | `GeneratedImage.source` `Source::Base64` | 媒体类型取自 `data[].mime_type`（SpaceX）、`data[].media_type`（OpenRouter）、回复的 `output_format`，否则为 OpenAI 的默认值 `image/png`。 |
| `data[].url` | `Source::Url` | 同时有 `b64_json` 和 `url`：链接放在 extras `url`。 |
| `data[].revised_prompt` | `revised_prompt` typed | |
| `background`、`output_format`、`size`、`quality` | `settings` typed | |
| `usage.input_tokens`（或 OpenRouter 的 `prompt_tokens`） | `usage.input_tokens`，不含 `input_tokens_details.cached_tokens` | 用量对象保留在 `usage.raw` 中。 |
| `usage.output_tokens`（或 `completion_tokens`） | `usage.output_tokens` | |
| `usage.input_tokens_details.image_tokens` | `usage.input_image_tokens` | |
| `usage.output_tokens_details.image_tokens`（或 `completion_tokens_details`） | `usage.output_image_tokens` | |
| 没有 `usage` | form `no_usage` | |
| `svg_usage`、`safety_identifier`、未知键 | extras | |

来自其他格式的回复会得到 OpenAI 的用量结构：`input_tokens`、`output_tokens`、`total_tokens`、`input_tokens_details`，已知图片输出 Token 时还有 `output_tokens_details`。

## 流式

`image_generation.partial_image` 和 `image_edit.partial_image` 变为 `StreamEvent::Image`，`ImageEvent.partial` 设为 `partial_image_index`；`*.completed` 事件变为带用量的完成图片。`created_at`、`size`、`quality`、`background` 和 `output_format` 进入 `created` 和 `settings`。事件名称跟随应用调用的端点：编辑请求的流使用 `image_edit.*`。

规范要求这些事件中除 `usage` 外的所有字段。来自其他格式的事件在设置未知时写 `auto`，格式未知时写 `png`，时间未知时写 `0`。流中的错误写为 `event: error`，内容为 `{"type": "error", "error": {type, code, message, param}}`，即 OpenRouter 文档中的形式，因为 OpenAI 的规范没有定义错误事件。其他事件，例如 OpenRouter 的 `image_generation.text_chunk`，保留为 `StreamEvent::Extra`。OpenRouter 结尾的 `data: [DONE]` 不会写回。

## 错误

使用 OpenAI 的错误结构，与 [OpenAI Responses](openai-responses.md#错误errorresponse) 共用；错误带有来源 `openai_images`。`moderation_blocked` 及 OpenAI 的其他代码保持其类型，OpenAI 的原始错误会原样到达 OpenAI 应用。

## 服务商及其形式

每个服务商的 `ImagesProfile` 说明它在哪里提供生成和编辑、如何写尺寸、图片和交付方式，以及缺少 OpenAI 的哪些字段。服务商缺少的字段会在发送前得到 `Unsupported`，`param` 中写明该字段。

| Profile | 生成 | 编辑 | 差异 | 来源 |
|---|---|---|---|---|
| `OPENAI` | `/v1/images/generations` | `/v1/images/edits`，上传用 multipart，图片为链接或文件 ID 时用 JSON | 流式发送部分图片。 | 规范 `926a7b8` |
| `VERCEL` | 相同 | 相同，不接受文件 ID | 不支持流式；用 `user` 代替 `safety_identifier`；`n` 没有上限。 | [Vercel image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| `XAI`（SpaceX） | 相同 | `/v1/images/edits`，只接受 JSON：`image` 或 `images[]`，元素为 `{type: "image_url", url}`，最多 5 个，不接受遮罩 | 用 `aspect_ratio` 和 `resolution` 代替 `size`；`quality` 为 `low`、`medium`、`auto`；没有输出格式、背景、压缩或审核。 | [xAI images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| `TOGETHER` | 相同 | 在生成端点上：`image_url` 中的一个链接；拒绝上传 | `width` 和 `height`；`response_format: "base64"`；`n` 最多 4；`jpeg` 或 `png`；`aspect_ratio`、`seed`、`negative_prompt`。 | [Together AI create image](https://docs.together.ai/reference/post-images-generations) |
| `QWENCLOUD` | 相同，位于 `/compatible-mode` 下 | 在生成端点上：`image`，一个 data URL 或链接，或最多 10 个的列表 | 只返回链接，有效期 24 小时：拒绝 `b64_json`；`n` 最多 6；`seed`、`negative_prompt`。`usage.output_image_type` 中的档位（例如 `qima_output_2k`）成为 `settings.resolution` `2K`。 | [QwenCloud OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai) |
| `ZAI` | `/api/paas/v4/images/generations` | 无 | 每次请求一张图片；`quality` 为 `hd` 或 `standard`；`user_id`；只返回链接，有效期 30 天。 | [Z.ai generate image](https://docs.z.ai/api-reference/image/generate-image) |
| `OPENROUTER` | `/v1/images`（位于 `/api` 下） | 在生成端点上：`input_references`，元素为 `{type: "image_url", image_url: {url}}`，最多 16 个 | 只返回字节：拒绝 `url`；`aspect_ratio`、`resolution`、`seed`；只含 `data:` 的流，以 `[DONE]` 结束。 | [OpenRouter image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |

`ImagesProfile::compatible(name)` 是换了名字的 OpenAI 形式，用于自定义服务商。

## Gemini 图片模型

Gemini 编解码器把图片请求写为 `generateContent`：提示词和编辑请求中的图片作为 `inlineData` 部分，并设置 `generationConfig.responseModalities: ["IMAGE"]`，与 Google 的[图片生成指南](https://aistudio.google.com/docs/image-generation)一致。来源：该指南、discovery 修订版 `20261106`，以及 Agent Platform 的 `GoogleCloudAiplatformV1ImageConfig`。

| KPP | Gemini | 说明 |
|---|---|---|
| `size` | `imageConfig.aspectRatio` 和 `imageSize` | 只接受指南表格中的尺寸：一张表适用于 Gemini 3 图片模型，另一张适用于 Gemini 2.5 Flash Image。`1024x1024` 为 `1K` 下的 `1:1`；`1536x1024` 无法生成，为 `Unsupported`。 |
| `aspect_ratio`、`resolution` | `imageConfig.aspectRatio`、`imageSize` | 不能与 `size` 同时使用。 |
| `seed` | `generationConfig.seed` | |
| `format`、`compression` | `imageConfig.imageOutputOptions` `mimeType`、`compressionQuality` | 仅限 Agent Platform（`GeminiProfile::image_output_options`）；`png` 或 `jpeg`。 |
| 大于 1 的 `count`、`quality`、`background`、`negative_prompt`、`mask`、以链接或文件 ID 给出的图片、`delivery` `Url` | `Unsupported` | Google 提示模型不一定遵循图片数量；Gemini 只返回字节。 |
| `user` | 省略（`kaeso-left-out: user`） | Gemini 没有终端用户字段。 |

回复中的图片部分成为图片；对于每个 Gemini 回复，`usageMetadata.candidatesTokensDetails` 中模态为 `IMAGE` 的计数成为 `usage.output_image_tokens`。模型在图片旁写的文字在图片回复中没有位置，因此被省略。没有图片的回复是错误：被 Google 的过滤器拦截时为 `ContentFiltered`，否则为 `ProviderError`。Imagen 不做映射：AI Studio 已关闭它，Agent Platform 也把其端点列为已停用。

## MiniMax Images

`minimax_images` 读写 `POST /v1/image_generation`，该端点位于 MiniMax 主机的根路径下（[MiniMax text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i)）。

| KPP | MiniMax | 说明 |
|---|---|---|
| `prompt`、`model`、`seed`、`count` | `prompt`、`model`、`seed`、`n` | `n` 最多 9。 |
| `size` `WIDTHxHEIGHT` | `width`、`height` | 由 MiniMax 检查范围（512 到 2,048，能被 8 整除）。 |
| `aspect_ratio` | `aspect_ratio` | |
| `delivery` | `response_format` `url`、`base64` | |
| `prompt_optimizer`、未知键 | extras | |
| 编辑、变体、`quality`、`output_format`、`output_compression`、`background`、`resolution`、`negative_prompt` | `Unsupported` | MiniMax 的图生图 API 接受一个角色参考图，这不是编辑。 |

回复中的 `data.image_urls` 成为链接，`data.image_base64` 成为字节，其媒体类型从开头的字节读出，因为 MiniMax 不说明格式。MiniMax 以 HTTP 200 回复，并在 `base_resp` 中报告失败：`1002` 和 `1039` 为 `RateLimited`，`1004` 和 `2049` 为 `Authentication`，`1008` 为 `Billing`，`1026` 和 `1027` 为 `ContentFiltered`，`2013` 为 `InvalidRequest`，其他为 `ProviderError`。该端点不支持流式。
