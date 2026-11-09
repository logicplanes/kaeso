---
title: DashScope
description: KPP 的视频任务如何映射到 QwenCloud 上 DashScope 的 Wan 模型视频合成。
order: 8
---

在这里查阅视频任务如何到达 QwenCloud 的 Wan 模型：某个模型接收哪种请求体、哪个字段放在哪里、如何查询任务，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::dashscope::DashScope`（`crates/kaeso-codecs/src/dashscope/`）
- **路径：**从 QwenCloud 主机的根路径开始，带 `Authorization: Bearer` 和 `X-DashScope-Async: enable`：`POST /api/v1/services/aigc/video-generation/video-synthesis`、`POST /api/v1/services/aigc/image2video/video-synthesis`（Wan 2.2 和 2.1 的首尾帧）和 `GET /api/v1/tasks/{task_id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/dashscope/`，依据 QwenCloud 的 OpenAPI 文档构建，没有录制的

**来源。**QwenCloud 关于 [Wan 2.6 及更早版本文生视频](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task)、[首帧图生视频](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task)、[Wan 2.7 文生视频](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task)及[其查询结果](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result)、[Wan 2.7 图生视频](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task)、[Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task) 和[异步任务](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks)的参考文档，2026-11-09 查阅。

## 决定

- **模型决定请求体。**`dashscope::family` 读取模型名称：`wan3.0-video*`、`wan2.7-t2v*`、`wan2.7-i2v*`、`*-kf2v-*`（首尾帧）、`*-t2v*` 和 `*-i2v*`（Wan 2.6 及更早版本）。编辑或参考视频的模型，例如 `wan2.7-r2v`、`wan2.7-videoedit`、`wan2.1-vace-plus` 和 `wan2.2-animate-*`，会被拒绝，`param` 为 `model`。
- **只使用 QwenCloud 列出的尺寸。**Wan 2.6 及更早版本接收以 `width*height` 表示的像素 `size`。Wan 2.7 文生视频接收档位和比例，因此只有当 `size` 是其参考文档列出的十种尺寸之一时才会转换，例如 `1104x832` 是 `4:3` 的 `720P`。图生视频只接收档位，因为宽高比由图片决定，所以在那里 `size` 会被拒绝。
- **不支持 remix 和删除。**DashScope 只能取消尚未开始的任务，这不是删除，因此 Kaeso 只会忘掉被删除的视频。
- **时间为北京时间。**`submit_time` 和 `end_time` 为不带偏移量的 UTC+8；`dashscope::unix_of_beijing` 负责读取。视频链接有效期为 24 小时，因此已完成的任务在 `end_time` 之后 24 小时过期。

## 请求

| KPP | Wan 2.6 及更早版本 | Wan 2.7 | Wan 3.0 |
| --- | --- | --- | --- |
| `model`、`prompt` | `model`、`input.prompt` | 相同 | 相同 |
| `negative_prompt` | `input.negative_prompt` | `input.negative_prompt` | 拒绝 |
| `FirstFrame` 图片 | `input.img_url`；Wan 2.2 和 2.1 的首尾帧：`input.first_frame_url` | `input.media[]` `{type: "first_frame", url}` | 相同 |
| `LastFrame` 图片 | `input.last_frame_url`，仅限网页链接 | `input.media[]` `{type: "last_frame", url}` | 相同 |
| `Reference` 图片 | 拒绝 | 拒绝 | `input.media[]` `{type: "reference_image", url}`，不能与首尾帧同时使用 |
| `size` | `parameters.size`，`1280*720`，也可由分辨率和宽高比生成（文生视频） | `parameters.resolution` 和 `ratio`，取自列出的尺寸（文生视频） | 拒绝 |
| `resolution` | `parameters.resolution`（图生视频），转为大写 | `parameters.resolution` | `parameters.resolution` |
| `aspect_ratio` | 拒绝 | `parameters.ratio`（文生视频） | `parameters.ratio` |
| `seconds`、`seed` | `parameters.duration`、`parameters.seed` | 相同 | 相同 |
| `audio` | `parameters.audio`，仅限 `wan2.6-i2v-flash` | 拒绝 | `parameters.audio` |
| `input` 和 `parameters` 的其他字段，例如 `audio_url`、`prompt_extend`、`watermark` | extras | extras | extras |

图片以链接或 data URL 发送，但 Wan 2.2 和 2.1 的首尾帧除外，QwenCloud 只接收公开链接。其他类型的媒体，例如 `driving_audio` 或 `first_clip`，在解码请求时会被拒绝。

## 回复

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`、`RUNNING`、`SUCCEEDED` | `queued`、`in_progress`、`completed` |
| `output.task_status` `FAILED`、`CANCELED`、`UNKNOWN` | `failed`，带 `output.code` 和 `output.message`，或以状态作为代码 |
| `output.video_url` | 视频的链接，`video/mp4` |
| `output.submit_time`、`output.end_time` | `created_at`、`completed_at` |
| `usage.output_video_duration`、`usage.SR`、`usage.ratio` | `seconds`、`resolution`、`aspect_ratio` |
| `request_id`、`output` 和 `usage` 的其他字段 | extras |

错误格式为 `{"request_id", "code", "message"}`：`InvalidApiKey` 是认证错误，`Throttling…` 是速率限制，`DataInspection…` 是内容过滤；其他代码按 HTTP 状态处理。
