---
title: Chat Completions 服务商
description: Chat 编解码器如何针对使用该格式的每个服务商调整请求，以及每个服务商额外提供的内容。
order: 6
nav: Chat 服务商
---

除 OpenAI 外，其他服务商也使用 [OpenAI Chat Completions](openai-chat.md) 格式，但各有差异。添加服务商或修改其配置之前，请在此查阅编解码器如何针对每个服务商调整请求，以及每个服务商额外提供的内容。

## 服务商配置

其他服务商使用这种格式时各有差异。`PROVIDERS`（`crates/kaeso-core/src/provider.rs`）中的每个服务商都在 `chat` 字段中指定一个 `kaeso_codecs::openai_chat::ChatProfile`。该配置包含服务商的名称，以及 Kaeso 向它发送请求时的设置（`ChatProfile::request`，即一个 `RequestProfile`），通过下面的构建方法设置。

网关通过 `EgressCtx::with_chat_profile` 把该配置交给编码器，编码器将其应用到每个请求，无论应用以哪种格式发送。默认的 `ChatProfile::OPENAI` 不做任何改变。`ChatProfile::compatible(name)` 只在 `request.verbosity` 上不同，该值为假：OpenAI Responses 应用的 `text.verbosity` 会被省略，而不是作为 `verbosity` 发送。只有 OpenAI 记载了这个字段，会拒绝未知字段的服务商也会拒绝它。出于同样的原因，自定义服务商使用 `ChatProfile::OPENAI.without_verbosity()`。

| 设置 | 编码器的行为 | 为什么允许 |
|---|---|---|
| `max_tokens()` | 把输出上限以 `max_tokens` 发送，把 `max_completion_tokens: null` 以 `max_tokens: null` 发送。同时带这两个字段且值不同的 Chat 请求为 `Unsupported`（`max_tokens`）。 | 只改变写法，值不变。 |
| `system_role()` | 把 `developer` 消息以 `system` 消息发送。 | 角色的写法是形式标记；两者都承载指令。 |
| `random_seed()` | 把 `seed` 以 `random_seed` 发送，`seed: null` 也一样。负数 seed，或同时带 `seed` 和 `random_seed`（任一可为 `null`）的请求，为 `Unsupported`（`seed`）。此规则依据 Mistral 的 `random_seed`，其 OpenAPI 文档规定 `minimum: 0`。 | `seed` 仍是语义字段：只会被改名或拒绝，绝不会被丢弃。 |
| `without_end_user()` | 省略 KPP `user`，而不是以 `user` 或 `safety_identifier` 发送；Chat 应用与 `safety_identifier` 一起发送的第二个 `user` 以及它们的 `null` 写法也一并省略。 | `user` 是提示字段（[字段类别](index.md#字段类别)）。 |
| `without_prefill()` | 拒绝以 assistant 消息结尾的对话，除非该消息调用工具，或是带有 `prefix: true` 的 Chat 应用消息：`Unsupported`（`assistant prefill`，param `messages[i]`）。 | 只能借助 `prefix: true` 续写此类消息的服务商会在回复中重复预填内容，应用因此得不到续写。 |
| `without_verbosity()` | 省略 OpenAI Responses 应用的 `text.verbosity`，而不是作为 `verbosity` 发送，并在 `kaeso-left-out` 中注明。`compatible(name)` 会设置它。 | `text.verbosity` 是提示（[字段类别](index.md#字段类别)）。 |
| `greedy_top_p()` | 只要请求带有 `temperature: 0`，无论设置了什么 `top_p`，都发送 `top_p: 1`。 | 在 `temperature: 0` 下，模型总是选择可能性最高的 token，因此 `top_p` 不起作用，应用的值也没有损失；默认 `top_p` 更低的提供商在没有 `top_p: 1` 时会拒绝贪婪采样。 |
| `refusing(fields)` | 如果请求体会包含其中某个顶层键，就拒绝该请求，例如 `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | extras 是语义字段，因此被拒绝，而不是丢弃。 |
| `refusing_in_messages(fields)` | 对 system、user 和 assistant 消息中的键同样处理。不检查 tool 消息。 | 同上。 |

Kaeso 读取回复时的设置（`ChatProfile::reply`，即一个 `ReplyProfile`）：

| 设置 | 解码器的行为 | 为何允许 |
|---|---|---|
| `thinking_parts()` | 把 `content` 数组中的 `thinking` 部分（`{type: "thinking", thinking: [文本部分], closed}`，不含其他内容）读作 `Block::Reasoning`，其 `text` 包含每个文本部分；流式发送的则读作 `BlockStart(Reasoning)` 和 `BlockDelta(ReasoningText)`。回答、拒绝或工具调用开始时，推理块结束；流中第一个真正的文本片段之前的空文本片段不会打开文本块（形式标记 `et`）。带有其他键或其他内部部分的 `thinking` 部分保持与没有此设置时相同。 | 不丢弃任何内容：Chat 应用收回同样的部分，其他格式以各自的推理形式得到思考内容，原始块保存在[推理数据块](reasoning.md#推理数据块)中。 |
| `reasoning_fields(fields)` | 把这些消息键中第一个含非空字符串的键（例如 DeepSeek 的 `reasoning_content` 或 Groq 的 `reasoning`）读作 `Block::Reasoning`，放在回复的其他块之前，并用形式 `reasoning_field` 记下键名。在流中，每个增量的该键变为同一个推理块的 `BlockDelta(ReasoningText)`，数据块上带形式 `rf`。回答、拒绝或工具调用开始时，推理块结束；第一个真正的文本片段之前的空文本片段不会打开文本块（形式 `et`）。空字符串、`null` 或其他类型仍是 extra。 | 不丢弃任何内容：Chat 应用按原样收回该键，增量中的其他键（Groq 的 `channel`）仍是数据块的 extras，其他格式以各自的推理形式得到推理，原始块保存在[推理数据块](reasoning.md#推理数据块)中。没有此设置时，每个流式推理片段都会是一个没有 KPP 事件的数据块，并各自记录为一个未知块。 |
| `error_detail(path)` | 把错误体中该路径上的字符串（例如 OpenRouter 的 `error.metadata.raw`）追加到错误消息之后：`<message>: <detail>`。该值缺失、为空、不是字符串，或已包含在消息中时，不做任何改变。流错误同样处理。 | 错误体保持原样：Chat 应用原样收到服务商的错误，其他格式则在消息中得到原因，而笼统的消息（`Provider returned error`）对它们毫无帮助。网关会从每条服务商错误消息中去掉类似密钥的字符串。 |

`refusing` 和 `refusing_in_messages` 各自接收完整列表；第二次调用会替换第一次。拒绝时按应用写下的方式命名字段。Chat 应用的键用它自己的名称和路径（`logit_bias`、`messages[1].name`），来自其他格式的拒绝块用它的 KPP 路径（`messages[1].blocks[0]`）。

拒绝发生在编码请求时，针对编码器构建好的请求、在写出请求体之前进行。因此不会发送任何内容，拒绝会以应用自己的格式返回给应用。

下面列出配置与 OpenAI 不同的每个内置服务商，以及每项设置的来源：服务商的文档、它的 OpenAPI 文档，或 `fixtures/openai_chat/providers/<id>/` 中的实时录制。

| 服务商 | 设置 | 来源 |
|---|---|---|
| DeepSeek（`deepseek`） | `max_tokens()`、`system_role()`、`reasoning_fields(["reasoning_content"])` | DeepSeek 的 API 参考和思考模式指南（`reasoning_content`），`responses/recorded-thinking-reasoning-content` 和 `streams/recorded-thinking-stream`；`fixtures/openai_chat/providers/deepseek/responses/recorded-max-completion-tokens-limit`：DeepSeek 忽略 `max_completion_tokens`（要求 20，实际 133 个 completion token），而其 API 参考记载的是 `max_tokens`；`fixtures/openai_chat/providers/deepseek/errors/recorded-plain-developer-string`：DeepSeek 对 `developer` 角色返回 422 `unknown variant` |
| Groq（`groq`） | `system_role()`、`reasoning_fields(["reasoning"])`；拒绝 `logit_bias`、`logprobs` 和 `top_logprobs`，以及消息中的 `name` 和 `refusal` | Groq 的 OpenAI 兼容性页面（https://console.groq.com/docs/openai，其中列出 `logprobs`、`logit_bias`、`top_logprobs` 和 `messages[].name` 不受支持）；`fixtures/openai_chat/providers/groq/errors/` 中的 `recorded-logit-bias`、`recorded-logprobs-top-logprobs-doc`、`recorded-tools-results-parallel-error`（assistant 消息中的 `refusal`）和 `recorded-tools-single`（Qwen 的聊天模板拒绝 `developer` 角色）；Groq 的推理指南（`reasoning`），`responses/recorded-reasoning-format-parsed` 和 `streams/recorded-stream-doc` |
| Mistral（`mistral`） | `max_tokens()`、`system_role()`、`random_seed()`、`without_end_user()`、`without_prefill()`、`greedy_top_p()`、`thinking_parts()`；拒绝 `audio`、`function_call`、`functions`、`logit_bias`、`moderation`、`prompt_cache_options`、`prompt_cache_retention`、`store`、`verbosity` 和 `web_search_options`，以及消息中的 `name` 和 `refusal` | Mistral 的 OpenAPI 文档（https://docs.mistral.ai/openapi.yaml，2026-10-19 查阅），其中 `ChatCompletionRequest` 禁止未声明的字段，并规定 `random_seed` 的 `minimum: 0`；`fixtures/openai_chat/providers/mistral/errors/` 中的 `422 extra_forbidden` 录制文件，每个被拒绝或改名的字段一个；`responses/recorded-audio-field`，其中 Mistral 接受 `audio` 却只以文本回答；另有 `recorded-assistant-prefill`（400 `invalid_request_message_order`）和 `responses/recorded-assistant-prefix`，其回复重复了预填内容；`thinking_parts()` 依据 `responses/recorded-reasoning-effort-high` 和 `streams/recorded-reasoning-effort-high-stream`；`greedy_top_p()` 依据 `errors/greedy-sampling`（400 `invalid_request_greedy_sampling`） |
| OpenRouter（`openrouter`） | `reasoning_fields(["reasoning"])`、`error_detail(["error", "metadata", "raw"])` | OpenRouter 的推理 token 指南（`reasoning`，旁边还有 `reasoning_details`）；`responses/recorded-free-reasoning` 和 `streams/recorded-free-reasoning-stream`；OpenRouter 的错误与调试页面（`metadata.raw`，即上游服务商的错误），`errors/recorded-upstream-rate-limited` 和 `errors/recorded-routing-fields` |
| SpaceX（`xai`） | `reasoning_fields(["reasoning_content"])` | xAI 的 Chat Completions 参考（`reasoning_content`）；`responses/recorded-reasoning-content` 和 `streams/recorded-reasoning-content-stream` |
| Together AI（`togetherai`） | `reasoning_fields(["reasoning_content", "reasoning"])` | Together AI 的 chat completions 参考；键名取决于模型：`responses/recorded-sampling-extras`（`reasoning_content`），`responses/recorded-reasoning-reply` 和 `streams/recorded-reasoning-stream`（`reasoning`） |

## 使用此格式的服务商

提供 Chat Completions 的其他服务商（OpenAI 以外）会录制到 `fixtures/openai_chat/providers/<id>/`（[fixture](fixtures.md)）。它们的额外字段遵循上述规则；本节列出每个服务商增加的内容。

### DeepSeek

来源：DeepSeek 的 [API 参考](https://api-docs.deepseek.com/api/create-chat-completion)、[思考模式指南](https://api-docs.deepseek.com/guides/thinking_mode/)和[错误码](https://api-docs.deepseek.com/quick_start/error_codes)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/deepseek/` 中的实时录制。

| DeepSeek 字段 | KPP | 说明 |
|---|---|---|
| `thinking`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Chat 应用带有该字段的助手轮次会将其发回 DeepSeek。Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项；它们发回时，该推理不会出现在发往 DeepSeek 的请求中（[推理](reasoning.md)），DeepSeek 在没有它的情况下照常回答。 |
| `usage.prompt_cache_hit_tokens`、`prompt_cache_miss_tokens` | 用量的 `raw` | 命中数等于 `prompt_tokens_details.cached_tokens`，后者按缓存读取类型化。 |
| 服务器繁忙时发送的 `: keep-alive` 行和空行 | `Extra` | 与其他 SSE 注释一样，仅限同一方言。 |
| HTTP 402 | `Billing` | 余额不足，无论 `code` 为何。 |
| `developer` 消息导致的 HTTP 422 | `InvalidRequest` | DeepSeek 不认识该角色；应用会收到 DeepSeek 的错误。 |
| 额外的模型字段（`name`、`context_window`、`max_output_tokens`、`input_modalities`、`output_modalities`、`effort`、`api_capabilities`） | 模型 extras | DeepSeek 的模型列表没有 `created`。 |

### SpaceX

来源：xAI 的 [Chat Completions 参考](https://docs.x.ai/developers/rest-api-reference/inference/chat-completions)和[模型参考](https://docs.x.ai/developers/rest-api-reference/inference/models)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/xai/` 中的实时录制。SpaceX 自身的格式是 [OpenAI Responses](openai-responses.md#使用此格式的服务商)；模型通过其 `egress_dialect` 使用 Chat Completions。

| SpaceX 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。 |
| `citations`、`system_fingerprint`、`service_tier` | extras | 发往其他方言时省略。 |
| `max_completion_tokens`（请求） | `sampling.max_output_tokens` | SpaceX 只把它用于可见输出，不用于推理 token（Chat Completions 参考；`responses/recorded-max-completion-tokens-reasoning`：要求 20，可见 18 个、推理 271 个 token）。用量两者都计入，因此输出可能超过上限；在硬性预算和 Token 速率下，网关为 SpaceX 的推理模型（`PROVIDERS` 中的 `reasoning_beyond_output_limit`）预留模型自身的输出上限（[限额与预算](../../console/limits-and-budgets.md)）。 |
| `usage.num_sources_used`、`cost_in_usd_ticks`、`prompt_tokens_details.text_tokens` 和 `image_tokens` | 用量的 `raw` | Token 数按上文的类型化字段处理。SpaceX 的 `completion_tokens` 不含推理，推理计入 `total_tokens`，因此输出为 `completion_tokens` 加 `reasoning_tokens`。 |
| `{"error": "<message>"}`，400 时带有 `"code": "invalid-argument"` | `KppError` | 该字符串即为错误消息，应用以 OpenAI 错误格式和 SpaceX 的状态码收到它。密钥错误时返回 400，消息以 `Incorrect API key provided` 开头；SpaceX 的 `refused_key_messages` 列出了该消息，因此网关会报告密钥被拒绝。 |
| 推理模型的 `presence_penalty`、不支持推理的模型的 `reasoning_effort` | | SpaceX 返回 400，应用会收到该错误。 |
| `search_parameters`（请求） | extras | 实时搜索已弃用，SpaceX 返回 410；应用会收到这个 410。 |
| 额外的模型字段（`aliases`、以美分计的每 1 亿 Token 价格、`capabilities.reasoning_effort`、图像的 `pricing`） | 模型 extras | |

### Together AI

来源：Together AI 的 [Chat Completions 参考](https://docs.together.ai/reference/chat-completions-1)和 [OpenAI 兼容性说明](https://docs.together.ai/docs/inference/openai-compatibility)（2026-10-19 阅读）、`togethercomputer/together-py` 中的 SDK 类型，以及 `fixtures/openai_chat/providers/togetherai/` 中的实时录制。

| Together AI 字段 | KPP | 说明 |
|---|---|---|
| `top_k`、`min_p`、`repetition_penalty`、`safety_model`、`context_length_exceeded_behavior`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning_content` 或 `message.reasoning`（视模型而定），以及增量中的同名字段 | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。流式推理片段带有 `content: ""`，它不会打开文本块。 |
| choice 上的 `matched_stop` 和 `seed`、流式 choice 上的 `text`、增量中的 `token_id`，以及顶层的 `prompt`、`metadata` 和 `warnings` | extras | 发往其他方言时省略。 |
| 与 `completion_tokens_details.reasoning_tokens` 并列的 `usage.reasoning_tokens` | 用量的 `raw` | Token 数按上文的类型化字段处理。 |
| 采用 OpenAI 错误结构、带 Together AI 自有 `type` 和 `code`（`start_generation_failed`、`model_not_available`）的错误，有时带顶层 `id` | `KppError` | 类别取决于状态码；密钥错误时返回 401 `invalid_api_key`，即 `Authentication`。 |
| `GET /v1/models`：模型的裸数组，带 `type`、`display_name`、`organization`、`context_length`、`config` 和 `pricing` | `ModelList`、模型 extras | 对含 `/` 的 ID，`GET /v1/models/{model}` 返回 404，因此模型检查依赖模型列表。 |

### Groq

来源：Groq 的 [API 参考](https://console.groq.com/docs/api-reference)和[推理指南](https://console.groq.com/docs/reasoning)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/groq/` 中的实时录制。

| Groq 字段 | KPP | 说明 |
|---|---|---|
| `reasoning_format`、`include_reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。 |
| `delta.channel`（GPT-OSS 推理中为 `analysis`） | 数据块的 extras | Chat 应用随推理一起收回；对其他方言省略。 |
| `x_groq`、`usage_breakdown`、`service_tier`（`on_demand`） | extras | 发往其他方言时省略。 |
| `usage.queue_time`、`prompt_time`、`completion_time`、`total_time` | 用量的 `raw` | Token 数（包括 `reasoning_tokens`）按上文的类型化字段处理。 |
| 结束数据块中的 `usage`，即使没有 `stream_options.include_usage` | `Usage` | 无论 Groq 是否另外发送 `include_usage` 请求的用量数据块，每种方言都会收到用量。 |
| 额外的模型字段（`active`、`context_window`、`max_completion_tokens`、`pricing`、`supported_features` 等） | 模型 extras | |

### OpenRouter

来源：OpenRouter 的 [API 参考](https://openrouter.ai/docs/api_reference/overview)、[流式传输](https://openrouter.ai/docs/api_reference/streaming)、[错误与调试](https://openrouter.ai/docs/api_reference/errors-and-debugging)和[推理 Token](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/openrouter/` 中的实时录制（均使用 `:free` 模型）。

| OpenRouter 字段 | KPP | 说明 |
|---|---|---|
| `provider`、`models`、`transforms`、`usage`、`reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。 |
| `message.reasoning_details`、`delta.reasoning_details` | extras | OpenRouter 结构化形式的同一推理；Chat 应用原样收回，其他方言从 `reasoning` 得到文本。 |
| 回复中的 `provider`、`native_finish_reason` | extras | 发往其他方言时省略。 |
| `usage.cost`、`is_byok`、`cost_details` 及额外的 Token 明细 | 用量的 `raw` | Kaeso 按目录中的价格计算费用，而不是按 `usage.cost`。 |
| 最后一个流式数据块，它在 `usage` 旁重复带 `finish_reason` 的 choice | `Usage` | 按 `choices: []` 的用量数据块读取。 |
| `: OPENROUTER PROCESSING` 注释 | `Extra` | 与其他 SSE 注释一样，仅限同一方言。 |
| `error.metadata.raw`，即上游服务商自己的错误，与笼统的 `message` `Provider returned error` 并列 | `KppError.message` | 通过配置的 `error_detail`，Anthropic Messages 和 Responses 应用以及请求日志会得到 `Provider returned error: <原因>`，例如 `google/gemma-4-31b-it:free is temporarily rate-limited upstream…`。Chat 应用收到 OpenRouter 发来的原样错误体。 |
| 200 之后出现的 `error` 对象，位于回复体中或与 `choices` 并列的数据块中 | `KppError` | 见[上方各行](openai-chat.md#流createchatcompletionstreamresponse)；数字 `code` 像状态码一样决定类别。 |
| `GET /api/v1/models`：没有 `object`，模型也没有 `object` 和 `owned_by`，带 `total_count` 和 `links` | `ModelList`、extras | `GET /api/v1/models/{model}` 对每个模型都返回 404，因此模型检查依赖模型列表，而 OpenRouter 提供该列表时不检查密钥。 |
