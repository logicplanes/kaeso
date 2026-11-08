---
title: Images API
description: Generate and edit images through Kaeso with OpenAI's Images API, on every provider that makes images.
order: 2
---

Apps generate and edit images through Kaeso with OpenAI's Images API and the official OpenAI SDKs. Kaeso sends each request to the model's provider in that provider's own form, and the reply comes back in OpenAI's.

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

Add the image model to the project's catalogue first ([models and aliases](../console/models-and-aliases.md)). Any [model name](model-api.md#model-names) or alias works, as for every other endpoint.

## Endpoints

| Method and path | What it does |
| --- | --- |
| `POST /v1/images/generations` | Makes images from a prompt (`client.images.generate`) |
| `POST /v1/images/edits` | Makes images from given images and a prompt, as an upload (`multipart/form-data`) or as JSON with `images` of `image_url` or `file_id` (`client.images.edit`) |
| `POST /v1/images/variations` | Answers 400 for every model: OpenAI retired variations and no other provider offers them. Send the image to `/v1/images/edits` with a prompt instead. |

Errors use OpenAI's error format. The largest request body, uploads included, is `body_limits.images`, 64 MiB by default ([configuration](../self-hosting/configuration.md)).

## Edit an image

Upload the images to change, and a mask whose transparent areas mark what to change, where the provider takes one:

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

Kaeso sends an upload to OpenAI as an upload, and as JSON with data URLs to providers that take edits only as JSON. It never converts or resizes an image.

## Stream partial images

With `stream: true`, OpenAI sends partial images while it draws, up to `partial_images` (0 to 3), then the finished image:

```python
stream = client.images.generate(model="gpt-image-2", prompt="A red fox", stream=True, partial_images=2)
for event in stream:
    print(event.type)
```

Providers that don't stream images get the request whole, and Kaeso writes their reply as one `image_generation.completed` event per image, as OpenAI does with `partial_images` 0. Asking those providers for partial images answers 400 with `param` `partial_images`.

## Providers

Every built-in provider's documentation was read for image generation on 2026-11-08. Providers serve the models below in their own form; Kaeso refuses a parameter the model's provider doesn't take before anything is sent, with the parameter in `param`.

| Provider | Generations | Edits | Models | Source |
| --- | --- | --- | --- | --- |
| OpenAI | Yes, streamed | Uploads or JSON, with a mask | `gpt-image-2.6-sunburst`, `gpt-image-2.6-flare`, `gpt-image-2.5-sunburst`, `gpt-image-2.5-flare`, `gpt-image-2`, `gpt-image-1.5`, `gpt-image-1-mini`, `chatgpt-image-latest` | [Image generation](https://developers.openai.com/api/docs/guides/image-generation) |
| SpaceX | Yes | Up to 5 images, no mask | `grok-imagine-image-2.0`, `grok-imagine-image` | [Images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| Google AI Studio | Yes | Images in the prompt, no mask | `gemini-nano-banana-2.1`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite-image`, `gemini-3-pro-image`, `gemini-2.5-flash-image` | [Image generation](https://aistudio.google.com/docs/image-generation) |
| Gemini Enterprise Agent Platform | Yes | Images in the prompt, no mask | The same Gemini image models | [Image generation](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/image-generation) |
| OpenRouter | Yes, streamed for OpenAI's models | As reference images | Its image catalogue, such as `openai/gpt-image-2` and `google/gemini-3.1-flash-image` | [Image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |
| Vercel | Yes | Uploads or JSON, with a mask where the model takes one | Its image models, such as `openai/gpt-image-2` and `bfl/flux-2-pro` | [Image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| Together AI | Yes | One image by link | Its image models, such as `black-forest-labs/FLUX.2-pro` | [Create image](https://docs.together.ai/reference/post-images-generations) |
| QwenCloud | Yes | Up to 10 images, no mask | `qwen-image-3.0-pro`, `qwen-image-3.0`, `qwen-image-2.1-pro` | [OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai) |
| Z.ai | Yes, one image per request | No | `glm-image` | [Generate image](https://docs.z.ai/api-reference/image/generate-image) |
| MiniMax | Yes | No | `image-01` | [Text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i) |
| Mistral | No: images only as a tool of its agents | No | — | [Image generation tool](https://docs.mistral.ai/studio/agents/agent-tools/image_generation) |
| Anthropic, DeepSeek, Groq, Cohere, Ollama Cloud, Moonshot, Cerebras | Not offered by the provider | — | — | Each provider's API reference |

Google retired Imagen, so Kaeso calls the Gemini image models only. A size reaches them only when Google's size tables list it, such as `1024x1024` (`1:1` at `1K`); send `aspect_ratio` and `resolution` for the others. SpaceX takes `aspect_ratio` and `resolution` instead of `size`. Z.ai and QwenCloud answer only with links, which expire after 30 days and 24 hours, so `response_format: "b64_json"` answers 400 there. Gemini and OpenRouter answer only with base64, so `response_format: "url"` answers 400 there. The [OpenAI Images mapping](../development/kpp/openai-images.md) lists every parameter and how each provider takes it.

## Costs and logs

Kaeso bills image tokens at the model's image prices from KPMR when the provider reports them, as OpenAI and Google do, and the rest at its token prices. Providers that bill per image or per megapixel get their prices per unit from KPMR; when the request doesn't name the size, quality or resolution a price depends on, the lowest listed price applies, since that is the provider's default. A price KPMR marks as an estimate is shown as one on the model's sheet. Spend budgets reserve each request's dearest case before it is sent.

Request logs show the endpoint, such as `images/edits`, the images sent and made, and the size and quality. They never keep image bytes: an upload is logged as its fields, each image as its media type and size ([request logs](../console/request-logs.md)).

## OpenAI's image tool in Responses

A Responses app can give an OpenAI model the `image_generation` tool. Kaeso passes the tool, its `image_generation_call` items and their streamed events between the app and OpenAI as they are. Other providers can't run OpenAI's tool, so a request with it to another provider answers 400.

## Next steps

- [Model API](model-api.md): Look up every endpoint, header and model name.
- [Request logs](../console/request-logs.md): Find an image request and what it cost.
- [Supported providers](../concepts/supported-providers.md): See which providers Kaeso calls and how to add their keys.
