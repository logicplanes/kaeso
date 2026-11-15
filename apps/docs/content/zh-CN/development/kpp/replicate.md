---
title: Replicate
description: KPP 的图片请求和视频任务如何映射到 Replicate 的 prediction。
order: 8
---

在这里查看图片请求或视频任务如何到达 Replicate：每个模型的各个字段去向、Kaeso 如何跟进一个 prediction，以及每项映射的来源。

- **编解码器：** `kaeso_codecs::replicate::Replicate`（`crates/kaeso-codecs/src/replicate/`）
- **路径：** 使用 `Authorization: Bearer r8_…`：`POST /v1/models/{owner}/{name}/predictions`，正文为 `{"input": …}`，然后 `GET /v1/predictions/{id}`；模型检查使用 `GET /v1/collections/official`
- **Fixtures：** `crates/kaeso-codecs/fixtures/replicate/`，根据 Replicate 的文档和各模型的 schema 构建，包括 Replicate 在模型页面上公开的 prediction，以及实际记录的被拒绝 token

**来源。** Replicate 的 [HTTP API](https://replicate.com/docs/reference/http) 及其 OpenAPI 文档、[创建 prediction](https://replicate.com/docs/topics/predictions/create-a-prediction)、[输出文件](https://replicate.com/docs/topics/predictions/output-files)、[数据保留](https://replicate.com/docs/topics/predictions/data-retention)、[速率限制](https://replicate.com/docs/topics/predictions/rate-limits)、[官方模型](https://replicate.com/docs/topics/models/official-models)，以及各模型的 schema（`https://replicate.com/{owner}/{name}/api/schema`），2026-11-14 阅读。

## Prediction

图片请求带 `Prefer: wait=60` 发送，因此 Replicate 在图片生成后才回复，大多数模型都在同一请求内完成。等待结束后仍为 `starting` 或 `processing` 的 prediction，会按其 ID 再次查询，直到结束，但不超过请求的时间限制。视频任务开始时不等待；任务 ID 即 prediction 的 ID，每次查询都请求 `GET /v1/predictions/{id}`。Kaeso 只请求由普通字符组成的 ID 的 `/v1/predictions/{id}`，从不请求 prediction 中给出的 `urls`。

Replicate 在 prediction 结束一小时后删除其输入、输出和文件。图片的 `url` 在这一小时内有效，与 OpenAI 自己的图片链接相同；视频文件在此之前可以下载，任务的 `expires_at` 给出截止时间。

## 模型

每个模型都有自己的输入，因此 Kaeso 只映射以下官方模型，其他模型一律拒绝并给出此列表：

| 模型 | 类型 | 接受 |
| --- | --- | --- |
| `black-forest-labs/flux-schnell`、`black-forest-labs/flux-dev` | 图片 | `n` 1 到 4，`aspect_ratio`（`1:1`、`16:9`、`21:9`、`3:2`、`2:3`、`4:5`、`5:4`、`3:4`、`4:3`、`9:16`、`9:21`），`seed`，`output_format` 为 `webp`、`jpeg` 或 `png`，`output_compression`；不接受像素尺寸 |
| `black-forest-labs/flux-1.1-pro` | 图片 | 一张图片；`size` 每边 256 到 1440、步长 32，或 `aspect_ratio`；`seed`、`output_format`、`output_compression` |
| `bytedance/seedream-4.5` | 图片和编辑 | `n` 1 到 15（模型可能生成更少）；`size` 每边 1024 到 4096，或 `aspect_ratio` 和 `resolution`（`2K` 或 `4K`）；最多 14 张图片；不接受 `seed` 和 `output_format` |
| `google/nano-banana-pro` | 图片和编辑 | 一张图片；`aspect_ratio` 和 `resolution`（`1K`、`2K` 或 `4K`）；最多 14 张图片；`output_format` 为 `jpeg` 或 `png` |
| `google/veo-3.1` | 视频 | `seconds` 4、6 或 8；`size` 为 720p 或 1080p、16:9 或 9:16；`audio`、`seed`、`negative_prompt`；首帧和尾帧，或 1 到 3 张参考图（8 秒、16:9） |
| `kwaivgi/kling-v2.5-turbo-pro` | 视频 | `seconds` 5 或 10；`aspect_ratio` 为 `16:9`、`9:16` 或 `1:1`，除非首帧决定形状；`negative_prompt`；首帧和尾帧 |

## 图片请求

| KPP | Replicate 的 `input` |
| --- | --- |
| `prompt` | `prompt` |
| `n` | FLUX.1 上为 `num_outputs`；Seedream 上为 `max_images` 加 `sequential_image_generation: "auto"` |
| `size` `宽x高` | FLUX1.1 [pro] 上为 `aspect_ratio: "custom"` 加 `width` 和 `height`，Seedream 上为 `size: "custom"` 加 `width` 和 `height`；`auto` 不发送 |
| `aspect_ratio` | `aspect_ratio` |
| `resolution` | Seedream 上为 `size`，Nano Banana Pro 上为 `resolution` |
| `output_format` | `output_format`，`jpeg` 写作 `jpg` |
| `output_compression` | `output_quality` |
| `seed` | `seed` |
| 要编辑的图片 | `image_input`：链接按原样，上传的图片作为 data URI |
| `response_format: b64_json` | 不发送：Kaeso 从 `replicate.delivery` 下载链接 |

各模型的 schema 没有说明 Replicate 是否拒绝它们没有的字段，因此 Kaeso 在发送前拒绝非 `auto` 的 `quality` 和 `background`、`negative_prompt`、`user`、蒙版、流式和变体，以及只生成一张图片的模型上大于 1 的 `n`。Replicate 格式请求自带的字段（例如 `guidance` 或 `safety_tolerance`）按原样发送。

## 图片结果

| Replicate | KPP |
| --- | --- |
| `output`：一个链接或链接列表 | 图片的链接 |
| 自定义尺寸时的 `input.width`、`input.height` | 回复的尺寸 |
| `input.resolution`，或 Seedream 的 `input.size` | 图片计费所按的分辨率 |
| `status` 为 `failed`、`canceled` 或 `aborted` | 服务商错误，以 `error` 为消息 |
| `id`、`version`、`input`、`logs`、`metrics`、`urls` 和各时间 | extras |

## 视频任务

| KPP | Replicate 的 `input` |
| --- | --- |
| `prompt` | `prompt` |
| `seconds` | `duration` |
| `size` | 由短边得出的 `resolution` 和 `aspect_ratio`，仅 Veo |
| `aspect_ratio`、`resolution` | `aspect_ratio`、`resolution` |
| `audio` | `generate_audio`，仅 Veo；除非关闭，Veo 会生成声音，价格翻倍 |
| `seed`、`negative_prompt` | `seed`（仅 Veo）、`negative_prompt` |
| 首帧和尾帧 | Veo 上为 `image` 和 `last_frame`，Kling 上为 `start_image` 和 `end_image` |
| 参考图 | `reference_images`，仅 Veo |

视频任务需要 `seconds`，因为 Replicate 按秒计费。Kaeso 拒绝大于 1 的 `n`、`user` 和 remix，在 Kling 上还拒绝 `size`、`seed` 和 `audio`：其 schema 保留了这些字段，但说明不会使用。

## 状态

| Replicate | KPP |
| --- | --- |
| `starting` | `queued` |
| `processing` | `in_progress` |
| `succeeded` | `completed`，以 `output` 作为视频链接；`expires_at` 为 `completed_at` 之后一小时 |
| `failed`、`canceled`、`aborted` | `failed`，以状态为代码、`error` 为消息 |

## 错误

| Replicate | KPP |
| --- | --- |
| `{"title", "detail", "status"}`（problem details） | 以 `title: detail` 为消息；类型取自状态码：401 `authentication`、402 `billing`、404 `not_found`、422 `invalid_request`、429 `rate_limited` |
| `{"detail": "…"}` | 以 `detail` 为消息，类型取自状态码 |
| `Retry-After` | 重试等待时间 |

## 模型检查

**检查可用性**读取 `GET /v1/collections/official`，即 Replicate 的官方模型，并按每个模型的 `owner/name` 匹配。该集合未列出的模型判为无法确定，而不是不可用。
