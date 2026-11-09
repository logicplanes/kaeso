---
title: MiniMax Images
description: KPP 的图片请求如何映射到 MiniMax 的文生图 API。
order: 8
---

在这里查阅图片请求如何到达 MiniMax：哪个字段放在哪里、哪些内容会被拒绝，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::minimax_images::MinimaxImages`（`crates/kaeso-codecs/src/minimax_images/`）
- **路径：**`POST /v1/image_generation`，从 MiniMax 主机的根路径开始，带 `Authorization: Bearer`
- **Fixture：**`crates/kaeso-codecs/fixtures/minimax_images/`，依据 MiniMax 的参考文档构建，没有录制的

**来源。**MiniMax 的[文生图](https://platform.minimax.io/docs/api-reference/image-generation-t2i)参考和[错误码](https://platform.minimax.io/docs/api-reference/errorcode)，2026-11-08 查阅。

## 决定

- **只支持生成。** MiniMax 的图生图 API 接受一个角色参考图，而不是要修改的图片，因此编辑会被拒绝。变体与其他地方一样被拒绝（[OpenAI Images](openai-images.md#决定)）。
- **错误可能随 HTTP 200 返回。** 失败的 `base_resp` 无论状态码如何都是错误，按其代码映射。
- **不流式传输。** MiniMax 返回完成的图片。Kaeso 请求完整回复，网关为流式应用把每张图片写成一个 `image_generation.completed` 事件。
- **从字节读取媒体类型。** MiniMax 不说明 base64 图片的格式，因此编解码器从开头的字节读出它。

## 请求

| KPP | MiniMax | 说明 |
| --- | --- | --- |
| `model`、`prompt` | `model`、`prompt` | |
| `size` `WIDTHxHEIGHT` | `width`、`height` | `auto` 两者都不发送；由 MiniMax 检查范围（512 到 2,048，能被 8 整除）。MiniMax 请求体中只有 `width` 没有 `height` 时无效（`param` 为 `width`）。 |
| `aspect_ratio` | `aspect_ratio` | |
| `count` | `n` | 最多 9。 |
| `seed` | `seed` | |
| `delivery` `Url`、`Base64` | `response_format` `url`、`base64` | 其他值无效，`param` 为 `response_format`。 |
| `user` | 省略，并在 `kaeso-left-out` 中写明 | MiniMax 没有终端用户字段。 |
| `prompt_optimizer` 和其他键 | extras | 只从 MiniMax 格式的请求发送；其他方言会拒绝它们。 |

## 拒绝项

以下每项都会在发送任何内容之前返回 `Unsupported`，并在 `param` 中写明字段：

| KPP | `param` | 原因 |
| --- | --- | --- |
| 编辑（`ImageOperation::Edit`） | `image` | 图生图 API 接受角色参考图，而不是要修改的图片。 |
| 变体 | `image` | 没有服务商提供变体。 |
| 既不是像素也不是 `auto` 的 `size` | `size` | 请发送 `WIDTHxHEIGHT` 或 `aspect_ratio`。 |
| 大于 9 的 `count` | `n` | MiniMax 的上限。 |
| `quality`、`format`、`compression`、`background`、`resolution`、`negative_prompt` | `quality`、`output_format`、`output_compression`、`background`、`resolution`、`negative_prompt` | MiniMax 的参考中没有这些字段。 |
| 流中大于 0 的 `partial_images` | `partial_images` | MiniMax 不流式传输。 |

## 回复

| MiniMax | KPP |
| --- | --- |
| `data.image_urls[]` | 作为 `Source::Url` 的图片 |
| `data.image_base64[]` | 作为 `Source::Base64` 的图片，媒体类型从开头的字节读出，否则为 `application/octet-stream` |
| `status_code` 为 0 的 `base_resp` | extras `base_resp` |
| `data` 的其他键 | extras `data.<key>` |
| `metadata` 和其他顶层键 | extras |

## 错误

`status_code` 不为 0 的 `base_resp` 是错误，即使 HTTP 状态为 200：

| `status_code` | KPP |
| --- | --- |
| `1002`、`1039` | `RateLimited` |
| `1004`、`2049` | `Authentication` |
| `1008` | `Billing` |
| `1026`、`1027` | `ContentFiltered` |
| `2013` | `InvalidRequest` |
| 其他 | `ProviderError` |

没有 `base_resp` 的请求体按 OpenAI 风格的错误读取。对 MiniMax 格式的应用，编解码器写出带有对应错误类别代码的 `base_resp`：速率限制为 `1002`，身份验证为 `1004`，计费为 `1008`，内容过滤为 `1026`，无效或被拒绝的请求为 `2013`，其余为 `1000`。[MiniMax Speech](minimax-speech.md) 以同样的方式读取错误。

## Fixture

| 文件 | 涵盖内容 | 构建依据 |
| --- | --- | --- |
| `requests/generate.json` | `aspect_ratio`、`n`、`prompt_optimizer` | 参考中的请求示例 |
| `requests/generate-size-seed.json` | `width`、`height`、`seed`、`response_format: base64` | 模式 `ImageGenerationReq` |
| `responses/generate-urls.json` | 三个链接、`metadata`、`base_resp` | 参考中的回复示例，补全了链接 |
| `responses/generate-base64.json` | 一张 base64 图片 | 模式中的 `data.image_base64` |
| `errors/sensitive-prompt-200.json` | HTTP 200 下的 `1026` | 错误码参考 |
| `errors/invalid-key-200.json` | HTTP 200 下的 `2049` | 错误码参考 |

没有可用于图片的 MiniMax 密钥，因此没有录制内容。该文件夹的 `README.md` 列出了来源，`*.meta.json` 给出 HTTP 状态。
