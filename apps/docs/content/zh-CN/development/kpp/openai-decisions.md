---
title: OpenAI Decisions
description: OpenAI Decisions API 的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 OpenAI Decisions API 的任意字段：它在 KPP 中的位置、来自其他决策格式的请求会变成什么，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_decisions::OpenAiDecisions`（`crates/kaeso-codecs/src/openai_decisions/`）
- **端点：**`POST /v1/decisions`，以及 OpenAI 的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_decisions/`，根据 API 参考和 `openai-python` 的类型构建，没有录制的

**来源。** OpenAI 的 [Create a decision](https://developers.openai.com/api/reference/resources/decisions/methods/create) 参考、[Decisions 指南](https://developers.openai.com/api/docs/guides/decisions)和[错误码](https://developers.openai.com/api/docs/guides/error-codes)，阅读于 2026-11-12。`openai-python` 3.49.0：`types/decision_create_params.py` 和 `types/decision.py`。

网关尚未转发决策请求；编解码器已为此做好准备。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `openai_decisions`；其他方言会拒绝它们，除非它们在[允许列表](cross-format.md#允许列表)中。
- **形式标记**：编解码器私有的 `Extras::form` 标记；`verbatim` 保留已声明键的 `null` 写法。

## 请求：`DecisionCreateParams`

决策请求解码为 `Request::Decide(DecideRequest)`。问题保持原有顺序，回复的 `answers[i]` 回答 `questions[i]`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `DecideRequest.model`（类型化） | 必填。 |
| `input` 字符串 | `input = DecisionInput::Text`（类型化） | |
| `input` 列表 | `input = DecisionInput::Messages`（类型化） | 每一项一个 `Message`（[输入项](#输入项)）。 |
| `questions` | `questions`（类型化） | 一个列表；其他任何内容都是 `InvalidRequest`。 |
| `questions[].type` | `Question.kind`（类型化） | `predicate`、`choice` 或 `score`。其他类型是 `InvalidRequest`。 |
| `questions[].name` | `Question.name`（类型化） | 可选。名称可以重复；`validate()` 允许这样。 |
| `questions[].instructions` 字符串 | `Instructions::Text`（类型化） | |
| `questions[].instructions` 列表 | `Instructions::Blocks`（类型化） | `input_text`、`input_image` 和 `input_json` 部分（[部分](#内容部分)）。 |
| `choices[].value` | `ChoiceOption.value`（类型化） | 字符串为 `ChoiceValue::Text`，布尔值为 `ChoiceValue::Bool`。其他任何内容都是 `InvalidRequest`。 |
| `choices[].description` | `ChoiceOption.description`（类型化） | `Description::Text`。 |
| `levels[].label` | `Level.label`（类型化） | 级别从低到高排列；级别 `i` 的分值为 `i`。 |
| `levels[].description` | `Level.description`（类型化） | `Description::Text`。 |
| `prompt_cache_key` | `DecideRequest.cache.key`（类型化） | |
| 带 `mode` 和 `ttl` 的 `prompt_cache_options` | `cache.mode`、`cache.ttl`（类型化） | 形式标记 `cache_options` 保留空对象。其他任何形状整体保留在 extras 中，属于提示。 |
| `safety_identifier` | `DecideRequest.user`（类型化） | |
| 未知键 | extras | 在每一层：请求、问题、选项、级别、输入项和部分。 |
| 请求头 `openai-beta` | extras `header:openai-beta` | 只转发给 OpenAI Decisions。 |

带 `choices` 或 `levels` 的 `predicate`、没有 `choices` 或带 `levels` 的 `choice`，以及没有 `levels` 或带 `choices` 的 `score`，都是 `InvalidRequest`。带 `state` 而没有 `input` 的请求体是 System One 请求。它是 `InvalidRequest`，消息中指明 `/v1/systemone`（D15）。

### 输入项

| 输入项 | KPP | 说明 |
|---|---|---|
| `role: "user"` 消息 | 带 `Role::User` 的 `Message` | `type: "message"` 是形式标记 `typed`；字符串内容是一个文本块，形式标记 `content_string`。 |
| `role: "developer"` 消息 | 带 `Role::System` 的 `Message` | 形式标记 `role` 保留 `developer`。只有文本部分。 |
| `role: "system"` 消息 | 带 `Role::System` 的 `Message` | 只有文本部分。 |
| `role: "assistant"` 消息 | 带 `Role::Assistant` 的 `Message` | `output_text` 和 `refusal` 部分；`phase` 保留在 extras 中。 |
| `function_call` | 含一个 `Block::ToolCall` 的助手 `Message` | `call_id`、`name`；`arguments` 以 JSON 字符串保存。`id`、`namespace` 和 `status` 保留在 extras 中。 |
| `custom_tool_call` | 含一个 `Block::ToolCall` 的助手 `Message` | 形式标记 `custom`；`input` 以 JSON 字符串保存。 |
| `function_call_output` | 含一个 `Block::ToolResult` 的用户 `Message` | `output` 为文本，或文本和图片部分；字符串是形式标记 `output_string`。 |
| `custom_tool_call_output` | 含一个 `Block::ToolResult` 的用户 `Message` | 形式标记 `custom`。 |
| 其他 `type` | 无 | `InvalidRequest`：文件、音频和项目引用不属于该 API。 |

### 内容部分

| 部分 | KPP | 说明 |
|---|---|---|
| `input_text`、`output_text` | `Block::Text` | 类型与其角色不符时，例如助手消息中的 `input_text`，为形式标记 `text_type`。 |
| 带 data URL 的 `input_image` | `Block::Image`，`Source::Base64` | 字节被解码；除 `data:<type>;base64` 以外的头部为形式标记 `data_url_header`。无法解码的 base64 是 `InvalidRequest`。 |
| 带 `http(s)` URL 的 `input_image` | `Block::Image`，`Source::Url` | Kaeso 从不获取它。 |
| `input_image.detail` | extras | 一个提示，请求跨格式时省略。 |
| `input_json` | `Block::Json` | 逐字节保留。 |
| `refusal` | `Block::Refusal` | 在助手消息中。 |
| `prompt_cache_breakpoint` | `Block.cache` | 一个按原样保存断点的 `CacheHint`，形式标记 `breakpoint`。 |
| 其他 `type` | `Block::Unknown` | 绑定到 `openai_decisions`；其他格式拒绝它。 |

## 写出请求

编码器写出 KPP 中的内容，并在发送之前拒绝 OpenAI 格式无法接收的内容。来自其他格式（例如 System One）的请求按以下规则跨格式；[跨格式](cross-format.md#决策)说明了原因。

| KPP | 写成 | 说明 |
|---|---|---|
| `DecisionInput::Text` | `input` 字符串 | |
| `DecisionInput::Json` | 一条用户消息，含一个 `input_json` 部分 | 只限一个对象或一个对象列表，且不超过 32 层、10,000 个值和 256 KiB 的紧凑 JSON。否则为一个按原样写出 JSON 的 `input_text` 部分，在 `adapted` 中记为 `state`（D13）。 |
| `Instructions::Json` | 按原样写出的 JSON，作为说明字符串 | 在 `adapted` 中记为 `instructions`。 |
| 缺少 `Instructions` | `""` | `instructions` 是必填的；保持原格式的请求则不写出它。 |
| `Predicate.when_true`、`.when_false` | 添加到说明后的行 | 空一行后写 `It is true when: …` 和 `It is false when: …`。由部分组成的说明得到一个文本部分。在 `adapted` 中记为 `criteria`（D11）。 |
| `Description::Json` | 按原样写出的 JSON，作为文本 | 在 `adapted` 中记为 `criteria`。 |
| 没有标签的 `Level` | 其描述作为 `label` | System One 的级别只有描述。空级别为 `""`。 |
| `ChoiceValue::Bool` | `true`、`false` | |
| 字符串中的资产引用 | 在引用处切分字符串，图片以 `input_image` 部分写在原位置 | 在输入、状态列表的字符串和说明中。其他位置的引用，例如对象内部，为 `Unsupported`（`image reference`）；在描述中则为 `image in criteria`。 |
| `Source::Base64` | data URL | |
| `Source::Url` | 该 URL | 只限 `http` 和 `https`；其他为 `Unsupported`（`image URL`）。 |
| `Source::FileId` | 无 | `Unsupported`（`image file id`）：请以 data URL 发送图片。 |
| 音频、视频和文档块 | 无 | `Unsupported`，指明该部分。 |
| `QuestionKind::Box` | 无 | `Unsupported`（`box question`）。 |
| 少于 2 个或多于 255 个选项的选择题 | 无 | `Unsupported`（`options`）。 |
| 少于 2 个或多于 10 个级别的评分题 | 无 | `Unsupported`（`levels`）。 |
| 问题数超过 `DecisionsProfile::max_questions` | 无 | `Unsupported`（`questions`）；OpenAI 为 400（D5）。 |
| `ToolResultBlock.is_error` | 无 | `Unsupported`（`tool error`）：该格式没有错误标记。 |

`EgressCtx` 保存发送的每个问题的名称和选项值，因此回复按位置读回。

## 回复：`Decision`

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `DecideResponse.model`（类型化） | |
| `answers[]` | `answers`（类型化） | 每个问题一个，按顺序排列。数量不符、类型与问题不符，或值未被提出，都是服务商错误。 |
| `answers[].name` | 无 | 根据请求写出。以其他名称指称问题的回复把该名称保留在形式标记 `name` 中。 |
| `predicate.probability` | `PredicateAnswer.probability`（类型化） | |
| `choice.choice` | `ChoiceAnswer.option`（类型化） | 具有该值的选项的索引；`true` 和 `"true"` 保持区分。 |
| `choice.probabilities[]` | `ChoiceAnswer.probabilities`（类型化） | 按选项排列；回复中没有条目的为 `None`。其他顺序为形式标记 `order`。 |
| `score.score` | `ScoreAnswer.score`（类型化） | |
| `score.probabilities[]` | `ScoreAnswer.probabilities`（类型化） | 按级别的索引 `value` 排列。原样写出的标签为形式标记 `labels`，回复跨格式时根据请求重新构建。 |
| `confidence` | `Answer.confidence`（类型化） | 见[置信度](#置信度)。 |
| `refusal` | `AnswerValue::Refusal`（类型化） | 同一回复中的其他答案照常有效。 |
| `usage.input_tokens` | `usage.input_tokens`（类型化） | OpenAI 在其中计入缓存读取和写入，KPP 不计入，因此两者都会被减去。 |
| `usage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens`（类型化） | |
| `usage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens`（类型化） | |
| `usage.output_tokens` | `usage.output_tokens`（类型化） | 在 OpenAI 为 0。 |
| `usage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens`（类型化） | |
| `usage` | `usage.raw` | `total_tokens` 和其他键在相同格式中原样返回。 |
| 以整数写出的数字 | 形式标记 `integers` | 只要值不变，就再次以整数写出。 |
| 未知键 | extras | 在每一层，包括概率条目。 |

编码器按问题顺序写出答案，并从 `EncodeCtx::request` 中应用的请求取得每个名称、选项值和级别标签。未命名问题的答案为 `name: null`。来自其他格式的回复，其 `input_tokens` 会加回缓存计数，`total_tokens` 为输入加输出。

### 置信度

来自 OpenAI Decisions 服务商的回复保留其发送的 `confidence`。OpenAI 没有说明它如何计算，但其指南中的两个示例都与 TypeSafe 的公式一致。因此来自其他格式的答案使用这些公式：选择题为 `(p_max − 1/n) / (1 − 1/n)`，评分题为 `max(0, 1 − Σ p_i·|i − m| / MAD_unif)`（D8）。是非题没有置信度。

## 流式传输

决策不以流式传输。流编码器对每个事件都返回 `Unsupported`（`streaming`）；流解码器报告一个带 `Unsupported`（`streaming`）的 `StreamEvent::Error`，之后不再报告任何内容。

## 错误

错误使用 OpenAI 的信封 `{"error": {"message", "type", "param", "code"}}`，读写方式与 [OpenAI Responses](openai-responses.md#错误errorresponse) 相同。OpenAI 没有为决策记录专门的错误码，因此错误类别取决于状态码、`type` 和 `code`。OpenAI 的错误按原样到达 OpenAI 应用。其他任何错误都以该信封和其类别对应的状态码写出，429 或 503 保留其 `retry-after`。

## 模型

该编解码器读写 OpenAI 的模型列表，如 OpenAI Chat Completions 页面的[模型](openai-chat.md#模型)部分所述。

## 使用此格式的服务商

每个服务商在 `EgressCtx::decisions_profile` 中有一个 `DecisionsProfile`。

### OpenAI

`DecisionsProfile::OPENAI`，`POST /v1/decisions`：最多 400 个问题，输入和问题说明中都可以有图片。只有 `gpt-6-luna` 回答决策。

### 其他服务器

`DecisionsProfile::GENERIC` 不检查问题数量限制。其路径来自 `EgressCtx::with_base_path` 或 `DecisionsProfile::with_path`。`DecisionsProfile::with_instruction_images(false)` 为只在输入中接收图片的服务器拒绝说明中的图片。
