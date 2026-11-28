---
title: 模型 API
description: 应用调用的端点、身份验证方式以及模型的指定方式。
order: 1
---

应用用 OpenAI 和 Anthropic 的官方 SDK 或 Google 的 Gen AI SDK 调用模型 API：把 SDK 指向您的实例，并传入一个 Kaeso API 密钥。Kaeso 会把每个请求转换为模型所属服务商的格式，再把回复转换回来，流式回复也不例外。

使用 OpenAI SDK 时，下面这个 Chat Completions 请求由 Claude 回答：

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const completion = await client.chat.completions.create({
  model: "claude-haiku-4-5",
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(completion.choices[0].message.content);
```

使用 Anthropic SDK 时，以 Messages 格式发送同一个请求：

```bash tab="curl"
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 200,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
for (const block of message.content) {
  if (block.type === "text") console.log(block.text);
}
```

使用 Google 的 Gen AI SDK（`google-genai`）时，以 Gemini 格式发送同一个请求：

```bash tab="curl"
curl https://kaeso.example.com/v1beta/models/claude-haiku-4-5:generateContent \
  -H "x-goog-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "contents": [
      {"role": "user", "parts": [{"text": "Write a haiku about gateways."}]}
    ]
  }'
```

```python tab="Python"
import os
from google import genai

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com"},
)

response = client.models.generate_content(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
print(response.text)
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com" },
});

const response = await ai.models.generateContent({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
console.log(response.text);
```

| SDK | 基础 URL | 密钥的发送方式 |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |
| Google Gen AI（`google-genai`） | `https://kaeso.example.com` | `x-goog-api-key` |

任何格式都能调用项目目录中的任何生成模型，因此 Gemini 应用可以调用 Claude，OpenAI 应用也可以调用 Gemini 模型。[调用模型](../get-started/calling-models.md)展示了流式传输以及每种 SDK 的更多用法。

## 端点

| 方法和路径 | 格式 |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI Chat Completions |
| `POST /v1/responses` | OpenAI Responses |
| `POST /v1/responses/input_tokens` | OpenAI Responses，统计输入 Token |
| `GET /v1/responses/{response_id}`、`DELETE /v1/responses/{response_id}` | OpenAI Responses，服务商为项目保留的回复（见[存储的回复](../console/organizations-and-projects.md#存储的回复)） |
| `POST /v1/embeddings` | OpenAI Embeddings |
| `POST /v1/rerank`, `POST /v2/rerank` | Cohere Rerank ([重排序 API](rerank.md)) |
| `POST /v1/images/generations`、`POST /v1/images/edits`、`POST /v1/images/variations` | OpenAI Images（[图片 API](images.md)） |
| `POST /v1/audio/speech` | OpenAI Speech（[音频 API](audio.md)） |
| `POST /v1/audio/transcriptions`、`POST /v1/audio/translations` | OpenAI Transcriptions（[音频 API](audio.md)） |
| `POST /v1/videos`、`GET /v1/videos`、`GET /v1/videos/{video_id}`、`DELETE /v1/videos/{video_id}`、`GET /v1/videos/{video_id}/content`、`POST /v1/videos/{video_id}/remix` | OpenAI Videos（[视频 API](videos.md)） |
| `POST /v1/files`、`GET /v1/files/{file_id}`、`DELETE /v1/files/{file_id}`、`GET /v1/files/{file_id}/content`、`POST /v1/batches`、`GET /v1/batches`、`GET /v1/batches/{batch_id}`、`POST /v1/batches/{batch_id}/cancel` | OpenAI Batch 和 Files（见[批处理 API](batches.md)） |
| `POST /v1/decisions` | OpenAI Decisions；带 Gemini 的密钥时为 Gemini Decisions（[决策 API](decisions.md)） |
| `POST /v1/systemone` | System One（[决策 API](decisions.md)） |
| `POST /v1/text-to-speech/{voice_id}`、`/v1/text-to-dialogue`、`/v1/sound-generation`、`/v1/music`、`/v1/voice-changer/{voice_id}`、`/v1/speech-to-text`、`/v1/flows/image`、`GET /v1/voices`、`/v2/voices` | ElevenLabs（[ElevenLabs API](elevenlabs.md)） |
| `POST /v1/messages` | Anthropic Messages |
| `POST /v1/messages/count_tokens` | Anthropic Messages，统计 Token |
| `GET /v1/models` | 该密钥可调用的模型（[列出模型](#列出模型)） |
| `GET /v1/models/{model}` | 其中某一个模型 |
| `POST /v1beta/models/{model}:generateContent` | Gemini |
| `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` | Gemini，流式 |
| `POST /v1beta/models/{model}:countTokens` | Gemini，统计 Token |
| `POST /v1beta/models/{model}:embedContent` | Gemini 嵌入 |
| `POST /v1beta/models/{model}:batchEmbedContents` | Gemini 嵌入，多个输入 |
| `GET /v1beta/models` | 该密钥可调用的模型，采用 Gemini 格式 |
| `GET /v1beta/models/{model}` | 其中某一个模型，采用 Gemini 格式 |
| `POST /v1beta/decisions` | Gemini Decisions（[决策 API](decisions.md)） |

与 Google 一样，Kaeso 也在 `/v1` 下提供 Gemini 的路径：`google-genai` 3 及以后的版本默认调用 `/v1`，更早的版本调用 `/v1beta`。`POST /v1/models/{model}:{method}` 始终是 Gemini。请求带有 Gemini 的密钥（`x-goog-api-key` 或 `?key=`）且没有 `Authorization` 请求头时，`GET /v1/models`、`GET /v1/models/{model}` 和 `POST /v1/decisions` 采用 Gemini 格式回答。在 Gemini 路径中，`{model}` 可以是任何[模型名称](#模型名称)，例如 `claude-haiku-4-5`。

`/v1/embeddings`、`:embedContent` 和 `:batchEmbedContents` 只服务嵌入模型，这类模型只有部分[服务商](../concepts/provider-apis.md)提供；嵌入模型也不服务其他端点。混用两者的请求，Kaeso 返回 400。同样，`/v1/decisions`、`/v1/systemone` 和 `/v1beta/decisions` 只服务决策模型，纯决策模型也不服务任何其他端点（[决策 API](decisions.md)）。`/v1/rerank` 只服务重排序模型，重排序模型也不服务任何其他端点（[重排序 API](rerank.md)）。转写模型只服务转写端点：`/v1/audio/transcriptions`、`/v1/audio/translations`，以及 ElevenLabs 的 `/v1/speech-to-text` 及其实时 WebSocket `/v1/speech-to-text/realtime`。当应用和 Kaeso 都使用 Chat Completions 或 Gemini 时，带音频的请求也能到达其中一些模型，限制见[音频 API](audio.md#转写和翻译)。

`/v1` 下的其他路径返回 404，已知路径使用了错误的方法则返回 405。请求带有 `anthropic-version` 请求头时，这两种错误采用 Anthropic 格式；带有 `xi-api-key` 请求头时采用 ElevenLabs 的格式；带有 Gemini 的密钥时采用 Gemini 格式；否则采用 OpenAI 格式。

在 `/v1beta` 下，以及 `/v1` 下的 Gemini 调用，所有错误都采用 Gemini 格式。其他方法（例如 `batchGenerateContent`）和其他路径（例如文件或缓存内容）返回 404。已知路径使用了错误的方法则返回 405。调用只接受查询参数 `key` 和 `alt`，其他参数返回 400。

### 列出模型

`GET /v1/models` 以同样的方式选择格式：带 `anthropic-version` 请求头时采用 Anthropic 的格式，带 `xi-api-key` 请求头时采用 ElevenLabs 的格式（[ElevenLabs API](elevenlabs.md#端点)），带 Gemini 的密钥且没有 `Authorization` 请求头时采用 Gemini 的格式。`GET /v1beta/models` 始终采用 Gemini 格式，并把每个模型命名为 `models/<name>`。列表先列出该密钥可调用的目录模型，再列出项目中指向这些模型的别名。每个模型以其自身的 ID 列出；如果其自身的 ID 不能指向它（例如另一个服务商提供同一 ID），则以 `provider/model` 列出（[模型名称](#模型名称)）。服务商不向项目的服务商密钥提供的模型不会列出（[可用性](../console/models-and-aliases.md#可用性)）。

| | OpenAI 格式 | Anthropic 格式 | Gemini 格式 |
| --- | --- | --- | --- |
| 分页 | 无，一次响应包含所有模型 | `limit`（1 到 1000，默认 20）、`after_id` 或 `before_id` | `pageSize`（默认 50），以及值为上一页 `nextPageToken` 的 `pageToken` |
| 已弃用的模型 | 不标注 | 按 KPMR 的标注带有 `lifecycle: "deprecated"` | 不标注 |
| 模型的描述 | `name`、`context_length`、`top_provider.max_completion_tokens`、`architecture`、`supported_parameters` 和 `pricing`，与 OpenRouter 列表的命名相同 | `display_name`、`max_input_tokens`、`max_tokens` | `displayName`、`inputTokenLimit`、`outputTokenLimit` |

在 OpenAI 格式中，每个模型用 [OpenRouter 模型列表](https://openrouter.ai/docs/api-reference/models/get-models)的字段描述自己。聊天应用和编程工具读取这些字段来显示模型名称、确定上下文大小，并区分聊天模型与图像或语音模型：

```json
{"id": "grok-4.7", "object": "model", "created": 1789948800, "owned_by": "xai", "shutdown_date": null,
 "name": "Grok 4.7", "context_length": 500000,
 "architecture": {"modality": "text+image+pdf->text", "input_modalities": ["text", "image", "pdf"], "output_modalities": ["text"]},
 "pricing": {"prompt": "0.000002", "completion": "0.000006", "input_cache_read": "0.0000005"},
 "top_provider": {"context_length": 500000, "max_completion_tokens": 500000},
 "supported_parameters": ["tools", "tool_choice", "reasoning", "reasoning_effort", "response_format", "structured_outputs", "temperature"]}
```

这些值来自项目的目录（包括覆盖设置）；别名显示其列出时所用模型的值。`pricing` 是 Kaeso 计算请求费用所用的基础价格，单位为每 Token 美元：`prompt`、`completion`、`input_cache_read`、`input_cache_write` 和 `internal_reasoning`。长提示词的价格以及按图像、秒或字符计的价格不会列出。`supported_parameters` 列出目录中该模型接受的请求字段：

- `tools` 和 `tool_choice`；
- 会推理的模型列出 `reasoning`，有推理强度等级时还列出 `reasoning_effort`；
- `response_format` 和 `structured_outputs`（按 JSON Schema 输出）；
- `temperature`。

不生成内容的模型以 OpenRouter 的命名（即其模型列表在 `?output_modalities=all` 时的写法）把它生成的东西列为输出模态：`embeddings`、`rerank` 或 `decisions`。目录中未知的字段会省略。

`GET /v1/models/{model}` 和 `GET /v1beta/models/{model}` 接受任何[模型名称](#模型名称)。模型不在目录中、密钥无权使用或服务商不提供该模型时，返回 404。

## 密钥

在任一端点上，通过以下任一请求头发送具有模型调用权限的 API 密钥：

```text
Authorization: Bearer sk-kso-…
x-api-key: sk-kso-…
x-goog-api-key: sk-kso-…
xi-api-key: sk-kso-…
```

请求发送了多个时，优先使用 `x-api-key`，其次是 `x-goog-api-key`，然后是 `xi-api-key`，最后是 `Authorization`。在 Gemini 的路径上（`/v1beta` 和 `/v1` 下），密钥也可以像 Google 的 REST 示例那样放在查询参数中，即 `?key=sk-kso-…`。只有在没有请求头携带密钥时，Kaeso 才使用它，而且查询参数中的密钥绝不会进入日志，也不会发给服务商。密钥决定所属项目。它以其成员或服务账号的权限和模型访问范围行事，该成员或服务账号需要 `api.request` 权限。

仅具有平台访问权限的密钥和组织平台密钥（`sk-kso-org-…`），Kaeso 以 403 拒绝：它们从不调用模型。如何创建和限制密钥，见 [API 密钥](../console/api-keys.md)。

## 模型名称

请求可以用三种形式指定模型。Kaeso 按以下顺序在项目目录中查找：

| 形式 | 示例 |
| --- | --- |
| 项目的别名 | `fast` |
| 服务商加模型 | `anthropic/claude-haiku-4-5` |
| 模型自身的 ID | `claude-haiku-4-5` |

目录之外的模型，以及密钥或其成员无权使用的模型，Kaeso 都返回 404。两者的消息相同，这样密钥无法得知项目还有哪些其他模型。如果项目中有多个服务商提供同一模型 ID，单独的 ID 会得到 400，消息会列出这些服务商；此时请改用 `provider/model`。目录的说明见[模型和别名](../console/models-and-aliases.md)。

## 请求体

Kaeso 读取以 `Content-Encoding: gzip`、`x-gzip` 或 `zstd` 发送的请求体，部分 SDK 会这样发送较大的请求体。Kaeso 先解压请求体，再检查和转换它，因此提供商收到的是未压缩的请求，保留内容的请求日志显示的也是解压后的请求体。请求体上限按解压后的大小计算，解压后超出上限的请求体返回 413（[请求体上限](../self-hosting/configuration.md#请求体与响应体)）。

一个请求体只能使用一种编码。其他编码（如 `br`）以及同时使用两种编码（如 `gzip, zstd`）都会以应用的格式返回 415。无法解压的请求体返回 400。

超出上限的请求体以应用的格式返回 413。Kaeso 在应答前会读完被拒绝请求的剩余请求体（例如密钥无效，或路径不接受该方法），最多 1 GiB，且不超过请求体读取期限 [`server.body_read_deadline`](../self-hosting/configuration.md#监听)。这样 Kaeso 前面的代理（如 Traefik）会转发这个应答，而不是自己返回 502。声明超过 1 GiB 的请求体不会被读取；发送 `Expect: 100-continue` 并等待的客户端会在发送请求体之前收到拒绝。

## 流式传输

按格式的约定请求流式回复，即 `"stream": true`。回复是采用应用所用格式的 SSE（服务器发送事件）流。

Gemini 应用则像 `google-genai` 那样调用 `:streamGenerateContent?alt=sse`。Kaeso 只以服务器发送事件的形式流式传输 Gemini 回复，因此不带 `alt=sse` 的流式请求返回 400。

- **保活。** 15 秒内没有输出时，Kaeso 会写入一条 SDK 会忽略的保活消息：OpenAI 格式为 SSE 注释，Anthropic 格式为 `ping` 事件，Gemini 格式为一个空行，因为 `google-genai` 会把其他每一行都当作 JSON 读取。`KAESO_STREAMS_KEEPALIVE_INTERVAL` 可更改间隔或关闭保活（[发往应用的流](../self-hosting/configuration.md#发往应用的流)）。
- **失败。** Kaeso 在服务商首次输出时，或第一个保活信号到期时开始流。服务商的流在此之前失败时，应用会收到带有其状态码（例如 429）的错误，与非流式请求相同。流一旦开始就无法再更改状态码：服务商之后出错，或在回复完成前结束流时，流会以采用应用所用格式的错误事件结束。
- **断开。** 应用在流的最后一个事件之前断开连接时，Kaeso 会立即停止向服务商发出的请求，并按已用量计费。最后一个事件已到达应用的流视为已完成，即使应用随即断开连接也是如此。

## 不记录内容

记录内容的项目可以用请求头 `kaeso-log-content: off` 让单个请求的内容不进入[请求日志](../console/request-logs.md)，例如包含客户支付信息的提示词。日志仍会保存该请求的元数据，例如模型、Token、费用、状态、耗时和密钥，但不保存应用发送或收到的内容，也不保存其媒体。该请求头只能关闭内容记录，不能开启，其他值返回 400。它不会让回复不进入[响应缓存](#响应缓存)；如需如此，请另加 `Cache-Control: no-store`。

## 超时

请求可以用请求头 `kaeso-timeout` 缩短 Kaeso 等待服务商的时间，单位为秒，取值 1 到 3600，例如最多等待 20 秒的应用可发送 `kaeso-timeout: 20`。它会把等待第一个字节的时间（以及非流式回复等待整个回复的时间）降到实例和模型的设置以下，但从不延长；流的空闲超时保持不变。在[别名路由](../concepts/alias-routes.md)后面，它适用于请求到达的每个步骤：超时的步骤以 `unavailable` 和 504 失败，因此在 `unavailable` 时回退的路由会继续并及时作答。其他值返回 400。SDK 自身的 `timeout` 则会直接关闭连接，应用因此永远收不到回退的回答。

应用可以用请求头 `kaeso-tags` 为请求加标签，取值为逗号分隔的列表，例如 `kaeso-tags: checkout,eval-42`，以便在密钥之下区分流量：区分不同功能，或区分评估运行与生产流量。一个请求最多带 10 个标签，每个标签由 1 到 64 个小写字母、数字、`.`、`_`、`:` 和 `-` 组成；其他取值会以应用的格式返回状态 400 并指明该请求头，因此标签绝不会被悄悄丢弃。[请求日志](../console/request-logs.md)会把标签与请求元数据一同保存，其 `tag` 筛选可找出带某个标签的请求，导出中也包含标签。有权查看日志元数据的人都能看到标签，因此不要在其中放入个人数据。

## 跨工具调用的推理

Chat Completions 没有存放模型推理内容的字段，Gemini 格式也没有存放其他服务商推理内容的字段。Chat Completions 或 Gemini 应用调用通过其他格式提供服务的模型时，Kaeso 会保存模型在每次工具调用之前的推理内容。在应用的下一轮请求中，Kaeso 会将这些推理内容加回（[格式转换](../concepts/format-translation.md#chat-completions)）。Gemini 应用调用 Gemini 模型时，Gemini 自己的思考签名原样往返。

## 响应缓存

项目可以用已存储的回复应答重复的相同请求（[响应缓存](../console/response-cache.md)）。发送 `Cache-Control: no-cache` 可重新询问服务商并存储新回复，发送 `Cache-Control: no-store` 可让该请求跳过缓存，发送 `Cache-Control: max-age=60` 只接受最近一分钟内存储的回复，发送 `Cache-Control: only-if-cached` 则从缓存应答或返回 504，从不调用服务商。

## 响应头

| 响应头 | 值 |
| --- | --- |
| `kaeso-request-id` | 请求的 ID，`req_…`，每个响应都有。请求日志显示同一 ID |
| `kaeso-upstream-request-id` | 服务商为其响应给出的 ID（其 `request-id`、`x-request-id`，或 Cloudflare AI Gateway 的 `cf-aig-log-id`），供其支持团队使用。出现在回复和服务商已作答的错误中，取自作答的路由步骤；命中响应缓存、尚无服务商作答或服务商未发送时没有此头。请求日志将其显示为**上游请求** |
| `kaeso-cost` | 非流式回复的费用，与其请求日志条目将记录的相同：以美元计的金额，命中响应缓存时为 `0`，模型对其产出没有价格时为 `unknown`。仅当密钥所有者在项目中拥有 `costs.view` 时才有；视频任务或图像生成的开始请求没有此头，它们在结束时计费 |
| `kaeso-provider` | 处理该请求的服务商，例如 `anthropic`；确定模型后即带有，错误响应也包括在内 |
| `kaeso-model` | 服务商的模型 ID；确定模型后即带有，错误响应也包括在内 |
| `kaeso-left-out` | Kaeso 因模型没有对应项而省略的内容，按请求中的名称列出，例如 `tools[1], text.verbosity`、只接受默认温度的模型的 `temperature`，若该模型是 Claude（例如 Claude Opus 4.7 及更新版本），还有 `top_p` 和 `top_k`，或系统消息推理强度的 `messages[4].output_config.effort`；格式没有惩罚参数的模型的值为 `0` 的 `frequency_penalty`；Chat Completions 消息的参与者名称为 `messages[1].name`；通过 OpenAI Responses 提供的模型的 `seed`；没有 top-k 的模型的 `top_k` 或 Gemini 应用的 `generationConfig.topK`；对话中的项按类型和 ID 列出，例如 `web_search_call ws_67c9…`。只出现在省略了内容的响应上（[省略的工具](../development/kpp/cross-format.md#省略的工具)、[没有推理强度等级的模型](../development/kpp/reasoning.md#没有推理强度等级的模型)、[中性值](../development/kpp/sampling.md#中性值)、[top K](../development/kpp/sampling.md#top-k)） |
| `kaeso-adapted` | 因模型不接受参数 schema 根部的联合、Kaeso 把参数 schema 合并为一个对象的工具，按它们在请求中的路径列出，例如 `tools[0].tools[3]`。模型会在工具描述中看到各个变体，它的调用原样到达应用。还包括对话中 Kaeso 为只接受对象的模型把参数变为对象的工具调用，以 `tool call` 加调用 ID 列出，例如 `tool call call_abc`。还包括 Kaeso 为 Chat Completions 模型把图像和文件移到下一条用户消息中的工具结果，以 `tool result` 加调用 ID 列出，例如 `tool result call_abc`。还包括 Kaeso 以 SHA-256 哈希发送的 Anthropic 应用的 `metadata.user_id`，因为服务商接受的最终用户 ID 更短，例如 OpenAI 的 128 个字符。还包括来自其他格式的请求中高于模型自身上限、被 Kaeso 降到模型最大值的输出上限，以应用中的字段名列出，例如 `max_tokens`。还包括发往 ChatGPT 套餐的请求的输出上限：该路由拒绝这个字段，因此 Kaeso 在估算的输出达到上限时自行结束回复，并使用该格式对应的停止原因（例如 `max_tokens`），以应用中的字段名列出。还包括其他格式发往通过 OpenAI Responses 提供的模型的请求中的停止序列和低于 16 个 Token 的输出上限：Kaeso 在第一个停止序列处或上限处自行结束回复，以应用中的字段名列出，例如 `stop`（[网关中的 OpenAI Responses](../development/kpp/sampling.md#网关中的-openai-responses)）。还包括对始终推理的模型关闭的思考，改发该模型的最低推理强度等级，以应用中的字段名列出，例如 `thinking`。还包括模型未列出、被改为其所列最近等级的推理强度，以应用中的字段名列出，例如 `output_config.effort`，以及对不支持自适应思考的 Claude 模型使用、被改为带预算的 `enabled` 思考的自适应思考，列为 `thinking`；还包括发给拒绝预算的 Claude Opus 4.7 及更新版本、被改为采用预算对应推理强度的自适应思考的思考预算，列为 `thinking`，Gemini 应用则列为 `generationConfig.thinkingConfig`。还包括 Anthropic 应用系统消息中的 `tool_addition` 和 `tool_removal` 块，Kaeso 把它们的工具变更应用到了其他格式的模型所收到的工具列表上，以块的路径列出，例如 `messages[4].blocks[1]`。还包括参数 schema 中含有 Kaeso 为编译 pattern 的服务商改写过的 `pattern` 的工具，以其路径列出，例如 `tools[2]`。还包括发往没有 JSON 模式的格式（例如 Claude）的模型的 JSON 模式，Kaeso 通过系统提示词末尾的一句话来要求 JSON，以应用中的字段名列出，例如 `response_format`。还包括 Kaeso 改写为 Claude 所接受子集的输出 schema，或为 Claude 或 Gemini 把描述或 `strict: false` 纳入 schema 的输出 schema，以应用中的字段名列出，例如 `response_format`；以及 Kaeso 为 Claude 改写了 schema 的严格工具，以其路径列出（[来自其他方言的请求](../development/kpp/anthropic-messages.md#来自其他方言的请求)）。只出现在调整了工具、调用、结果、工具变更、最终用户 ID、输出上限、输出格式或推理强度的响应上（[结构化输出](../development/kpp/cross-format.md#结构化输出)、[始终推理的模型](../development/kpp/reasoning.md#始终推理的模型)、[联合类型的工具 schema](../development/kpp/cross-format.md#联合类型的工具-schema)、[工具调用参数](../development/kpp/cross-format.md#工具调用参数)、[工具结果中的媒体](../development/kpp/media.md#工具结果中的媒体)、[内联工具](../development/kpp/cross-format.md#内联工具)、[服务商配置](../development/kpp/openai-chat-providers.md#服务商配置)） |
| `kaeso-route` | 请求在别名路由中到达的每个步骤，按顺序以结构化字段列表（[RFC 9651](https://www.rfc-editor.org/rfc/rfc9651)）给出：步骤的路径、以 `provider/model` 表示的 `model`、`status`（提供商返回的状态，或 Kaeso 为自身失败给出的状态：无法连接提供商或提供商拒绝 Kaeso 的密钥时为 502，未及时应答时为 504），以及失败时的耗时 `dur`（毫秒）。响应本身在应用的格式中可能带有不同的状态，例如 Anthropic 的 529 发给 OpenAI 应用时为 503。密钥无权使用的步骤带 `skipped`，不带模型；因刚刚失败而被跳过的步骤带 `skipped` 和它的模型。步骤多次发送请求时，`tries` 给出发送次数，其中包括重试和尝试过的项目在该服务商下的密钥。例如 `"route";model="anthropic/claude-sonnet-5-5";status=529;dur=1204, "route.fallback";model="openai/gpt-5";status=200`。只出现在别名请求的响应中（[别名路由](../concepts/alias-routes.md#应用和日志看到的内容)） |
| `kaeso-limits` | 每项适用的速率限制和硬性预算还剩多少，采用结构化字段列表：限额的层级、类型和周期，例如 `"key.requests.minute"`，模型自身的限额带 `model`，另有 `limit`、`remaining` 和以秒计的 `reset`（总预算没有）。花费为美元字符串。`remaining` 为本次请求之前的值，只有请求速率已计入本次请求。仅当密钥所有者有权查看项目限额（`limits.view`）时返回，花费还需 `costs.view`；Kaeso 返回的 429 带有拒绝它的限额，`remaining=0`（[限额和预算](../console/limits-and-budgets.md#应用看到的内容)） |
| `kaeso-cache` | [响应缓存](../console/response-cache.md)应答时为 `hit`，请求发往服务商时为 `miss`，请求无法缓存或应用发送了 `Cache-Control: no-store` 时为 `bypass`。只出现在开启缓存的项目中 |
| `age` | 缓存的回复存储以来的秒数。只出现在 `kaeso-cache: hit` 时 |
| `kaeso-guardrails` | 检查过请求或回复的每条[防护规则](../console/guardrails.md#应用看到的内容)各占一项：其名称、`phase`、`action`、`category`、`failure` 以及以毫秒计的 `dur`。只出现在请求经过防护规则的项目中 |
| `Deprecation`、`Sunset`、`Link` | 弃用日期、停用日期，以及指向替代模型和服务商公告的链接。只出现在服务商即将停用的模型上（[模型生命周期](../concepts/model-lifecycle.md#响应中带有日期)） |
| `server-timing` | `upload;dur=…`：应用发送请求体所用的毫秒数。随后是 `kaeso;dur=…`：Kaeso 调用服务商之前自身花费的毫秒数，不含上传，也不含别名中失败步骤等待服务商的时间；请求未到达服务商时，则是到 Kaeso 作出响应为止的时间。最后是 `upstream;dur=…`：服务商返回第一个字节所用的时间，得知后即带有。流式回复随响应头一起发送它，因此 Kaeso 转换流所花的时间只记录在[请求日志](../console/request-logs.md#查看请求)中 |

## 从浏览器调用

只有当另一个源以 CORS 响应头应答时，浏览器才允许网页调用它。在 `KAESO_CORS_ALLOWED_ORIGINS` 列出您的 Web 应用的源（例如 `https://app.example.com,http://localhost:5173`）之前，Kaeso 不发送任何 CORS 响应头（[浏览器应用](../self-hosting/configuration.md#浏览器应用)）。列出之后，以下规则仅适用于 `/v1`、`/v1beta` 和 `/v2`：

- **精确匹配源。** 协议、主机和端口都必须一致：`https://app.example.com` 既不放行 `http://app.example.com`，也不放行 `https://www.app.example.com`。Kaeso 会在启动时拒绝 `*`，因为它会让任何网站都能用访问者持有的密钥发送请求。
- **预检请求。** 来自已列出源、带有 `Origin` 和 `Access-Control-Request-Method` 的 `OPTIONS` 请求无需密钥即返回 204。响应带有值为该源的 `Access-Control-Allow-Origin`、`Access-Control-Allow-Methods: GET, POST, DELETE` 和 `Access-Control-Max-Age: 7200`。`Access-Control-Allow-Headers` 列出所请求的请求头里官方 SDK 会发送的那些：`authorization`、`x-api-key`、`content-type`、`accept`、`user-agent`、`anthropic-version`、`anthropic-beta`、`anthropic-dangerous-direct-browser-access`、`openai-organization`、`openai-project`、`openai-beta`、`idempotency-key`、`x-goog-api-key`、`x-goog-api-client`、`xi-api-key` 和 `x-stainless-…`，以及[响应缓存](#响应缓存)读取的 `cache-control` 、`kaeso-timeout`（[超时](#超时)）、`kaeso-log-content`（[不记录内容](#不记录内容)）、`kaeso-tags`、`traceparent` 和 `tracestate`（[追踪](../self-hosting/monitoring.md#追踪)），另加 `KAESO_CORS_ALLOWED_HEADERS` 中的名称（[配置](../self-hosting/configuration.md#浏览器应用)）。预检请求不是模型请求：不记录日志、不计费，也不计入限额。
- **响应。** 发往已列出源的每个响应（包括错误响应）都带有值为该源的 `Access-Control-Allow-Origin`，以及 `Access-Control-Expose-Headers: kaeso-request-id, kaeso-upstream-request-id, kaeso-cost, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, kaeso-route, kaeso-limits, kaeso-cache, kaeso-guardrails, server-timing, age, retry-after, retry-after-ms, x-should-retry`，以便应用读取这些响应头。
- **其他源**得不到 CORS 响应头，因此浏览器会拦截调用，它们的预检请求得到空的 204。`/v1`、`/v1beta` 和 `/v2` 上的每个响应都带有 `Vary: Origin`，以便缓存区分这些响应。
- **不使用 Cookie。** 由于密钥通过请求头传递，Kaeso 从不发送 `Access-Control-Allow-Credentials`。Platform API、健康检查和指标从不响应跨源请求。

OpenAI 和 Anthropic 的 SDK 都只有在应用传入 `dangerouslyAllowBrowser: true` 时才会在浏览器中运行。

> [!WARNING]
> 使用页面的每个人都能读取页面中的密钥。请为浏览器应用单独创建密钥：仅具有模型调用权限、有效期短、只开放所需模型、限额从严。或者由您自己的后端发放短期密钥（[API 密钥](../console/api-keys.md)）。

## Kaeso 不做的事

- **重试。** 只有项目或别名步骤要求时，Kaeso 才会在连接中断、返回 5xx 或服务商过载后，且在任何回复到达应用之前，把请求重新发送给模型（[别名路由](../concepts/alias-routes.md#回退)）。否则每个请求只发送一次；SDK 会在错误允许时自行重试。
- **存储状态。** Kaeso 自己不保存对话，唯一的例外是模型在工具调用之前写下的内容。对于由另一种格式的模型服务的 Chat Completions 或 Gemini 应用，这是推理内容；对于保持 `store` 开启的 OpenAI Responses 应用，这是推理内容，以及在项目允许在服务商处存储回复时调用之前紧接着的文本。网关把它们保存在 Valkey 中，时长为 `continuity.reasoning_ttl`，默认一小时，用实例的主密钥加密，并且只返还给使用同一 API 密钥的请求。设为 `0s` 即全部关闭；关闭项目设置**允许在服务商处存储回复**后，网关不再保留文本（[格式转换](../concepts/format-translation.md#保持-store-开启的-responses-应用)）。如果防护规则会在回复中恢复被遮盖的值，保留的文本就含有这些服务商从未见过的值。只有在项目允许在服务商处存储回复、且服务商会保留该模型回复时，`store: true` 和 `previous_response_id` 才可用（见[存储的回复](../console/organizations-and-projects.md#存储的回复)）；其他情况下 Kaeso 会拒绝它们，`conversation` 和 `background: true` 则一律拒绝。否则请每次发送完整对话。
- **悄然更改。** 无法传递给模型服务商的字段，Kaeso 会拒绝并在错误中指出该字段，绝不会悄悄丢弃。在 Chat Completions、Messages 和 Embeddings 中，名称以 `header:` 开头或包含 `.` 的字段会得到 400，因为 Kaeso 无法传递这样的名称；Responses 则会把这类字段保留在请求正文中。格式之间能传递哪些内容，见[格式转换](../concepts/format-translation.md)。
