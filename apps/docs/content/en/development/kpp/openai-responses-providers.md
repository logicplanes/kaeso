---
title: Responses providers
description: How the Responses codec adapts its requests to each provider that speaks the format, and what each one adds.
order: 7
nav: Responses providers
---

Other providers than OpenAI speak [OpenAI Responses](openai-responses.md) with differences of their own. Look up here how the codec adapts each request to a provider, and what each provider adds, before you add a provider or change its profile.

Providers other than OpenAI that serve Responses record into `fixtures/openai_responses/providers/<id>/` ([fixtures](fixtures.md)). Their extra fields follow the rules of [OpenAI Responses](openai-responses.md); this page names what each one adds.

Each provider in `PROVIDERS` names a `kaeso_codecs::openai_responses::ResponsesProfile` in its `responses` field, and the gateway hands it to the encoder with `EgressCtx::with_responses_profile`. `ResponsesProfile::OPENAI`, the default, sends requests as the app wrote them. `ResponsesProfile::compatible(name)` differs in `tool_namespaces: false` ([tool namespaces](cross-format.md#tool-namespaces)) and `stores_responses: false`, as only OpenAI is known to keep responses for `previous_response_id`; custom providers get `compatible`, since only OpenAI documents namespaces.

| Setting | What the encoder does |
|---|---|
| `xai_web_search()` | Sends hosted search as xAI's `web_search` tool ([hosted web search](cross-format.md#hosted-web-search)). |
| `without_null_reasoning_content()` | Leaves out an explicit `content: null` of a reasoning item the app sends back. |
| `tool_schema_root(root)` | Merges a function tool's root union that the provider can't take into one object schema; `ToolSchemaRoot::ObjectUnions` keeps a union of inline objects ([union tool schemas](cross-format.md#union-tool-schemas)). |
| `without_openai_encrypted_reasoning()` | Leaves out reasoning items the app sends back with OpenAI's `encrypted_content`. It is a Fernet token ([Fernet spec](https://github.com/fernet/spec/blob/master/Spec.md)), so it starts with `gAAAAA`, as in every OpenAI recording in `fixtures/openai_responses/`. |

## SpaceX

Sources: xAI's [Responses reference](https://docs.x.ai/developers/rest-api-reference/inference/responses) and [comparison with Chat Completions](https://docs.x.ai/developers/model-capabilities/text/comparison), read 2026-10-19, and the live recordings in `fixtures/openai_responses/providers/xai/`. Responses is SpaceX's `generation` dialect.

| SpaceX field | KPP | Notes |
|---|---|---|
| `reasoning` items with `summary`, and `encrypted_content`, which `grok-4.3` always sends | as OpenAI's | SpaceX refuses an `encrypted_content` that another provider wrote with a 400 `invalid-argument` (`errors/recorded-reasoning-encrypted.json`, for OpenAI's). An app's history doesn't say which provider wrote an item, so items with OpenAI's are left out (`without_openai_encrypted_reasoning()`), as when a Codex thread switches from GPT to Grok. |
| `web_search_call` items, `url_citation` annotations | as OpenAI's | |
| `namespace` tools (request) | flattened to function tools | SpaceX refuses them with a 422 (`unknown variant namespace`); its profile is `compatible` ([tool namespaces](cross-format.md#tool-namespaces)). |
| `web_search` tool (request) | xAI's tool (`xai_web_search()`) | SpaceX refuses `external_web_access` (`Argument not supported: external_web_access`) and always searches live ([hosted web search](cross-format.md#hosted-web-search)). |
| tool parameters with a root union whose branches aren't all inline objects, or a root `allOf` (request) | one object schema (`tool_schema_root(ToolSchemaRoot::ObjectUnions)`) | SpaceX refuses them with `invalid_client_tool_schema` ([union tool schemas](cross-format.md#union-tool-schemas)). |
| `content: null` on a `reasoning` item sent back (request) | left out (`without_null_reasoning_content()`) | Codex sends its earlier reasoning items back this way. With the `null`, SpaceX can't read the item's `encrypted_content` (`Could not decode the compaction blob`). |
| `usage.num_sources_used`, `num_server_side_tools_used`, `server_side_tool_usage_details`, `cost_in_usd_ticks`, `context_details` | `raw` usage | Responses apps get them back; the token counts are typed as for OpenAI. |
| input item types (request) | only `XAI_INPUT_ITEMS` (`xai_input_items()`) | SpaceX refuses any other type with a 422 that lists the ones it takes (`errors/recorded-codex-agent-message.json`), and takes `custom_tool_call_output` as well, which that list leaves out (`responses/recorded-codex-gpt-history.json`). Codex's `agent_message` becomes a message, and `local_shell_call` and `local_shell_call_output` a function call and its output; any other type is `Unsupported` before the request is sent. |
| `tool_choice` in a request without tools (request) | left out | SpaceX refuses it (`A tool_choice was set on the request but no tools were specified`), as Codex's helper calls send it ([tools left out](cross-format.md#tools-left-out)). |
| `presence_penalty` (request) | extras | SpaceX ignores it and echoes `0.0`. |
| `max_output_tokens` (request) | `sampling.max_output_tokens` | SpaceX applies it only to visible output, not to reasoning tokens (Responses reference), as for [Chat Completions](openai-chat-providers.md#spacex). |
| `{"error": "<message>"}` | `KppError` | See the [error table](openai-responses.md#errors-errorresponse). |

## Vercel

Sources: Vercel's [OpenAI Responses API](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses) pages, read 2026-10-29, and the live recordings in `fixtures/openai_responses/providers/vercel/`. A Vercel model is sent as Responses when its **Sends as** value says so; Vercel's `generation` dialect is Chat Completions. Its profile is `compatible`.

| Vercel field | KPP | Notes |
|---|---|---|
| `response.reasoning.delta` and `response.reasoning.done` events | as `response.reasoning_text.delta` and `response.reasoning_text.done` | Vercel's names for OpenAI's reasoning text events, with the same fields; Responses apps get them as Vercel sent them, other dialects get the reasoning (`streams/recorded-reasoning-stream`). |
| `reasoning` items with `content` (`reasoning_text`) and no `encrypted_content` | as OpenAI's | OpenAI's models add `encrypted_content` when the request asks for it (`responses/recorded-gpt-reasoning-encrypted`). |
| `provider_metadata` on the response, `usage.input_tokens_details.cache_write_tokens` | extras, `raw` usage | As for Chat Completions. |
| `POST /v1/responses/input_tokens` | — | Vercel answers 404, and Kaeso estimates token counts for Vercel models without sending anything. |
| errors | `KppError` | OpenAI's envelope with Vercel's own `type`, as for [Chat Completions](openai-chat-gateways.md#vercel). |
