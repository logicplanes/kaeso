---
title: Chat Completions inference clouds
description: What the inference clouds that host open models add to Chat Completions, and how the codec meets them.
order: 6
nav: Inference clouds
---

Inference clouds that host open models speak [OpenAI Chat Completions](openai-chat.md) with differences of their own. Look up here what each one adds before you change its profile; their profiles are in the table of [provider profiles](openai-chat-providers.md#provider-profiles), and the other providers are on [Chat Completions providers](openai-chat-providers.md).

Their fixtures are in `fixtures/openai_chat/providers/<id>/` ([fixtures](fixtures.md)).

## Amazon Bedrock

Sources: AWS's [endpoints](https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html), [Chat Completions](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions.html), [Messages](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html) and [API keys](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html) guides and its [API compatibility](https://docs.aws.amazon.com/bedrock/latest/userguide/models-api-compatibility.html) table, read 2026-11-14. Not tested with a successful call: AWS publishes no reply of these routes, so `fixtures/openai_chat/providers/amazon-bedrock/` holds the guides' requests and errors answered live with made-up keys. Each key calls one region, at `https://bedrock-runtime.{region}.amazonaws.com`, with `Authorization: Bearer`; the provider's `Regional` entry in `provider.rs` lists the regions and gives each dialect its API root there.

| Amazon Bedrock field | KPP | Notes |
|---|---|---|
| Chat Completions at `/openai/v1/chat/completions` | `EgressCtx::api_root` | `/openai/v1` takes the place of `/v1`; the body is OpenAI's. Claude models go to `/anthropic/v1/messages` instead ([Messages providers](anthropic-messages-providers.md#amazon-bedrock)). |
| `amazon-bedrock-guardrailConfig` | extras | Bedrock's own body field for a guardrail (`requests/guardrail-config-doc`); refused when the request goes to another dialect. |
| 401 `permission_denied_error` with `code: "access_denied"` | `KppError` | `Authentication` from the status; a malformed, an unsigned and an unknown key each have a message of their own (`errors/recorded-*-key*`). |
| `{"Message": …}` with `x-amzn-ErrorType` | `KppError` | The shape bedrock-runtime answers its own operations with; the message is read from `Message`, the kind from the status (`errors/recorded-native-invalid-api-key`). |
| `GET /openai/v1/models` | — | Not served (404 `UnknownOperationException`), so a model check is inconclusive and sends nothing. |

A model the catalogue names no format for is served only through Bedrock's own InvokeModel and Converse APIs, whose AWS event streams KPP doesn't read, and is refused before sending. Bedrock's Responses route is unused: Responses apps reach it through Chat Completions. Token counts are refused: bedrock-runtime counts them only in its own format, at `/model/{modelId}/count-tokens`.

## Baseten

Sources: Baseten's [Model APIs](https://docs.baseten.co/inference/model-apis/overview) overview, its Chat Completions, Messages and Responses OpenAPI documents, its [reasoning](https://docs.baseten.co/inference/model-apis/reasoning) and [streaming](https://docs.baseten.co/inference/streaming) guides and its [inference errors](https://docs.baseten.co/inference/errors) page, read 2026-11-10. Not tested with a successful call: the reply in `fixtures/openai_chat/providers/baseten/` is the reasoning guide's own example and the stream the streaming guide's own excerpt, completed from the schema; the error was answered live, without a valid key. Kaeso's base URL is `https://inference.baseten.co`, which takes `Authorization: Bearer`.

| Baseten field | KPP | Notes |
|---|---|---|
| `message.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Thinking that an app sends back reaches Baseten as `reasoning_content`, which Beam requires on earlier assistant messages. |
| `chat_template_args`, `reasoning` with `max_tokens`, `top_k`, `min_p`, `repetition_penalty` and Baseten's other request fields | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `max_completion_tokens` | `max_tokens` | The schema has only `max_tokens` (`max_tokens()`). |
| the `developer` role | `Role::System` | The schema lists `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| `user` | `user` | The schema has no `safety_identifier`, so the end user's id goes as `user` (`end_user_as_user()`). |
| errors as `{"error": "<message>"}` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| a stream that ends without `[DONE]` | `KppError` | Baseten can end a stream early without an error event; apps get the stream decoder's error that the stream ended before the reply was complete. |

Baseten's Messages endpoint isn't used: its schema takes only text and tool blocks, without thinking or images. Its Responses API, which stores nothing, isn't used either: Messages and Responses apps reach Baseten through Chat Completions. Model APIs serve no embeddings, images, audio or token counts.

## Cerebras

Sources: Cerebras's [Chat Completions reference](https://inference-docs.cerebras.ai/api-reference/chat-completions) and [OpenAI compatibility](https://inference-docs.cerebras.ai/resources/openai) pages, read 2026-11-02, and the live recordings in `fixtures/openai_chat/providers/cerebras/`, made with `gpt-oss-120b` and `qwen-3.8-27b`. Kaeso's base URL is `https://api.cerebras.ai`, which takes `Authorization: Bearer`.

| Cerebras field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`, as for Groq. Thinking that an app in another format sends back reaches Cerebras as `reasoning`, which it takes (`responses/recorded-reasoning-sent-back`). A Chat app's `reasoning_content`, such as Grok CLI's, gets 400 from Cerebras, so it goes as `reasoning` too (`replay_reasoning_as_read()`). |
| `time_info`, `system_fingerprint`, `choices[].reasoning_logprobs` | extras | Left out towards other dialects. |
| `completion_tokens_details.accepted_prediction_tokens` and `rejected_prediction_tokens`, `prompt_tokens_details.image_tokens` | `raw` usage | The cached and reasoning tokens are typed as in [usage](openai-chat.md#usage-completionusage). |
| `usage` and `time_info` on the finish chunk, also without `stream_options.include_usage` | `Usage` | Cerebras never sends a chunk with `choices: []`. |
| `reasoning_effort` | `reasoning.effort` | `gpt-oss-120b` takes `low`, `medium` and `high`, and `qwen-3.8-27b` also `none`, as KPMR lists; Kaeso refuses other levels before sending, and Cerebras answers them with 400 (`errors/recorded-reasoning-effort-*`). `max_completion_tokens` counts the reasoning. |
| `reasoning_format`, `clear_thinking` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| fields Cerebras doesn't declare, such as `safety_identifier`, `metadata`, `store` or `web_search_options` | sent | Cerebras answers 400 `property '…' is unsupported`, which reaches the app. The profile sends the end user's id as `user` (`end_user_as_user()`), so Claude Code's `metadata.user_id` arrives there. |
| `logit_bias`, `logprobs`, `top_logprobs`, both penalties, `seed`, `stop`, `prompt_cache_key`, `parallel_tool_calls`, `service_tier`, `prediction` | sent | Cerebras takes them (`responses/recorded-logit-bias-honoured` shows the bias at work). A `stop` sequence also ends the reasoning, which can leave a reply without `content` (`responses/recorded-sampling`). |
| the `developer` role, a `system` message after the first turn | `Role::System` | Qwen's chat template refuses both with 400. `system_role()` sends `system`, and `leading_system_only()` moves the text of a later system message from an app in another format, such as Claude Code or Codex, into the next user turn. |
| a final assistant message | refused | Cerebras answers it anew and repeats it (`responses/recorded-prefill`), so `without_prefill()` refuses it before sending. |
| `refusal` on an assistant message, also `null` | refused | Cerebras answers 400 (`errors/recorded-assistant-refusal`); the profile refuses it before sending. |
| a function without `parameters` | `parameters` | `with_tool_parameters()` sends the empty object schema that `gpt-oss-120b`'s chat template requires (`errors/recorded-tool-without-parameters`). |
| `tools` together with `response_format` on `gpt-oss-120b`, an image as an `https` URL or to a model without images | sent | Cerebras answers 400 (`errors/recorded-tools-with-response-format`, `errors/recorded-image-url`, `errors/recorded-image-gpt-oss`), which reaches the app. |
| errors with OpenAI's `message`, `type`, `param` and `code` at the top level, without the `error` object | `KppError` | The message and `param` are Cerebras's, and the kind follows the status: a wrong key (401 `wrong_api_key`) is a refused key, and the account's tokens-a-minute limit (429 `too_many_tokens_error`, `retry-after: 60`) a rate limit. Chat apps get the error in OpenAI's shape. |
| `GET /v1/models` | `ModelList` | The models the key may call, with `created: 0` and `owned_by: "Cerebras"`; a wrong key gets 401, so a model check spots it. |

Cerebras has no Responses, Messages, embeddings or token-count endpoint (all answer 404), so `embeddings` and `count_tokens` are `None`.

## Azure

Sources: Azure's v1 OpenAPI document (`azure-v1-v1-generated.json` in [azure-rest-api-specs](https://github.com/Azure/azure-rest-api-specs/tree/main/specification/ai/data-plane/OpenAI.v1)) and the [API lifecycle](https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle), [endpoints](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints), [content streaming](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/content-streaming) and [content filter](https://learn.microsoft.com/en-us/azure/foundry-classic/openai/concepts/content-filter-annotations) pages, read 2026-11-14. No key was available: one reply in `fixtures/openai_chat/providers/azure/` is a reply Azure sent, published in a Stack Overflow answer, and the others and the streams are the documentation's samples; the key errors were answered live, with a made-up key. Each key carries its resource's endpoint, such as `https://example.openai.azure.com`, and Kaeso calls `{endpoint}/openai/v1/…` with the key in `api-key`, without an `api-version`.

| Azure field | KPP | Notes |
|---|---|---|
| `model` | the deployment's name | The model's `upstream_model` when it has one, its id otherwise. |
| `prompt_filter_results` (or `prompt_annotations`), `choices[].content_filter_results` | extras | Round-trip for Chat apps, left out towards other dialects (`responses/greeting-published`). |
| a first chunk with empty `id`, `object` and `model` and `choices: []` | start | The reply's id and model come from the next chunk (`streams/filter-pass-doc`). |
| chunks with only `content_filter_results` and `content_filter_offsets` in a choice | extras | Also after `finish_reason: "stop"`; they carry no text. |
| `finish_reason: "content_filter"` | `StopReason::ContentFilter` | Also on a chunk without `delta` (`streams/filter-blocked-doc`), and on a reply without `content` (`responses/filtered-blocklist-doc`). |
| a 400 with `code: "content_filter"` and `innererror` | `KppError` | `ContentFiltered`, with Azure's message (`errors/content-filter-prompt-doc`). |
| `error.code` as a numeric string, such as `"401"` | `KppError` | The kind follows the status (`errors/recorded-invalid-api-key`). |
| `{"statusCode": 401, "message": …}` | `KppError` | Azure's answer to an Entra ID token it doesn't accept (`errors/recorded-entra-token`); for a key that signs in with Entra ID, a refused key that names the role the service principal needs. |
| `GET /openai/v1/models` | `ModelList` | The models a resource may deploy, not its deployments, so a model check only shows whether the key and its endpoint work (`ModelListing::Catalogue`). |

The v1 API serves Responses with OpenAI's request body, plus `content_filters` in replies, which stay in extras; Kaeso sends a model as Responses when its **Sends as** value says so (`fixtures/openai_responses/providers/azure/`). Claude deployments answer only Anthropic's Messages API under `{endpoint}/anthropic`, and images, speech, transcriptions and videos are in the v1 API's preview only, so Kaeso calls none of them yet. Token counts are refused.

## DeepInfra

Sources: DeepInfra's [OpenAPI document](https://api.deepinfra.com/openapi.json) and its [Chat Completions](https://docs.deepinfra.com/chat/overview.md), [streaming](https://docs.deepinfra.com/chat/streaming.md), [tool calling](https://docs.deepinfra.com/chat/tool-calling.md), [prompt caching](https://docs.deepinfra.com/chat/prompt-caching.md) and [authentication](https://docs.deepinfra.com/account/authentication.md) guides, read 2026-11-14. No key was available: one reply in `fixtures/openai_chat/providers/deepinfra/` is a reply DeepInfra sent, published in another project's issue, and the others and the stream are built from the guides; the listing and the key errors were answered live, without a valid key. DeepInfra's guides write the base URL as `https://api.deepinfra.com/v1/openai`; its OpenAPI document serves the same API at `/v1/…`, so Kaeso's base URL is `https://api.deepinfra.com`, which takes `Authorization: Bearer`.

| DeepInfra field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`; assistant messages take it back. |
| `max_completion_tokens` | `max_tokens` | The schema has only `max_tokens` (`max_tokens()`). |
| the `developer` role | `Role::System` | The schema lists `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| `user` | `user` | The schema has no `safety_identifier`, so the end user's id goes as `user` (`end_user_as_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()`. |
| `parallel_tool_calls`, `top_logprobs`, `logit_bias`, `metadata`, `store` and the other fields the schema leaves out | refused | DeepInfra drops undeclared fields without a word, so the profile refuses them; `parallel_tool_calls: true` only allows parallel calls and is left out, as is a [neutral value](sampling.md#neutral-values) such as `store: false`. |
| `top_k`, `min_p`, `repetition_penalty`, `fail_fast`, `models`, `prompt_cache_options`, `cache_salt`, `chat_template_kwargs` and the `reasoning` object | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `usage.estimated_cost`, `prompt_tokens_details` with `cache_write_tokens`, `name: null` | extras and raw usage | Left out towards other dialects (`responses/greeting-published`, `responses/cached-doc`). |
| a 403 `{"detail": {"error": …}}` for a key without the `inference` scope | `KppError` | `PermissionDenied`, with DeepInfra's message (`errors/missing-scope-doc`). |
| `GET /v1/models` with `root`, `parent` and DeepInfra's `metadata` | `ModelList` | Answered without a key (`models/list-recorded`), so a model check can't spot a wrong key. There is no `GET /v1/models/{model}`. |

DeepInfra's Responses API stores nothing and isn't used: Responses apps reach DeepInfra through Chat Completions. Its Anthropic-compatible endpoint and token count live under `/anthropic/v1`, outside the `/v1` of the other formats, so Kaeso refuses token counts. Its images, speech, transcriptions and videos in OpenAI's formats aren't used until KPMR lists the models with their prices.

## Fireworks AI

Sources: Fireworks' [Create Chat Completion](https://docs.fireworks.ai/api-reference/post-chatcompletions) reference (its OpenAPI document), [OpenAI compatibility](https://docs.fireworks.ai/tools-sdks/openai-compatibility) and [reasoning](https://docs.fireworks.ai/guides/reasoning) pages, read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/fireworks-ai/` are built from them; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.fireworks.ai/inference`, which takes `Authorization: Bearer` with a `fw_` key.

| Fireworks field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Thinking that an app in another format sends back reaches Fireworks as `reasoning_content`, which its reasoning guide asks for during tool calls. The documentation describes no `<think>` tags in `content`. |
| `reasoning_effort` with `xhigh`, `max`, booleans or a token count; `thinking` in Anthropic's form; `reasoning_history` | `reasoning.effort`, extras | The catalogue's effort levels decide what other formats send; a Chat app's own values go as written. |
| `top_k`, `min_p`, `typical_p`, `repetition_penalty`, `mirostat_*`, `context_length_exceeded_behavior`, `prompt_cache_isolation_key`, `perf_metrics_in_response`, `raw_output` and Fireworks' other request fields | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `max_tokens` beyond what the context window leaves | sent | Fireworks lowers it instead of refusing it, unless the request sets `context_length_exceeded_behavior: "error"`. |
| `user` | `user` | Fireworks' schema has no `safety_identifier`, so the profile sends the end user's id as `user` (`end_user_as_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()`; Fireworks' video models take them (`requests/video-url`). |
| `perf_metrics` on a reply or the finish chunk | extras | Left out towards other dialects. |
| usage by default in a trailing chunk with `choices: []` | `Usage` | Also without `stream_options.include_usage`. |
| errors in OpenAI's `error` object with an upper-case `code` such as `UNAUTHORIZED`, `type: "error"` and a top-level `request_id` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | A wrong key gets 401 (`errors/list-invalid-api-key`), so a model check spots it. Fireworks documents no listing on this path; the console takes its models from KPMR. |

Fireworks documents no image, speech, transcription, video or token-count endpoint, so those stay empty and `count_tokens` is `None`. Its Responses API reports usage as `prompt_tokens` and `completion_tokens` instead of OpenAI's names, so Kaeso doesn't send Responses to it; Responses apps reach Fireworks through Chat Completions.

## Groq

Sources: Groq's [API reference](https://console.groq.com/docs/api-reference) and [reasoning guide](https://console.groq.com/docs/reasoning), read 2026-10-19, and the live recordings in `fixtures/openai_chat/providers/groq/`.

| Groq field | KPP | Notes |
|---|---|---|
| `reasoning_format`, `include_reasoning` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `user`, `safety_identifier` (request) | KPP `user` | Groq takes `user` and refuses `safety_identifier`, so the profile sends the end user's id as `user` (`end_user_as_user()`). |
| `prompt_cache_key` (request) | extras | Groq refuses it, so a Chat app's own key is refused before sending, and an OpenAI Responses app's key, which Codex sends on every request, is left out and named in `kaeso-left-out` (`without_cache_key()`). |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). |
| `delta.channel` (`analysis` on GPT-OSS reasoning) | extras of the chunk | Chat apps get it back with the reasoning; left out towards other dialects. |
| `x_groq`, `usage_breakdown`, `service_tier` (`on_demand`) | extras | Left out towards other dialects. |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time` | `raw` usage | The token counts, `reasoning_tokens` included, are typed as in [usage](openai-chat.md#usage-completionusage). |
| `usage` on the finish chunk, also without `stream_options.include_usage` | `Usage` | Every dialect gets the usage, whether or not Groq also sends the usage chunk that `include_usage` asks for. |
| extra model fields (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features` and others) | model extras | |

## Hugging Face

Sources: Hugging Face's [Inference Providers](https://huggingface.co/docs/inference-providers/index) overview, its [chat completion](https://huggingface.co/docs/inference-providers/tasks/chat-completion) reference with the schemas in `huggingface.js`, and its [Claude Code](https://huggingface.co/docs/inference-providers/integrations/claude-code) and [Responses API](https://huggingface.co/docs/inference-providers/guides/responses-api) guides, read 2026-11-10. Not tested with a successful call: the reply and the stream in `fixtures/openai_chat/providers/huggingface/` are built from the reference, with the thinking in the field the provider behind the router documents; the model listing and the error were answered live, without a valid key. Kaeso's base URL is `https://router.huggingface.co`, which takes `Authorization: Bearer` with an `hf_` token.

| Hugging Face field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `message.reasoning` and their deltas | `Block::Reasoning` | Through the profile's `reasoning_fields`; the router passes on the field of the provider behind it. |
| `max_completion_tokens` | `max_tokens` | The schema has only `max_tokens` (`max_tokens()`). |
| the `developer` role | `Role::System` | `role` goes on as written to providers that don't all take `developer`, so `system_role()` sends `system`. |
| an end user's id | left out | The schema has no `user` or `safety_identifier` (`without_end_user()`). |
| a `:provider`, `:fastest`, `:cheapest` or `:preferred` suffix on the model id | `model` | Sent as written; the reply names the model without it (`responses/provider-suffix`). |
| `tool_prompt` | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| errors as `{"error": "<message>"}` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` with `architecture` and a `providers` array of status, context length, pricing and speed | `ModelList` | Answered without a key (`models/list-recorded`), so a model check can't spot a wrong key. |

The router's OpenAI-compatible API is for chat only: `/v1/embeddings`, `/v1/images/generations` and `/v1/audio/*` answer 404. Its Responses API isn't used, as it answers a refused token with 200 and `status: "failed"`. Its Messages endpoint isn't used either: it answers a refused token with `api_error`, which would reach apps as a failed provider instead of a refused key. `/v1/messages/count_tokens` answers without checking the token and isn't documented, so Kaeso refuses token counts.

## IONOS AI Model Hub

Sources: IONOS's [create chat completion](https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/chat-completions/create-chat-completion) reference (OpenAPI 3.0.3), its [error codes](https://docs.ionos.com/cloud/ai/ai-model-hub/error-codes) and its model pages, read 2026-11-14. No token was available, so the replies in `fixtures/openai_chat/providers/ionos/` are the model pages' examples and the stream is built around the final chunk the docs show; the key errors were answered live, without a valid token. Kaeso's base URL is `https://openai.inference.de-txl.ionos.com`, in Berlin, which takes `Authorization: Bearer` with an IONOS Cloud API token.

| IONOS field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `message.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`; GPT OSS sends the same thinking in both, Qwen3.8 only in `reasoning` (`responses/gpt-oss-reasoning-doc`, `responses/qwen-reasoning-doc`). |
| the `developer` role | `Role::System` | Only some models take `developer`; IONOS advises `system` (`system_role()`). |
| `user` | `user` | The schema has no `safety_identifier` (`end_user_as_user()`). |
| `prompt_cache_key` | left out | IONOS caches prefixes by itself and has no such field (`without_cache_key()`). |
| `store`, `metadata`, `service_tier` and the other fields the schema doesn't declare | refused | The docs don't say whether IONOS refuses or drops them. |
| `stop_reason`, `token_ids`, `prompt_logprobs`, `prompt_token_ids`, `kv_transfer_params` and the `null` message fields | extras | Left out towards other dialects (`responses/mistral-image-doc`). |
| `{"httpStatus", "messages": [{"errorCode", "message"}]}` | `KppError` | IONOS's own error shape, the messages joined; the kind follows the status (`errors/recorded-invalid-api-key`, `errors/validation-error-doc`). Only 529 comes in OpenAI's shape. |
| `GET /v1/models` | `ModelList` | It needs a token and gives only ids; there is no `GET /v1/models/{model}`. |

IONOS's Responses API keeps no state and isn't used: Responses apps reach IONOS through Chat Completions. Its images (FLUX), transcriptions (Whisper) and rerank aren't used until KPMR lists those models; it serves no speech, video, Messages or token counts.

## Nebius Token Factory

Sources: Nebius' [OpenAPI document](https://api.tokenfactory.nebius.com/openapi.json) (version `20261105-4c58718c5`), its [text generation](https://docs.tokenfactory.nebius.com/api-reference/examples/text-generation) example and its [overview](https://docs.tokenfactory.nebius.com/ai-models-inference/overview), read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/nebius/` are built from them, and one is the documentation's own example; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.tokenfactory.nebius.com`, which takes `Authorization: Bearer`.

| Nebius field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content`, `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`; the field depends on the model, as for Together AI. |
| the `developer` role | `Role::System` | Nebius lists only `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| `user` | `user` | Nebius' schema has no `safety_identifier`, so the end user's id goes as `user` (`end_user_as_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()` (`requests/video-url`). |
| vLLM sampling fields such as `top_k` and `repetition_penalty`, `extra_body`, `partial` and Kimi's `tools` on messages | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `service_tier` on every reply, `stop_reason`, `function_call: null`, `tool_calls: []`, `system_fingerprint: null` | extras | Left out towards other dialects (`responses/text-generation-doc`). |
| `response_format` with a JSON schema | sent | Nebius' description allows only `json_object` and `text`; Nebius answers a schema itself. |
| errors with OpenAI's `error` object and a top-level `detail` | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |
| `GET /v1/models` | `ModelList` | A wrong key gets 401 with only `{"detail": "…"}` (`errors/list-invalid-api-key`), which apps get in OpenAI's shape with Nebius' message, so a model check spots it. |

Nebius serves no images, audio, Messages or token counts: its OpenAPI document has no such path, and `/v1/images/generations`, `/v1/audio/transcriptions` and `/v1/messages` answer 404. Its Responses API isn't used: Responses apps reach Nebius through Chat Completions.

## Novita AI

Sources: Novita AI's [chat completion](https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md), [embeddings](https://docs.novita.ai/api-reference/model-apis-llm-create-embeddings.md), [list models](https://docs.novita.ai/api-reference/model-apis-llm-list-models.md) and [error code](https://docs.novita.ai/api-reference/basic-error-code.md) references and its [reasoning](https://docs.novita.ai/guides/llm-reasoning.md), [interleaved thinking](https://docs.novita.ai/guides/llm-interleaved-thinking.md), [prompt cache](https://docs.novita.ai/guides/llm-prompt-cache.md) and [FAQ](https://docs.novita.ai/guides/LLM-FAQ.md) guides, read 2026-11-14. No key was available: the replies in `fixtures/openai_chat/providers/novita-ai/` are examples from the guides, and the stream is built from the reference; the listing, one model and the key errors were answered live, without a valid key. The base URL is `https://api.novita.ai/openai`, under which the reference serves `/v1/chat/completions`, `/v1/embeddings` and `/v1/models`, with `Authorization: Bearer`.

| Novita AI field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Only with `separate_reasoning: true`, which the profile sends (`separate_reasoning()`); without it the reasoning stays inside `content`. A Chat app's own `separate_reasoning` is kept. |
| `enable_thinking` | `ReasoningConfig` | `thinking_switch()`; an Anthropic app's budget goes as `thinking_budget`, which the reference doesn't name. |
| `max_completion_tokens` | `max_tokens` | The reference has only `max_tokens`, which counts reasoning tokens too. |
| the `developer` role | `Role::System` | The reference lists `system`, `user` and `assistant`; its guides send `tool` messages too. |
| `user`, `safety_identifier` | left out | The reference has no end-user field (`without_end_user()`). |
| `video_url` parts | `Block::Video` | `with_video_url()`. |
| `store`, `metadata`, `prompt_cache_key`, `parallel_tool_calls`, `service_tier` and the other fields the reference leaves out | refused | The reference doesn't say whether Novita AI refuses undeclared fields, so the profile refuses them; `parallel_tool_calls: true` is left out. `tool_choice`, also missing from the reference, is sent, as tool calling needs it. |
| `reasoning_details` and `name` on the reply message, `cache_creation_Prompt_tokens` and `cache_read_Prompt_tokens` in `prompt_tokens_details` | extras and raw usage | Left out towards other dialects (`responses/interleaved-tool-call-doc`, `responses/cached-doc`). |
| errors as `{"code", "reason", "message", "metadata"}` | `KppError` | The kind follows the status: 401 `FAILED_TO_AUTH`, 403 `INVALID_API_KEY` or `NOT_ENOUGH_BALANCE`, 404 `MODEL_NOT_FOUND`, 429, 503 (`errors/`). |
| `GET /v1/models` with prices in 1/10,000 USD, `pricing`, `features` and `endpoints` | `ModelList` | Answered without a key and lists chat models only (`models/list-recorded`); `GET /v1/models/{model}` also answers for embeddings models (`models/retrieve-embedding-recorded`), so a model check finds a model but not a wrong key (`ModelListing::Partial`). |

Novita AI's Anthropic-compatible endpoint lives under `/anthropic`, outside its OpenAI base URL, and `/v1/responses` is undocumented, so Kaeso calls only Chat Completions and refuses token counts. Its `/v1/completions`, `/v1/rerank`, batch API, its image model in OpenAI's Images API and its own task APIs for speech, transcription and video aren't used.

## NVIDIA

Sources: NVIDIA's [LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) reference and the OpenAPI documents of its model pages, such as [`nvidia/nemotron-3-super-120b-a12b`](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer), read 2026-11-10. Not tested with a successful call: the reply and the stream in `fixtures/openai_chat/providers/nvidia/` are built from them, the stream's request the reference's own example; the model listing and the errors were answered live, without a valid key. Kaeso's base URL is `https://integrate.api.nvidia.com`, which takes `Authorization: Bearer` with an `nvapi-` key.

| NVIDIA field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. |
| `chat_template_kwargs` (`enable_thinking`, `low_effort`), `reasoning_budget` | extras | The models' own thinking switches; a Chat app's go as written, refused when the request goes to another dialect. |
| `max_completion_tokens` | `max_tokens` | The request schemas have only `max_tokens` (`max_tokens()`). |
| the `developer` role | `Role::System` | The schemas list `system`, `context`, `user` and `assistant`, so `system_role()` sends `system`. |
| an end user's id | left out | The schemas have no `user` or `safety_identifier` (`without_end_user()`). |
| errors as RFC 9457 problem documents with `status`, `title` and `detail` | `KppError` | The kind follows the status: a key NVIDIA doesn't know gets 403 (`errors/recorded-invalid-api-key`), a retired model 410 (`errors/recorded-retired-model`). |
| a 202 with an `NVCF-REQID` header | — | NVIDIA's reply when the result isn't ready, to poll for; Kaeso doesn't poll. |
| `GET /v1/models` | `ModelList` | Answered without a key (`models/list-recorded`), so a model check can't spot a wrong key. |

NVIDIA serves no Responses, Messages or token counts: `/v1/responses` and `/v1/messages/count_tokens` answer 404, and `/v1/messages` answers `model not found` for every model. Its embeddings models take `input_type` and `truncate`, which stay in extras ([OpenAI Embeddings](openai-embeddings.md#nvidia)).

## Ollama Cloud

Sources: Ollama's [cloud](https://docs.ollama.com/cloud), [authentication](https://docs.ollama.com/api/authentication), [OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility) and [errors](https://docs.ollama.com/api/errors) pages, read 2026-10-30, and the live recordings in `fixtures/openai_chat/providers/ollama-cloud/`, made with `gpt-oss:20b` and `gemma4:31b` on the free plan.

| Ollama Cloud field | KPP | Notes |
|---|---|---|
| model ids with a tag, such as `gpt-oss:20b` | `model` | Sent as written, also in `GET /v1/models/{model}`. |
| `max_completion_tokens` (ignored), `max_tokens` (honoured) | `max_output_tokens` | The profile's `max_tokens()` sends every app's output limit as `max_tokens`. |
| `reasoning: {effort}` (request) | extras | Ollama's spelling beside `reasoning_effort`; refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`, as for Groq. Reasoning chunks carry `content: ""` beside it, which opens no text block. Thinking that an app in another format sends back reaches Ollama as `reasoning` on the assistant turn. |
| `system_fingerprint` (`fp_ollama`) | extras | Left out towards other dialects. |
| `usage.prompt_tokens_details.cached_tokens` | `Usage.cache_read_tokens` | Ollama reports no reasoning tokens. |
| the finish chunk with an empty `delta`, then a usage chunk with `choices: []` | `Usage` | As OpenAI sends them. |
| `logprobs`, `top_logprobs`, `logit_bias` | refused | Ollama accepts them and returns nothing for them; the profile refuses them before sending. |
| `tool_choice` other than `auto`, `response_format` | sent | Ollama's cloud models ignored both in a live probe on 2026-10-30; Kaeso sends them as written. |
| a `refusal` content part on an assistant message | sent | Ollama answers 400 `invalid message format` (`errors/recorded-content-array-all-roles`), which reaches the app in its own format. |
| errors in OpenAI's envelope with `type` `api_error`, `not_found_error` or `invalid_request_error` | `KppError` | The kind follows the status: 401 for a wrong key, 402 for a model the plan doesn't include (`errors/recorded-model-outside-plan`), 404 for an unknown model. |
| `GET /v1/models`: OpenAI's list, `owned_by: "ollama"`, and a top-level `models` array in Codex's model-catalogue shape | `ModelList`, extras | Ollama answers the listing and `GET /v1/models/{model}` without checking the key, so a model check can't spot a wrong key. |

## OVHcloud AI Endpoints

Sources: AI Endpoints' [OpenAPI document](https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/gpt-oss-120b/openapi.json) and its [getting started](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-getting-started.md), [capabilities](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-capabilities.md), [function calling](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-function-calling.md) and [structured output](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-structured-output.md) guides, read 2026-11-14. AI Endpoints answers without a key, at 2 requests a minute per IP and model, so the replies, the stream, the listing and the errors in `fixtures/openai_chat/providers/ovhcloud/` were recorded live on 2026-11-14 without a key, the key error with a made-up key. Kaeso's base URL is `https://oai.endpoints.kepler.ai.cloud.ovh.net`, the documented base without its `/v1`, which takes `Authorization: Bearer`. OVHcloud hosts the models in France.

| AI Endpoints field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`; assistant messages take it back (`requests/recorded-tool-result`). A reply cut while thinking has no `content` (`responses/recorded-thinking-length`). |
| the `developer` role | `Role::System` | Mistral Small 3.2 answers it with 400 (`errors/recorded-developer-role-mistral`), so `system_role()` sends `system`. |
| `user`, `safety_identifier` | left out | `user` gets 400, and the schema declares no other end-user field (`without_end_user()`). |
| `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_key`, `web_search_options`, `functions`, `function_call` | refused | Not declared, and the parser lets at least `metadata`, `store`, `prediction`, `prompt_cache_key` and `web_search_options` through without a word; a Responses app's cache key is left out (`without_cache_key()`). |
| other undeclared fields, such as `service_tier`, `verbosity` and `top_k` | sent as written | AI Endpoints answers 400 naming each of them (`errors/recorded-service-tier`). |
| `tool_calls: []`, `"name": "assistant"` on deltas, `system_fingerprint` on the usage chunk | extras | Left out towards other dialects. |
| a bare `{"message": …}`, or vLLM's error as JSON text inside it | `KppError` | The message is quoted with the status, the nested `error.message` read out of the text. A 403 "Forbidden: authentication failed." is a refused key (`refused_key_messages`, `errors/recorded-invalid-api-key`). |
| `GET /v1/models` with `pricing` in USD per token, `context_length` and `max_completion_tokens` | `ModelList` | Lists every model kind and refuses a wrong key with 403 (`models/list-recorded`). |

AI Endpoints' Responses API needs `store: false` and isn't used: Responses apps reach it through Chat Completions. It serves no Messages or token counts. Model ids are case-sensitive; a lowercase alias is answered with the listed id in `model`, so KPMR lists the ids `GET /v1/models` names. Its transcriptions aren't used until KPMR lists the models with their prices.

## SambaNova

Sources: SambaNova's [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) (SambaNova cloud API 1.2.0), its [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility), [text generation](https://docs.sambanova.ai/docs/en/features/text-generation) and [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes) pages, read 2026-11-09. No key was available: `fixtures/openai_chat/providers/sambanova/` holds the document's own example reply and stream and replies built from it, the model listing as SambaCloud serves it without a key, and errors answered live without a valid key. Kaeso's base URL is `https://api.sambanova.ai`, which takes `Authorization: Bearer`.

| SambaNova field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `delta.reasoning`, `channel` | `Block::Reasoning`, extras | Through the profile's `reasoning_fields`. Assistant messages take `reasoning` back. |
| `chat_template_kwargs` (`enable_thinking`), `top_k`, `repetition_penalty` | extras | DeepSeek-V3.2 and Gemma 4 think only with `enable_thinking`, which only a Chat app can send. Refused when the request goes to another dialect. |
| `frequency_penalty`, `presence_penalty` | refused | SambaNova accepts and ignores them, so the profile refuses them before sending; `0` is left out ([neutral values](sampling.md#neutral-values)). |
| an end user's id, a Responses app's `prompt_cache_key` | left out | The request schema has neither and refuses undeclared properties (`without_end_user()`, `without_cache_key()`). Other undeclared fields go as written, for SambaNova's own 400. |
| `video_url` parts | `Block::Video` | `with_video_url()`; Gemma 4 takes video. |
| a float `created`, `system_fingerprint` | extras | `responses/palindrome-poem-doc`. |
| `usage` with SambaNova's timing fields (`time_to_first_token`, `total_latency`, `acceptance_rate`, `is_last_response` and others) | `Usage`, `raw` usage | In replies and in the last chunk of a stream, which has `choices: []` (`streams/stream-length-doc`). |
| errors with OpenAI's `error` object and a top-level `request_id`; 410 `model_deprecated` | `KppError` | The kind follows the status: 401 is a refused key, 410 not found. |
| `GET /v1/models`, `GET /v1/models/{id}` | `ModelList`, `ModelInfo` | Answered without a key, with `context_length`, `max_completion_tokens` and `pricing` (`models/list-recorded`), so a model check can't spot a wrong key. |

SambaCloud serves no embeddings and no Whisper; SambaNova documents both for SambaStack only, and SambaCloud answers `model_not_found` for their models (`errors/recorded-embeddings-model-not-found`). Its Responses API isn't used: Responses apps reach SambaNova through Chat Completions.

## Scaleway

Sources: Scaleway's [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/), its [reasoning guide](https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/), [audio guide](https://www.scaleway.com/en/docs/generative-apis/how-to/query-audio-models/) and [errors page](https://www.scaleway.com/en/docs/generative-apis/api-cli/understanding-errors/), read 2026-11-09. No key was available, so the replies and the stream in `fixtures/openai_chat/providers/scaleway/` are built from them, the stream's first chunks from the reasoning guide's own example; the errors were answered live, without a valid key. Kaeso's base URL is `https://api.scaleway.ai`, which takes `Authorization: Bearer` with an IAM secret key and reaches the key's default Project.

| Scaleway field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Only legacy models put thinking in `<think>` tags in `content`, and those have reached end of life. |
| the `developer` role | `Role::System` | Scaleway lists only `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| an end user's id | left out | The request schema has no `user` or `safety_identifier` (`without_end_user()`). |
| `input_audio` parts in `wav` or `mp3` | `Block::Audio` | As OpenAI's (`requests/input-audio`). |
| `token_ids` on stream choices | extras | Left out towards other dialects. |
| errors with the HTTP status as a number in `error.code`, `type` such as `unauthorized` or `forbidden`, and `param: ""` | `KppError` | The kind follows the status. A key Scaleway doesn't know gets 403 `forbidden` (`errors/recorded-invalid-api-key`), like a key without a Generative APIs permission, so apps get it as a key that isn't allowed the request. |
| the legacy error shape, `{"object": "error", "message", "type", "param", "code"}` | `KppError` | Older models still send it (`errors/legacy-model-error-doc`); apps get an OpenAI error with its message. |

Scaleway serves no Messages, images, speech or token counts: `/v1/messages`, `/v1/images/generations` and `/v1/audio/speech` answer 404 `route_not_found`. Its Responses API, stateless and with function tools only, isn't used: Responses apps reach Scaleway through Chat Completions.

## SiliconFlow

Sources: SiliconFlow's [chat completions](https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions) reference, its [reasoning](https://docs.siliconflow.com/en/userguide/capabilities/reasoning) guide and its [error handling](https://docs.siliconflow.com/en/faqs/error-code) page, read 2026-11-10. Not tested with a successful call: the requests in `fixtures/openai_chat/providers/siliconflow/` are the reasoning guide's own examples, and the reply and the stream are built from the reference; the error was answered live, without a valid key. Kaeso's base URL is `https://api.siliconflow.com`, which takes `Authorization: Bearer` with an `sk-` key.

| SiliconFlow field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. Stream deltas carry `content: null` beside the field they fill. |
| `enable_thinking`, `thinking_budget` | `reasoning` | `thinking_switch()`, as for QwenCloud: an app's thinking turns them on or off; a Chat app's own values go as written. |
| `max_tokens` | `max_tokens` | It limits only the answer, so `max_tokens()` and the provider's `reasoning_beyond_output_limit`. |
| the `developer` role | `Role::System` | The reference lists `system`, `user` and `assistant`, so `system_role()` sends `system`. |
| an end user's id | left out | The reference has no `user` or `safety_identifier` (`without_end_user()`). |
| `min_p`, `top_k` | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| errors as `{"code", "message", "data"}` | `KppError` | SiliconFlow's message; the kind follows the status: 401 is a refused key (`errors/recorded-invalid-api-key`). |

SiliconFlow serves no Responses or token counts: `/v1/responses` and `/v1/messages/count_tokens` answer 404. Its Messages endpoint isn't used, as its reference documents only text and tool blocks. Its embeddings, speech and transcriptions use OpenAI's forms ([OpenAI Embeddings](openai-embeddings.md#siliconflow)); its images and videos don't.

## Together AI

Sources: Together AI's [chat completions reference](https://docs.together.ai/reference/chat-completions-1) and [OpenAI compatibility](https://docs.together.ai/docs/inference/openai-compatibility), read 2026-10-19, the SDK types in `togethercomputer/together-py`, and the live recordings in `fixtures/openai_chat/providers/togetherai/`.

| Together AI field | KPP | Notes |
|---|---|---|
| `top_k`, `min_p`, `repetition_penalty`, `safety_model`, `context_length_exceeded_behavior` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `message.reasoning_content` or `message.reasoning`, and the same in deltas, by model | `Block::Reasoning` | Through the profile's `reasoning_fields`. Chat apps get them back unchanged; Anthropic Messages apps get a `thinking` block and Responses apps a reasoning item ([reasoning](reasoning.md)). A streamed piece of reasoning comes with `content: ""`, which opens no text block. |
| `matched_stop` and `seed` on a choice, `text` on a stream choice, `token_id` in a delta, top-level `prompt`, `metadata` and `warnings` | extras | Left out towards other dialects. |
| `usage.reasoning_tokens` beside `completion_tokens_details.reasoning_tokens` | `raw` usage | The token counts are typed as in [usage](openai-chat.md#usage-completionusage). |
| errors in OpenAI's envelope with Together AI's own `type` and `code` (`start_generation_failed`, `model_not_available`), sometimes with a top-level `id` | `KppError` | The kind follows the status; a wrong key is a 401 `invalid_api_key`, so `Authentication`. |
| `GET /v1/models`: a bare array of models with `type`, `display_name`, `organization`, `context_length`, `config` and `pricing` | `ModelList`, model extras | `GET /v1/models/{model}` answers 404 for ids with `/`, so a model check relies on the listing. |

## Volcengine Ark

Sources: Ark's [base URL and authentication](https://docs.volcengine.com/docs/zh/ark/base-url-and-authentication.md) page, its [Chat API](https://docs.volcengine.com/docs/zh/ark/chat-api.md) reference (updated 2026-11-12), its [deep thinking](https://docs.volcengine.com/docs/zh/ark/deep-thinking.md) guide, its [model listing](https://docs.volcengine.com/docs/zh/ark/models-api.md) and [error codes](https://docs.volcengine.com/docs/zh/ark/error-codes.md), read 2026-11-14. Not tested with a successful call: the replies, the stream and the listing in `fixtures/openai_chat/providers/volcengine/` are the reference's and the guide's samples, one stream was published in another project's issue, and the key errors were answered live, without a valid key. Kaeso's base URL is `https://ark.cn-beijing.volces.com/api/v3`, with `Authorization: Bearer`.

| Volcengine Ark field | KPP | Notes |
|---|---|---|
| `<base>/chat/completions`, `<base>/models` | the provider's API root | Ark's API sits at `/api/v3`, not `/v1`; the base URL ends there and the root is empty ([API roots](#api-roots)). |
| `thinking: {"type": "enabled" \| "disabled"}` | `reasoning` | `thinking_type()`: the models think by default, and `minimal` turns Seed's thinking off but is `low` on DeepSeek V4.1 Flash, so thinking turned off goes as the switch alone. |
| `reasoning_effort` (`none` to `max`) | `reasoning.effort` | Every model that grades thinking takes all seven values and maps them to its own levels. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`; assistant messages take it back. |
| `message.encrypted_content`, `delta.encrypted_content` | extras, and an opaque of the reasoning block | `encrypted_reasoning_field`: a Chat app gets and sends it as written; other apps carry it in their reasoning carrier, and Kaeso writes it back on the assistant message, as Ark requires in multi-turn tool calls (`responses/tools-encrypted-content-doc`, `requests/tools-encrypted-content-replay-doc`). |
| `max_completion_tokens`, `max_tokens` | `max_output_tokens` | `max_completion_tokens` counts thinking and answer, up to 65536; `max_tokens` only the answer on most models. Sent as the app wrote them, Kaeso's own limit as `max_completion_tokens`; the provider has `reasoning_beyond_output_limit`. |
| an end user's id | left out | No `user` or `safety_identifier` (`without_end_user()`). |
| `service_tier` (`auto`, `default`, `fast`, `flex`) | extras | Sent as written; OpenAI's `priority` and `scale` are refused (`refusing_values`). |
| `web_search_options` | extras | Doubao Search only; `{}` turns it on. `search_context_size` and `user_location` have no effect and are refused. Replies carry `url_citation` annotations without offsets and `usage.tool_usage.web_search` (`responses/web-search-doc`). |
| `seed`, `n`, `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_retention`, `functions`, `function_call` | refused | Not in the schema, and Ark doesn't document whether it refuses undeclared fields. |
| `prompt_cache_key`, `video_url` parts, `logprobs`, `logit_bias`, `parallel_tool_calls` | sent as written | In the schema; reasoning models refuse some of them themselves. |
| ids without `chatcmpl-`, `service_tier` on replies, `usage.prompt_tokens_details.audio_tokens` | extras and raw usage | Round-trip for Chat apps. |
| errors in OpenAI's envelope with Ark's `code` and an HTTP-like `type`, the request id at the end of the message | `KppError` | The kind follows the status: 401 is a refused key (`errors/recorded-*-api-key`). |
| `GET /models` with Ark's `name`, `version`, `domain`, `token_limits`, `modalities` and `features`, no `owned_by` | `ModelList` | Needs a key. It lists base model versions, not the account's endpoints (`ep-…`), so the listing is partial (`models/list-doc`). |

Ark's embeddings serve only `/embeddings/multimodal`, which returns one vector for the whole input, so the provider has no embeddings dialect. Its Responses and Anthropic Messages endpoints aren't used, and there are no token counts. BytePlus ModelArk serves the same API at `https://ark.ap-southeast.bytepluses.com/api/v3`, with its own keys and model ids, through the base URL.

### API roots

A provider whose API isn't under `/v1` names its API root in `Provider::api_root`: what takes the place of `/v1` in every dialect's paths. The gateway passes it to the codecs with `EgressCtx::with_api_root` (`pipeline::egress_ctx`, and `availability::models_ctx` for the model listing and model checks). Chat Completions, Responses, Messages and embeddings send their `/v1/…` path under the root, and the OpenAI model listing sends `/v1/models` as `<root>/models`. `Some("")` means the base URL itself ends at the root, as Volcengine Ark's `…/api/v3`, so `/v1/chat/completions` becomes `/chat/completions`; `None`, for every other provider, keeps `/v1`. A key's account scope, as Cloudflare AI Gateway's `/client/v4/accounts/{account_id}/ai/v1`, is applied afterwards and takes the place of this root; a custom provider's base URL names its root itself. Because the default base URL ends at the root, an administrator's base URL is the documented one as it is, such as BytePlus ModelArk's.

## StepFun

Sources: StepFun's [Chat Completions reference](https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create), its [reasoning](https://platform.stepfun.ai/docs/en/guides/developer/reasoning), [prompt cache](https://platform.stepfun.ai/docs/en/guides/developer/prompt-cache), [tool call](https://platform.stepfun.ai/docs/en/guides/developer/tool-call) and [video](https://platform.stepfun.ai/docs/en/guides/developer/video-chat) guides and its [error codes](https://platform.stepfun.ai/docs/en/api-reference/error-codes), and the [reference](https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create) and [error codes](https://platform.stepfun.com/docs/zh/api-reference/error-codes) of its China service, read 2026-11-14. No key was available: the replies and the text stream in `fixtures/openai_chat/providers/stepfun-ai/` are the reference's and the guides' examples, the replies and the stream with reasoning are built from the reference, and the key errors were answered live, without a valid key. Kaeso's base URL is `https://api.stepfun.ai` (`https://api.stepfun.com` for the China service), which takes `Authorization: Bearer`.

| StepFun field | KPP | Notes |
|---|---|---|
| `message.reasoning`, `delta.reasoning`, and `reasoning_content` with `reasoning_format: "deepseek-style"` | `Block::Reasoning` | Through the profile's `reasoning_fields`. The China service's reference sends both fields with the same text: the first is read and the other stays an extra (`responses/both-reasoning-fields-schema`). |
| `reasoning_format` (request) | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `usage.cached_tokens` beside the counts | `cache_read_tokens` | Read when `prompt_tokens_details.cached_tokens`, which the China service's reference names, is absent (`responses/cached-doc`); `prompt_tokens` includes the cached tokens. |
| `max_completion_tokens`, the `developer` role | `max_tokens`, `Role::System` | The reference has only `max_tokens` and the roles `system`, `user`, `assistant` and `tool` (`max_tokens()`, `system_role()`). |
| `video_url` parts | `Block::Video` | `with_video_url()`. |
| `user`, `safety_identifier`, a Responses app's `prompt_cache_key` | left out | The reference has no end-user or cache-key field, and StepFun caches prompts by itself (`without_end_user()`, `without_cache_key()`). |
| `stream_options` | left out | Every chunk carries the running usage without being asked (`without_stream_options()`, `streams/text-doc`). |
| `presence_penalty`, `seed`, `logprobs`, `logit_bias`, `parallel_tool_calls`, `prompt_cache_key`, `store` and the other fields the reference leaves out | refused | Whether StepFun refuses or drops undeclared fields isn't documented, so the profile refuses them before sending; `parallel_tool_calls: true` only allows parallel calls and is left out, as are a [neutral value](sampling.md#neutral-values) such as `store: false` and `name` on messages ([participant names](openai-chat.md#participant-names)). `tool_choice`, which StepFun's guides send, is sent. |
| `finish_reason: ""` and `role: ""` before the last chunk | none | Read as no value, as for other providers. |
| a tool call with `finish_reason: "stop"` | `StopReason::EndTurn` | Both documented tool-call replies end this way (`responses/tools-doc`). The Chat mapping reads `stop` as the end of a turn, so an Anthropic app gets the `tool_use` block with `stop_reason: "end_turn"`. |
| errors as `{"error": {"message", "type"}}` without `code` | `KppError` | The kind follows the status, and StepFun's identifier is in `type`. The China service's 429s `project_credit_limit_exceeded` and `member_project_credit_limit_exceeded` are billing refusals, not rate limits. |
| `GET /v1/models` | `ModelList` | Needs a key and lists chat and speech models alike (`models/list-doc`), so a model check spots a wrong key. |

StepFun's Anthropic Messages endpoint, its Responses API and its token count (`/v1/token/count`) aren't used: every app reaches StepFun through Chat Completions, and Kaeso refuses token counts. `input_audio` takes a data URL without `format`, for StepFun's audio chat models, which KPMR doesn't list; its speech, transcription and realtime APIs aren't used either.

## Tencent TokenHub

Sources: TokenHub's [API usage](https://cloud.tencent.com/document/product/1823/130078), [Chat Completions field reference](https://cloud.tencent.com/document/product/1823/135872), [thinking](https://cloud.tencent.com/document/product/1823/131208), [Hunyuan guide](https://cloud.tencent.com/document/product/1823/132252), [error codes](https://cloud.tencent.com/document/product/1823/131595) and [model list](https://cloud.tencent.com/document/product/1823/130051) pages, read 2026-11-14. No key was available: the replies and the stream in `fixtures/openai_chat/providers/tencent-tokenhub/` are the Hunyuan guide's examples, two chunks of a real stream, published in another project's issue, match the guide's stream, and the key errors were answered live, without a valid key. Every path sits under `/v1` on the host, so Kaeso's base URL is `https://tokenhub.tencentmaas.com` (Guangzhou), which takes `Authorization: Bearer`; other regions and Tencent Cloud's international site are other hosts, each with its own keys.

| TokenHub field | KPP | Notes |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`; assistant messages take it back, which Hy4 and Hy3 need to keep their thinking across turns (`requests/reasoning-sent-back-doc`). |
| the `developer` role | `Role::System` | The reference lists `system`, `user`, `assistant` and `tool`, so `system_role()` sends `system`. |
| `max_tokens`, `max_completion_tokens` | as written | TokenHub takes both and prefers `max_completion_tokens`. |
| `video_url` parts | `Block::Video` | `with_video_url()`. |
| `audio`, `modalities`, `logit_bias`, `metadata`, `store`, `service_tier` and the other fields the reference leaves out | refused | TokenHub ignores undeclared fields without a word ("会被静默忽略"), so the profile refuses them. |
| `thinking`, `enable_thinking`, `thinking_budget`, `interleaved_thinking`, `reasoning_split`, `web_search_options`, `extra_body` | extras | Refused when the request goes to another dialect, like other Chat-only fields. |
| `reasoning_effort` `no_think` | not sent | Hy4 and Hy3 turn thinking off with `no_think`, which has no KPP level; the catalogue lists `high` (Hy4) and `low`, `high` (Hy3), so `none` becomes the model's lowest level, named in `kaeso-adapted` ([models that always reason](reasoning.md#models-that-always-reason)). |
| `reasoning_details`, `search_info`, `usage.cache_read_tokens`, `usage.cache_write_tokens` | extras and raw usage | Left out towards other dialects. |
| errors with `message_zh`, a six-digit `code`, `type`, `source`, `request_id` | `KppError` | The kind follows the status; `type` differs between the reference and live answers, and a 429 may carry `code` as a number without `type` (`errors/rate-limit-doc`). The `code` gives the kind where the status says less: `400003` (input beyond the context) `ContextTooLong`, `400004` (unknown model) `NotFound`, `451001` `ContentFiltered`. |
| `GET /v1/models` with `name` and `status` (`online`, `pre-offline`) | `ModelList` | Needs the key (`models/list-doc`); there is no `GET /v1/models/{model}`. |

TokenHub's Responses and Anthropic Messages endpoints serve only some models and aren't used: Responses and Messages apps reach TokenHub through Chat Completions, and Kaeso refuses token counts, which TokenHub doesn't serve. Its images, videos, speech and the multimodal embeddings use formats of its own.

## Venice

Sources: Venice's [OpenAPI document](https://docs.venice.ai/swagger.yaml) (version `20261113.174621`), [API spec](https://docs.venice.ai/api-reference/api-spec), [OpenAI migration guide](https://docs.venice.ai/guides/getting-started/openai-migration), [error codes](https://docs.venice.ai/api-reference/error-codes) and [reasoning models](https://docs.venice.ai/guides/features/reasoning-models) pages, read 2026-11-14. No key was available: the replies and streams in `fixtures/openai_chat/providers/venice/` are the OpenAPI document's example and replies another project's recorded tests publish, and the key error was answered live without a valid key.

| Venice field | KPP | Notes |
|---|---|---|
| `venice_parameters` (request) | extras | Venice prepends a system prompt of its own unless `include_venice_system_prompt` is `false`, so the profile's `with_option_defaults` writes that key when the request doesn't set it, also into a Chat app's own `venice_parameters`, whose other keys go as written. Refused when the request goes to another dialect. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | Through the profile's `reasoning_fields`. |
| `user` | left out | Venice discards it (`without_end_user()`); its own `anon_user_id` stays in extras. |
| `metadata`, `store` | refused | Venice takes them without acting on them. |
| other fields Venice doesn't know | sent | Venice refuses them with 400 itself (`additionalProperties: false`), so the profile refuses nothing else. |
| `video_url` parts, `input_audio` in other formats than `wav` and `mp3` | `Block::Video`, `Block::Audio` | `with_video_url()`, `any_audio_format()`. |
| `cost`, the reply's `venice_parameters`, `choices[].stop_reason`, `prompt_logprobs` | extras | Left out towards other dialects. |
| a last stream chunk with `choices: []`, `usage` and `cost` | usage | Venice sends it on every stream, also without `stream_options.include_usage`. |
| errors `{"error": "<text>"}` and `{"error", "details", "issues"}` | `KppError` | The kind follows the status, and the text is the message (`errors/recorded-invalid-api-key`, `errors/unknown-model-published`). |
| `GET /v1/models` | `ModelList` | Lists text models only (`models/list-recorded`), so the listing is partial. |

Venice's Responses API is in beta and isn't used: Responses and Messages apps reach Venice through Chat Completions, and Kaeso refuses token counts, which Venice doesn't serve.

## Baidu Qianfan

Sources: Qianfan's [chat](https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4) and [vision](https://cloud.baidu.com/doc/qianfan-api/s/ymwbl48ky) references, its [common headers](https://cloud.baidu.com/doc/qianfan-api/s/Nmwbl43w1), [error codes](https://cloud.baidu.com/doc/qianfan-api/s/rmwbl43x7), [deep thinking](https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv) and [web search](https://cloud.baidu.com/doc/qianfan-docs/s/Wm8r4sw29) guides, read 2026-11-14, and `fixtures/openai_chat/providers/qianfan/`: the reference's requests and replies, a published reply, the web-search guide's stream (its elided middle left out), and refused-key and gateway errors answered live without a valid key.

| Qianfan field | KPP | Notes |
|---|---|---|
| `<base>/chat/completions`, `<base>/embeddings`, `<base>/models` | the provider's API root | Qianfan's API sits at `/v2`; the base URL ends there and the root is empty ([API roots](#api-roots)). |
| `system` role, no `developer` | `Role::System` | `system_role()`. |
| `user` | end user | `end_user_as_user()`. |
| `thinking: {"type": "enabled" \| "disabled"}`, `reasoning_effort` | `reasoning` | `thinking_type()`; `low` and `medium` act as `high`, `xhigh` as `max`. |
| `message.reasoning_content`, `delta.reasoning_content` | `Block::Reasoning` | `reasoning_fields`. |
| `max_completion_tokens`, `max_tokens` | `max_output_tokens` | Both count thinking and answer. |
| `choices[].flag`, `ban_round`, `search_results`, `delta_tag` | extras | Qianfan's safety marks and web-search sources; round-trip for Chat apps. |
| `web_search`, `penalty_score`, `enable_thinking`, `thinking_budget`, `metadata` | extras | Sent as a Chat app wrote them. |
| `logprobs`, `top_logprobs`, `logit_bias`, `store`, `service_tier`, `prompt_cache_key`, `prompt_cache_retention`, `verbosity`, `modalities`, `audio`, `prediction`, `web_search_options`, `functions`, `function_call` | refused | Not in the reference, and Qianfan doesn't document whether it refuses undeclared fields. |
| errors in OpenAI's envelope with Qianfan's `code`; BCE gateway errors as `{"requestId", "code", "message"}` | `KppError` | `tokens_too_long` and `characters_too_long` are a context too long, `account_overdue` a billing error, `model_offline` not found. A fake key gets 401 `invalid_iam_token` on chat and 429 `OverRateLimit` on the listing (`errors/recorded-*`). |
| `GET /models` with Qianfan's `status`, `pricing`, `features` and `endpoints` | `ModelList` | Needs a key; partial, as deployed services aren't listed (`models/list-doc`). |
