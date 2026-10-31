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
| `with_message_bookkeeping()` | 发送 Chat 应用助手消息中的簿记键，即 `cross::CHAT_ASSISTANT_BOOKKEEPING`（目前是 Grok CLI 的 `model_id`，包括 `null`）。没有此设置时，编码器省略它们。 | 模型从不读取它们，而多数 Chat 提供商会拒绝未声明的消息字段（Groq、Mistral 以及兼容 OpenAI 的代理），因此只有已知接受它们的提供商才会收到。 |
| `tool_schema_root(root)` | 把服务商不接受的函数工具根部联合合并为一个对象 schema，并在 `kaeso-adapted` 中注明该工具；`ToolSchemaRoot::ObjectUnions` 保留由内联对象组成的联合。 | 工具、它的属性和模型的参数都保留；只有根部的形状改变（[联合类型的工具 schema](cross-format.md#联合类型的工具-schema)）。 |
| `partial_prefill()` | 当对话的最后一条 assistant 消息由其他格式的应用发送且不调用工具时，为其加上 `partial: true`；Anthropic Messages 应用就是这样预填回复开头的。Chat 应用自己的消息按原样发送。 | 消息及其文本不变；该标记只是告诉借助它续写预填内容的服务商（例如 Moonshot）接着该消息续写，而不是重新回答并重复预填内容。 |
| `stop_list()` | 将单个 `stop` 字符串作为只含一项的列表发送。 | 只改变写法，停止序列不变。把 `stop` 定义为列表的服务商会拒绝字符串。 |
| `thinking_switch()` | 将请求的思考以 `enable_thinking` 发送：思考关闭或强度为 `none`、`minimal` 时为 `false`，并省略 `reasoning_effort`；其他情况为 `true`。没有强度的预算以 `thinking_budget` 发送，而不是映射成的强度；强度仍以 `reasoning_effort` 发送。Chat 应用自己发送的键保持不变。 | 服务商用自己的键开关思考；在模型默认不思考时，仅有 `reasoning_effort` 不会开启思考。开或关、预算和强度都原样到达服务商。 |
| `greedy_top_p()` | 只要请求带有 `temperature: 0`，无论设置了什么 `top_p`，都发送 `top_p: 1`。 | 在 `temperature: 0` 下，模型总是选择可能性最高的 token，因此 `top_p` 不起作用，应用的值也没有损失；默认 `top_p` 更低的提供商在没有 `top_p: 1` 时会拒绝贪婪采样。 |
| `refusing(fields)` | 如果请求体会包含其中某个顶层键，就拒绝该请求，例如 `` `logit_bias` is not supported by Example Cloud; remove it from the request. `` | extras 是语义字段，因此被拒绝，而不是丢弃。 |
| `refusing_in_messages(fields)` | 对 system、user 和 assistant 消息中的键同样处理。不检查 tool 消息。 | 同上。 |

Kaeso 读取回复时的设置（`ChatProfile::reply`，即一个 `ReplyProfile`）：

| 设置 | 解码器的行为 | 为何允许 |
|---|---|---|
| `thinking_parts()` | 逐个读取 `content` 数组中的每个部分：文本部分是文本，`thinking` 部分（`{type: "thinking", thinking: [块], ...}`）是 `Block::Reasoning`，其 `text` 包含它的文本块，并带上它的其他键（`closed`、`signature`）；它包含文本以外的内容时，整个 `thinking` 数组放在形式标记 `thinking` 中。其他部分（`reference`）为 `Block::Unknown`。流式的思考内容变为 `BlockStart(Reasoning)` 和 `BlockDelta(ReasoningText)`，流把其他部分和 `thinking` 数组中的其他块保留为 `BlockStart(Unknown)` 块（[流式传输](openai-chat.md#流createchatcompletionstreamresponse)）；回答、拒绝或工具调用开始时，推理块结束；流中第一个真正的文本片段之前的空文本片段不会打开文本块（形式标记 `et`）。 | Chat 客户端把 `content` 当作字符串读取，因此 Chat 应用永远不会收到数组：文本是 `content`，思考是 `reasoning_content`（多个思考部分以空行连接，为空时不发送），与 DeepSeek 相同。Chat 没有字段可承载的其他部分只保留在记录的回复中；不含可读内容的流块（例如 Mistral 用来结束思考的空 `thinking` 部分）到达它们时不带 `content`。文本与思考之间不保留顺序。Chat 应用重放的推理（`reasoning_content`、`reasoning`、`reasoning_details`）以文本之前的 `thinking` 部分发回提供商，其他格式的应用发回的思考块也一样；没有可读文本的推理会被省略。`thinking` 部分的 `closed` 和引用在经过其他格式后不会保留，因为推理数据块不保存形式标记。其他格式以各自的推理形式得到思考内容，原始块保存在[推理数据块](reasoning.md#推理数据块)中。 |
| `reasoning_fields(fields)` | 把这些消息键中第一个含非空字符串的键（例如 DeepSeek 的 `reasoning_content` 或 Groq 的 `reasoning`）读作 `Block::Reasoning`，放在回复的其他块之前，并用形式 `reasoning_field` 记下键名。在流中，每个增量的该键变为同一个推理块的 `BlockDelta(ReasoningText)`，数据块上带形式 `rf`。回答、拒绝或工具调用开始时，推理块结束；第一个真正的文本片段之前的空文本片段不会打开文本块（形式 `et`）。空字符串、`null` 或其他类型仍是 extra。 | 不丢弃任何内容：Chat 应用按原样收回该键，其他格式的应用发回的推理以这些键中的第一个到达提供商，增量中的其他键（Groq 的 `channel`）仍是数据块的 extras，其他格式以各自的推理形式得到推理，原始块保存在[推理数据块](reasoning.md#推理数据块)中。没有此设置时，每个流式推理片段都会是一个没有 KPP 事件的数据块，并各自记录为一个未知块。 |
| `error_detail(path)` | 把错误体中该路径上的字符串（例如 OpenRouter 的 `error.metadata.raw`）追加到错误消息之后：`<message>: <detail>`。该值缺失、为空、不是字符串，或已包含在消息中时，不做任何改变。流错误同样处理。 | 错误体保持原样：Chat 应用原样收到服务商的错误，其他格式则在消息中得到原因，而笼统的消息（`Provider returned error`）对它们毫无帮助。网关会从每条服务商错误消息中去掉类似密钥的字符串。 |

`refusing` 和 `refusing_in_messages` 各自接收完整列表；第二次调用会替换第一次。拒绝时按应用写下的方式命名字段。Chat 应用的键用它自己的名称和路径（`logit_bias`、`messages[1].name`），来自其他格式的拒绝块用它的 KPP 路径（`messages[1].blocks[0]`）。

拒绝发生在编码请求时，针对编码器构建好的请求、在写出请求体之前进行。因此不会发送任何内容，拒绝会以应用自己的格式返回给应用。

下面列出配置与 OpenAI 不同的每个内置服务商，以及每项设置的来源：服务商的文档、它的 OpenAPI 文档，或 `fixtures/openai_chat/providers/<id>/` 中的实时录制。

| 服务商 | 设置 | 来源 |
|---|---|---|
| Cohere（`cohere`） | `stop_list()`、`reasoning_fields(["reasoning_content"])`；拒绝 `audio`、`function_call`、`functions`、`logit_bias`、`metadata`、`modalities`、`parallel_tool_calls`、`prediction`、`service_tier`、`store`、`top_logprobs`、`verbosity` 和 `web_search_options` | Cohere 的 [Compatibility API](https://docs.cohere.com/docs/compatibility-api) 页面（2026-10-30 查阅），其中列出 `store`、`metadata`、`logit_bias`、`top_logprobs`、`n`、`modalities`、`prediction`、`audio`、`service_tier` 和 `parallel_tool_calls` 不受支持；`fixtures/openai_chat/providers/cohere/errors/` 中的 422 录制，每个字段一份；`responses/recorded-functions-field` 和 `responses/recorded-web-search-options-field`，Cohere 以 200 回答并忽略该字段，`verbosity` 也是如此；`stop_list()` 依据 `errors/recorded-stop-string`（422，`stop` 必须是列表）；`responses/recorded-reasoning` 和 `streams/recorded-reasoning-stream`（`reasoning_content`） |
| DeepSeek（`deepseek`） | `max_tokens()`、`system_role()`、`reasoning_fields(["reasoning_content"])` | DeepSeek 的 API 参考和思考模式指南（`reasoning_content`），`responses/recorded-thinking-reasoning-content` 和 `streams/recorded-thinking-stream`；`fixtures/openai_chat/providers/deepseek/responses/recorded-max-completion-tokens-limit`：DeepSeek 忽略 `max_completion_tokens`（要求 20，实际 133 个 completion token），而其 API 参考记载的是 `max_tokens`；`fixtures/openai_chat/providers/deepseek/errors/recorded-plain-developer-string`：DeepSeek 对 `developer` 角色返回 422 `unknown variant` |
| Groq（`groq`） | `system_role()`、`reasoning_fields(["reasoning"])`；拒绝 `logit_bias`、`logprobs` 和 `top_logprobs`，以及消息中的 `name` 和 `refusal` | Groq 的 OpenAI 兼容性页面（https://console.groq.com/docs/openai，其中列出 `logprobs`、`logit_bias`、`top_logprobs` 和 `messages[].name` 不受支持）；`fixtures/openai_chat/providers/groq/errors/` 中的 `recorded-logit-bias`、`recorded-logprobs-top-logprobs-doc`、`recorded-tools-results-parallel-error`（assistant 消息中的 `refusal`）和 `recorded-tools-single`（Qwen 的聊天模板拒绝 `developer` 角色）；Groq 的推理指南（`reasoning`），`responses/recorded-reasoning-format-parsed` 和 `streams/recorded-stream-doc` |
| Mistral（`mistral`） | `max_tokens()`、`system_role()`、`random_seed()`、`without_end_user()`、`without_prefill()`、`greedy_top_p()`、`thinking_parts()`；拒绝 `audio`、`function_call`、`functions`、`logit_bias`、`moderation`、`prompt_cache_options`、`prompt_cache_retention`、`store`、`verbosity` 和 `web_search_options`，以及消息中的 `name` 和 `refusal` | Mistral 的 OpenAPI 文档（https://docs.mistral.ai/openapi.yaml，2026-10-19 查阅），其中 `ChatCompletionRequest` 及其消息类型（如 `AssistantMessage`）禁止未声明的字段，并规定 `random_seed` 的 `minimum: 0`；`fixtures/openai_chat/providers/mistral/errors/` 中的 `422 extra_forbidden` 录制文件，每个被拒绝或改名的字段一个；`responses/recorded-audio-field`，其中 Mistral 接受 `audio` 却只以文本回答；另有 `recorded-assistant-prefill`（400 `invalid_request_message_order`）和 `responses/recorded-assistant-prefix`，其回复重复了预填内容；`thinking_parts()` 依据 `responses/recorded-reasoning-effort-high` 和 `streams/recorded-reasoning-effort-high-stream`；`greedy_top_p()` 依据 `errors/greedy-sampling`（400 `invalid_request_greedy_sampling`） |
| Moonshot（`moonshotai`） | `system_role()`、`partial_prefill()`、`reasoning_fields(["reasoning_content"])`；拒绝 `audio`、`metadata`、`modalities`、`parallel_tool_calls`、`service_tier`、`store`、`verbosity` 和 `web_search_options` | Moonshot 的 Chat Completions 参考（https://platform.kimi.ai/docs/api/chat，2026-10-30 查阅），其中有 `reasoning_content` 和 Partial Mode 的 `partial: true`；`errors/recorded-developer-role`（400 `role 'developer' is not allowed`）；`responses/recorded-prefill-without-partial`，其回复重复了预填内容，以及续写预填内容的 `responses/recorded-partial`；`responses/recorded-reasoning` 和 `streams/recorded-reasoning-stream`；`responses/recorded-parallel-tool-calls-false`，其中 Moonshot 不顾 `parallel_tool_calls: false` 调用了两次工具；2026-10-30 的实时测试中，Moonshot 对 `audio`（连同 `modalities`）、`metadata`、`service_tier`、`store`、`verbosity` 和 `web_search_options` 返回 200，且均未处理 |
| Ollama Cloud（`ollama-cloud`） | `max_tokens()`、`reasoning_fields(["reasoning"])`；拒绝 `logit_bias`、`logprobs` 和 `top_logprobs` | Ollama 的 OpenAI 兼容性页面（消息和增量中的 `reasoning`）；`responses/recorded-reasoning-effort` 和 `streams/recorded-reasoning-stream`；`responses/recorded-max-completion-tokens-ignored`（要求 20，生成 150 个 completion token）和 `responses/recorded-max-tokens-honoured`（要求 20，生成 20，`finish_reason: "length"`）；2026-10-30 的实时测试中，Ollama 对 `logprobs`、`top_logprobs` 和 `logit_bias` 返回 200，且没有返回对数概率 |
| OpenRouter（`openrouter`） | `reasoning_fields(["reasoning"])`、`error_detail(["error", "metadata", "raw"])` | OpenRouter 的推理 token 指南（`reasoning`，旁边还有 `reasoning_details`）；`responses/recorded-free-reasoning` 和 `streams/recorded-free-reasoning-stream`；OpenRouter 的错误与调试页面（`metadata.raw`，即上游服务商的错误），`errors/recorded-upstream-rate-limited` 和 `errors/recorded-routing-fields` |
| QwenCloud（`alibaba`） | `system_role()`、`thinking_switch()`、`reasoning_fields(["reasoning_content"])`；拒绝 `audio`、`logit_bias`、`metadata`、`modalities`、`prediction`、`service_tier`、`store`、`verbosity` 和 `web_search_options` | 百炼的[深度思考](https://www.alibabacloud.com/help/en/model-studio/deep-thinking)页面（`enable_thinking`、`thinking_budget`、`reasoning_content`，2026-10-30 查阅）；`fixtures/openai_chat/providers/alibaba/errors/recorded-developer-role`（`developer` 角色返回 400）；`responses/recorded-reasoning-effort-alone`（在 `qwen-flash` 上使用 `reasoning_effort: "high"`：没有思考）和 `responses/recorded-enable-thinking`（100 token 预算用满 100 个推理 token）；`responses/recorded-thinking` 和 `streams/recorded-thinking-stream`；`responses/recorded-logit-bias-field`、`recorded-store-metadata-fields`、`recorded-prediction-field`、`recorded-audio-fields` 和 `recorded-web-search-options-field`，百炼对它们以 200 回答，就像没有这些字段 |
| SpaceX（`xai`） | `reasoning_fields(["reasoning_content"])`、`with_message_bookkeeping()`、`tool_schema_root(ToolSchemaRoot::ObjectUnions)` | xAI 的 Chat Completions 参考（`reasoning_content`）；xAI 的函数调用指南（对象根部或对象组成的联合）；Grok CLI（xAI 自己的客户端）在每个助手轮次都向它发送 `model_id`（`requests/grok-cli-model-id.json`）；`responses/recorded-reasoning-content` 和 `streams/recorded-reasoning-content-stream` |
| Together AI（`togetherai`） | `reasoning_fields(["reasoning_content", "reasoning"])` | Together AI 的 chat completions 参考；键名取决于模型：`responses/recorded-sampling-extras`（`reasoning_content`），`responses/recorded-reasoning-reply` 和 `streams/recorded-reasoning-stream`（`reasoning`） |
| Vercel（`vercel`） | `reasoning_fields(["reasoning"])` | Vercel 的 Chat Completions 推理页面（`message.reasoning` 和 `delta.reasoning`，旁边还有 `reasoning_details`）；`responses/recorded-reasoning` 和 `streams/recorded-reasoning-stream` |

## 使用此格式的服务商

提供 Chat Completions 的其他服务商（OpenAI 以外）会录制到 `fixtures/openai_chat/providers/<id>/`（[fixture](fixtures.md)）。它们的额外字段遵循上述规则；本节列出每个服务商增加的内容。

### DeepSeek

来源：DeepSeek 的 [API 参考](https://api-docs.deepseek.com/api/create-chat-completion)、[思考模式指南](https://api-docs.deepseek.com/guides/thinking_mode/)和[错误码](https://api-docs.deepseek.com/quick_start/error_codes)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/deepseek/` 中的实时录制。

| DeepSeek 字段 | KPP | 说明 |
|---|---|---|
| `thinking`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Chat 应用带有该字段的助手轮次会将其发回 DeepSeek。Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项；它们发回时，该推理会以 `reasoning_content` 返回给 DeepSeek（[推理](reasoning.md)），这正是其思考模式在工具调用轮次中所期望的。 |
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

### Ollama Cloud

来源：Ollama 的 [Cloud](https://docs.ollama.com/cloud)、[身份验证](https://docs.ollama.com/api/authentication)、[OpenAI 兼容性](https://docs.ollama.com/api/openai-compatibility)和[错误](https://docs.ollama.com/api/errors)页面（2026-10-30 阅读），以及 `fixtures/openai_chat/providers/ollama-cloud/` 中用免费套餐的 `gpt-oss:20b` 和 `gemma4:31b` 录制的实时记录。

| Ollama Cloud 字段 | KPP | 说明 |
|---|---|---|
| 带标签的模型 ID，例如 `gpt-oss:20b` | `model` | 原样发送，在 `GET /v1/models/{model}` 中也是如此。 |
| `max_completion_tokens`（被忽略）、`max_tokens`（生效） | `max_output_tokens` | 配置中的 `max_tokens()` 把每个应用的输出上限作为 `max_tokens` 发送。 |
| `reasoning: {effort}`（请求） | extras | Ollama 在 `reasoning_effort` 之外的写法；请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置中的 `reasoning_fields`，与 Groq 相同。推理块旁边带有 `content: ""`，不会打开文本块。其他格式的应用发回的推理会以助手轮次中的 `reasoning` 送达 Ollama。 |
| `system_fingerprint`（`fp_ollama`） | extras | 发往其他方言时省略。 |
| `usage.prompt_tokens_details.cached_tokens` | `Usage.cache_read_tokens` | Ollama 不报告推理 token。 |
| `delta` 为空的结束块，随后是 `choices: []` 的用量块 | `Usage` | 与 OpenAI 的发送方式相同。 |
| `logprobs`、`top_logprobs`、`logit_bias` | 拒绝 | Ollama 接受这些字段但不返回任何结果；配置在发送前拒绝它们。 |
| `auto` 以外的 `tool_choice`、`response_format` | 发送 | 在 2026-10-30 的实时测试中，Ollama 的云端模型忽略了这两个字段；Kaeso 原样发送。 |
| 助手消息中的 `refusal` 内容部分 | 发送 | Ollama 返回 400 `invalid message format`（`errors/recorded-content-array-all-roles`），应用会以自己的格式收到。 |
| OpenAI 格式的错误，`type` 为 `api_error`、`not_found_error` 或 `invalid_request_error` | `KppError` | 类型取决于状态码：401 表示密钥错误，402 表示套餐不包含该模型（`errors/recorded-model-outside-plan`），404 表示模型不存在。 |
| `GET /v1/models`：OpenAI 的列表、`owned_by: "ollama"`，以及顶层采用 Codex 模型目录格式的 `models` 数组 | `ModelList`、extras | Ollama 返回模型列表和 `GET /v1/models/{model}` 时不检查密钥，因此模型检查无法发现错误的密钥。 |

### Moonshot

来源：Moonshot 的 [Chat Completions](https://platform.kimi.ai/docs/api/chat)、[模型参数](https://platform.kimi.ai/docs/api/models-overview)、[思考模型](https://platform.kimi.ai/docs/guide/use-thinking-models)、[Partial Mode](https://platform.kimi.ai/docs/guide/use-partial-mode-feature-of-kimi-api)和[错误](https://platform.kimi.ai/docs/api/errors)页面（2026-10-30 阅读），以及 `fixtures/openai_chat/providers/moonshotai/` 中用 `kimi-k2.6` 和 `kimi-k3` 录制的实时记录。

| Moonshot 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置中的 `reasoning_fields`，与 DeepSeek 相同。关闭思考时，Moonshot 发送 `reasoning_content: ""`，它保留为 extras。其他格式的应用发回的推理会以助手轮次中的 `reasoning_content` 送达 Moonshot，这正是 Moonshot 对其思考模型的要求。 |
| `thinking: {type}`（请求） | extras | `kimi-k2.6` 的开关；`kimi-k2.7-code` 只接受 `enabled`（否则返回 400）。Chat 应用发送的按原样发送；请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| `reasoning_effort` | `reasoning.effort` | K3 模型的文档列出 `low`、`high` 和 `max`，KPMR 也是如此，因此 Kaeso 在发送前拒绝其他级别。Moonshot 本身也接受 `none`，此时关闭思考（`responses/recorded-reasoning-effort-none`）。`kimi-k2.6` 在 `none` 时关闭思考；`kimi-k2.7-code` 始终思考，对 `none` 返回 400。 |
| `temperature`、`top_p`、`n`、`frequency_penalty`、`presence_penalty` | `sampling` | 每个模型只接受一个值：`temperature` 为 1（关闭思考的 `kimi-k2.6` 为 0.6），`top_p` 为 0.95，`n` 为 1，惩罚参数为 0。其他值会得到 400 `invalid temperature: only 1 is allowed for this model`（`errors/recorded-temperature`、`errors/recorded-top-p`）。目录将这些模型标记为不接受 `temperature`，因此 Kaeso 在发送前拒绝 1 以外的值；它从不修改该值。 |
| `max_tokens`、`max_completion_tokens` | `max_output_tokens` | 两者都生效（`responses/recorded-max-tokens-honoured`、`responses/recorded-max-completion-tokens-honoured`）；思考内容计入上限。 |
| 最后一条 assistant 消息中的 `partial: true` | 消息 extras | Partial Mode：回复接着该消息续写，不含预填内容。`partial_prefill()` 为其他格式的应用加上它；没有它时，Moonshot 会重新回答并重复预填内容（`responses/recorded-prefill-without-partial`）。 |
| `developer` 角色 | `Role::System` | 返回 400 被拒绝；`system_role()` 改为发送 `system`。 |
| `usage.cached_tokens`（与 `usage.prompt_tokens_details.cached_tokens` 并列）以及 `prompt_tokens_details.cache_write_tokens` | `Usage` | 缓存数量从 `prompt_tokens_details` 读取；顶层的副本保留在原始用量中。 |
| `logprobs`、`top_logprobs`、`stop`、`seed`、`response_format`、`tool_choice`、`prediction`、`user`、`safety_identifier`、`prompt_cache_key`、消息中的 `name` | 发送 | Moonshot 接受这些字段。模型思考时，它对 `tool_choice: "required"` 返回 400（`errors/recorded-tool-choice-required-thinking`）。 |
| `logit_bias`、`functions`、助手消息中的 `refusal` 内容部分 | 发送 | Moonshot 返回 400，应用会以自己的格式收到（`errors/recorded-logit-bias-refused`）。 |
| `audio`、`metadata`、`modalities`、`parallel_tool_calls`、`service_tier`、`store`、`verbosity`、`web_search_options` | 拒绝 | Moonshot 返回 200 并忽略它们；配置在发送前拒绝它们。 |
| OpenAI 格式的错误，带有 Moonshot 自己的 `type`，例如 `invalid_authentication_error`、`resource_not_found_error` 或 `rate_limit_reached_error` | `KppError` | 类型取决于状态码；根据 Moonshot 的错误页面，`exceeded_current_quota_error`（余额用尽时的 429）属于计费拒绝，`engine_overloaded_error`（429）属于过载（`errors/exceeded-current-quota`、`errors/engine-overloaded`）。速率限制消息中含有账户的组织和密钥 ID（`ak-…`）；网关会从每条服务商错误消息中去掉两者。 |
| `GET /v1/models`：OpenAI 的列表，`owned_by: "moonshot"`，每个模型还带有 `context_length`、`supports_reasoning`、`reasoning`、`modalities` 等字段 | `ModelList`、extras | 错误的密钥会得到 401，因此模型检查能发现它。 |

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

### Vercel

来源：Vercel 的 [Chat Completions](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions)、[reasoning](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/reasoning)和[advanced configuration](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions/advanced) 页面（2026-10-29 阅读），以及 `fixtures/openai_chat/providers/vercel/` 中的实时录制。Chat Completions 是 Vercel 的 `generation` 方言，它列出的每个模型都支持。

| Vercel 字段 | KPP | 说明 |
|---|---|---|
| `providerOptions`（`gateway` 下的路由，例如 `order`、`only`、`sort`、`models`、`byok`、`tags`、`user`）、`models`、`provider`、`reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同（`requests/gateway-options`）。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`，与 OpenRouter 相同。 |
| `message.reasoning_details`、`delta.reasoning_details` | extras | OpenRouter 结构化形式的同一推理；对于既非 OpenAI 也非 Anthropic 的模型，`format` 为 `"unknown"`。 |
| 消息中和最后一个数据块 delta 中的 `provider_metadata`（`gateway.routing`、`cost`、`marketCost`、`gatewayCost`、`generationId`），以及顶层的 `generationId` | extras | 发往其他方言时省略。录制中的 `clientSessionId` 是客户端指纹，已替换为零。 |
| `usage.cost`、`market_cost`、`gateway_cost`、`surcharge_cost`、`reporting_tag_write_cost`、`is_byok`、`cost_details`、`cache_creation_input_tokens` 及额外的 Token 明细 | 用量的 `raw` | Kaeso 按目录中的价格计算费用。`usage.cost` 只是推理费用：`requests/gateway-options` 中的标签产生了 0.00015 美元的附加费，只有 `gateway_cost` 包含它。 |
| 最后一个流式数据块，同时带 `finish_reason`、`provider_metadata` 和 `usage` | `Usage` | 按 `choices: []` 的用量数据块读取。 |
| OpenAI 错误结构中的错误，带 Vercel 自己的 `type`（`model_not_found`、`no_providers_available`、`authentication_error`）、作为对象的 `param`，以及与 `error` 并列的 `providerMetadata` | `KppError` | 类别取决于状态码：401 表示密钥错误，403 表示账户无权调用该模型（`errors/recorded-restricted-model`，免费套餐账户调用 Claude），404 表示模型未知。Chat 应用收到 Vercel 发来的原样错误体。 |
| `GET /v1/models`：OpenAI 的结构，每个模型带 Vercel 自己的字段（`name`、`type`、`context_window`、`max_tokens`、`pricing`、`tags`、`supported_parameters`） | `ModelList`、extras | 模型列表对错误的密钥返回 401（`errors/list-invalid-api-key`），但模型检查首先询问的 `GET /v1/models/{model}` 不检查密钥就会回答。 |

### Cohere

来源：Cohere 的 [Compatibility API](https://docs.cohere.com/docs/compatibility-api) 页面（2026-10-30 查阅），以及 `fixtures/openai_chat/providers/cohere/` 中使用 `command-a-03-2025`、`command-a-reasoning-08-2025` 和 `command-a-vision-07-2025` 的实时录制。Kaeso 的基础 URL 为 `https://api.cohere.ai/compatibility`，使用 `Authorization: Bearer`。

| Cohere 字段 | KPP | 说明 |
|---|---|---|
| `developer` 消息、`max_completion_tokens`、`seed`、`tool_choice`（`auto`、`none`、`required`、指定函数）、严格工具、带 JSON Schema 的 `response_format`、视觉模型上的 `image_url` 部分 | 与 OpenAI 相同 | 录制于 `responses/`。`developer` 消息的作用与 `system` 消息相同。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；`completion_tokens_details.reasoning_tokens` 计入其中。 |
| `reasoning_effort` | `reasoning.effort` | 仅 `none` 和 `high`；目录为 Cohere 的推理模型提供这两个级别，因此 Kaeso 会在发送前拒绝或映射其他级别。Cohere 对 `medium` 和 `low` 返回 422（`errors/recorded-reasoning-effort-medium`）。 |
| Cohere 未声明的字段 | — | Cohere 以 200 回答并忽略它们，因此配置会拒绝应用所依赖的字段，例如 `functions` 和 `web_search_options`。 |
| `usage`，含为零的 `audio_tokens` 和预测 token 明细 | `Usage`、`raw` 用量 | 流只有在设置 `stream_options.include_usage`（网关会设置）时才发送用量，位于 `choices: []` 的最后一个块中（`streams/recorded-stream`；`streams/recorded-stream-no-options` 没有）。 |
| 错误：`{"id", "message"}`，有时带 `error_type` | `KppError` | 没有 OpenAI 的 `error` 对象。错误类型取决于状态码：密钥错误为 401，未知模型为 404，Cohere 拒绝的请求为 400 和 422。应用会收到带有 Cohere 消息的 OpenAI 错误，即 ``The provider returned an error (HTTP 404): model '…' not found, …``；`openai_chat::tests::cohere_error_bodies_reach_apps_with_cohere_message_and_status` 代替往返测试检查每个录制的响应体。有些值要到发送后才被 Cohere 拒绝：同时使用 `frequency_penalty` 和 `presence_penalty`、大多数模型上的 `logprobs`、超过模型输出上限的 `max_tokens`、发给无视觉能力模型的图像，以及 `file` 和 `input_audio` 部分。 |
| `GET /v1/models`、`GET /v1/models/{model}` | `ModelList`、`ModelInfo` | OpenAI 的格式，带 `owned_by: "cohere"`；两者都以 401 拒绝错误的密钥（`errors/list-invalid-api-key`）。 |

Cohere 自身 Chat API 的字段（`documents`、`citation_options`、`connectors`）和重排序无法通过 Compatibility API 使用。录制时，Cohere 有时会流式返回与问题无关的 token，还有一次返回 422 `NO_VALID_RESPONSE_GENERATED`（`errors/recorded-no-valid-response`）；两种情况重试后都恢复正常。

### QwenCloud

来源：阿里云百炼（Model Studio，现为 QwenCloud）的 [OpenAI 兼容](https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope)、[深度思考](https://www.alibabacloud.com/help/en/model-studio/deep-thinking)和 [API 密钥](https://www.alibabacloud.com/help/en/model-studio/get-api-key)页面（2026-10-30 查阅），以及 `fixtures/openai_chat/providers/alibaba/` 中在国际站（新加坡）地域使用 `qwen3.7-flash`、`qwen3.8-flash`、`qwen-flash` 和 `qwen3-vl-plus` 的实时录制。Kaeso 的基础 URL 为 QwenCloud 的 `https://ap-southeast-1.maas.qwencloudapi.com/compatible-mode`，使用 `Authorization: Bearer`。录制是在百炼的 `https://dashscope-intl.aliyuncs.com/compatible-mode` 上进行的，它提供相同的 API，使用相同的密钥。

| QwenCloud 字段 | KPP | 说明 |
|---|---|---|
| `max_completion_tokens`、`seed`、`stop`、两种惩罚参数、`logprobs`、`tool_choice`（`auto`、`none`、`required`、指定函数）、`parallel_tool_calls`、带 JSON Schema 的 `response_format`、旧版 `functions`、预填的助手消息、视觉模型上的 `image_url` 部分 | 与 OpenAI 相同 | 录制于 `responses/`。指定函数的 `tool_choice` 以 `finish_reason: "tool_calls"` 回答；`tool_choice` 为 `allowed_tools` 时返回 400（`errors/recorded-allowed-tools`）。 |
| `enable_thinking`、`thinking_budget`（请求） | `reasoning.mode`、`reasoning.budget_tokens` | 通过配置的 `thinking_switch`。QwenCloud 以 400 拒绝为 0 或超过 131072 的预算、不小于 `max_completion_tokens` 的预算、`kimi-k3` 上的 `thinking_budget`（`errors/recorded-thinking-budget-*`），以及只能思考的模型上的 `enable_thinking: false`（`errors/recorded-thinking-only-model-off`）。 |
| `reasoning_effort` | `reasoning.effort` | 与 `enable_thinking: true` 一起发送给目录条目列出了强度级别的模型，例如 `qwen3.8-max` 和 `kimi-k3`；其他模型会在发送前拒绝强度。QwenCloud 自己也会检查取值（`errors/recorded-reasoning-effort-max`）。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。思考块旁带有 `content: ""`，不会开启文本块。应用回传的思考以助手轮次上的 `reasoning_content` 发送给 QwenCloud，QwenCloud 会接受（`responses/recorded-reasoning-sent-back`）。 |
| `max_tokens` | `max_output_tokens` | 只限制回答：开启思考时 `max_tokens: 50` 得到 453 个 completion token（`responses/recorded-max-tokens-beyond-reasoning`），而 `max_completion_tokens` 同时限制两者（`responses/recorded-max-completion-tokens-thinking`）。除非 Chat 应用写了 `max_tokens`，Kaeso 都发送 `max_completion_tokens`；预留花费时，对思考模型按模型的输出上限计算。 |
| `usage.prompt_tokens_details.text_tokens`、`image_tokens`、`completion_tokens_details.text_tokens` | `raw` 用量 | QwenCloud 自己的 token 明细。 |
| `system_fingerprint`、错误顶层的 `request_id` | 扩展字段 | 发往其他方言时省略。 |
| 流中 `id: ""`、随后 `id: null` 的参数块 | `Delta::ToolCall` | 它们延续第一个块开启的调用（`streams/recorded-tools-stream`）。 |
| QwenCloud 未声明的字段 | — | QwenCloud 以 200 回答并忽略它们，因此配置会拒绝应用所依赖的字段，例如 `logit_bias`、`store` 和 `web_search_options`。 |
| OpenAI 格式的错误，`code` 例如 `invalid_api_key`、`model_not_found` 或 `invalid_parameter_error` | `KppError` | 错误类型取决于状态码：密钥错误为 401，免费额度账号用完某个模型的免费额度时为 403（`errors/recorded-free-tier-exhausted`），未知模型为 404，其余为 400，其中包括只回答流式请求的模型（`errors/recorded-stream-only-model`）。 |
| `GET /v1/models`、`GET /v1/models/{model}` | `ModelList`、`ModelInfo` | OpenAI 的格式，带 `owned_by: "system"`，所有模型在一页中返回，`first_id` 为 `model-id-0`；两者都以 401 拒绝错误的密钥（`errors/list-invalid-api-key`）。 |

在这个地域，QwenCloud 还在 `/compatible-mode/v1/responses` 提供 OpenAI Responses，在 `/apps/anthropic/v1/messages` 提供 Anthropic Messages；只有 Chat Completions 和 Embeddings 经过录制并列出。密钥只能在其所属地域使用：中国（北京）地域为 `https://dashscope.aliyuncs.com/compatible-mode`，使用该地域自己的密钥。
