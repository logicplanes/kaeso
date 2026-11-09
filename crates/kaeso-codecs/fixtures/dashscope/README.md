# Fixtures: `dashscope`

QwenCloud's DashScope API:

- video synthesis: `POST /api/v1/services/aigc/video-generation/video-synthesis` (and `/api/v1/services/aigc/image2video/video-synthesis` for Wan 2.2 and 2.1 first and last frames), and `GET /api/v1/tasks/{task_id}`;
- images, Qwen3-TTS and Qwen-Audio-ASR: `POST /api/v1/services/aigc/multimodal-generation/generation`, answered in the same request (`images/`, `speech/` and `transcriptions/` under `requests/` and `responses/`);
- Wan 2.5 and earlier text-to-image: `POST /api/v1/services/aigc/text2image/image-synthesis` with `X-DashScope-Async: enable`, and `GET /api/v1/tasks/{task_id}` (`requests/images/*-task.json`, `polls/images/`);
- Qwen-Audio-TTS: `POST /api/v1/services/audio/tts/SpeechSynthesizer`, streamed with `X-DashScope-SSE: enable` (`streams/speech/`).

## Sources

- https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task, `…/wan-image-to-video-first-frame/create-task`, `…/wan27-text-to-video/create-task` and `…/query-result`, `…/wan27-image-to-video/create-task`, `…/wan30-video/create-task` and `…/query-result`, the image-to-video first and last frame guide, and https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks, read 2026-11-09.
- https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image, `…/qwen-image-editing`, `…/wan27-image-gen-edit/synchronous`, `…/wan26-image-gen-edit/synchronous`, `…/wan-text-to-image-v2/synchronous`, `…/wan-text-to-image-v2/create-task` and `…/query-result`, and `…/z-image`; https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api and `…/qwen-tts`; https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api; the Qwen-Audio-TTS voice list for the error stream; read 2026-11-09.
- No fixture is recorded: no QwenCloud key was used, so every file is spec-built.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a task's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/invalid-api-key-401.json` | an error, with its status in `.meta.json` | doc: DashScope error response (`InvalidApiKey`) |
| `errors/rate-quota-429.json` | an error, with its status in `.meta.json` | doc: asynchronous tasks (`Throttling.RateQuota`) |
| `errors/invalid-parameter-400.json` | an error, with its status in `.meta.json` | doc: Z-Image `400` example |
| `requests/images/qwen-image-edit.json` | Qwen-Image edit of three images with `n`, a size and a negative prompt | doc: Qwen image editing, multi-image example |
| `requests/images/qwen-image-generate.json` | Qwen-Image from a prompt with a size | doc: Qwen synchronous, schema example prompt and curl parameters |
| `requests/images/wan26-edit.json` | Wan 2.6 Image edit, prompt before the images, a tier | doc: Wan 2.6 synchronous, edit example |
| `requests/images/wan27-edit-tier.json` | Wan 2.7 Image edit of two images with a tier | doc: Wan 2.7 synchronous, edit example |
| `requests/images/wan27-generate.json` | Wan 2.7 Image from a prompt with thinking mode | doc: Wan 2.7 synchronous, text-to-image example |
| `requests/images/z-image-generate.json` | Z-Image from a prompt | doc: Z-Image curl example |
| `requests/images/wan26-t2i.json` | Wan 2.6 text-to-image from a prompt, synchronously | doc: Wan text-to-image V2 synchronous, request table and recommended size |
| `requests/images/wan22-t2i-flash-task.json` | Wan 2.2 Flash text-to-image as a task, with a negative prompt and a seed | spec-built from the create-task table (`input.prompt`, `parameters`) |
| `requests/speech/qwen-audio-tts.json` | Qwen-Audio-TTS in wav at 24 kHz | doc: Qwen-Audio-TTS HTTP API, non-streaming example |
| `requests/speech/qwen3-tts.json` | Qwen3-TTS with a language | doc: Qwen-TTS reference, Java example's text and `language_type` |
| `requests/transcriptions/asr-context.json` | Qwen-Audio-ASR with a context text and a language hint | spec-built from the Qwen-Audio-ASR request table |
| `requests/transcriptions/asr-link.json` | Qwen-Audio-ASR of a linked file | doc: Qwen-Audio-ASR HTTP API, non-streaming example |
| `responses/images/qwen-image-edit.json` | two edited images in one choice, with `rewrite_status` | doc: `QwenImageEditResponse` example |
| `responses/images/qwen-image-generate.json` | one image with Qwen-Image 3.0's usage | doc: `QwenImageSyncResponse` example |
| `responses/images/wan27.json` | Wan 2.7's image with `type`, token usage and the output size | doc: `Wan27ImageResponse` example |
| `responses/images/z-image.json` | Z-Image's image and its prompt | doc: Z-Image success example |
| `responses/images/wan26-t2i.json` | Wan 2.6 text-to-image's image and its size | doc: Wan text-to-image V2 synchronous, response object |
| `polls/images/started.json` | the reply that starts a text-to-image task | doc: create task, `200` example |
| `polls/images/running.json` | a running text-to-image task | spec-built from the query-result fields |
| `polls/images/succeeded.json` | a finished task with one image and one that failed alone | spec-built from the query-result fields (`results[]` with `url`, `actual_prompt`, or `code` and `message`) |
| `polls/images/failed.json` | a failed text-to-image task | spec-built from the query-result fields (`code`, `message`) |
| `responses/speech/qwen-audio-tts-link.json` | Qwen-Audio-TTS answered whole, with a link | doc: Qwen-Audio-TTS non-streaming response |
| `responses/speech/qwen3-tts-link.json` | Qwen3-TTS answered with a link to a wav file | doc: Qwen-TTS response object |
| `responses/transcriptions/asr.json` | a transcript with its sentence, words and duration | doc: Qwen-Audio-ASR non-streaming response |
| `streams/speech/invalid-voice.sse` | an error event for a voice the model lacks | spec-built: the SSE frame of the ASR reference with the voice list's error message |
| `streams/speech/qwen-audio-tts.sse` | sentence begin, two audio pieces, sentence end with usage, and the final event | spec-built: the reference's event types and examples; the audio bytes are made up |
| `streams/speech/qwen3-tts-pcm.sse` | two pcm pieces and the final event with the link | spec-built: the response object streamed as the reference describes; the audio bytes and the characters are made up |
| `polls/failed.json` | a failed task | doc: Wan 2.7 query result, `FAILED` example |
| `polls/running.json` | a running task | doc: Wan 2.7 query result, `RUNNING` example |
| `polls/succeeded.json` | a finished task with its video and usage | doc: Wan 2.7 query result, `SUCCEEDED` example |
| `polls/unknown.json` | an expired task | doc: Wan 2.7 query result, `UNKNOWN` example |
| `requests/wan22-first-last-frame.json` | Wan 2.2 first and last frames | doc: first and last frame guide |
| `requests/wan26-first-frame.json` | Wan 2.6 Flash image to video with sound | doc: first-frame create task |
| `requests/wan26-text-to-video.json` | Wan 2.6 text to video with a size and audio | doc: text-to-video create task |
| `requests/wan27-first-last-frame.json` | Wan 2.7 frames in `media` | doc: Wan 2.7 image-to-video create task |
| `requests/wan27-text-to-video.json` | Wan 2.7 text to video with a tier and ratio | doc: Wan 2.7 text-to-video create task |
| `requests/wan30-reference-images.json` | Wan 3.0 reference images | doc: Wan 3.0 create task |
| `responses/started.json` | the reply that starts a task | doc: `AsyncTaskSubmitResponse` |
