---
title: DashScope
description: KPP 的图片、语音、转写和视频任务如何映射到 QwenCloud 自己的 DashScope API。
order: 8
---

在这里查阅请求如何到达 DashScope 上的 QwenCloud 模型：某个模型接收哪种请求体、哪个字段放在哪里、回复如何返回，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::dashscope::DashScope`（`crates/kaeso-codecs/src/dashscope/`）
- **路径：**从 QwenCloud 主机的根路径开始，带 `Authorization: Bearer`：
  - 视频：`POST /api/v1/services/aigc/video-generation/video-synthesis`、`POST /api/v1/services/aigc/image2video/video-synthesis`（Wan 2.2 和 2.1 的首尾帧），两者都带 `X-DashScope-Async: enable`，以及 `GET /api/v1/tasks/{task_id}`；
  - 图片、Qwen3-TTS 和 Qwen-Audio-ASR：`POST /api/v1/services/aigc/multimodal-generation/generation`，在同一个请求中回复；
  - Qwen-Audio-TTS：`POST /api/v1/services/audio/tts/SpeechSynthesizer`，带 `X-DashScope-SSE: enable`。
- **Fixture：**`crates/kaeso-codecs/fixtures/dashscope/`，依据 QwenCloud 的参考文档构建，没有录制的

**来源。**QwenCloud 关于 [Wan 2.6 及更早版本文生视频](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task)、[首帧图生视频](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task)、[Wan 2.7 文生视频](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task)及[其查询结果](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result)、[Wan 2.7 图生视频](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task)、[Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task)、[异步任务](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks)、[Qwen-Image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image)及[其编辑](https://docs.qwencloud.com/api-reference/image-generation/qwen-image-editing)、[Wan 2.7 Image](https://docs.qwencloud.com/api-reference/image-generation/wan27-image-gen-edit/synchronous)、[Wan 2.6 Image](https://docs.qwencloud.com/api-reference/image-generation/wan26-image-gen-edit/synchronous)、[Z-Image](https://docs.qwencloud.com/api-reference/image-generation/z-image)、[Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api)及[其音色](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-audio-tts)、[Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts)及[其音色](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-tts)，以及 [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api) 的参考文档，2026-11-09 查阅。

## 视频的决定

- **模型决定请求体。**`dashscope::family` 读取模型名称：`wan3.0-video*`、`wan2.7-t2v*`、`wan2.7-i2v*`、`*-kf2v-*`（首尾帧）、`*-t2v*` 和 `*-i2v*`（Wan 2.6 及更早版本）。编辑或参考视频的模型，例如 `wan2.7-r2v`、`wan2.7-videoedit`、`wan2.1-vace-plus` 和 `wan2.2-animate-*`，会被拒绝，`param` 为 `model`。
- **只使用 QwenCloud 列出的尺寸。**Wan 2.6 及更早版本接收以 `width*height` 表示的像素 `size`。Wan 2.7 文生视频接收档位和比例，因此只有当 `size` 是其参考文档列出的十种尺寸之一时才会转换，例如 `1104x832` 是 `4:3` 的 `720P`。图生视频只接收档位，因为宽高比由图片决定，所以在那里 `size` 会被拒绝。
- **不支持 remix 和删除。**DashScope 只能取消尚未开始的任务，这不是删除，因此 Kaeso 只会忘掉被删除的视频。
- **时间为北京时间。**`submit_time` 和 `end_time` 为不带偏移量的 UTC+8；`dashscope::unix_of_beijing` 负责读取。视频链接有效期为 24 小时，因此已完成的任务在 `end_time` 之后 24 小时过期。

## 视频请求

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

## 视频回复

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`、`RUNNING`、`SUCCEEDED` | `queued`、`in_progress`、`completed` |
| `output.task_status` `FAILED`、`CANCELED`、`UNKNOWN` | `failed`，带 `output.code` 和 `output.message`，或以状态作为代码 |
| `output.video_url` | 视频的链接，`video/mp4` |
| `output.submit_time`、`output.end_time` | `created_at`、`completed_at` |
| `usage.output_video_duration`、`usage.SR`、`usage.ratio` | `seconds`、`resolution`、`aspect_ratio` |
| `request_id`、`output` 和 `usage` 的其他字段 | extras |

## 图片

`dashscope::images::image_family` 读取模型名称：`qwen-image-*`（Qwen-Image 2.x 和 3.0）、`wan2.7-image*`、`wan2.6-image*`、`z-image*`、`wan2.6-t2i*` 以及较旧的 `wan*-t2i*` 文生图模型。其他名称会被拒绝，`param` 为 `model`。图片和提示词放在同一条用户消息里，图片在前；较旧的文生图模型改用 `input.prompt`。

| KPP | Qwen-Image | Wan 2.7 Image | Wan 2.6 Image | Z-Image |
| --- | --- | --- | --- | --- |
| `prompt` | `{"text"}` 部分 | 相同 | 相同 | 相同 |
| `images` | `{"image"}` 部分，链接或 data URL；2.1 为 1 到 10 张，其他为 1 到 3 张 | 0 到 9 张 | 1 到 4 张，必填 | 拒绝 |
| `count` | `parameters.n`，最多 6 | `parameters.n` | `parameters.n`，最多 4；应用未填写时为 `1`，因为模型默认生成 4 张 | 不发送；大于 1 会被拒绝 |
| `size` | `parameters.size`，格式为 `width*height`；`auto` 时省略 | 相同 | 相同 | 相同 |
| `resolution` | 拒绝 | `parameters.size` 为 `1K`、`2K` 或 `4K` | `1K` 或 `2K` | 拒绝 |
| `negative_prompt` | `parameters.negative_prompt`，2.1 拒绝 | 拒绝 | `parameters.negative_prompt` | 拒绝 |
| `seed` | `parameters.seed` | 相同 | 相同 | 相同 |
| `user` | 省略，并在 `kaeso-left-out` 中注明 | 相同 | 相同 | 相同 |
| `mask`、`quality`、`output_format`、`output_compression`、`background`、`aspect_ratio`、文件 ID | 拒绝 | 拒绝 | 拒绝 | 拒绝 |

- **只有链接。**DashScope 返回有效期 24 小时的链接，因此编解码器拒绝 `b64_json`；网关会把它从请求中去掉，并为要求字节的应用下载图片（[图片 API](../../api/images.md#服务商)）。
- **扩展字段会转过去。**OpenAI 格式应用中的 `prompt_extend`、`prompt_extend_mode`、`enable_thinking` 和 `watermark` 会放进 `parameters`，与 QwenCloud 的 OpenAI 兼容模式文档一致。其他外来字段都会被拒绝。
- **默认无水印。**这些模型在 `watermark` 不为 `true` 时都不加水印，因此 Kaeso 不发送该字段。
- **完整回复。**图片在一个回复中返回；流式请求会以完成事件的形式收到它们。
- **文生图任务。**Wan 2.5 及更早的文生图模型（`wan2.5-t2i-preview`、`wan2.2-t2i-*`、`wan2.1-t2i-*`）只以任务方式应答：请求带 `X-DashScope-Async: enable` 发往 `/api/v1/services/aigc/text2image/image-synthesis`，提示词在 `input.prompt`，反向提示词在 `input.negative_prompt`。任务运行期间由 `dispatch::pending_task` 给出任务，网关一直询问 `GET /api/v1/tasks/{task_id}`，直到任务结束。`wan2.6-t2i` 在同一个请求中应答。两者都只接收提示词、最多 4 的 `n`（应用省略时为 `1`）和以像素表示的 `size`。

| DashScope 回复 | KPP |
| --- | --- |
| `output.choices[].message.content[].image` | 各张图片，按顺序 |
| 图片旁的 `text` 部分（Z-Image） | 每张图片的 `revised_prompt` |
| `usage.output_width` 和 `output_height`、`width` 和 `height` 或 `size` | `settings.size` |
| `usage.input_tokens`、`output_tokens` | `usage`，完整的 `usage` 会保留 |
| `request_id`、`output.finished`、`output.rewrite_status` 及 choices 的其他字段 | extras |
| 任务的 `output.results[].url` | 图片，按顺序；带 `code` 和 `message` 的结果单独失败，不生成图片 |
| `output.results[].actual_prompt` | 每张图片的 `revised_prompt` |
| `output.task_status` 为 `FAILED`，带 `output.code` 和 `output.message` | 按其代码给出的错误 |

## 语音合成

`dashscope::speech::speech_family` 读取模型名称：`qwen-audio-*-tts-*`（Qwen-Audio-TTS 3.0 和 3.1）和 `qwen3-tts-flash*`。实时模型只接受 WebSocket 连接，会被拒绝。

| KPP | Qwen-Audio-TTS | Qwen3-TTS Flash |
| --- | --- | --- |
| `input`、`voice` | `input.text`、`input.voice` | 相同 |
| `format` | `input.format`：`mp3`（默认）、`wav`、`opus`，`pcm16` 作为 24 kHz 的 `pcm` | `wav`（默认）或 `pcm16`；其他拒绝 |
| `sample_rate` | `input.sample_rate` | 仅 24,000 |
| `speed` | `input.rate`，0.5 到 2.0 | 拒绝 |
| `instructions` | `input.instruction` | 拒绝 |
| `language` | `input.language_hints` `[language]` | `input.language_type`，`zh` 对应 `Chinese`，另有九种 |
| `seed` | `input.seed` | 拒绝 |
| `bitrate` | `input.bit_rate`，仅限 `opus` | 拒绝 |
| `timestamps`、`duration`、对话、源音频 | 拒绝 | 拒绝 |

- **只要格式允许就使用流式。**Qwen-Audio-TTS 总是带 `X-DashScope-SSE: enable`，并以所请求格式的 base64 片段发送。Qwen3-TTS 只以 24 kHz 的 pcm 流式输出，因此 `pcm16` 使用流式，`wav` 则以文件链接完整回复。
- **音色来自列表。**命名音色必须是该模型的系统音色，或名为 `<model>-<suffix>` 的基础音色。显式音色 ID（例如克隆音色）原样发送。
- **不做转换。**模型不生成的格式会在发送前被拒绝。

| DashScope 回复 | KPP |
| --- | --- |
| 事件中的 `output.audio.data` | 音频的一个片段，标注为所请求的格式 |
| `output.finish_reason` 为 `stop` 的事件 | 结束，带 `usage`，`usage.characters` 作为计费字符数 |
| 完整回复中的 `output.audio.url` | 以链接形式提供的音频 |
| `error` 事件，或不在 2xx 范围内的 `:HTTP_STATUS/` 行 | 错误 |

## 转写

Qwen-Audio-ASR Flash（`qwen-audio-3.0-asr-flash`、`qwen-audio-3.1-asr-flash`）在同一个请求中回复。文件转写模型只回复异步任务，流式、实时和 message 模型只接受 WebSocket 连接，因此会被拒绝。

| KPP | Qwen-Audio-ASR |
| --- | --- |
| 音频 | `input.messages[].content[]` `{"type": "input_audio", "input_audio": {"data"}}`，链接或 data URL |
| 其媒体类型或文件名 | `parameters.format`：`wav`、`mp3` 或 `opus`；其他音频拒绝 |
| `prompt` | 音频之前的一条带 `input_text` 部分的用户消息 |
| `language` | `parameters.language_hints` `[language]` |
| `format` | `json` 和 `text`；带时间的格式会被拒绝 |
| `temperature`、`timestamp_granularities`、`diarize`、说话人、翻译 | 拒绝 |

回复中的 `output.text` 是转写文本，`usage.duration` 是 QwenCloud 计费的秒数；句子及其词保留在 extras 中。

## 错误

错误格式为 `{"request_id", "code", "message"}`：`InvalidApiKey` 是认证错误，`Throttling…` 是速率限制，`DataInspection…` 是内容过滤；其他代码按 HTTP 状态处理。
