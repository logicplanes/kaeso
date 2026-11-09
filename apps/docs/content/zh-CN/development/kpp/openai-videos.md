---
title: OpenAI Videos
description: OpenAI 的 Videos API 如何映射到 KPP 的视频任务，以及 Together AI 如何接收它。
order: 8
---

在这里查阅 OpenAI Videos API 的任意字段：它在 KPP 中的位置、之后如何查询视频任务，以及每项映射的来源。修改视频编解码器读写某个字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_videos::OpenAiVideos`（`crates/kaeso-codecs/src/openai_videos/`），所有视频方言共用的辅助函数在 `crates/kaeso-codecs/src/video_jobs.rs`
- **端点：**`POST /v1/videos`（JSON 或 `multipart/form-data`）、`POST /v1/videos/{video_id}/remix`、`GET /v1/videos`、`GET /v1/videos/{video_id}`、`GET /v1/videos/{video_id}/content` 和 `DELETE /v1/videos/{video_id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_videos/`，依据规范和 Together AI 的参考文档构建，没有录制的

**来源。**OpenAI OpenAPI 规范，提交 `926a7b8133226630bac025350e84b0ca5c31d45a`（2026-11-07）：`CreateVideoJsonBody`、`CreateVideoMultipartBody`、`ImageRefParam-2`、`CreateVideoRemixBody`、`VideoResource`、`VideoListResource`、`DeletedVideoResource`、`VideoContentVariant`、`Error-2`，以及 openai-python 3.41.0 写出的字段名。Together AI：[创建视频](https://docs.together.ai/reference/create-videos)和[获取视频元数据](https://docs.together.ai/reference/get-videos-id)，2026-11-09 查阅。

## 决定

维护者把这些交给了媒体工作；每一项都记录在这里。

- **视频是一个任务。**`Request::Video(VideoRequest)` 启动一个任务，`VideoOperation` 为 `Generate` 或 `Remix`；回复是 `Reply::Video(VideoJob)`，即服务商报告的任务。之后查询任务不是 KPP 请求：`Codec` trait 的 `encode_video_poll` 和 `decode_video_poll` 构建并读取这次调用，`encode_video_delete` 在服务商支持时删除任务，`encode_video_file` 和 `decode_video_file` 把文件 ID 变为链接。
- **图片说明自己的用途。**`VideoImage` 带有 `VideoImageRole` `FirstFrame`、`LastFrame` 或 `Reference`。OpenAI 的 `input_reference` 是 Sora 视频的第一帧，因此解码为 `FirstFrame`。
- **输出指明文件所在。**`VideoJob.outputs` 以链接、内联字节或文件 ID 的形式保存每个文件，并带有其 `VideoVariant`（`video`、`thumbnail`、`spritesheet`）。以 `/` 开头的链接是服务商处的路径。
- **没有这种格式的服务商。**OpenAI 于 2026-09-24 关闭了其视频模型。应用仍然使用这个 API，因此 Kaeso 以这种格式回答；`VideosProfile::OPENAI` 保留用于往返测试，只有 Together AI 以它的一种形式接收任务。
- **尺寸只在服务商列出时转换。**只有当服务商同时记录了分辨率和宽高比时，`size` 才会变为该服务商的分辨率和宽高比，例如 1280x720 是 16:9 的 720p。Kaeso 不缩放也不转换。

## 如何阅读表格

- **typed**：有类型的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `openai_videos`；其他方言会拒绝它们（[extras](cross-format.md#extras)）。
- **form**：编解码器私有的 `Extras::form` 标记。

## 请求

| 线上字段 | KPP | 类别 |
| --- | --- | --- |
| `model` | `VideoRequest.model` | typed |
| `prompt` | `prompt` | typed |
| `input_reference` 上传 | `images[]` `FirstFrame`，带文件名的内联字节 | typed、form `body: multipart` |
| `input_reference.image_url` | `images[]` `FirstFrame`，链接或 data URL 的字节 | typed |
| `input_reference.file_id` | `Source::FileId`，来源 `openai_videos` | typed |
| `seconds`，数字字符串 | `seconds` | typed |
| `seconds`，数值 | `seconds`，带标记 `seconds_number` | typed、form |
| `size` | `size`，`WIDTHxHEIGHT` | typed |
| 其他任何字段 | 该字段 | extras |

remix 的请求体只有 `prompt`；路径指明视频。`openai_videos::decode_remix` 以空模型解码它，由调用方填入该视频的模型及其服务商的任务 ID（`remix_of`）。

## 回复

| `VideoResource` | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status`（`queued`、`in_progress`、`completed`、`failed`） | `status` |
| `progress` | `progress`；对于没有给出进度的已完成任务写为 100，否则为 0 |
| `created_at`、`completed_at`、`expires_at` | 相同，Unix 秒 |
| `prompt`、`model`、`remixed_from_video_id` | `prompt`、`model`、`remixed_from` |
| `seconds` | `seconds`，写为字符串 |
| `size` | `size`，或由 `resolution` 和 `aspect_ratio` 生成的尺寸，或 `""`，因为该字段是必填的 |
| `error` `{code, message, param, type}` | `error` |

`openai_videos::encode_list` 写出带 `first_id`、`last_id` 和 `has_more` 的 `VideoListResource`；`encode_deleted` 写出 `DeletedVideoResource`。`decode_list_query` 读取 `after`、`limit`（0 到 100，默认 20）和 `order`；`decode_content_query` 读取 `variant`。错误是 OpenAI 的 `ErrorResponse`，与其他 OpenAI 方言相同。

## Together AI

`VideosProfile::TOGETHER` 发送 `POST /v2/videos` 并查询 `GET /v2/videos/{id}`；Together 没有记录删除。

| KPP | Together AI |
| --- | --- |
| `size` | `width`、`height` |
| `resolution`、`aspect_ratio` | `resolution`、`ratio` |
| `seconds` | `seconds`，字符串 |
| `seed`、`negative_prompt`、`audio` | `seed`、`negative_prompt`、`generate_audio` |
| `FirstFrame`、`LastFrame` 图片 | `media.frame_images[]` `{input_image, frame}` |
| `Reference` 图片 | `media.reference_images[]` |
| 回复 `status` `in_progress`、`completed`、`failed` | `status`；Together 没有记录 `queued` |
| 回复 `outputs.video_url` | 输出链接；`outputs.cost` 留在 extras 中 |

Together 只接收链接形式的图片（`input_image` 是“URL path to hosted image”），因此上传会被拒绝（参数 `input_reference`），remix 也会被拒绝（参数 `video_id`）。应用的其他字段作为 extras 保留，按原样到达 Together，例如 `steps`；`input_reference` 中除 `image_url` 之外的字段会被拒绝。
