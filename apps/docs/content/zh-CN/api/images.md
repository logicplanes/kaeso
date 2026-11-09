---
title: 图片 API
description: 通过 Kaeso 使用 OpenAI 的 Images API 生成和编辑图片，适用于所有能生成图片的服务商。
order: 2
---

应用通过 Kaeso，使用 OpenAI 的 Images API 和 OpenAI 官方 SDK 生成和编辑图片。Kaeso 按模型所属服务商自己的形式发送每个请求，回复以 OpenAI 的形式返回。

```bash tab="curl"
curl https://kaeso.example.com/v1/images/generations \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-image-2", "prompt": "A lighthouse at dusk, watercolor", "size": "1024x1024"}'
```

```python tab="Python"
import base64
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

image = client.images.generate(
    model="gpt-image-2",
    prompt="A lighthouse at dusk, watercolor",
    size="1024x1024",
)
with open("lighthouse.png", "wb") as f:
    f.write(base64.b64decode(image.data[0].b64_json))
```

```ts tab="TypeScript"
import { writeFile } from "node:fs/promises";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const image = await client.images.generate({
  model: "gpt-image-2",
  prompt: "A lighthouse at dusk, watercolor",
  size: "1024x1024",
});
await writeFile("lighthouse.png", Buffer.from(image.data[0].b64_json!, "base64"));
```

先把图片模型加入项目的目录（[模型和别名](../console/models-and-aliases.md)）。与其他端点一样，任何[模型名称](model-api.md#模型名称)或别名都可以使用。

## 端点

| 方法和路径 | 作用 |
| --- | --- |
| `POST /v1/images/generations` | 根据提示词生成图片（`client.images.generate`） |
| `POST /v1/images/edits` | 根据给定图片和提示词生成图片，可以是上传（`multipart/form-data`），也可以是带 `image_url` 或 `file_id` 组成的 `images` 的 JSON（`client.images.edit`） |
| `POST /v1/images/variations` | 对所有模型都返回 400：OpenAI 已停用变体，其他服务商也不提供。请改为把图片连同提示词发送到 `/v1/images/edits`。 |

错误使用 OpenAI 的错误格式。最大请求体（包括上传文件）为 `body_limits.images`，默认 64 MiB（[配置](../self-hosting/configuration.md)）。

## 编辑图片

上传要修改的图片；如果服务商接受遮罩，再上传一张遮罩，其透明区域标出要修改的部分：

```python tab="Python"
image = client.images.edit(
    model="gpt-image-2",
    image=[open("room.png", "rb")],
    mask=open("mask.png", "rb"),
    prompt="Add a reading lamp next to the sofa",
)
```

```ts tab="TypeScript"
import { createReadStream } from "node:fs";

const image = await client.images.edit({
  model: "gpt-image-2",
  image: [createReadStream("room.png")],
  mask: createReadStream("mask.png"),
  prompt: "Add a reading lamp next to the sofa",
});
```

Kaeso 把上传文件以上传形式发送给 OpenAI，对只接受 JSON 编辑的服务商则以带 data URL 的 JSON 发送。Kaeso 从不转换或缩放图片。

## 流式接收部分图片

设置 `stream: true` 后，OpenAI 会在绘制过程中发送部分图片，最多 `partial_images` 张（0 到 3），然后发送完成的图片：

```python
stream = client.images.generate(model="gpt-image-2", prompt="A red fox", stream=True, partial_images=2)
for event in stream:
    print(event.type)
```

不支持图片流式的服务商会收到完整请求。Kaeso 把它们的回复写成每张图片一个 `image_generation.completed` 事件，与 OpenAI 在 `partial_images` 为 0 时的做法相同。向这类服务商请求部分图片会得到 400，`param` 为 `partial_images`。

## 服务商

2026-11-08 查阅了每个内置服务商关于图片生成的文档。服务商以各自的形式提供下列模型。对模型所属服务商不接受的参数，Kaeso 会在发送任何内容之前拒绝，并在 `param` 中写明该参数。

| 服务商 | 生成 | 编辑 | 模型 | 来源 |
| --- | --- | --- | --- | --- |
| OpenAI | 支持，可流式 | 上传或 JSON，支持遮罩 | `gpt-image-2.6-sunburst`、`gpt-image-2.6-flare`、`gpt-image-2.5-sunburst`、`gpt-image-2.5-flare`、`gpt-image-2`、`gpt-image-1.5`、`gpt-image-1-mini`、`chatgpt-image-latest` | [Image generation](https://developers.openai.com/api/docs/guides/image-generation) |
| SpaceX | 支持 | 最多 5 张图片，不支持遮罩 | `grok-imagine-image-2.0`、`grok-imagine-image` | [Images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| Google AI Studio | 支持 | 提示词中的图片，不支持遮罩 | `gemini-nano-banana-2.1`、`gemini-3.1-flash-image`、`gemini-3.1-flash-lite-image`、`gemini-3-pro-image`、`gemini-2.5-flash-image` | [Image generation](https://aistudio.google.com/docs/image-generation) |
| Gemini Enterprise Agent Platform | 支持 | 提示词中的图片，不支持遮罩 | 相同的 Gemini 图片模型 | [Image generation](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/image-generation) |
| OpenRouter | 支持，OpenAI 的模型可流式 | 作为参考图片 | 其图片目录，例如 `openai/gpt-image-2` 和 `google/gemini-3.1-flash-image` | [Image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |
| Vercel | 支持 | 上传或 JSON，模型接受时支持遮罩 | 其图片模型，例如 `openai/gpt-image-2` 和 `bfl/flux-2-pro` | [Image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| Together AI | 支持 | 通过链接提供一张图片 | 其图片模型，例如 `black-forest-labs/FLUX.2-pro` | [Create image](https://docs.together.ai/reference/post-images-generations) |
| QwenCloud | 支持，`wan2.6-image` 除外 | Qwen-Image 2.1 最多 10 张，其他 Qwen-Image 模型 3 张，Wan 2.7 为 9 张，Wan 2.6 为 1 到 4 张，Z-Image 不支持；不支持遮罩 | OpenAI 兼容模式：`qwen-image-3.0-pro`、`qwen-image-3.0`、`qwen-image-2.1-pro`；DashScope：`qwen-image-2.1-turbo`、`qwen-image-2.0-pro`、`qwen-image-2.0`、`wan2.7-image-pro`、`wan2.7-image`、`wan2.6-image`、`z-image-turbo`、`wan2.6-t2i`、`wan2.5-t2i-preview`、`wan2.2-t2i-plus`、`wan2.2-t2i-flash`、`wan2.1-t2i-plus`、`wan2.1-t2i-turbo` | [OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai), [Qwen-Image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image), [Wan 2.7 Image](https://docs.qwencloud.com/api-reference/image-generation/wan27-image-gen-edit/synchronous), [Wan 2.6 Image](https://docs.qwencloud.com/api-reference/image-generation/wan26-image-gen-edit/synchronous), [Z-Image](https://docs.qwencloud.com/api-reference/image-generation/z-image), [Wan text-to-image](https://docs.qwencloud.com/api-reference/image-generation/wan-text-to-image-v2/synchronous), [Wan text-to-image tasks](https://docs.qwencloud.com/api-reference/image-generation/wan-text-to-image-v2/create-task) |
| Z.ai | 支持，每次请求一张图片 | 不支持 | `glm-image` | [Generate image](https://docs.z.ai/api-reference/image/generate-image) |
| MiniMax | 支持 | 不支持 | `image-01` | [Text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i) |
| Mistral | 不支持：图片只作为其智能体的工具提供 | 不支持 | — | [Image generation tool](https://docs.mistral.ai/studio/agents/agent-tools/image_generation) |
| Anthropic、DeepSeek、Groq、Cohere、Ollama Cloud、Moonshot、Cerebras | 服务商不提供 | — | — | 各服务商的 API 参考 |

Google 已停用 Imagen，因此 Kaeso 只调用 Gemini 图片模型。只有 Google 尺寸表中列出的尺寸才能传给它们，例如 `1024x1024`（`1K` 下的 `1:1`）。其他尺寸请发送 `aspect_ratio` 和 `resolution`。

SpaceX 用 `aspect_ratio` 和 `resolution` 代替 `size`。Z.ai 只返回链接，30 天后过期，因此在那里 `response_format: "b64_json"` 会得到 400。QwenCloud 也返回链接，有效期 24 小时。对于 `b64_json`，Kaeso 只通过 https 从 QwenCloud 的结果主机（`dashscope-*.aliyuncs.com`）下载这些链接，不带服务商密钥，不跟随重定向，并受服务商回复上限和请求超时的限制。Gemini 和 OpenRouter 只返回 base64，因此在它们那里 `response_format: "url"` 会得到 400。

QwenCloud 的 OpenAI 兼容模式只提供 Qwen-Image 3.0 和 2.1 Pro，因此它的其他图片模型发往它自己的 DashScope API，该 API 在 Wan 上接收 `resolution`（`1K`、`2K`、`4K`）。它的 2.6 之前的 Wan 文生图模型只以任务方式生成图片：Kaeso 启动任务，并在请求的超时内等待图片，因此应用在同一个请求中拿到图片。Qwen-Image 3.0 按 QwenCloud 报告的档位 `1K` 或 `2K` 计费；应用发送 `prompt_extend: true` 时，Z-Image 按每张图片两倍的价格计费，与 QwenCloud 的计费一致。除非应用发送 `watermark: true`，它的模型都不加水印。

[OpenAI Images 映射](../development/kpp/openai-images.md)列出了每个参数以及各服务商如何接收它；[DashScope](../development/kpp/dashscope.md) 列出了 QwenCloud 自己的 API。

## 费用和日志

当服务商报告图片 Token 时（如 OpenAI 和 Google），Kaeso 按 KPMR 中模型的图片价格计费。其余部分按 Token 价格计费。按图片或按百万像素计费的服务商使用 KPMR 中的按单位价格。

当请求没有指明价格所依据的尺寸、质量或分辨率时，按所列最低价格计费。那是服务商的默认档。KPMR 标记为估算的价格在模型详情中显示为估算。支出预算会在发送前为每个请求预留最贵的情况。

请求日志显示端点（例如 `images/edits`）、发送和生成的图片数量，以及尺寸和质量。日志从不保存图片字节：上传内容只记录其字段，每张图片只记录媒体类型和大小（[请求日志](../console/request-logs.md)）。

## Responses 中 OpenAI 的图片工具

Responses 应用可以给 OpenAI 模型提供 `image_generation` 工具。Kaeso 在应用和 OpenAI 之间原样传递该工具、其 `image_generation_call` 项及其流式事件。其他服务商无法运行 OpenAI 的工具，因此带该工具发送给其他服务商的请求会得到 400。

## 后续步骤

- [模型 API](model-api.md)：查阅每个端点、请求头和模型名称。
- [请求日志](../console/request-logs.md)：查找图片请求及其费用。
- [支持的服务商](../concepts/supported-providers.md)：了解 Kaeso 调用哪些服务商以及如何添加它们的密钥。
