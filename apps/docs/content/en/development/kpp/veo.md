---
title: Veo
description: How KPP's video jobs map to Veo's long-running operations on Gemini Enterprise Agent Platform.
order: 8
---

Look up how a video job reaches Veo on Gemini Enterprise Agent Platform here: which field goes where, how the operation is asked about, and the source of each mapping.

- **Codec:** `kaeso_codecs::veo::Veo` (`crates/kaeso-codecs/src/veo/`)
- **Paths:** `{models}/{model}:predictLongRunning` and `{models}/{model}:fetchPredictOperation`, where `{models}` is the platform's publisher path (`EgressCtx::gemini_profile`)
- **Fixtures:** `crates/kaeso-codecs/fixtures/veo/`, built from the platform's reference, none recorded; Kaeso never calls the platform live

**Sources.** Agent Platform's [generate videos from text](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text), [Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate), `VideoGenerationModelInstance` and `VideoGenerationModelParams`, read 2026-11-09.

## Decisions

- **The job id is the operation's name.** Asking about it is `POST …:fetchPredictOperation` with `{"operationName": …}`, on the model that started it.
- **Videos come inline.** Kaeso names no `storageUri`, so the platform returns each video as `bytesBase64Encoded` in the operation's `response.videos`. A `gcsUri` is kept as a link, which Kaeso can't fetch.
- **A service account only.** Express mode lists no Veo models ([express mode](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/express-mode/overview)), so the gateway refuses an express key for them.
- **A filtered operation fails.** A finished operation without videos and with `raiMediaFilteredCount` is `failed`, code `moderation_blocked`.

## Request

| KPP | Veo |
| --- | --- |
| `prompt` | `instances[0].prompt` |
| `FirstFrame` image | `instances[0].image` `{bytesBase64Encoded, mimeType}` or `{gcsUri}` |
| `LastFrame` image | `instances[0].lastFrame` |
| `Reference` images | `instances[0].referenceImages[]` with `referenceType: "asset"` |
| `seconds` (4, 6 or 8) | `parameters.durationSeconds` |
| `size`, or `resolution` and `aspect_ratio` | `parameters.resolution` (`720p`, `1080p`, `4k`), `parameters.aspectRatio` (`16:9`, `9:16`) |
| `seed`, `negative_prompt`, `audio`, `count` (1 to 4) | `seed`, `negativePrompt`, `generateAudio`, `sampleCount` |
| other instance and parameter fields | extras |

Images by web link are refused: the platform takes bytes or `gs://` links.

## Reply

| Veo | KPP |
| --- | --- |
| `name` | `VideoJob.id` |
| `done: false` or absent | `in_progress` |
| `response.videos[]` | `completed`, an output per video |
| `error` `{code, message}` | `failed` with that error |
