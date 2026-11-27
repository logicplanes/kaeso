---
title: Responses 服务商
description: Responses 编解码器如何针对使用该格式的每个服务商调整请求，以及每个服务商额外提供的内容。
order: 7
nav: Responses 服务商
---

OpenAI 以外的服务商使用 [OpenAI Responses](openai-responses.md) 时各有差异。在添加服务商或更改其配置之前，请在此查阅编解码器如何针对每个服务商调整请求，以及每个服务商额外提供的内容。

提供 Responses 的其他服务商（OpenAI 以外）会录制到 `fixtures/openai_responses/providers/<id>/`（[fixture](fixtures.md)）。它们的额外字段遵循 [OpenAI Responses](openai-responses.md) 的规则；本页列出每个服务商增加的内容。

`PROVIDERS` 中的每个服务商都在其 `responses` 字段中指定一个 `kaeso_codecs::openai_responses::ResponsesProfile`，网关通过 `EgressCtx::with_responses_profile` 把它交给编码器。默认的 `ResponsesProfile::OPENAI` 按应用写出的样子发送请求。`ResponsesProfile::compatible(name)` 在 `tool_namespaces: false`（参见[工具命名空间](cross-format.md#工具命名空间)）和 `stores_responses: false` 上不同，因为目前只知道 OpenAI 会为 `previous_response_id` 保留回复；自定义服务商使用 `compatible`，因为只有 OpenAI 记载了命名空间。

| 设置 | 编码器的行为 |
|---|---|
| `xai_web_search()` | 把托管搜索作为 xAI 的 `web_search` 工具发送（参见[托管网页搜索](cross-format.md#托管网页搜索)）。 |
| `without_null_reasoning_content()` | 省略应用发回的推理项上显式的 `content: null`。 |
| `tool_schema_root(root)` | 把服务商不接受的函数工具根部联合合并为一个对象 schema；`ToolSchemaRoot::ObjectUnions` 保留由内联对象组成的联合（[联合类型的工具 schema](cross-format.md#联合类型的工具-schema)）。 |
| `without_openai_encrypted_reasoning()` | 省略应用发回的、带 OpenAI 的 `encrypted_content` 的推理项。它是一个 Fernet 令牌（[Fernet 规范](https://github.com/fernet/spec/blob/master/Spec.md)），因此以 `gAAAAA` 开头，`fixtures/openai_responses/` 中 OpenAI 的每份录制都是如此。 |

## SpaceX

来源：xAI 的 [Responses 参考](https://docs.x.ai/developers/rest-api-reference/inference/responses)和[与 Chat Completions 的比较](https://docs.x.ai/developers/model-capabilities/text/comparison)（2026-10-19 阅读），以及 `fixtures/openai_responses/providers/xai/` 中的实时录制。Responses 是 SpaceX 的 `generation` 方言。

| SpaceX 字段 | KPP | 说明 |
|---|---|---|
| 带 `summary` 的 `reasoning` 项，以及 `grok-4.3` 总会发送的 `encrypted_content` | 与 OpenAI 相同 | 对其他服务商生成的 `encrypted_content`，SpaceX 返回 400 `invalid-argument`（`errors/recorded-reasoning-encrypted.json`，针对 OpenAI 的）。应用的历史不会说明某个项由哪个服务商生成，因此带 OpenAI 的 `encrypted_content` 的项会被省略（`without_openai_encrypted_reasoning()`），例如 Codex 线程从 GPT 切换到 Grok 时。 |
| `web_search_call` 项、`url_citation` 标注 | 与 OpenAI 相同 | |
| `namespace` 工具（请求） | 展开为函数工具 | SpaceX 以 422 拒绝它们（`unknown variant namespace`）；它的配置是 `compatible`（参见[工具命名空间](cross-format.md#工具命名空间)）。 |
| `web_search` 工具（请求） | xAI 的工具（`xai_web_search()`） | SpaceX 拒绝 `external_web_access`（`Argument not supported: external_web_access`），并且总是实时搜索（参见[托管网页搜索](cross-format.md#托管网页搜索)）。 |
| 根部联合的分支并非都是内联对象、或根部带有 `allOf` 的工具参数（请求） | 一个对象 schema（`tool_schema_root(ToolSchemaRoot::ObjectUnions)`） | SpaceX 会以 `invalid_client_tool_schema` 拒绝它们（[联合类型的工具 schema](cross-format.md#联合类型的工具-schema)）。 |
| 发回的 `reasoning` 项上的 `content: null`（请求） | 省略（`without_null_reasoning_content()`） | Codex 以这种方式发回之前的推理项。带着这个 `null` 时，SpaceX 无法读取该项的 `encrypted_content`（`Could not decode the compaction blob`）。 |
| `usage.num_sources_used`、`num_server_side_tools_used`、`server_side_tool_usage_details`、`cost_in_usd_ticks`、`context_details` | 用量的 `raw` | Responses 应用会收到它们；Token 数按 OpenAI 的类型化字段处理。 |
| 输入 item 类型（请求） | 仅 `XAI_INPUT_ITEMS`（`xai_input_items()`） | SpaceX 以 422 拒绝其他类型，并列出它接受的类型（`errors/recorded-codex-agent-message.json`）；它也接受该列表遗漏的 `custom_tool_call_output`（`responses/recorded-codex-gpt-history.json`）。Codex 的 `agent_message` 变为一条消息，`local_shell_call` 和 `local_shell_call_output` 变为函数调用及其输出；其他类型在发送前即为 `Unsupported`。 |
| 没有工具的请求中的 `tool_choice`（请求） | 省略 | SpaceX 拒绝它（`A tool_choice was set on the request but no tools were specified`），Codex 的辅助调用会这样发送（[省略的工具](cross-format.md#省略的工具)）。 |
| `presence_penalty`（请求） | extras | SpaceX 会忽略它，并回显 `0.0`。 |
| `max_output_tokens`（请求） | `sampling.max_output_tokens` | SpaceX 只把它用于可见输出，不用于推理 token（Responses 参考），与 [Chat Completions](openai-chat-providers.md#spacex) 相同。 |
| `{"error": "<message>"}` | `KppError` | 见[错误表](openai-responses.md#错误errorresponse)。 |

## Vercel

来源：Vercel 的 [OpenAI Responses API](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses) 页面（2026-10-29 阅读），以及 `fixtures/openai_responses/providers/vercel/` 中的实时录制。当 Vercel 模型的**发送格式**为 Responses 时，它以 Responses 发送；Vercel 的 `generation` 方言是 Chat Completions。它的配置是 `compatible`。

| Vercel 字段 | KPP | 说明 |
|---|---|---|
| `response.reasoning.delta` 和 `response.reasoning.done` 事件 | 与 `response.reasoning_text.delta` 和 `response.reasoning_text.done` 相同 | Vercel 对 OpenAI 推理文本事件的命名，字段相同；Responses 应用原样收到 Vercel 发送的事件，其他方言得到推理内容（`streams/recorded-reasoning-stream`）。 |
| 带 `content`（`reasoning_text`）、不带 `encrypted_content` 的 `reasoning` 项 | 与 OpenAI 相同 | 请求要求时，OpenAI 的模型会加上 `encrypted_content`（`responses/recorded-gpt-reasoning-encrypted`）。 |
| 回复中的 `provider_metadata`、`usage.input_tokens_details.cache_write_tokens` | extras、用量的 `raw` | 与 Chat Completions 相同。 |
| `POST /v1/responses/input_tokens` | — | Vercel 返回 404，Kaeso 不发送任何内容，自行估算 Vercel 模型的 Token 数。 |
| 错误 | `KppError` | OpenAI 的错误结构，带 Vercel 自己的 `type`，与 [Chat Completions](openai-chat-gateways.md#vercel) 相同。 |
