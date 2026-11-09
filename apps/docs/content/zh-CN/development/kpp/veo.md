---
title: Veo
description: KPP 的视频任务如何映射到 Gemini Enterprise Agent Platform 上 Veo 的长时间运行操作。
order: 8
---

在这里查阅视频任务如何到达 Gemini Enterprise Agent Platform 上的 Veo：哪个字段放在哪里、如何查询操作，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::veo::Veo`（`crates/kaeso-codecs/src/veo/`）
- **路径：**`{models}/{model}:predictLongRunning` 和 `{models}/{model}:fetchPredictOperation`，其中 `{models}` 是该平台的发布者路径（`EgressCtx::gemini_profile`）
- **Fixture：**`crates/kaeso-codecs/fixtures/veo/`，依据该平台的参考文档构建，没有录制的；Kaeso 从不实时调用该平台

**来源。**Agent Platform 的[根据文本生成视频](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text)、[Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate)、`VideoGenerationModelInstance` 和 `VideoGenerationModelParams`，2026-11-09 查阅。

## 决定

- **任务 ID 是操作的名称。**查询它使用 `POST …:fetchPredictOperation` 和 `{"operationName": …}`，发往启动它的模型。
- **视频以内联方式返回。**Kaeso 不指定 `storageUri`，因此该平台在操作的 `response.videos` 中以 `bytesBase64Encoded` 返回每个视频。`gcsUri` 作为链接保留，Kaeso 无法获取它。
- **仅限服务账号。**快捷模式不列出任何 Veo 模型（[快捷模式](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/express-mode/overview)），因此网关会拒绝用于这些模型的快捷模式密钥。
- **被过滤的操作算作失败。**已完成但没有视频且带有 `raiMediaFilteredCount` 的操作为 `failed`，代码为 `moderation_blocked`。

## 请求

| KPP | Veo |
| --- | --- |
| `prompt` | `instances[0].prompt` |
| `FirstFrame` 图片 | `instances[0].image` `{bytesBase64Encoded, mimeType}` 或 `{gcsUri}` |
| `LastFrame` 图片 | `instances[0].lastFrame` |
| `Reference` 图片 | 带 `referenceType: "asset"` 的 `instances[0].referenceImages[]` |
| `seconds`（4、6 或 8） | `parameters.durationSeconds` |
| `size`，或 `resolution` 和 `aspect_ratio` | `parameters.resolution`（`720p`、`1080p`、`4k`）、`parameters.aspectRatio`（`16:9`、`9:16`） |
| `seed`、`negative_prompt`、`audio`、`count`（1 到 4） | `seed`、`negativePrompt`、`generateAudio`、`sampleCount` |
| 其他实例和参数字段 | extras |

网页链接形式的图片会被拒绝：该平台接收字节或 `gs://` 链接。

## 回复

| Veo | KPP |
| --- | --- |
| `name` | `VideoJob.id` |
| `done: false` 或不存在 | `in_progress` |
| `response.videos[]` | `completed`，每个视频一个输出 |
| `error` `{code, message}` | 带该错误的 `failed` |
