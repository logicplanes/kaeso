---
title: OpenRouter Videos
description: KPP 的视频任务如何映射到 OpenRouter 的视频生成 API。
order: 8
---

在这里查阅视频任务如何到达 OpenRouter：哪个字段放在哪里、在哪里获取其文件，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::openrouter_videos::OpenRouterVideos`（`crates/kaeso-codecs/src/openrouter_videos/`）
- **路径：**在 OpenRouter 的基础 URL（`https://openrouter.ai/api`）之后：`POST /v1/videos`、`GET /v1/videos/{jobId}` 和 `GET /v1/videos/{jobId}/content`
- **Fixture：**`crates/kaeso-codecs/fixtures/openrouter_videos/`，依据 OpenRouter 的 OpenAPI 文档构建，没有录制的

**来源。**OpenRouter 的[提交](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request)、[轮询](https://openrouter.ai/docs/api/api-reference/video-generation/poll-video-generation-status)和[内容](https://openrouter.ai/docs/api/api-reference/video-generation/download-generated-video-content)参考文档，2026-11-09 查阅。

## 决定

- **不支持 remix。**`previous_job_id` 会编辑或延长一个已完成的任务，而参考文档没有说明是哪一种，因此 remix 会被拒绝（参数 `video_id`）。
- **文件在哪里。**先获取已完成任务的签名 `urls`，然后用密钥获取 `unsigned_urls`，最后是内容路径。

## 请求

| KPP | OpenRouter |
| --- | --- |
| `model`、`prompt`、`user` | `model`、`prompt`、`user` |
| `seconds` | `duration` |
| `size`、`resolution`、`aspect_ratio` | `size`、`resolution`、`aspect_ratio`，各自取自 OpenRouter 的列表 |
| `seed`、`audio` | `seed`、`generate_audio` |
| `FirstFrame`、`LastFrame` 图片 | 带 `frame_type` 的 `frame_images[]` |
| `Reference` 图片 | `input_references[]` `image_url` 条目 |
| `provider`、`callback_url`、`session_id`、`trace`；音频或视频参考 | extras |

## 回复

| OpenRouter | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `pending`、`in_progress`、`completed` | `queued`、`in_progress`、`completed` |
| `status` `failed`、`cancelled`、`expired` | `failed`，以该状态作为代码 |
| `error`（字符串） | `error.message` |
| `expires_at` | `expires_at` |
| `urls`、`unsigned_urls` | 输出链接，以及 extras |
| `usage`、`polling_url`、`generation_id` | extras |
