---
title: 视频 API
description: 通过 Kaeso 使用 OpenAI 的 Videos API 生成视频，适用于每个提供视频生成的服务商。
order: 2
---

应用通过 Kaeso 使用 OpenAI 的 Videos API 和 OpenAI 官方 SDK 生成视频。Kaeso 以模型所属服务商自己的格式在该服务商处启动任务，用自己的 ID 跟踪它，并以 OpenAI 的格式回答。

```bash tab="curl"
curl https://kaeso.example.com/v1/videos \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "grok-imagine-video-1.5", "prompt": "A lighthouse at dusk, waves rolling in", "seconds": "8", "size": "1280x720"}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

video = client.videos.create_and_poll(
    model="grok-imagine-video-1.5",
    prompt="A lighthouse at dusk, waves rolling in",
    seconds="8",
    size="1280x720",
)
if video.status == "completed":
    client.videos.download_content(video.id).write_to_file("lighthouse.mp4")
```

```ts tab="TypeScript"
import { writeFile } from "node:fs/promises";
import { setTimeout } from "node:timers/promises";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

let video = await client.videos.create({
  model: "grok-imagine-video-1.5",
  prompt: "A lighthouse at dusk, waves rolling in",
  seconds: "8",
  size: "1280x720",
});
while (video.status === "queued" || video.status === "in_progress") {
  await setTimeout(5000);
  video = await client.videos.retrieve(video.id);
}
const file = await client.videos.downloadContent(video.id);
await writeFile("lighthouse.mp4", Buffer.from(await file.arrayBuffer()));
```

先把视频模型添加到项目的目录中（[模型和别名](../console/models-and-aliases.md)）。与其他所有端点一样，任何[模型名称](model-api.md#模型名称)或别名都可以使用。

## 端点

| 方法和路径 | 作用 |
| --- | --- |
| `POST /v1/videos` | 根据提示词启动一个视频，以 JSON 或上传（`multipart/form-data`）方式发送，上传时 `input_reference` 图片作为第一帧（`client.videos.create`） |
| `GET /v1/videos/{video_id}` | 向服务商询问进展后返回视频的任务（`client.videos.retrieve`、`client.videos.poll`） |
| `GET /v1/videos/{video_id}/content` | 完成的文件，`variant` 为 `video`（默认）、`thumbnail` 或 `spritesheet`（`client.videos.download_content`） |
| `POST /v1/videos/{video_id}/remix` | 根据一个已完成的视频和新的提示词启动一个新视频（`client.videos.remix`） |
| `GET /v1/videos` | 项目的视频，支持 `after`、`limit`（最多 100，默认 20）和 `order`（默认 `desc`）（`client.videos.list`） |
| `DELETE /v1/videos/{video_id}` | 删除视频（`client.videos.delete`） |
| `POST /v1/videos/edits`、`/v1/videos/extensions`、`/v1/videos/characters` | 返回 404：这些端点属于 Sora。请改为 remix 一个视频或创建新视频。 |

错误使用 OpenAI 的错误格式。最大请求体（包括上传）为 `body_limits.model_api`，默认 32 MiB（[配置](../self-hosting/configuration.md#请求体与响应体)）。

Kaeso 读取 OpenAI 的字段：`model`、`prompt`、`seconds`、`size` 和 `input_reference`。只有当服务商同时列出分辨率和宽高比时，`size` 才会到达接收这两者的服务商，例如 `1280x720` 是 `16:9` 的 `720p`。其他任何字段只会到达采用 OpenAI 格式的 Together AI；其他服务商都会返回 400，并在 `param` 中给出该字段。

## 任务如何运行

创建视频会立即返回其任务，状态为 `queued` 或 `in_progress`，ID 形如 `video_06a1…`。该 ID 属于 Kaeso，从不显示服务商的 ID。

任务运行期间，每次获取都会向服务商询问进展并保存其回答。服务商无法回答时，获取会返回 Kaeso 最后一次看到的任务状态，请稍后再问。已结束的任务不会再被询问。SDK 的 `poll` 和 `create_and_poll` 会一直获取，直到任务结束。

列表按 Kaeso 最后一次看到的状态显示每个视频，不询问服务商。视频属于启动它的密钥所在的项目；其他项目的密钥会得到 404，与不存在的 ID 得到的回答相同。启动视频的服务商密钥被替换后，Kaeso 无法再访问该任务，并返回 409。

会询问服务商的获取、下载、删除和 remix 与任何模型请求一样计入密钥的[请求速率](../console/limits-and-budgets.md)，因为每一次都用组织的密钥消耗服务商的配额；获取已结束的任务时由 Kaeso 的记录回答，不计入。

## 下载视频

Kaeso 从服务商存放文件的位置流式传输文件，不做存储，并透传 `Range`。Veo 和 Gemini Omni 在回答中直接发送文件；Kaeso 每次下载都会重新读取，上限为 `body_limits.provider_response`（默认 128 MiB）。 位于服务商自身 API 以外主机上的链接只从公网地址获取，无论 [`custom_providers.allowed_networks`](../self-hosting/configuration.md) 开放了什么，且从不携带服务商密钥。

服务商只在有限时间内保存文件。对于发放短期链接的服务商，例如 MiniMax 和 ElevenLabs，Kaeso 每次下载都会获取新链接。超过任务的 `expires_at` 后，下载返回 404。只有 Z.ai 提供 `thumbnail`，没有服务商提供 `spritesheet`；请求它们会返回 404，`param` 为 `variant`。

删除视频会在能够删除的服务商处删除它，即 Google AI Studio 和 MiniMax 的 H3 API；在其他服务商处，Kaeso 只会忘掉它。

## Remix 视频

Remix 根据一个已完成的视频和新的提示词，在同一服务商处用同一模型启动一个新视频：

```python
remix = client.videos.remix(video.id, prompt="The same lighthouse in a snowstorm")
```

SpaceX 通过其编辑 API 进行 remix，Gemini Omni 延续先前的交互。其他服务商都会返回 400，`param` 为 `video_id`。

## 服务商

2026-11-09 查阅了每个内置服务商关于视频生成的文档。模型所属服务商不接收的字段，Kaeso 会在发送前拒绝，并在 `param` 中给出该字段。

| 服务商 | 模型 | 图片 | Remix | 来源 |
| --- | --- | --- | --- | --- |
| SpaceX | `grok-imagine-video-1.5`、`grok-imagine-video-1.5-lite`、`grok-imagine-video` | 第一帧，上传或链接 | 是 | [Videos](https://docs.x.ai/developers/rest-api-reference/inference/videos) |
| Google AI Studio | `gemini-omni-1.2-flash`、`gemini-omni-1.1-flash` | 第一帧，上传或链接 | 是 | [Gemini Omni](https://aistudio.google.com/docs/omni) |
| Gemini Enterprise Agent Platform | `veo-3.1-generate-001`、`veo-3.1-fast-generate-001`、`veo-3.1-lite-generate-001`，需服务账号密钥 | 第一帧，上传 | 否 | [Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate) |
| Gemini Enterprise Agent Platform | `gemini-omni-1.2-flash`、`gemini-omni-1.1-flash-preview`、`gemini-omni-flash-preview`，需服务账号密钥 | 第一帧，上传或 `gs://` 链接，`gemini-omni-flash-preview` 除外 | 是 | [根据文本生成视频](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text) |
| MiniMax | `MiniMax-Hailuo-2.3`、`MiniMax-Hailuo-2.3-Fast`、`MiniMax-Hailuo-02`、`MiniMax-H3` 及其变体 | 第一帧，上传或链接 | 否 | [视频生成](https://platform.minimax.io/docs/api-reference/video-generation-t2v) |
| Z.ai | `cogvideox-3` | 第一帧，上传或链接 | 否 | [生成视频](https://docs.z.ai/api-reference/video/generate-video) |
| OpenRouter | 其视频模型，例如 `google/veo-3.1` 和 `alibaba/wan-3.0` | 第一帧，上传或链接 | 否 | [视频生成](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request) |
| Together AI | 其视频模型，例如 `ByteDance/Seedance-2.0` 和 `Wan-AI/wan2.7-t2v` | 第一帧，仅限链接 | 否 | [创建视频](https://docs.together.ai/reference/create-videos) |
| ElevenLabs | `veo-3.1-generate-001`、`veo-3.1-fast-generate-001`、`bytedance-seedance-v2` 及其变体 | 第一帧，上传 | 否 | [OpenAPI 文档](https://api.elevenlabs.io/openapi.json) |
| OpenAI | 否：OpenAI 于 2026 年 9 月 24 日关闭了 Sora，因此 `sora-*` 模型返回 400 | — | — | [弃用](https://platform.openai.com/docs/deprecations) |
| Vercel | 否：Vercel 只通过其 AI SDK 记录视频功能，而不是通过 HTTP | — | — | [视频生成](https://vercel.com/docs/ai-gateway/modalities/video-generation) |
| QwenCloud | Wan 文生视频和图生视频，例如 `wan3.0-video`、`wan2.7-t2v` 和 `wan2.6-i2v-flash` | 第一帧，上传或链接，适用于图生视频模型和 Wan 3.0 | 否 | [视频生成](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task) |
| Anthropic、DeepSeek、Groq、Mistral、Cohere、Ollama Cloud、Moonshot、Cerebras | 服务商不提供 | — | — | 各服务商的 API 参考 |

当 KPMR 或项目自己的条目表明某个模型生成视频时，项目会把它列为视频模型。Gemini Enterprise Agent Platform 上的 Veo 需要服务账号密钥：快捷模式不列出任何 Veo 模型，因此 Kaeso 会拒绝用于这些模型的快捷模式密钥。Agent Platform 上的 Gemini Omni 同样需要服务账号密钥，因为快捷模式不列出任何 Omni 模型；无论密钥指定哪个位置，它都发往平台的全局位置，因为 Omni 只在那里提供。

[OpenAI Videos 映射](../development/kpp/openai-videos.md)及其旁边的各服务商页面列出了每个字段以及每个服务商如何接收它。

## 费用和日志

启动视频不产生费用。当第一个请求看到视频完成时，Kaeso 按 KPMR 中该模型的价格计费一次：按秒或按视频计价，价格不同时按分辨率、声音和时长区分。Gemini Omni 按其 Token 计费。失败的视频不产生费用，KPMR 中没有价格的模型记录日志时不带费用。Kaeso 会把 Together AI 的 `outputs.cost` 保存在任务中，但不按它计费：Together 的参考文档把它写成不带单位的整数。

支出预算在视频启动前检查其最贵的情况，视频结束后才计入其费用。请求日志把启动显示为 `videos` 或 `videos/remix`，把完成的视频显示为 `videos/completed`，带有片段数、秒数和分辨率。日志从不保存视频或图片字节（[请求日志](../console/request-logs.md)）。

## 后续步骤

- [模型 API](model-api.md)：查阅每个端点、请求头和模型名称。
- [请求日志](../console/request-logs.md)：查找视频及其费用。
- [支持的服务商](../concepts/supported-providers.md)：了解 Kaeso 调用哪些服务商以及如何添加它们的密钥。
