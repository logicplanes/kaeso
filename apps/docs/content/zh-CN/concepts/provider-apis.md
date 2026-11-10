---
title: 各服务商的 API
description: 查阅 Kaeso 在每个内置服务商上提供哪些模型 API，以及对应的模型和来源。
order: 2
---

找到一个服务商，即可看到 Kaeso 的哪些模型 API 能调用它的模型。每一列都链接到该 API 的页面，那里有字段、限额和费用。

## 如何阅读此表

- **支持。** Kaeso 为此 API 调用该服务商，单元格中列出了模型。项目目录为此 API 列出的该服务商的任何模型都可以使用；列出的名称只是示例。
- **拒绝。** 服务商提供此功能，但 Kaeso 不调用它。Kaeso 在发送任何内容之前返回 400，单元格中写明原因。
- **不提供。** 最后一列链接的服务商文档中没有这样的 API。

服务商文档查阅于 2026-11-09。除非单元格给出自己的来源，否则一行的来源适用于该行所有单元格。

## 服务商

| 服务商 | [聊天](../api/model-api.md) | [嵌入](../api/model-api.md) | [图片](../api/images.md) | [语音合成](../api/audio.md) | [转写](../api/audio.md) | [翻译](../api/audio.md) | [视频](../api/videos.md) | [音乐和音效](../api/elevenlabs.md) | 来源 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Anthropic `anthropic` | 支持：Claude 模型 | 不提供：Anthropic 推荐 Voyage AI（[Embeddings](https://platform.claude.com/docs/en/build-with-claude/embeddings)） | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [Models](https://platform.claude.com/docs/en/about-claude/models/overview) |
| Cerebras `cerebras` | 支持：`gpt-oss-120b`、`qwen-3.8-27b` | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [Models](https://inference-docs.cerebras.ai/models/overview) |
| Cohere `cohere` | 支持：Command 模型 | 支持：Embed 模型，例如 `embed-v4.0`，作为自定义模型添加 | 不提供 | 不提供 | 支持：`cohere-transcribe-03-2026` | 不提供 | 不提供 | 不提供 | [Models](https://docs.cohere.com/docs/models) |
| DeepSeek `deepseek` | 支持：DeepSeek 模型 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [API docs](https://api-docs.deepseek.com/) |
| ElevenLabs `elevenlabs` | 不提供：ElevenLabs 不生成文本 | 不提供 | 支持：通过其图片生成调用 OpenAI 的 GPT Image、Google 的 Gemini 图片模型和字节跳动的 Seedream（[图片和视频指南](https://elevenlabs.io/docs/eleven-api/guides/cookbooks/image-and-video)） | 支持：`eleven_v3`、`eleven_multilingual_v2`、`eleven_flash_v2_5` 等 | 支持：`scribe_v2`、`scribe_v2_medical`，以及[实时会话](../api/elevenlabs.md#实时会话)中的 `scribe_v2_realtime` | 不提供 | 支持：`veo-3.1-*`、`bytedance-seedance-v2*` | 支持：`music_v1`、`music_v2*`、`eleven_text_to_sound_v2`、`eleven_text_to_sound_v3` | [OpenAPI document](https://api.elevenlabs.io/openapi.json) |
| Fireworks AI `fireworks-ai` | 支持：Fireworks 的无服务器模型 | 支持：`qwen3-embedding-8b`、`nomic-ai/nomic-embed-text-v1.5` 及其他较旧模型 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [API reference](https://docs.fireworks.ai/llms.txt) |
| Gemini Enterprise Agent Platform `gemini-agent-platform` | 支持：Gemini、Claude 和合作伙伴模型 | 拒绝：该平台只通过 `:predict` 记录其嵌入模型（[Text embeddings](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/models/text-embeddings-api)） | 支持：Gemini 图片模型 | 支持：`gemini-3.8-flash-tts`、`gemini-2.5-*-tts` | 支持：使用 `audioTranscriptionConfig` 的 Gemini 模型 | 不提供 | 支持：`veo-3.1-*`、`gemini-omni-*`，需要服务账号密钥 | 拒绝：Kaeso 没有映射 Lyria（`lyria-002`、`lyria-3-*`）（[Generate music](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/music/generate-music)） | [Models](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models) |
| Google AI Studio `google` | 支持：Gemini 和 Gemma 模型 | 支持：`gemini-embedding-2`、`gemini-embedding-001` | 支持：`gemini-*-image`、`gemini-nano-banana-2.1` | 支持：`gemini-3.8-flash-tts`、`gemini-3.8-flash-lite-tts` | 支持：`gemini-3.5-transcribe` | 不提供 | 支持：`gemini-omni-*` | 拒绝：Kaeso 没有映射 Lyria（`lyria-3-clip-preview`、`lyria-3.5`）（[Music generation](https://aistudio.google.com/docs/music-generation)） | [Gemini API](https://aistudio.google.com/docs) |
| Groq `groq` | 支持：Groq 的模型 | 不提供 | 不提供 | 支持：Orpheus | 支持：`whisper-large-v3`、`whisper-large-v3-turbo` | 支持：`whisper-large-v3` | 不提供 | 不提供 | [Models](https://console.groq.com/docs/models) |
| Hugging Face `huggingface` | 支持：路由器背后各服务商的开放模型 | 不提供：路由器的 OpenAI 兼容 API 只用于聊天（[chat completion](https://huggingface.co/docs/inference-providers/tasks/chat-completion)） | 不提供：同嵌入 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [Inference Providers](https://huggingface.co/docs/inference-providers/index) |
| MiniMax `minimax` | 支持：`MiniMax-M3`、`MiniMax-M3.1-Flash` | 不提供 | 支持：`image-01` | 支持：`speech-2.8-*`、`speech-2.6-*`、`speech-02-*` | 支持：`asr-1.0` | 不提供 | 支持：`MiniMax-Hailuo-*`、`MiniMax-H3*` | 拒绝：Kaeso 没有映射 MiniMax 的音乐生成（[Music generation](https://platform.minimax.io/docs/api-reference/music-generation)） | [API overview](https://platform.minimax.io/docs/api-reference/api-overview) |
| Mistral `mistral` | 支持：Mistral 模型 | 支持：`mistral-embed`、`codestral-embed` | 拒绝：Mistral 只把图片生成作为其智能体的工具提供（[Image generation tool](https://docs.mistral.ai/studio/agents/agent-tools/image_generation)） | 支持：`voxtral-mini-tts-3-0` | 支持：Voxtral Transcribe 3 | 不提供 | 不提供 | 不提供 | [Models](https://docs.mistral.ai/getting-started/models) |
| Moonshot `moonshotai` | 支持：Kimi 模型 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [Kimi API](https://platform.kimi.ai/docs) |
| Nebius Token Factory `nebius` | 支持：Nebius 的无服务器模型 | 支持：`Qwen/Qwen3-Embedding-8B` | 不提供：Nebius 的 OpenAPI 文档中没有图像端点 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [OpenAPI document](https://api.tokenfactory.nebius.com/openapi.json) |
| NVIDIA `nvidia` | 支持：build.nvidia.com 上的模型 | 支持：Nemotron 和 NV-Embed 模型，例如 `nvidia/nemotron-3.5-embed-1b` | 不提供：NVIDIA 的图像模型使用自己的路径，而不是 OpenAI 的 Images API | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) |
| Ollama Cloud `ollama-cloud` | 支持：Ollama 的云端模型 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | 不提供 | [Cloud](https://docs.ollama.com/cloud) |
| OpenAI `openai` | 支持：GPT 模型 | 支持：`text-embedding-3-*` | 支持：`gpt-image-*`、`chatgpt-image-latest` | 支持：`gpt-4o-mini-tts`、`tts-1`、`tts-1-hd` | 支持：`gpt-transcribe`、`gpt-4o-*-transcribe`、`whisper-1` | 支持：`whisper-1` | 拒绝：OpenAI 已于 2026 年 9 月 24 日关闭 Sora（[Deprecations](https://platform.openai.com/docs/deprecations)） | 不提供 | [Models](https://developers.openai.com/api/docs/models) |
| OpenRouter `openrouter` | 支持：其目录 | 支持：其嵌入模型 | 支持：其图片模型 | 支持：其语音合成模型 | 支持：其转写模型 | 不提供 | 支持：其视频模型 | 不提供 | [OpenAPI document](https://openrouter.ai/openapi.json) |
| QwenCloud `alibaba` | 支持：Qwen 模型和其他厂商的模型 | 支持：`text-embedding-v4`，作为自定义模型添加 | 支持：Qwen-Image、Wan Image、Wan 文生图模型 `wan2.6-t2i` 到 `wan2.1-t2i-*`、Z-Image | 支持：Qwen-Audio-TTS、`qwen3-tts-flash`；实时模型被拒绝 | 支持：`qwen3-asr-flash`、Qwen-Audio-ASR Flash；文件和实时模型被拒绝 | 拒绝：QwenCloud 只通过实时 WebSocket 连接翻译语音（[Livetranslate](https://docs.qwencloud.com/api-reference/speech-translation/livetranslate-realtime/directory)） | 支持：Wan 文生视频和图生视频 | 音效：Qwen-Audio-TTS-Next（`qwen-audio-3.1-tts-next`）可生成音效和描述的场景（[音频生成](https://docs.qwencloud.com/api-reference/audio-generation)）。音乐：不提供；QwenCloud 的模型列表中没有音乐模型（[音频生成模型](https://docs.qwencloud.com/models/model-list-audio-generation)） | [API reference](https://docs.qwencloud.com/llms.txt) |
| SambaNova `sambanova` | 支持：SambaCloud 的模型 | 不提供：SambaCloud 只在 SambaStack 上提供嵌入（[embeddings](https://docs.sambanova.ai/docs/en/features/embeddings)） | 不提供 | 不提供 | 不提供：Whisper 只在 SambaStack 上运行（[audio](https://docs.sambanova.ai/docs/en/features/audio)） | 不提供：同转录 | 不提供 | 不提供 | [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) |
| Scaleway `scaleway` | 支持：Scaleway 的无服务器模型 | 支持：`bge-multilingual-gemma2`、`qwen3-embedding-8b` | 不提供：Scaleway 计划提供图像 API，但目前还没有（[OpenAI compatibility](https://www.scaleway.com/en/docs/generative-apis/reference-content/openai-compatibility/)） | 不提供 | 支持：`whisper-large-v3`，仅 `json` | 不提供 | 不提供 | 不提供 | [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/) |
| SpaceX `xai` | 支持：Grok 模型 | 不提供 | 支持：`grok-imagine-image*` | 支持：`grok-tts` | 支持：`grok-voice-transcribe-2.0` | 不提供 | 支持：`grok-imagine-video*` | 不提供 | [REST API reference](https://docs.x.ai/developers/rest-api-reference) |
| Together AI `togetherai` | 支持：Together 的模型 | 支持：专用端点上的模型，因为 Together 没有无服务器嵌入模型 | 支持：FLUX、Seedream、Qwen-Image | 支持：Orpheus、Cartesia Sonic、Kokoro | 支持：Whisper、Parakeet、Nemotron ASR | 支持：`openai/whisper-large-v3` | 支持：Seedance、Wan 等 | 不提供 | [Serverless models](https://docs.together.ai/docs/serverless-models) |
| Vercel `vercel` | 支持：其目录 | 支持：其嵌入模型 | 支持：其图片模型 | 支持：其语音合成模型 | 支持：其转写模型 | 不提供 | 拒绝：Vercel 只通过其 AI SDK 记录视频，没有 HTTP 接口 | 拒绝：Kaeso 只为语音合成调用 Vercel 的语音模型 | [Modalities](https://vercel.com/docs/ai-gateway/modalities) |
| Z.ai `zai` | 支持：GLM 模型 | 不提供 | 支持：`glm-image` | 不提供 | 支持：`glm-asr-2512` | 不提供 | 支持：`cogvideox-3` | 不提供 | [API reference](https://docs.z.ai/llms.txt) |

自定义服务商只有在您为其开启相应 API 后，才能使用 OpenAI 的 Images、音频和 Videos API（[自定义服务商](../console/custom-providers.md)）。
