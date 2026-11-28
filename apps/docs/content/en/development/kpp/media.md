---
title: Media across formats
nav: Media
description: How the codecs carry audio, video and images between the API formats, in requests, replies and tool results.
order: 3
---

Audio, video and images reach a model in different shapes in each API format, and some formats carry none of them. This page lists what crosses, what is refused, and how media in tool results reaches a Chat Completions model. The other rules for crossing formats are in [crossing formats](cross-format.md).

## Audio and video

`Block::Audio` and `Block::Video` hold a `Source`, a cache hint and extras, as images do. Each format takes them as its documentation says, and refuses the rest before sending:

| Target | Audio | Video |
| --- | --- | --- |
| Gemini | `inlineData` or `fileData`, with the media type as sent | `inlineData`, `fileData`, or a YouTube URL without a MIME type |
| Chat Completions | `input_audio` in user messages, base64 only, in the formats the provider takes: `wav` and `mp3` for OpenAI, any for OpenRouter | `video_url` for providers that document it (OpenRouter, Moonshot); `Unsupported` elsewhere |
| Anthropic Messages | `Unsupported`: send a transcript as text | MiniMax's `video` block for MiniMax; `Unsupported` elsewhere: send frames as images |
| Responses | `Unsupported` | `Unsupported` |

A provider file id works only with the provider that issued it ([files](cross-format.md#files)). Gemini's `videoMetadata` (clip and frame rate) and `mediaResolution` stay extras of their part: only Gemini has them, and clipping changes what the model sees, so other formats refuse them. No transcoding happens: a media type the target has no name for, such as `audio/webm` for Chat Completions, is `Unsupported`.

The gateway also refuses audio and video for a model whose catalogue entry doesn't list `audio` or `video` among its inputs, as it does for images and PDFs: `` The model `gpt-4o` does not accept audio. ``

## Audio and image replies

`GenerateRequest::output_modalities` names what the reply should hold (`Text`, `Image`, `Audio`), and `audio_output` the voice and format of audio. An audio reply is a `Block::Audio` with base64 data, its media type and, where the format has one, a `transcript`; streams carry it as `BlockStart(Audio)` and `Audio` deltas of data and transcript text.

| App's format | Asks for | Gets |
| --- | --- | --- |
| Chat Completions | `modalities: ["text", "audio"]` with `audio.voice` and `audio.format` | `message.audio` and `delta.audio`, from OpenAI or from a Gemini speech model |
| Gemini | `responseModalities: ["AUDIO"]` or `["IMAGE"]` with `speechConfig` and `responseFormat` | `inlineData` audio and images, from Gemini or from an OpenAI audio model |
| Anthropic Messages, Responses | nothing: their formats have no setting for it | text only; audio and images a provider sends unasked are left out |

- **Voices** belong to their provider. A voice crosses only when the target lists it: OpenAI's built-in voices for Chat Completions, Gemini's 30 prebuilt voices for Gemini; anything else is `Unsupported` with the voice in `param`.
- **Formats** are never transcoded. `wav`, `mp3` and `pcm16` cross between Chat Completions and Gemini (`AUDIO_WAV`, `AUDIO_MP3`, `AUDIO_L16` at 24 kHz); `flac`, `opus` and `aac` reach only OpenAI. Without a format, a Chat Completions target gets `wav`, or `pcm16` for a stream.
- **Chat's text.** In `["text", "audio"]` the text is the audio's transcript, so Gemini gets `["AUDIO"]`, which its speech models need.
- **Replay.** OpenAI keeps an audio reply for a while and takes it back as `audio: {id}`. That id works only there, like a file id; another target refuses it unless the audio block carries a transcript, which Chat Completions then sends as text.
- **Images.** Gemini image models return images as parts. Only Gemini apps get them: Chat Completions and Messages have no image in a reply, and a Responses `image_generation_call` belongs to OpenAI's image tool, so a Gemini image would read as a tool call the app never made. Asking those formats for image output is `Unsupported`.
- **Catalogue.** The gateway refuses audio output for a model whose catalogue outputs lack `audio`, and image output without `image`: `` The model `gemini-3.5-flash` does not produce audio. `` A speech model that writes no text takes Chat's `["text", "audio"]`.

## Media in tool results

Claude's `tool_result` and Responses' `function_call_output` can hold images and files, such as the screenshot a tool took or the image Claude Code's `Read` and Codex's `view_image` open. A Chat Completions `tool` message holds only text (`ChatCompletionToolMessageParam`), and OpenAI and DeepSeek refuse an image there. So for Chat Completions models, Kaeso moves them:

- **The tool message** keeps the result's text, followed by `The rest of this result is in the next message.`
- **The user message after the results** starts with `` The rest of the result of tool call `<id>`: `` and the result's images, files and audio, in their order, for each such result. Text the app sent after the results follows them.
- **The header.** Each result is named in the reply's `kaeso-adapted` header as `tool result` and the call's id, such as `tool result call_abc`.

Anthropic and Responses models get the media in the tool result, as sent, and Gemini models in `functionResponse.parts` ([Gemini API](gemini.md)). Fixtures: `anthropic_messages/requests/tool-result-multi-turn.json`, `openai_responses/requests/function-call-output-array.json`.
