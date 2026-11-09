---
title: SpaceX Videos
description: KPP 的视频任务如何映射到 SpaceX 的 Grok Imagine 视频 API。
order: 8
---

在这里查阅视频任务如何到达 SpaceX：哪个字段放在哪里、如何查询任务，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::xai_videos::XaiVideos`（`crates/kaeso-codecs/src/xai_videos/`）
- **路径：**`POST /v1/videos/generations`、`POST /v1/videos/edits` 和 `GET /v1/videos/{request_id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/xai_videos/`，依据 SpaceX 的参考文档构建，没有录制的

**来源。**SpaceX 的[视频参考](https://docs.x.ai/developers/rest-api-reference/inference/videos)和[视频生成指南](https://docs.x.ai/developers/model-capabilities/video/generation)，2026-11-09 查阅。

## 决定

- **remix 就是编辑。**`POST /v1/videos/edits` 用新的提示词修改一个已完成的视频，这正是 OpenAI 的 remix 所做的。它通过链接接收先前的视频，因此网关先向 SpaceX 获取该链接，再以 `video.url` 发送（`VideoRequest.source_video`）。
- **任务 ID 是 `request_id`。**启动任务只返回 `request_id`；任务的状态、链接和时长来自 `GET /v1/videos/{request_id}`。
- **被扣留的视频算作失败。**`status: "done"` 且 `respect_moderation: false` 时没有链接；KPP 将其报告为 `failed`，代码为 `moderation_blocked`。`expired` 为 `failed`，代码为 `expired`。

## 请求

| KPP | SpaceX |
| --- | --- |
| `model`、`prompt`、`user` | `model`、`prompt`、`user` |
| `seconds`（1 到 15） | `duration` |
| `aspect_ratio`、`resolution`（`480p`、`720p`、`1080p`） | `aspect_ratio`、`resolution` |
| `size` | `resolution` 和 `aspect_ratio`，SpaceX 两者都列出时 |
| `FirstFrame` 图片 | `image` `{url}` 或 `{file_id}` |
| `Reference` 图片 | `reference_images[]` |
| remix 的 `source_video` | 编辑路径上的 `video.url` |
| `keyframes`、`reference_audios`、`output`、`storage_options` | extras |

SpaceX 不接收最后一帧、种子、负面提示词或数量：每一项都会在发送前被拒绝，并在 `param` 中给出其字段。

## 回复

| SpaceX | KPP |
| --- | --- |
| `request_id` | `VideoJob.id` |
| `status` `pending` | `progress` 为 0 时为 `queued`，之后为 `in_progress` |
| `status` `done`、`failed`、`expired` | `completed`、`failed`、`failed` |
| `progress`、`model` | `progress`、`model` |
| `video.url`、`video.duration` | 输出链接、`seconds` |
| `error` `{code, message}` | `error` |
| `usage`、`video.file_output` | extras |
