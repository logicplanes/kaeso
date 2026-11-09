---
title: Gemini Interactions
description: How KPP's video jobs map to Gemini Omni on the Interactions API of Google AI Studio and Gemini Enterprise Agent Platform.
order: 8
---

Look up how a video job reaches Gemini Omni on Google AI Studio and Gemini Enterprise Agent Platform here: which field goes where, how the job is asked about, and the source of each mapping. Only the video part of the Interactions API is mapped.

- **Codec:** `kaeso_codecs::gemini_interactions::GeminiInteractions` (`crates/kaeso-codecs/src/gemini_interactions/`)
- **Paths:** `POST /v1beta/interactions`, `GET /v1beta/interactions/{id}` and `DELETE /v1beta/interactions/{id}` on Google AI Studio; `POST` and `GET` under `/v1beta1/projects/{project}/locations/global/interactions` on the Agent Platform
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini_interactions/`; `responses/recorded-started.json` and `polls/recorded-failed-quota.json` were recorded with a free-tier key, the rest built from the reference

**Sources.** Google AI Studio's [Gemini Omni Flash](https://aistudio.google.com/docs/omni) and [create interaction](https://aistudio.google.com/api-reference/interactions-api/create-interaction) references, and the Agent Platform's [Generate videos from text](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text), read 2026-11-09.

## Decisions

- **Not `generateContent`.** The models list `generateContent`, but Google documents Omni's video only through the Interactions API, so Kaeso sends video jobs there.
- **In the background.** Every job is sent with `background: true`, which answers at once with the interaction's id; asking for it later returns the video inline.
- **The task follows the images.** First and last frames send `generation_config.video_config.task: "image_to_video"`, reference images `"reference_to_video"`; without images the model infers the task.
- **A remix continues the interaction.** `previous_interaction_id` refines the earlier video with the new prompt, as Google's stateful editing does.
- **The Agent Platform: global, service accounts, no delete.** Omni is served there only in the `global` location, so Kaeso sends it there whatever location the key names. Express mode lists no Omni models, so only a service account reaches it. Its reference documents no delete, so Kaeso forgets a deleted video. Kaeso sends no `gcs_uri`, so the video comes back inline, as on Google AI Studio.
- **Omni always makes sound.** `audio: false` is refused (param `audio`); `audio: true` sends nothing.

## Request

| KPP | Interactions API |
| --- | --- |
| `model` | `model` |
| `prompt` | `input`, a string, or a `text` item after the images |
| images | `input[]` `{type: "image", data, mime_type}` or `{uri}` |
| `seconds` (3 to 15) | `response_format.duration`, such as `"8s"` |
| `size`, or `resolution` and `aspect_ratio` | `response_format.resolution` (`360p`, `720p`, `1080p`, `4k`), `response_format.aspect_ratio` (`16:9`, `9:16`) |
| a remix's `remix_of` | `previous_interaction_id` |

Omni takes no seed, negative prompt or count, and no frames together with references. The first preview, `gemini-omni-flash-preview`, starts from no first or last frame either; it takes reference images.

## Reply

| Interactions API | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` `in_progress` | `in_progress` |
| `status` `completed` with a video | `completed` |
| `status` `failed`, `cancelled`, `incomplete`, or `completed` without a video | `failed`, the first of `errors[]` as the error |
| `steps[]` `model_output` `video` items | outputs, inline bytes or links |
| `usage` `total_input_tokens`, `total_output_tokens`, `total_thought_tokens` | `usage`; thought tokens count as output and reasoning |
| `created`, `updated` | `created_at`, and `completed_at` once it ended |
