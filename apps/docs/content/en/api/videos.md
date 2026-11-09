---
title: Videos API
description: Make videos through Kaeso with OpenAI's Videos API, on every provider that makes video.
order: 4
---

Apps make videos through Kaeso with OpenAI's Videos API and the official OpenAI SDKs. Kaeso starts the job at the model's provider in that provider's own form, keeps track of it under an id of its own, and answers in OpenAI's form.

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

Add the video model to the project's catalogue first ([models and aliases](../console/models-and-aliases.md)). Any [model name](model-api.md#model-names) or alias works, as for every other endpoint.

## Endpoints

| Method and path | What it does |
| --- | --- |
| `POST /v1/videos` | Starts a video from a prompt, as JSON or as an upload (`multipart/form-data`) with an `input_reference` image as the first frame (`client.videos.create`) |
| `GET /v1/videos/{video_id}` | The video's job, after asking its provider how it stands (`client.videos.retrieve`, `client.videos.poll`) |
| `GET /v1/videos/{video_id}/content` | The finished file, with `variant` `video` (the default), `thumbnail` or `spritesheet` (`client.videos.download_content`) |
| `POST /v1/videos/{video_id}/remix` | Starts a new video from a finished one and a new prompt (`client.videos.remix`) |
| `GET /v1/videos` | The project's videos, with `after`, `limit` (up to 100, 20 by default) and `order` (`desc` by default) (`client.videos.list`) |
| `DELETE /v1/videos/{video_id}` | Deletes the video (`client.videos.delete`) |
| `POST /v1/videos/edits`, `/v1/videos/extensions`, `/v1/videos/characters` | Answers 404: these were Sora's. Remix a video or create a new one instead. |

Errors use OpenAI's error format. The largest request body, uploads included, is `body_limits.model_api`, 32 MiB by default ([configuration](../self-hosting/configuration.md#request-and-response-bodies)).

Kaeso reads OpenAI's fields: `model`, `prompt`, `seconds`, `size` and `input_reference`. A `size` reaches a provider that takes a resolution and an aspect ratio only when the provider lists both, as `1280x720` is `720p` at `16:9`. Any other field reaches only Together AI, which takes OpenAI's form; every other provider answers 400 with the field in `param`.

## How a job runs

Creating a video answers at once with its job, `queued` or `in_progress`, under an id such as `video_06a1…`. The id is Kaeso's own and never shows the provider's.

Each retrieve asks the provider how the job stands while it runs, and keeps what it says. When the provider can't answer, the retrieve returns the job as Kaeso last saw it, so ask again later. A job that ended is never asked about again. The SDKs' `poll` and `create_and_poll` call retrieve until the job ends.

A listing shows each video as Kaeso last saw it, without asking providers. A video belongs to the project whose key started it; another project's key gets 404, the same answer as for an id that doesn't exist. Once the provider key that started a video is replaced, Kaeso can't reach the job and answers 409.

Retrieves, downloads, deletes and remixes that ask the provider count against the key's [request rates](../console/limits-and-budgets.md) like any model request, since each spends the provider's quota with the organization's key; a retrieve of a job that ended answers from Kaeso's record and doesn't count.

## Download a video

Kaeso streams the file from where the provider keeps it, without storing it, and passes `Range` through. Veo and Gemini Omni send the file inside their answer; Kaeso reads it again for each download, bounded by `body_limits.provider_response` (128 MiB by default). A link on a host other than the provider's own API is fetched from public addresses only, whatever [`custom_providers.allowed_networks`](../self-hosting/configuration.md) opens, and never gets the provider key.

Providers keep files for a limited time. Kaeso fetches a fresh link for each download where the provider hands out short-lived links, as MiniMax and ElevenLabs do. After the job's `expires_at`, the download answers 404. Only Z.ai offers a `thumbnail` and no provider offers a `spritesheet`; asking for one answers 404 with `param` `variant`.

Deleting a video deletes it at providers that can, Google AI Studio and MiniMax's H3 API, and Kaeso forgets it everywhere else.

## Remix a video

A remix starts a new video from a completed one and a new prompt, at the same provider with the same model:

```python
remix = client.videos.remix(video.id, prompt="The same lighthouse in a snowstorm")
```

SpaceX remixes through its edits API, and Gemini Omni continues the earlier interaction. Every other provider answers 400 with `param` `video_id`.

## Providers

Every built-in provider's documentation was read for video generation on 2026-11-09. Kaeso refuses a field the model's provider doesn't take before anything is sent, with the field in `param`.

| Provider | Models | Images | Remix | Source |
| --- | --- | --- | --- | --- |
| SpaceX | `grok-imagine-video-1.5`, `grok-imagine-video-1.5-lite`, `grok-imagine-video` | First frame, uploaded or linked | Yes | [Videos](https://docs.x.ai/developers/rest-api-reference/inference/videos) |
| Google AI Studio | `gemini-omni-1.2-flash`, `gemini-omni-1.1-flash` | First frame, uploaded or linked | Yes | [Gemini Omni](https://aistudio.google.com/docs/omni) |
| Gemini Enterprise Agent Platform | `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `veo-3.1-lite-generate-001`, with a service account key | First frame, uploaded | No | [Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate) |
| Gemini Enterprise Agent Platform | `gemini-omni-1.2-flash`, `gemini-omni-1.1-flash-preview`, `gemini-omni-flash-preview`, with a service account key | First frame, uploaded or as a `gs://` link, except on `gemini-omni-flash-preview` | Yes | [Generate videos from text](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text) |
| MiniMax | `MiniMax-Hailuo-2.3`, `MiniMax-Hailuo-2.3-Fast`, `MiniMax-Hailuo-02`, `MiniMax-H3` and its variants | First frame, uploaded or linked | No | [Video generation](https://platform.minimax.io/docs/api-reference/video-generation-t2v) |
| Z.ai | `cogvideox-3` | First frame, uploaded or linked | No | [Generate video](https://docs.z.ai/api-reference/video/generate-video) |
| OpenRouter | Its video models, such as `google/veo-3.1` and `alibaba/wan-3.0` | First frame, uploaded or linked | No | [Video generation](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request) |
| Together AI | Its video models, such as `ByteDance/Seedance-2.0` and `Wan-AI/wan2.7-t2v` | First frame, linked only | No | [Create video](https://docs.together.ai/reference/create-videos) |
| ElevenLabs | `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `bytedance-seedance-v2` and its variants | First frame, uploaded | No | [OpenAPI document](https://api.elevenlabs.io/openapi.json) |
| OpenAI | No: OpenAI shut Sora down on 24 September 2026, so a `sora-*` model answers 400 | — | — | [Deprecations](https://platform.openai.com/docs/deprecations) |
| Vercel | No: Vercel documents video only through its AI SDK, not over HTTP | — | — | [Video generation](https://vercel.com/docs/ai-gateway/modalities/video-generation) |
| QwenCloud | Wan text to video and image to video, such as `wan3.0-video`, `wan2.7-t2v` and `wan2.6-i2v-flash` | First frame, uploaded or linked, on image to video models and Wan 3.0 | No | [Video generation](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task) |
| Anthropic, DeepSeek, Groq, Mistral, Cohere, Ollama Cloud, Moonshot, Cerebras | Not offered by the provider | — | — | Each provider's API reference |

A project lists a model for video when KPMR or its own entry says the model makes video. Veo on Gemini Enterprise Agent Platform needs a service account key: express mode lists no Veo models, so Kaeso refuses an express mode key for them. Gemini Omni on the Agent Platform needs a service account key too, since express mode lists no Omni models, and goes to the platform's global location whatever location the key names, since Omni is served only there.

The [OpenAI Videos mapping](../development/kpp/openai-videos.md) and the provider pages beside it list every field and how each provider takes it.

## Costs and logs

Starting a video costs nothing. Kaeso bills it once, when the first request sees it completed, at the model's prices from KPMR: per second or per video, by resolution, sound and length where the prices differ. Gemini Omni is billed by its tokens. A failed video costs nothing, and a model without prices in KPMR is logged without a cost. Kaeso keeps Together AI's `outputs.cost` with the job but doesn't bill it: Together's reference gives it as a whole number without a unit.

Spend budgets check a video's dearest case before it starts, and its cost counts once it ends. Request logs show the start as `videos` or `videos/remix` and the finished video as `videos/completed`, with its clips, seconds and resolution. They never keep video or image bytes ([request logs](../console/request-logs.md)).

## Next steps

- [Model API](model-api.md): Look up every endpoint, header and model name.
- [Request logs](../console/request-logs.md): Find a video and what it cost.
- [Supported providers](../concepts/supported-providers.md): See which providers Kaeso calls and how to add their keys.
