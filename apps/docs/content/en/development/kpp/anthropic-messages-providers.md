---
title: Messages providers
description: How the Messages codec adapts its requests to each provider that speaks the format, and what each one adds.
order: 4
nav: Messages providers
---

Other providers than Anthropic speak [Anthropic Messages](anthropic-messages.md) with differences of their own. Look up here how the codec adapts each request to a provider, and what each provider adds, before you add a provider or change its profile.

Providers other than Anthropic that serve Messages record into `fixtures/anthropic_messages/providers/<id>/` ([fixtures](fixtures.md)). Their extra fields follow the rules of [Anthropic Messages](anthropic-messages.md); this page names what each one adds.

Each provider in `PROVIDERS` names a `kaeso_codecs::anthropic_messages::MessagesProfile` in its `messages` field, and the gateway hands it to the encoder with `EgressCtx::with_messages_profile`, which applies it to every request, whatever format the app sent. `MessagesProfile::ANTHROPIC`, the default, sends requests as the app wrote them; `MessagesProfile::compatible(name)` is the same with the provider's name. Custom providers get `ANTHROPIC`.

| Setting | What the encoder does | Why it is allowed |
|---|---|---|
| `refusing(fields)` | Refuses a request that sets one of these fields, by their Anthropic names: `top_k`, `stop_sequences` (named as the app wrote it, such as a Chat app's `stop`), or a top-level field of an Anthropic app, such as `container`: `` `top_k` is not supported by Example Cloud; remove it from the request. `` A field that may be left out when a request crosses formats, such as Claude Code's `context_management` that only clears thinking ([extras](cross-format.md#extras)), is left out instead and named in `kaeso-left-out`; any other value of it is refused with the hint for Claude Code. | These fields are semantic, and a provider that accepts them without acting on them would drop them silently. |
| `sampling_fixed_at(values)` | Refuses a `temperature` or `top_p` other than the value the provider always samples at (`FixedValue`, in thousandths); the value itself goes as written. | The provider ignores the field, so only its own value is honoured. |
| `auto_tool_choice_only()` | Refuses a `tool_choice` other than `auto`, from every format. | The provider would choose tools itself whatever the app asked for. |
| `with_video(media_types)` | Sends `Block::Video` as a `video` block; a base64 source of another media type is `Unsupported` (`` video media type `…` ``), and a file id or text source too. | Only a provider that documents the block gets it. |
| `raw_predict()` | Sends a generation to `{base_path}/{model}:rawPredict` (`:streamRawPredict` when it streams) without `model`, and a token count to `{base_path}/count-tokens:rawPredict` with it; both bodies get `anthropic_version` `vertex-2023-10-16` unless the app sent its own. Errors in Google's shape are read as Gemini errors. | How Google's platform serves Claude ([use Claude](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/use-claude), [count tokens](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/count-tokens), Anthropic's SDK). Gemini Enterprise Agent Platform uses it and refuses `mcp_servers` and `container`, which [Anthropic lists](https://platform.claude.com/docs/en/build-with-claude/claude-models-on-google-cloud/google-managed) as unavailable there. |

Refusals happen while the request is adapted for the target, before it is encoded, so nothing is sent and the refusal reaches the app in its own format.

| Provider | Settings | Sources |
|---|---|---|
| MiniMax (`minimax`) | refuses `top_k`, `stop_sequences`, `mcp_servers`, `container` and `context_management`; `sampling_fixed_at([("temperature", 1), ("top_p", 0.95)])`; `auto_tool_choice_only()`; `with_video(MINIMAX_VIDEO_MEDIA_TYPES)` (MP4, AVI, MOV, MKV, from `MediaSource`) | MiniMax's [Anthropic API compatibility](https://platform.minimax.io/docs/api-reference/text-anthropic-api) page, read 2026-11-08, whose table lists `temperature`, `top_p`, `top_k`, `stop_sequences`, `mcp_servers`, `context_management` and `container` as ignored, says MiniMax always uses a `temperature` of 1 and a `top_p` of 0.95, and supports only `auto` for `tool_choice` |

## Vercel

Sources: Vercel's [Anthropic Messages API](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api) and [extended thinking](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api/reasoning) pages, read 2026-10-29, and the live recordings in `fixtures/anthropic_messages/providers/vercel/`. A Vercel model is sent as Messages when its **Sends as** value says so; Vercel's `generation` dialect is Chat Completions.

| Vercel field | KPP | Notes |
|---|---|---|
| `provider_metadata` on the reply | extras | Anthropic apps get it back unchanged. |
| `thinking` blocks without `signature` | `Block::Reasoning` | Anthropic apps get them as Vercel sent them. |
| errors in OpenAI's envelope | `KppError` | See [errors](anthropic-messages.md#errors). |
| `POST /v1/messages/count_tokens` | — | Vercel counts exactly only for Claude models and estimates the rest, so Kaeso refuses token counts for every Vercel model. |

## Z.ai

Sources: Z.ai's [error codes](https://docs.z.ai/api-reference/api-code), [quick start](https://docs.z.ai/devpack/quick-start) and [thinking settings](https://docs.z.ai/devpack/latest-model) pages, read 2026-11-08, and the errors recorded and probed live in `fixtures/anthropic_messages/providers/zai/`. Messages is Z.ai's `generation` dialect, at `https://api.z.ai/api/anthropic`. Z.ai documents no Anthropic reply, stream or list of fields it takes, so requests go as they would to Anthropic, nothing is refused for Z.ai, and Z.ai's own errors reach the app. Its thinking settings page names `thinking.type` `adaptive` and `output_config.effort`, which another dialect's effort level becomes.

| Z.ai field | KPP | Notes |
|---|---|---|
| Z.ai's numeric code as `error.type`, in OpenAI's envelope (`errors/recorded-invalid-api-key`) | `KppError` kind | Z.ai answers some codes with a status that means something else in Anthropic's table, such as 429 for an empty balance. The code wins: `1113`, `1309` and `1314` → `Billing`; `1211` → `NotFound`; `1220`, `1311` and `1315` → `PermissionDenied`; `1261` → `ContextTooLong`; `1301` → `ContentFiltered`; `1305` → `Overloaded`; `1000`, `1001`, `1003` and `1005` → `Authentication`. Other codes follow the status. |
| `{"code", "msg", "success": false}`, also with HTTP 200 (`errors/list-invalid-api-key`) | `KppError` | Z.ai's platform envelope, which answers a path it doesn't serve, `GET /v1/models` among them, before it checks the key. `decode_models` and `decode_model` read it as an error whatever the status, so a model check with a wrong key reports the refused key. |
| `POST /v1/messages/count_tokens` | — | Not documented; Kaeso refuses token counts for Z.ai models before sending anything. |
## MiniMax

Sources: MiniMax's [Anthropic API compatibility](https://platform.minimax.io/docs/api-reference/text-anthropic-api) page and its [OpenAPI document](https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json), read 2026-11-08, the examples from that document in `fixtures/anthropic_messages/providers/minimax/`, and one live error recorded with a made-up key. Messages is MiniMax's `generation` dialect, at `https://api.minimax.io/anthropic`, for `MiniMax-M3` and `MiniMax-M3.1-Flash`.

| MiniMax field | KPP | Notes |
|---|---|---|
| reply `id` of 32 hex digits without `msg_`, no `stop_sequence` | `id`, form `none` | Anthropic apps get the reply as MiniMax sent it (`responses/deep-thinking`). |
| `thinking` blocks with MiniMax's own `signature`, and `signature_delta` | `Block::Reasoning` with an opaque Anthropic signature from the origin `minimax` | MiniMax asks for the blocks back unchanged in later turns. Other formats replay them only to MiniMax ([reasoning blobs](reasoning.md#reasoning-blobs)); an Anthropic app's own replay goes as written, and Anthropic refuses MiniMax's signatures. |
| `service_tier` on `message_start` | extras | `streams/image-url-stream`. |
| `video` content blocks (request), base64 or URL, also `mm_file://` | `Block::Video` | MiniMax's own block (`requests/video-url`); only MiniMax's profile sends it, Anthropic refuses it, and Gemini takes it as `inlineData` or a `fileData` URL. |
| `thinking.type` `adaptive` and `output_config.effort` (request) | `reasoning` | MiniMax documents only `adaptive` and `disabled`; `MiniMax-M3.1-Flash` refuses `disabled` with 400, which reaches the app. |
| `POST /v1/messages/count_tokens` | — | MiniMax's documented answer puts the counts under `usage` instead of at the top level, so Kaeso refuses token counts for MiniMax models before sending anything. |
| `GET /v1/models`, `GET /v1/models/{model_id}` | `ModelList`, `ModelInfo` | Anthropic's shape (`models/list-documented`, `models/retrieve-documented`); a wrong key gets 401, so **Check availability** spots it. |
| errors | `KppError` | Anthropic's shape with a `request_id` of MiniMax's own (`errors/recorded-invalid-api-key`). |

## Fireworks AI

Sources: Fireworks' [Create a Message](https://docs.fireworks.ai/api-reference/anthropic-messages) reference and [Anthropic compatibility](https://docs.fireworks.ai/tools-sdks/anthropic-compatibility) page, read 2026-11-09, and `fixtures/anthropic_messages/providers/fireworks-ai/`, built from them, with the error answered live without a valid key. A Fireworks model sends as Messages when its **Sends as** value says so; the key goes as `Authorization: Bearer`.

| Fireworks field | KPP | Notes |
|---|---|---|
| `output_config.effort`, `thinking` with `budget_tokens` | `reasoning` | Fireworks maps both to its `reasoning_effort`. |
| `message_start` with zero usage, the counts on the one `message_delta` | `Usage` | As the compatibility page describes; `cache_creation_input_tokens` is always 0. |
| `document` blocks, `image` blocks with a file source, server tools, a forced `tool_choice` or a prefill while thinking | sent | Fireworks answers 400, which reaches the app. |
| errors in Anthropic's envelope with a `request_id` (`errors/recorded-invalid-api-key`) | `KppError` | An unknown model or path gets Fireworks' own `{"error": {"code": …}}` body instead, read like OpenAI's. |
| `POST /v1/messages/count_tokens` | — | Fireworks answers 404, so Kaeso refuses token counts before sending anything. |

## Next steps

- [Anthropic Messages](anthropic-messages.md): Find where every field of the Anthropic Messages format goes in KPP.
- [Fixtures and recordings](fixtures.md): Record a provider's fixtures with `cargo xtask record anthropic_messages --provider <id>`.
