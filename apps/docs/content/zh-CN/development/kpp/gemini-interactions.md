---
title: Gemini Interactions
description: KPP 的视频任务如何映射到 Google AI Studio Interactions API 上的 Gemini Omni。
order: 8
---

在这里查阅视频任务如何到达 Google AI Studio 上的 Gemini Omni：哪个字段放在哪里、如何查询任务，以及每项映射的来源。这里只映射 Interactions API 的视频部分。

- **编解码器：**`kaeso_codecs::gemini_interactions::GeminiInteractions`（`crates/kaeso-codecs/src/gemini_interactions/`）
- **路径：**`POST /v1beta/interactions`、`GET /v1beta/interactions/{id}` 和 `DELETE /v1beta/interactions/{id}`
- **Fixture：**`crates/kaeso-codecs/fixtures/gemini_interactions/`；`responses/recorded-started.json` 和 `polls/recorded-failed-quota.json` 使用免费套餐的密钥录制，其余依据参考文档构建

**来源。**Google AI Studio 的 [Gemini Omni Flash](https://aistudio.google.com/docs/omni) 和[创建交互](https://aistudio.google.com/api-reference/interactions-api/create-interaction)参考文档，2026-11-09 查阅。

## 决定

- **不使用 `generateContent`。**这些模型列出了 `generateContent`，但 Google 只通过 Interactions API 记录 Omni 的视频功能，因此 Kaeso 把视频任务发往那里。
- **在后台运行。**每个任务都带 `background: true` 发送，它会立即返回交互的 ID；之后查询时以内联方式返回视频。
- **任务类型跟随图片。**首帧和尾帧发送 `generation_config.video_config.task: "image_to_video"`，参考图片发送 `"reference_to_video"`；没有图片时由模型推断任务类型。
- **remix 延续交互。**`previous_interaction_id` 用新的提示词改进先前的视频，与 Google 的有状态编辑相同。
- **Omni 总是生成声音。**`audio: false` 会被拒绝（参数 `audio`）；`audio: true` 不发送任何内容。

## 请求

| KPP | Interactions API |
| --- | --- |
| `model` | `model` |
| `prompt` | `input`，字符串，或图片之后的 `text` 条目 |
| 图片 | `input[]` `{type: "image", data, mime_type}` 或 `{uri}` |
| `seconds`（3 到 15） | `response_format.duration`，例如 `"8s"` |
| `size`，或 `resolution` 和 `aspect_ratio` | `response_format.resolution`（`360p`、`720p`、`1080p`、`4k`）、`response_format.aspect_ratio`（`16:9`、`9:16`） |
| remix 的 `remix_of` | `previous_interaction_id` |

Omni 不接收种子、负面提示词或数量，也不接收同时带有帧和参考图片的请求。

## 回复

| Interactions API | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `in_progress` | `in_progress` |
| `status` `completed` 且带视频 | `completed` |
| `status` `failed`、`cancelled`、`incomplete`，或 `completed` 但没有视频 | `failed`，以 `errors[]` 的第一项作为错误 |
| `steps[]` `model_output` 中的 `video` 条目 | 输出，内联字节或链接 |
| `usage` `total_input_tokens`、`total_output_tokens`、`total_thought_tokens` | `usage`；思考 token 计为输出和推理 |
| `created`、`updated` | `created_at`，结束后还有 `completed_at` |
