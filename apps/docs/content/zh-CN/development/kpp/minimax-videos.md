---
title: MiniMax Videos
description: KPP 的视频任务如何映射到 MiniMax 的 Hailuo 和 H3 视频 API。
order: 8
---

在这里查阅视频任务如何到达 MiniMax：模型使用哪个 API、哪个字段放在哪里，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::minimax_videos::MinimaxVideos`（`crates/kaeso-codecs/src/minimax_videos/`）
- **路径：**从 MiniMax 主机的根路径开始，带 `Authorization: Bearer`：`POST /v1/video_generation`、`GET /v1/query/video_generation`、`GET /v1/files/retrieve`；`POST /v2/video_generation`、`GET /v2/query/video_generation/{task_id}`、`DELETE /v2/video_generation/{task_id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/minimax_videos/`，依据 MiniMax 的 OpenAPI 文档构建，没有录制的

**来源。**MiniMax 的[文生视频](https://platform.minimax.io/docs/api-reference/video-generation-t2v)、[图生视频](https://platform.minimax.io/docs/api-reference/video-generation-i2v)、[首尾帧](https://platform.minimax.io/docs/api-reference/video-generation-fl2v)、[查询](https://platform.minimax.io/docs/api-reference/video-generation-query)、[获取文件](https://platform.minimax.io/docs/api-reference/file-management-retrieve)和 [H3 API](https://platform.minimax.io/docs/api-reference/video-generation-v2-create)，2026-11-09 查阅。

## 决定

- **模型决定 API。**`MiniMax-H3` 及其变体使用 H3 API（`/v2`）；Hailuo 模型使用 `/v1`。
- **Hailuo 的文件是文件 ID。**完成的 Hailuo 任务给出 `file_id`；每次获取文件时，`encode_video_file` 都向 `GET /v1/files/retrieve` 请求其 `download_url`，该链接有效期为一小时。
- **错误可能随 HTTP 200 返回。**失败的 `base_resp` 无论状态码如何都是错误，与 MiniMax 的其他 API 一样按其代码映射。
- **H3 需要时长和分辨率。**MiniMax 要求两者都提供，因此没有 `seconds` 或尺寸的请求会在发送前被拒绝（参数 `seconds` 和 `size`）。

## 请求

| KPP | Hailuo（`/v1`） | H3（`/v2`） |
| --- | --- | --- |
| `model` | `model` | `model` |
| `prompt` | `prompt` | `content[]` `{type: "text"}` |
| `seconds` | `duration` | `duration` |
| `resolution` | `resolution`（`512P`、`720P`、`768P`、`1080P`） | `resolution`（`480P`、`768P`、`2K`） |
| `size` | 拒绝 | `resolution` 和 `ratio`，与列出的比例相差不超过 1% |
| `aspect_ratio` | 拒绝 | `ratio` |
| `FirstFrame`、`LastFrame` 图片 | `first_frame_image`、`last_frame_image` | 带 `role` 的 `content[]` `image_url` |
| `Reference` 图片 | 拒绝 | `content[]` `image_url`，`role: "reference_image"` |
| `prompt_optimizer`、`fast_pretreatment`、`extra`、`callback_url` | extras | extras |

参考视频和音频（`video_url`、`audio_url` 条目）会被拒绝，参数为 `content`。

## 回复

| Hailuo | H3 | KPP |
| --- | --- | --- |
| `task_id` | `task_id` | `VideoJob.id` |
| `Preparing`、`Queueing` | `queued` | `queued` |
| `Processing` | `running` | `in_progress` |
| `Success` | `succeeded` | `completed` |
| `Fail` | `failed`、`cancelled` | `failed` |
| `file_id` | `content.url` | 输出的文件 ID 或链接 |
| `video_width`、`video_height` | `resolution`、`duration`、`ratio` | `size`，或 `resolution`、`seconds`、`aspect_ratio` |
| | `error` `{code, message}` | `error` |
