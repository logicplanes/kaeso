---
title: DashScope
description: How KPP's images, speech, transcriptions and video jobs map to QwenCloud's own DashScope API.
order: 8
---

Look up how a request reaches QwenCloud's models on DashScope here: which body a model takes, which field goes where, how a reply comes back, and the source of each mapping.

- **Codec:** `kaeso_codecs::dashscope::DashScope` (`crates/kaeso-codecs/src/dashscope/`)
- **Paths:** from the root of QwenCloud's host, with `Authorization: Bearer`:
  - video: `POST /api/v1/services/aigc/video-generation/video-synthesis`, `POST /api/v1/services/aigc/image2video/video-synthesis` (Wan 2.2 and 2.1 first and last frames), both with `X-DashScope-Async: enable`, and `GET /api/v1/tasks/{task_id}`;
  - images, Qwen3-TTS and Qwen-Audio-ASR: `POST /api/v1/services/aigc/multimodal-generation/generation`, answered in the same request;
  - Qwen-Audio-TTS: `POST /api/v1/services/audio/tts/SpeechSynthesizer` with `X-DashScope-SSE: enable`.
- **Fixtures:** `crates/kaeso-codecs/fixtures/dashscope/`, built from QwenCloud's references, none recorded

**Sources.** QwenCloud's references for [Wan 2.6 and older text to video](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task), [first-frame image to video](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task), [Wan 2.7 text to video](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task) and [its query result](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result), [Wan 2.7 image to video](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task), [Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task), [asynchronous tasks](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks), [Qwen-Image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image) and [its editing](https://docs.qwencloud.com/api-reference/image-generation/qwen-image-editing), [Wan 2.7 Image](https://docs.qwencloud.com/api-reference/image-generation/wan27-image-gen-edit/synchronous), [Wan 2.6 Image](https://docs.qwencloud.com/api-reference/image-generation/wan26-image-gen-edit/synchronous), [Z-Image](https://docs.qwencloud.com/api-reference/image-generation/z-image), [Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api) and [its voices](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-audio-tts), [Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts) and [its voices](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-tts), and [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api), read 2026-11-09.

## Video decisions

- **The model picks the body.** `dashscope::family` reads the model name: `wan3.0-video*`, `wan2.7-t2v*`, `wan2.7-i2v*`, `*-kf2v-*` (first and last frames), `*-t2v*` and `*-i2v*` (Wan 2.6 and older). Models that edit or reference video, such as `wan2.7-r2v`, `wan2.7-videoedit`, `wan2.1-vace-plus` and `wan2.2-animate-*`, are refused with `param` `model`.
- **Sizes only where QwenCloud lists them.** Wan 2.6 and older take a pixel `size` as `width*height`. Wan 2.7's text to video takes a tier and a ratio, so a `size` crosses only when it is one of the ten sizes its reference lists, as `1104x832` is `720P` at `4:3`. Image to video takes a tier only, since the image sets the aspect ratio, so a `size` is refused there.
- **No remix, no delete.** DashScope cancels only tasks that haven't started, which is not a delete, so Kaeso forgets a deleted video.
- **Times are Beijing time.** `submit_time` and `end_time` are UTC+8 without an offset; `dashscope::unix_of_beijing` reads them. The video's link is valid for 24 hours, so a finished job expires 24 hours after `end_time`.

## Video request

| KPP | Wan 2.6 and older | Wan 2.7 | Wan 3.0 |
| --- | --- | --- | --- |
| `model`, `prompt` | `model`, `input.prompt` | the same | the same |
| `negative_prompt` | `input.negative_prompt` | `input.negative_prompt` | refused |
| `FirstFrame` image | `input.img_url`; Wan 2.2 and 2.1 first and last frames: `input.first_frame_url` | `input.media[]` `{type: "first_frame", url}` | the same |
| `LastFrame` image | `input.last_frame_url`, web links only | `input.media[]` `{type: "last_frame", url}` | the same |
| `Reference` images | refused | refused | `input.media[]` `{type: "reference_image", url}`, never with frames |
| `size` | `parameters.size`, `1280*720`, also made from a resolution and an aspect ratio (text to video) | `parameters.resolution` and `ratio`, from the listed sizes (text to video) | refused |
| `resolution` | `parameters.resolution` (image to video), uppercased | `parameters.resolution` | `parameters.resolution` |
| `aspect_ratio` | refused | `parameters.ratio` (text to video) | `parameters.ratio` |
| `seconds`, `seed` | `parameters.duration`, `parameters.seed` | the same | the same |
| `audio` | `parameters.audio`, `wan2.6-i2v-flash` only | refused | `parameters.audio` |
| other `input` and `parameters` fields, such as `audio_url`, `prompt_extend`, `watermark` | extras | extras | extras |

Images go as links or data URLs, except Wan 2.2 and 2.1 first and last frames, which QwenCloud takes as public links only. Media of other types, such as `driving_audio` or `first_clip`, are refused when a request is decoded.

## Video reply

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`, `RUNNING`, `SUCCEEDED` | `queued`, `in_progress`, `completed` |
| `output.task_status` `FAILED`, `CANCELED`, `UNKNOWN` | `failed`, with `output.code` and `output.message`, or the status as the code |
| `output.video_url` | the video's link, `video/mp4` |
| `output.submit_time`, `output.end_time` | `created_at`, `completed_at` |
| `usage.output_video_duration`, `usage.SR`, `usage.ratio` | `seconds`, `resolution`, `aspect_ratio` |
| `request_id`, other `output` and `usage` fields | extras |

## Images

`dashscope::images::image_family` reads the model name: `qwen-image-*` (Qwen-Image 2.x and 3.0), `wan2.7-image*`, `wan2.6-image*` and `z-image*`. Other names, such as the older `wan2.6-t2i`, are refused with `param` `model`. The images and the prompt are one user message, images first.

| KPP | Qwen-Image | Wan 2.7 Image | Wan 2.6 Image | Z-Image |
| --- | --- | --- | --- | --- |
| `prompt` | `{"text"}` part | the same | the same | the same |
| `images` | `{"image"}` parts, links or data URLs; 1 to 10 on 2.1, 1 to 3 otherwise | 0 to 9 | 1 to 4, required | refused |
| `count` | `parameters.n`, at most 6 | `parameters.n` | `parameters.n`, at most 4; `1` when an app leaves it out, since the model's default is 4 | not sent; above 1 refused |
| `size` | `parameters.size` as `width*height`; `auto` leaves it out | the same | the same | the same |
| `resolution` | refused | `parameters.size` `1K`, `2K` or `4K` | `1K` or `2K` | refused |
| `negative_prompt` | `parameters.negative_prompt`, refused on 2.1 | refused | `parameters.negative_prompt` | refused |
| `seed` | `parameters.seed` | the same | the same | the same |
| `user` | left out, named in `kaeso-left-out` | the same | the same | the same |
| `mask`, `quality`, `output_format`, `output_compression`, `background`, `aspect_ratio`, file ids | refused | refused | refused | refused |

- **Links only.** DashScope answers with links valid for 24 hours, so the codec refuses `b64_json`.
- **Extensions carry over.** `prompt_extend`, `prompt_extend_mode`, `enable_thinking` and `watermark` from an OpenAI-format app go to `parameters`, as QwenCloud's OpenAI-compatible mode documents them. Every other foreign field is refused.
- **No watermark by default.** Every one of these models leaves the watermark off unless `watermark` is `true`, so Kaeso sends nothing.
- **Whole replies.** The images come in one reply; a streamed request gets them as finished events.

| DashScope reply | KPP |
| --- | --- |
| `output.choices[].message.content[].image` | the images, in order |
| a `text` part beside the images (Z-Image) | each image's `revised_prompt` |
| `usage.output_width` and `output_height`, `width` and `height`, or `size` | `settings.size` |
| `usage.input_tokens`, `output_tokens` | `usage`, with the whole `usage` kept |
| `request_id`, `output.finished`, `output.rewrite_status` and the choices' other fields | extras |

## Speech

`dashscope::speech::speech_family` reads the model name: `qwen-audio-*-tts-*` (Qwen-Audio-TTS 3.0 and 3.1) and `qwen3-tts-flash*`. Real-time models take only WebSocket connections and are refused.

| KPP | Qwen-Audio-TTS | Qwen3-TTS Flash |
| --- | --- | --- |
| `input`, `voice` | `input.text`, `input.voice` | the same |
| `format` | `input.format`: `mp3` (the default), `wav`, `opus`, `pcm16` as `pcm` at 24 kHz | `wav` (the default) or `pcm16`; others refused |
| `sample_rate` | `input.sample_rate` | 24,000 only |
| `speed` | `input.rate`, 0.5 to 2.0 | refused |
| `instructions` | `input.instruction` | refused |
| `language` | `input.language_hints` `[language]` | `input.language_type`, `Chinese` for `zh` and nine others |
| `seed` | `input.seed` | refused |
| `bitrate` | `input.bit_rate`, with `opus` only | refused |
| `timestamps`, `duration`, dialogue, source audio | refused | refused |

- **Streamed whenever the format allows.** Qwen-Audio-TTS always gets `X-DashScope-SSE: enable` and sends base64 pieces of the asked format. Qwen3-TTS streams only 24 kHz pcm, so `pcm16` is streamed and `wav` is answered whole as a link to the file.
- **Voices from the lists.** A named voice must be a system voice of the model, or one of its base voices named `<model>-<suffix>`. An explicit voice id, such as a cloned voice, goes as it is.
- **No conversion.** A format the model doesn't make is refused before sending.

| DashScope reply | KPP |
| --- | --- |
| an event's `output.audio.data` | a piece of the audio, labelled with the asked format |
| the event whose `output.finish_reason` is `stop` | the end, with `usage` and `usage.characters` as the billed characters |
| `output.audio.url` of a whole reply | the audio as a link |
| an `error` event, or a `:HTTP_STATUS/` line outside 2xx | the error |

## Transcription

Qwen-Audio-ASR Flash (`qwen-audio-3.0-asr-flash`, `qwen-audio-3.1-asr-flash`) answers in the same request. The file transcription models answer only asynchronous tasks, and the streaming, real-time and message models only WebSocket connections, so they are refused.

| KPP | Qwen-Audio-ASR |
| --- | --- |
| the audio | `input.messages[].content[]` `{"type": "input_audio", "input_audio": {"data"}}`, a link or a data URL |
| its media type or file name | `parameters.format`: `wav`, `mp3` or `opus`; other audio refused |
| `prompt` | a user message with an `input_text` part before the audio |
| `language` | `parameters.language_hints` `[language]` |
| `format` | `json` and `text`; the timed forms are refused |
| `temperature`, `timestamp_granularities`, `diarize`, speakers, translations | refused |

The reply's `output.text` is the transcript and `usage.duration` the seconds QwenCloud bills; the sentence and its words stay in extras.

## Errors

Errors are `{"request_id", "code", "message"}`: `InvalidApiKey` is an authentication error, `Throttling…` a rate limit and `DataInspection…` a content filter; other codes follow the HTTP status.
