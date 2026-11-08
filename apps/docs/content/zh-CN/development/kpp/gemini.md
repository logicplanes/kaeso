---
title: Gemini API
description: Gemini API 的每个字段如何映射到 KPP 以及如何映射回来，涵盖生成、Token 计数、嵌入和模型列表。
order: 9
---

在这里查阅 Gemini API 的任意字段：它在 KPP 中的位置，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::gemini::Gemini`（`crates/kaeso-codecs/src/gemini/`）和 `kaeso_codecs::gemini_embeddings::GeminiEmbeddings`（`crates/kaeso-codecs/src/gemini_embeddings/`）
- **端点：**`POST {models}/{model}:generateContent`、`:streamGenerateContent?alt=sse`、`:countTokens`、`:embedContent` 和 `:batchEmbedContents`，以及[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/gemini/` 和 `crates/kaeso-codecs/fixtures/gemini_embeddings/`

**来源。** 这些映射遵循以下来源：

- Gemini API 的发现文档 `https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta`，修订版 `20261106`。下文的 schema 名称都指向它。
- [Gemini API 文档](https://aistudio.google.com/docs)，阅读于 2026-11-08：文本生成、思考、思考签名、函数调用、结构化输出、Token 计数、嵌入、文件输入方式、模型和问题排查。
- 官方 Python SDK `google-genai` 2.32.0，[SDK 一致性测试套件](../testing.md#sdk-一致性)用它测试编码器。

## 表格的读法

- **KPP**：线路值映射到的类型化字段。
- **字段**：原样保存在 `Extras::fields` 中，绑定到 `gemini`（或 `gemini_embeddings`）。其他方言会以 `Unsupported` 拒绝它们（[Extras](cross-format.md#extras)）。嵌套对象使用带点的键路径，例如 `generationConfig.responseModalities`。
- **形式标记**：`Extras::form` 中编解码器私有的表现形式标记，其他方言会忽略。`verbatim` 指 `wire::Ext` 的通用机制（`null`、`false` 和 `[]` 的写法）。
- **原样**：完整保存在 `RawItem` 或 `RawUsage` 中。
- **自有**数据带有 extras 或来源 `gemini`；**外来**数据来自其他方言。

## 路径

Gemini 请求在路径中写明模型和方法，因此编解码器根据请求构建路径，并依据两者解码请求。

| 部分 | 规则 |
|---|---|
| 模型路径 | `EgressCtx::gemini_profile` 上的 `GeminiProfile::models_path`。`GeminiProfile::GOOGLE_AI_STUDIO` 为 `/v1beta/models`；`with_models_path` 可为每个请求设置其他路径，例如 `/v1/projects/{project}/locations/{location}/publishers/google/models`。 |
| 模型 | 一个经过百分号编码的路径段，`:` 也会编码。开头的 `models/` 是同一模型的资源名，会被去掉；`tunedModels/<id>` 指向模型路径旁边的微调模型。`.` 和 `..` 为 `InvalidRequest`，与[获取模型](codec-design.md#模型列表)相同。 |
| 方法 | `GeminiAction`：`generateContent` 和 `streamGenerateContent`（`RequestKind::Generate`，是否流式由方法决定）、`countTokens`（`CountTokens`）、`embedContent` 和 `batchEmbedContents`（`Embed`，即方言 `gemini_embeddings`）。`streamGenerateContent` 会加上 `?alt=sse`。 |
| 解码 | `dispatch::decode_gemini_request(model, action, body, headers)`；对 `limits::Checked` 已计数的请求体使用 `decode_checked_gemini_request`。`gemini::split_model_action` 拆分 `gemini-3.5-flash:streamGenerateContent` 这样的路径段。不带路径的 `dispatch::decode_request` 会以空模型解码生成请求体。 |
| 每个 content 都带角色 | `GeminiProfile::role_on_every_content`：应用发送的不带 `role` 的 content 会得到 `user`。Google AI Studio 下关闭。 |

## 请求

Schema `GenerateContentRequest`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `contents[]`（`Content`） | `messages[]` | 每个 content 一条消息，相邻的同角色 content 按原样保留。未知键 → 消息的字段。 |
| `contents[].role` `user` / `model` | `Role::User` / `Role::Assistant` | 缺失 → `Role::User`，形式标记 `role: "absent"`。其他角色 → 一条用户消息，把整个 content 作为一个未知块保存，跨方言时以 `` `<role>` message `` 拒绝（[请求](cross-format.md#请求)）。Gemini 曾回答过角色为 `assistant` 的 content（`requests/role-assistant.json`）。 |
| `systemInstruction` | `system` | 其 part 作为块；`role` → 形式标记 `systemInstruction.role`；未知键 → 字段 `systemInstruction.*`。系统指令只包含文本，因此外来的非文本系统块为 `Unsupported`。 |
| 只含 `functionDeclarations` 的 `tools[]` | `tools[]`，即 `ToolDef::Function` | 每个声明所在 Tool 的索引 → 形式标记 `group`，使同一方言内编码时保留分组。外来工具放进同一个 Tool。 |
| 含其他键（`googleSearch`、`urlContext`、`codeExecution`、`googleMaps`、`fileSearch`、`computerUse`、`mcpServers`、`googleSearchRetrieval`）或 `functionDeclarations` 为空的 `tools[]` | `ToolDef::Unknown`（原样） | 跨方言时被拒绝。 |
| `functionDeclarations[].name`、`.description` | `name`、`description` | |
| `functionDeclarations[].parametersJsonSchema` | `parameters` | 完整的 JSON Schema。外来工具总是得到 `parametersJsonSchema`，因此不会因 OpenAPI 子集而丢失内容。 |
| `functionDeclarations[].parameters`（OpenAPI `Schema`） | `parameters` | 形式标记 `schema: "parameters"`，使 Gemini 在 `parameters` 中收回它。其他方言按原样把它当作 JSON Schema 接收；需要标准类型名的服务商会拒绝 `OBJECT` 这样的大写类型名。旁边同时有 `parametersJsonSchema` 时 → 字段 `parameters`。 |
| `functionDeclarations[].response`、`.responseJsonSchema`、`.behavior` | 工具的字段 | |
| `toolConfig.functionCallingConfig` `mode: AUTO` / `NONE` / `ANY` | `tool_choice` `Auto` / `None` / `Required` | |
| `mode: ANY` 且 `allowedFunctionNames` 只有一项 | `ToolChoice::Tool(name)` | |
| `mode: VALIDATED`、带多个名称的 `ANY`，以及其他任何组合 | `ToolChoice::Unknown`（原样） | 跨方言时被拒绝。 |
| `toolConfig.retrievalConfig`、`.includeServerSideToolInvocations` | 字段 `toolConfig.*` | |
| `generationConfig.temperature`、`.topP`、`.topK`、`.maxOutputTokens`、`.stopSequences`、`.seed`、`.presencePenalty`、`.frequencyPenalty` | `sampling.*` | 数字在方言内保留写法（形式标记 `integers`）。 |
| `generationConfig.candidateCount` | 形式标记 `candidate_count` | 大于 1 → 解码时为 `Unsupported`，与 Chat 的 `n` 相同（[采样参数](cross-format.md#采样参数)）。 |
| `responseMimeType: application/json` 加 `responseJsonSchema` | `output = JsonSchema` | |
| `responseMimeType: application/json` 加 `responseSchema` | `output = JsonSchema` | 形式标记 `schema: "responseSchema"`；其他方言按原样收到 OpenAPI schema。 |
| 只有 `responseMimeType: application/json` | `output = JsonObject` | |
| 其他任何 `responseMimeType`，以及没有 `application/json` 的 schema | 字段 `generationConfig.*` | |
| `generationConfig.thinkingConfig` | `reasoning` | 见[思考](#思考)。 |
| `generationConfig.responseModalities`、`.speechConfig`、`.imageConfig`、`.mediaResolution`、`.responseLogprobs`、`.logprobs`、`.audioTranscriptionConfig`、`.translationConfig`、`.enableAffectiveDialog`、`.enableEnhancedCivicAnswers`、`.responseFormat` | 字段 `generationConfig.*` | |
| `safetySettings`、`cachedContent`、`labels`、`serviceTier`、`store`、`continuationToken` | 字段 | |
| `model`（在请求体中） | 形式标记 `model` | 模型由路径决定。 |

### Part

Schema `Part`。一个 part 恰好带有 `text`、`inlineData`、`fileData`、`functionCall` 和 `functionResponse` 之一；其他任何 part 都完整保留。

| 线路 part | KPP | 说明 |
|---|---|---|
| `text` | `Block::Text` | `thought: false` → 形式标记 `thought`。 |
| 带 `thought: true` 的 `text` | `Block::Reasoning` | 其文本，以及作为不透明数据的 `thoughtSignature`。空的思考 part → 形式标记 `thought_part`。 |
| 其他任何 part 上的 `thoughtSignature` | 该 part 的块之前的一个 `Block::Reasoning` | 见[思考签名](#思考签名)。 |
| `inlineData` `image/*` | `Block::Image`，Base64 | `displayName` → 字段 `inlineData.*`。 |
| 其他类型的 `inlineData` | `Block::Document`，Base64 | |
| 带 Files API URI（`files/…` 或 `https://generativelanguage.googleapis.com/v1beta/files/…`）的 `fileData` | 图像或文档，`Source::FileId` | 只能在 Gemini 使用（[文件](cross-format.md#文件)）。`mimeType` → 形式标记 `mime_type`。 |
| 带其他 URI 的 `fileData` | 图像或文档，`Source::Url` | `mimeType` 为 `image/*` 时为图像。 |
| `audio/*` 或 `video/*` 的 `inlineData` 或 `fileData` | `Block::Unknown`（原样） | KPP 尚无音频或视频块；跨方言时被拒绝。 |
| `functionCall` | `Block::ToolCall` | 见[函数调用](#函数调用)。 |
| `functionResponse` | `Block::ToolResult` | 见[函数调用](#函数调用)。 |
| `executableCode`、`codeExecutionResult`、`toolCall`、`toolResponse`、未知 part、带两个数据字段的 part | `Block::Unknown`（原样） | 该 part 保留自己的 `thoughtSignature`。跨方言时被拒绝。 |
| `videoMetadata`、`mediaResolution`、`partMetadata`、`mediaProcessing` 以及 part 的其他键 | 块的字段 | |

### 函数调用

| 线路字段 | KPP | 说明 |
|---|---|---|
| `functionCall.id` | `ToolCall.id` | 缺失 → `kaeso_fc_<seed>_<n>`，形式标记 `no_id`。seed 是回复的 `responseId`，在请求中则是 content 和 part 的索引。带此前缀的 ID 发往 Gemini 时会再次省略。 |
| `functionCall.name`、`.args` | `name`、`arguments`（原样） | 缺少 `args` → `{}`，形式标记 `no_args`。 |
| `functionResponse.id` | `ToolResult.call_id` | 缺失 → 同名的最早一个未答复调用，形式标记 `no_id`。 |
| `functionResponse.name` | — | 从调用推导；名称不同 → 形式标记 `name`。 |
| `functionResponse.response` `{"output": "<text>"}` | 一个文本块 | 文档约定的输出写法（`FunctionResponse.response`）。 |
| `functionResponse.response` `{"error": "<text>"}` | 一个文本块，`is_error` | 文档约定的错误写法。 |
| `functionResponse.response`，其他任何对象 | 一个包含紧凑 JSON 的文本块 | 形式标记 `response: "json"`：Gemini 收回该对象；其他方言把 JSON 当作文本读取。 |
| `functionResponse.parts[]` | 文本之后的图像和文档块 | 未知 part 保持原样。 |
| `functionResponse.willContinue`、`.scheduling` | 字段 `functionResponse.*` | |

### 来自其他方言的请求

- **系统消息**出现在第一轮之后时，会并入下一个用户轮次，与 [Anthropic](cross-format.md#系统消息) 相同；开头的系统消息并入 `systemInstruction`。
- **工具结果**若分布在相邻的多条用户消息中（Chat Completions 就是这样发送的），会放进同一个 content，因此并行调用以 `FC1, FC2, FR1, FR2` 的顺序答复；思考签名指南表示，调用与结果交错时 API 会返回 400。
- **函数响应**从同一 ID 的调用获取名称。调用不在请求中的结果为 `InvalidRequest`。文本变为 `{"output": …}`，错误变为 `{"error": …}`，各文本块拼接在一起；图像和文档放进 `functionResponse.parts`，Gemini 3 模型接受这种写法。
- 其他方言的 **ID** 作为 `functionCall.id` 和 `functionResponse.id` 发送。
- 不是对象的**参数**会变为对象，与 Anthropic 相同（[工具调用参数](cross-format.md#工具调用参数)）。
- **图像 URL** 按 URL 的文件扩展名得到 `fileData.mimeType`（`.png`、`.jpg`、`.jpeg`、`.webp`、`.gif`、`.heic`、`.heif`；文档另有 `.pdf`、`.txt`），这是文件输入指南的要求；没有这类扩展名的图像 URL 为 `Unsupported`。文本文档以 Base64 `inlineData` 发送。
- 以 `Unsupported` **拒绝**：Gemini 没有对应 part 的拒答块（与 Anthropic 相同）、文档的标题、上下文或引用、超过五个停止序列（`GenerationConfig.stopSequences`）、`parallel_tool_calls: false`、带描述或 `strict: false` 的输出 schema，以及 Gemini 没有对应设置的推理强度或思考模式。
- **严格工具。** 带 `strict: true` 的函数工具在自动 `tool_choice` 下发送 `mode: VALIDATED`，按 schema 校验调用。
- **托管网页搜索**会被[省略](cross-format.md#省略的工具)。
- **缓存提示**会被丢弃：Gemini 会隐式缓存。
- **`user`** 会被丢弃：Gemini 没有终端用户字段。

### 思考

| 线路字段 | KPP | 说明 |
|---|---|---|
| `thinkingLevel` `MINIMAL` / `LOW` / `MEDIUM` / `HIGH` | `reasoning.effort` | 其他写法 → 形式标记 `level`；未知值 → `ReasoningEffort::Other`。 |
| `thinkingBudget: 0` | `reasoning.mode = Disabled` | |
| `thinkingBudget: -1` | `reasoning.mode = Adaptive` | 动态思考。 |
| 大于 0 的 `thinkingBudget` | `mode = Enabled`、`budget_tokens` | 其他负值 → 推理字段 `thinkingBudget`。 |
| `includeThoughts: true` | `reasoning.summary = Auto` | `false` → 形式标记 `include_thoughts`。 |

来自其他方言时，推理强度 `minimal`、`low`、`medium` 和 `high` 变为 `thinkingLevel`，`xhigh` 和 `max` 变为 `HIGH`。预算变为 `thinkingBudget`，Gemini 3 会把它映射到最接近的级别。推理强度 `none` 和关闭的思考变为 `thinkingBudget: 0`，两者都没有的自适应或开启的思考变为 `-1`。应用请求了推理摘要，或请求了不为 `omitted` 的 Anthropic 思考时，会发送 `includeThoughts: true`。规则见[推理](reasoning.md#推理强度与预算)。

### 思考签名

Gemini 对其思考签名，并把签名放在一个 part 上：函数调用、文本、图像，或流末尾的空文本 part（[思考签名](https://aistudio.google.com/docs/thought-signatures)）。

- **解码。** `thoughtSignature` 变为 `Opaque { kind: GeminiThoughtSignature }`。在带 `thought: true` 的 part 上，它留在该 part 的推理块中；在其他任何已知 part 上，它成为一个独立的推理块，来源为 `gemini`，没有文本、只有签名，紧挨在该 part 的块之前。
- **编码。** 这样只有签名的块会把签名放到 Kaeso 写出的下一个 part 上；后面没有 part 时，放到一个空文本 part 上。签名只发往 Gemini；其他方言的推理会被丢弃（[推理](reasoning.md)）。
- **跨应用。** Anthropic 和 Responses 应用在[推理数据块](reasoning.md#推理数据块)中收到该块。Chat Completions 应用收不到任何内容，网关通过 `continuity::reasoning_for_tool_calls` 和 `ReasoningCollector` 按工具调用保存签名，与保存 Anthropic 签名的方式相同（[由网关保存的推理](reasoning.md#由网关保存的推理)）。
- **占位签名。** 外来的函数调用如果是当前轮次（即最后一个不只含函数响应的用户 content 之后）中某个模型 content 的第一个调用，且前面没有签名，就会得到文档中的 `"skip_thought_signature_validator"`。`EgressCtx::adapted` 以 `thought signature of tool call <id>` 记录它。Gemini 自己的调用按收到的原样发送。

## 回复

Schema `GenerateContentResponse`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `responseId` | `id` | 缺失 → 形式标记 `no_response_id`。 |
| `modelVersion` | `model` | 缺失 → 形式标记 `no_model_version`。 |
| `candidates[0].content.parts` | `blocks` | 与请求中相同。图像 part 变为 `Block::Image`。 |
| 多于一个 candidate | `Unsupported` | |
| `candidates[0].finishReason` | `stop` | 见[结束原因](#结束原因)。 |
| `candidates[0]` 的其他键（`index`、`finishMessage`、`safetyRatings`、`citationMetadata`、`groundingMetadata`、`urlContextMetadata`、`avgLogprobs`、`logprobsResult`、`tokenCount`） | 字段 `candidate.*` | `citationMetadata` 指向 candidate 的全部文本，而 KPP 引用绑定在单个块上，无法表达这一点。 |
| `candidates[0].content.role` | — | 其他值或缺失 → 形式标记 `no_role`。 |
| 没有 `candidates` | 没有块，`stop = ContentFilter` | 形式标记 `no_candidates`；`promptFeedback` 保留为字段。 |
| `usageMetadata` | `usage` | 见[用量](#用量)。 |
| `promptFeedback`、`modelStatus` 以及其他键 | 字段 | |

来自其他方言、发往 Gemini 应用的回复会得到 `role: "model"`、`index: 0`，以及回复的 ID 和模型。其他方言的推理会被省略，与 Chat Completions 应用相同（[回复](cross-format.md#回复)）：思考 part 无法携带[推理数据块](reasoning.md#推理数据块)，因为 `google-genai` SDK 把 `thoughtSignature` 当作 Base64 读取。

### 结束原因

| `finishReason` | KPP | 回到 Gemini |
|---|---|---|
| `STOP` | `EndTurn`；回复调用函数时为 `ToolUse` | `STOP`，`StopSequence` 也是 |
| `MAX_TOKENS` | `MaxTokens` | `MAX_TOKENS` |
| `SAFETY`、`BLOCKLIST`、`PROHIBITED_CONTENT`、`SPII`、`RECITATION`、`IMAGE_SAFETY`、`IMAGE_PROHIBITED_CONTENT`、`IMAGE_RECITATION` | `ContentFilter`，原值记在形式标记 `finish` 中 | `ContentFilter` 和 `Refusal` 为 `SAFETY` |
| 其他任何值，例如 `MALFORMED_FUNCTION_CALL` | `Other(value)` | 发现文档列有该值时为该值，否则为 `OTHER` |

### 用量

`promptTokenCount` 包含已缓存的 Token 和工具使用的提示 Token，因此 `input_tokens = promptTokenCount + toolUsePromptTokenCount − cachedContentTokenCount`，`cache_read_tokens = cachedContentTokenCount`，`output_tokens = candidatesTokenCount + thoughtsTokenCount`，`reasoning_tokens = thoughtsTokenCount`（[用量](cross-format.md#用量)）。`Usage::raw` 保存完整的 `usageMetadata`。其他方言的用量写为 `promptTokenCount`、`candidatesTokenCount`、`totalTokenCount`，不为 0 时另写 `cachedContentTokenCount` 和 `thoughtsTokenCount`。

## Token 计数

Schema `CountTokensRequest` 和 `CountTokensResponse`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `contents` | `messages` | |
| `generateContentRequest` | 整个请求 | 形式标记 `count: "wrapped"`；其 `model` → 形式标记 `model`。两者同时发送为 `InvalidRequest`。 |
| `totalTokens` | `input_tokens` | |
| `cachedContentTokenCount`、`promptTokensDetails`、`cacheTokensDetails` | 字段 | |

来自其他方言、带系统提示或工具的计数请求以 `generateContentRequest` 发送，带 `model: "models/<id>"`，不带 `generationConfig`；否则以 `contents` 发送。

## 流

`streamGenerateContent?alt=sse` 把每个 chunk 作为完整的 `GenerateContentResponse` 放在一行 `data:` 中发送。函数调用整体到达：文档中没有流式的函数调用参数。

- **块。** 文本 part 延续当前的文本块，思考 part 延续当前的推理块。函数调用是一个完整的工具调用块：开始、一个参数增量、结束。思考 part 上的签名是其块中的 `Opaque` 增量；其他 part 上的签名是它之前一个只有签名的推理块。其他 part 是未知块。
- **用量与结束。** 每个 chunk 都带 `usageMetadata`；带 `finishReason` 的那个在最后一个块结束后变为 `ResponseDelta`。没有 `finishReason` 就结束的流以错误结束。
- **同方言回放。** 每个 chunk 的第一个事件带有形式标记 `chunk`：去掉 part、结束原因和最终用量之后的 chunk。编码器把它们放回去，因此每个 chunk 都按收到的原样输出。
- **其他方言**每个文本增量得到一个 chunk，每个函数调用一个 chunk，最后一个 chunk 带空文本 part、`finishReason` 和 `usageMetadata`。
- **错误。** 带 `error` 对象的 `data:` 行是错误事件；编码器也以同样方式写出错误。
- **保活消息。** 一个空行（[保活消息](cross-format.md#保活消息)）。`google-genai` SDK 把每个不以 `data: ` 开头的行当作 JSON 读取，因此注释会让它出错。
- 注释、具名事件以及不是 JSON 对象的帧作为 `Extra` 传递。

## 错误

Google 的错误体 `{"error": {"code", "message", "status", "details"}}`（[问题排查](https://aistudio.google.com/docs/troubleshooting)）。

| `status` | KPP | 说明 |
|---|---|---|
| `INVALID_ARGUMENT` | `InvalidRequest` | `details[].reason` 为 `API_KEY_INVALID` 时为 `Authentication`（`errors/recorded-error-invalid-key.json`）；消息说明输入超过最大 Token 数时为 `ContextTooLong`。 |
| `FAILED_PRECONDITION`、`OUT_OF_RANGE` | `InvalidRequest` | 例如密钥所在国家不提供图像生成。 |
| `UNAUTHENTICATED` | `Authentication` | |
| `PERMISSION_DENIED` | `PermissionDenied` | |
| `NOT_FOUND` | `NotFound` | |
| `RESOURCE_EXHAUSTED` | `RateLimited` | `RetryInfo.retryDelay`（例如 `"23s"`）→ `retry_after`。 |
| `UNAVAILABLE` | `Overloaded` | |
| `DEADLINE_EXCEEDED` | `Timeout` | |
| `INTERNAL` 及其他代码 | `ProviderError` | |
| 没有 `status` | 按 HTTP 状态码 | |

`BadRequest.fieldViolations[0].field` → `param`。Gemini 错误按原样到达 Gemini 应用。其他方言的错误以其种类对应的代码和状态写出，429 或 503 带 `RetryInfo` 和 `retry-after`：`RateLimited` 和 `Billing` 为 429 `RESOURCE_EXHAUSTED`，`Overloaded` 为 503 `UNAVAILABLE`，`Authentication` 为 401 `UNAUTHENTICATED`。

## 嵌入

编解码器 `gemini_embeddings`。Schema `EmbedContentRequest`、`BatchEmbedContentsRequest`、`EmbedContentResponse` 和 `BatchEmbedContentsResponse`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| 只有一个文本 part 的 `content` | 一个 `EmbedInput::Text` | 其他任何 content → 字段 `content`（或 `requests[i].content`）和一个空输入，跨方言时被拒绝。 |
| `requests[]` | 每项一个输入 | |
| `outputDimensionality`、`embedContentConfig.outputDimensionality` | `dimensions` | 批量中的请求各不相同时 → 字段 `requests[i].outputDimensionality`。`embedContentConfig` → 形式标记 `config_dimensions`。 |
| `taskType`、`title`、`embedContentConfig` 的其他键 | 字段 | |
| 请求体中的 `model` | — | 从 `models/<路径中的模型>` 推导；其他写法 → 形式标记 `models`。 |
| `embedding.values`、`embeddings[].values` | `data[].vector`（浮点） | `shape` → 嵌入的字段。 |
| `usageMetadata.promptTokenCount` | `usage.input_tokens` | `gemini-embedding-2` 和 `gemini-embedding-001` 都会报告它；`Usage::raw` 保存它。 |

OpenAI Embeddings 请求以 `batchEmbedContents` 发送，每个请求带 `model: "models/<id>"` 和 `outputDimensionality`；Token ID 输入为 `Unsupported`。回复按应用调用的方法返回：`embedContent` 为 `embedding`，`batchEmbedContents` 为 `embeddings`；其他方言的 Base64 向量会变为浮点数。嵌入从不流式传输。

## 模型

`GET {models}` 和 `GET {models}/{model}`（schema `ListModelsResponse` 和 `Model`）。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `name` `models/<id>` | `id` | 没有前缀 → 形式标记 `bare_name`。 |
| `displayName` | `display_name` | |
| `inputTokenLimit`、`outputTokenLimit` | `context_window`、`max_output_tokens` | |
| `version`、`baseModelId`、`description`、`supportedGenerationMethods`、`thinking`、`temperature`、`maxTemperature`、`topP`、`topK` 以及其他键 | 字段 | |
| `nextPageToken` | `has_more`、字段 `nextPageToken` | |
| 查询参数 `pageSize` | `limit` | |
| 查询参数 `pageToken` | `after_id` | Gemini 应用把令牌视为不透明值，因此 Kaeso 自己的列表用上一页的最后一个 ID 作为令牌。 |

`dispatch::encode_models_request_with` 和 `encode_model_request_with` 使用 `EgressCtx::gemini_profile` 的模型路径。列表请求把 `limit` 作为 `pageSize`、`after_id` 作为 `pageToken` 发送；`before_id`、生命周期过滤和其他查询参数为 `Unsupported`。

## 往返

每个 fixture 解码再编码后都得到相同的 JSON 值：带路径的请求、回复、Token 计数、嵌入、模型列表和错误。每个流式 fixture 在任意分块方式下都解码出相同的事件，并逐个 chunk 回放。录制的 fixture 没有例外。

## 下一步

- [跨格式转换](cross-format.md)：查阅数据从一种 API 格式转到另一种时会发生什么，以及原因。
- [跨格式的推理](reasoning.md)：了解思考级别、预算和思考签名如何跨格式转换。
- [Fixture 与录制](fixtures.md)：用 `cargo xtask record gemini` 录制 Gemini fixture。
