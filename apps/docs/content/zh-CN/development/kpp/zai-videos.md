---
title: Z.ai Videos
description: KPP 的视频任务如何映射到 Z.ai 的 CogVideoX API。
order: 8
---

在这里查阅视频任务如何到达 Z.ai：哪个字段放在哪里、如何查询任务，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::zai_videos::ZaiVideos`（`crates/kaeso-codecs/src/zai_videos/`）
- **路径：**从 Z.ai 主机的根路径开始，带 `Authorization: Bearer`：`POST /api/paas/v4/videos/generations` 和 `GET /api/paas/v4/async-result/{id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/zai_videos/`，依据 Z.ai 的 OpenAPI 文档构建，没有录制的

**来源。**Z.ai 的[生成视频](https://docs.z.ai/api-reference/video/generate-video)和[获取结果](https://docs.z.ai/api-reference/video/get-video-status)，2026-11-09 查阅。

## 请求

| KPP | Z.ai |
| --- | --- |
| `model`、`prompt` | `model`、`prompt` |
| `seconds`（5 或 10） | `duration` |
| `size`，或 `resolution` 加 `aspect_ratio` | `size`，Z.ai 列出的尺寸之一 |
| `audio` | `with_audio` |
| `user` | `user_id` |
| `FirstFrame`、`LastFrame` 图片 | `image_url`，一个链接或两个链接的列表 |
| `quality`、`fps`、`request_id` | extras |

Z.ai 不接收种子、负面提示词或参考图片，也不支持 remix：每一项都会在发送前被拒绝。

## 回复

| Z.ai | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `task_status` `PROCESSING`、`SUCCESS`、`FAIL` | `in_progress`、`completed`、`failed` |
| `video_result[].url` | 视频的链接 |
| `video_result[].cover_image_url` | `thumbnail` 的链接 |
