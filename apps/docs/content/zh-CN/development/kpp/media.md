---
title: 跨格式的媒体
nav: 媒体
description: 编解码器如何在 API 格式之间传递音频、视频和图像，包括请求、回复和工具结果中的媒体。
order: 3
---

音频、视频和图像在每种 API 格式中以不同形式到达模型，有些格式完全不支持。本页列出哪些内容可以跨格式传递、哪些会被拒绝，以及工具结果中的媒体如何到达 Chat Completions 模型。跨格式的其他规则见[跨格式转换](cross-format.md)。

## 音频和视频

`Block::Audio` 和 `Block::Video` 与图像一样，包含一个 `Source`、一个缓存提示和 extras。每种格式按其文档接受它们，并在发送前拒绝其余的内容：

| 目标 | 音频 | 视频 |
| --- | --- | --- |
| Gemini | `inlineData` 或 `fileData`，媒体类型按发送时的写法 | `inlineData`、`fileData`，或不带 MIME 类型的 YouTube URL |
| Chat Completions | 用户消息中的 `input_audio`，只接受 base64，格式取决于服务商：OpenAI 为 `wav` 和 `mp3`，OpenRouter 接受任何格式 | 在文档中写明 `video_url` 的服务商（OpenRouter、Moonshot）使用它；其他服务商为 `Unsupported` |
| Anthropic Messages | `Unsupported`：请以文本发送转写稿 | MiniMax 使用 MiniMax 的 `video` 块；其他服务商为 `Unsupported`：请把视频帧作为图像发送 |
| Responses | `Unsupported` | `Unsupported` |

服务商的文件 ID 只能用于签发它的服务商（[文件](cross-format.md#文件)）。Gemini 的 `videoMetadata`（剪辑和帧率）和 `mediaResolution` 保留为所在 part 的 extras：只有 Gemini 有这些字段，而剪辑会改变模型看到的内容，因此其他格式会拒绝它们。网关不做转码：目标没有对应名称的媒体类型，例如 Chat Completions 的 `audio/webm`，为 `Unsupported`。

对于目录条目的输入中没有 `audio` 或 `video` 的模型，网关也会拒绝音频和视频，与图像和 PDF 的处理相同：`` The model `gpt-4o` does not accept audio. ``

## 音频和图像回复

`GenerateRequest::output_modalities` 说明回复应包含什么（`Text`、`Image`、`Audio`），`audio_output` 说明音频的声音和格式。音频回复是一个 `Block::Audio`，包含 base64 数据、媒体类型，以及格式提供时的 `transcript`；流式回复以 `BlockStart(Audio)` 和携带数据与转写文本的 `Audio` 增量传输。

| 应用的格式 | 请求方式 | 收到的内容 |
| --- | --- | --- |
| Chat Completions | `modalities: ["text", "audio"]`，加上 `audio.voice` 和 `audio.format` | `message.audio` 和 `delta.audio`，来自 OpenAI 或 Gemini 语音模型 |
| Gemini | `responseModalities: ["AUDIO"]` 或 `["IMAGE"]`，加上 `speechConfig` 和 `responseFormat` | `inlineData` 音频和图像，来自 Gemini 或 OpenAI 音频模型 |
| Anthropic Messages、Responses | 无：这些格式没有对应设置 | 只有文本；服务商未经请求发送的音频和图像会被省略 |

- **声音**属于各自的服务商。只有目标列出的声音才会传过去：Chat Completions 为 OpenAI 的内置声音，Gemini 为其 30 个预置声音；其他声音为 `Unsupported`，`param` 中给出该声音。
- **格式**从不转码。`wav`、`mp3` 和 `pcm16` 可在 Chat Completions 和 Gemini 之间传递（`AUDIO_WAV`、`AUDIO_MP3`、24 kHz 的 `AUDIO_L16`）；`flac`、`opus` 和 `aac` 只能发往 OpenAI。没有格式时，Chat Completions 目标收到 `wav`，流式请求收到 `pcm16`。
- **Chat 的文本。** 在 `["text", "audio"]` 中，文本是音频的转写稿，因此 Gemini 收到 `["AUDIO"]`，这是其语音模型所要求的。
- **回放。** OpenAI 会把音频回复保存一段时间，并以 `audio: {id}` 接收回放。这个 ID 只在那里有效，与文件 ID 一样；其他目标会拒绝它，除非音频块带有转写稿，这时 Chat Completions 把转写稿作为文本发送。
- **图像。** Gemini 图像模型以 part 返回图像。只有 Gemini 应用会收到：Chat Completions 和 Messages 的回复中没有图像，而 Responses 的 `image_generation_call` 属于 OpenAI 的图像工具，Gemini 的图像会被理解为应用从未发起的工具调用。向这些格式请求图像输出为 `Unsupported`。
- **目录。** 对于目录输出中没有 `audio` 的模型，网关拒绝音频输出；没有 `image` 的模型拒绝图像输出：`` The model `gemini-3.5-flash` does not produce audio. `` 不写文本的语音模型可以接受 Chat 的 `["text", "audio"]`。

## 工具结果中的媒体

Claude 的 `tool_result` 和 Responses 的 `function_call_output` 可以包含图像和文件，例如工具截取的屏幕截图，或 Claude Code 的 `Read` 与 Codex 的 `view_image` 打开的图像。Chat Completions 的 `tool` 消息只包含文本（`ChatCompletionToolMessageParam`），OpenAI 和 DeepSeek 会拒绝其中的图像。因此对于 Chat Completions 模型，Kaeso 会移动它们：

- **工具消息**保留结果的文本，后接 `The rest of this result is in the next message.`
- **结果之后的用户消息**对每个这样的结果，以 `` The rest of the result of tool call `<id>`: `` 开头，后接该结果的图像、文件和音频，保持原有顺序。应用在结果之后发送的文本跟在它们后面。
- **响应头。** 每个结果都会在回复的 `kaeso-adapted` 头中以 `tool result` 加调用 ID 的形式列出，例如 `tool result call_abc`。

Anthropic 和 Responses 模型会在工具结果中原样收到这些媒体，Gemini 模型则在 `functionResponse.parts` 中收到（[Gemini API](gemini.md)）。夹具：`anthropic_messages/requests/tool-result-multi-turn.json`、`openai_responses/requests/function-call-output-array.json`。
