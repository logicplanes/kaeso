---
title: Gemini Decisions
description: Gemini Decisions API 的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 Gemini Decisions API 的任意字段：它在 KPP 中的位置、来自其他决策格式的请求会变成什么，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::gemini_decisions::GeminiDecisions`（`crates/kaeso-codecs/src/gemini_decisions/`）
- **端点：**`POST /v1beta/decisions`，以及 Gemini 的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/gemini_decisions/`，根据指南、API 参考和 `google-genai` 的类型构建，没有录制的

**来源。** Google 的 [Decisions API 指南](https://aistudio.google.com/docs/decisions-api)和 [Decisions API 参考](https://ai.google.dev/api/decisions)，阅读于 2026-11-12。Python 版 `google-genai` 3.3.0：`types/decisions` 及其复用的 Interactions 内容块。

网关尚未转发决策请求；编解码器已为此做好准备。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `gemini_decisions`；其他方言会拒绝它们，除非它们在[允许列表](cross-format.md#允许列表)中。
- **形式标记**：编解码器私有的 `Extras::form` 标记；`verbatim` 保留已声明键的 `null` 写法。

## 请求：`CreateDecisionRequest`

决策请求解码为 `Request::Decide(DecideRequest)`。问题保持 `questions` 对象中的顺序，回复的 `answers[i]` 回答 `questions[i]`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `DecideRequest.model`（类型化） | 必填。 |
| `input` 字符串 | `input = DecisionInput::Text`（类型化） | |
| `input` 内容块 | `input = DecisionInput::Messages`（类型化） | 一条含一个块的用户 `Message`（[内容块](#内容块)）；形式标记 `object`。 |
| `input` 内容块列表 | `input = DecisionInput::Messages`（类型化） | 一条用户 `Message`，块按顺序排列。 |
| `questions` | `questions`（类型化） | 以问题键为键的对象。列表是 `InvalidRequest`，消息中指明 `/v1/decisions`。 |
| `questions` 的键 | `Question.name`（类型化） | |
| `questions.*.type` | `Question.kind`（类型化） | `noul` 为 `QuestionKind::Predicate`，`choice` 和 `score` 保持原名。其他类型是 `InvalidRequest`。 |
| `instructions` 字符串 | `Instructions::Text`（类型化） | |
| `instructions` 文本块或文本块列表 | `Instructions::Blocks`（类型化） | 只限文本块；单个块为形式标记 `object`。媒体块是 `InvalidRequest`。 |
| `noul` 的 `criteria` | `Predicate`（类型化） | `true_description` 和 `false_description` 即 `when_true` 和 `when_false`；两者都缺少时，形式标记 `criteria` 保留该对象。 |
| `choice` 的 `criteria` | `Choice.options`（类型化） | 每个键为一个 `ChoiceValue::Text`，其字符串为 `Description::Text`，`""` 也是。不是字符串的值是 `InvalidRequest`。 |
| `score` 的 `criteria` | `Score.levels`（类型化） | 每个字符串为一个级别的 `Description::Text`；级别从低到高排列。 |
| 未知键 | extras | 在每一层：请求、问题、`noul` 的 `criteria` 和块。 |

带 `state` 而没有 `input` 的请求体是 System One 请求，是 `InvalidRequest`，消息中指明 `/v1/systemone`。

### 内容块

| 块 | KPP | 说明 |
|---|---|---|
| `text` | `Block::Text` | `annotations` 保留在 extras 中。 |
| 带 `data` 的 `image`、`audio`、`video`、`document` | 带 `Source::Base64` 的 `Block::Image`、`Audio`、`Video`、`Document` | 带 `data` 时必须有 `mime_type`。无法解码的 base64 是 `InvalidRequest`。 |
| 带 `uri` 的同类块 | Files API URI 为 `Source::FileId`，否则为 `Source::Url` | `mime_type` 为形式标记 `mime_type`。Kaeso 从不获取 URL。 |
| 同时有 `data` 和 `uri`，或两者都没有 | 无 | `InvalidRequest`。 |
| `resolution` | extras | 在图片和视频上。一个[允许列表](cross-format.md#允许列表)中的提示。 |
| 其他 `type` | `Block::Unknown` | 绑定到 `gemini_decisions`；其他格式拒绝它。 |

## 写出请求

编码器写出 KPP 中的内容，并在发送之前拒绝 Gemini 格式无法接收的内容。来自其他格式的请求按以下规则跨格式；[跨格式的决策](decisions-across-formats.md)说明了原因。

| KPP | 写成 | 说明 |
|---|---|---|
| `Question.name` | 问题键 | 当每个问题都有名称、名称不重复且符合 `[A-Za-z0-9_-]{1,64}` 时保留；否则每个问题都写成 `q0`…`qN-1`，在 `adapted` 中记为 `names`（D19）。 |
| `DecisionInput::Json` | `input` 字符串，内容为按原样写出的 JSON | Google 的指南要求以文本发送 JSON（D13）；不记录任何调整。 |
| `Block::Json` | 一个按原样写出 JSON 的文本块 | |
| 字符串中的资产引用 | 在引用处切分字符串，图片以 `image` 块写在原位置 | 在输入字符串和状态列表的字符串中；一个列表项与下一个列表项的文本以换行连接。其他位置为 `Unsupported`（`image reference`），在说明中为 `image in question instructions`，在描述中为 `image in criteria`。 |
| `Instructions::Json`、`Description::Json` | 按原样写出的 JSON，作为文本 | 在 `adapted` 中记为 `instructions` 或 `criteria`。 |
| 缺少 `Instructions` | `""` | `instructions` 是必填的；保持原格式的请求则不写出它。 |
| `ChoiceValue::Bool` | `"true"`、`"false"` | 在 `adapted` 中记为 `booleans`（D18）；同时含有 `true` 和 `"true"` 的问题为 `Unsupported`。 |
| Google 拒绝的选项键 | 无 | `Unsupported`（`option key`）：超过 128 个字符或 512 字节、两端有空格、含控制字符或非 ASCII 空格。 |
| 缺少 `ChoiceOption.description` | `""` | |
| 同时有标签和描述的 `Level` | `"label: description"` | 在 `adapted` 中记为 `levels`。没有文本的级别为 `Unsupported`（`score level`）。 |
| `Source::Base64`、data URL | `data` 和 `mime_type` | |
| `Source::Url` | `uri` 和 `mime_type` | 只限 `https`。MIME 类型取自文件扩展名，与 [Gemini](gemini.md) 相同；没有扩展名的图片为 `Unsupported`。`http`、`gs://` 和 YouTube 的 URL 为 `Unsupported`。 |
| Gemini 的 `Source::FileId` | `uri` | 其他服务商的文件 ID 为 `Unsupported`。 |
| `Source::Text` | 无 | `Unsupported`（`text document`）：请以文本块发送该文本。 |
| OpenAI 的 `detail` 为 `low` 或 `high` | `resolution` | `auto` 和 `original` 被省略。 |
| 说明中的图片、拒答、工具项 | 无 | `Unsupported`，指明该部分。 |
| `QuestionKind::Box` | 无 | `Unsupported`（`box question`）。 |
| 少于 2 个或多于 255 个选项或级别 | 无 | `Unsupported`（`options`、`levels`）。 |
| 多于 1,000 个问题 | 无 | `Unsupported`（`questions`）（D5）。 |
| 对话 | 无 | `Unsupported`（`conversation input`）（D12）。 |
| `cache`、`user`、缓存断点 | 无 | 省略，在 `left_out` 中记为 `cache` 和 `user`。 |

`EgressCtx` 保存发送的每个问题的键和选项键以及模型，因此回复按位置读回。

## 回复：`CreateDecisionResponse`

| 线路字段 | KPP | 说明 |
|---|---|---|
| 无 | `DecideResponse.model`（类型化） | Gemini 的回复不指明模型；解码器取请求中的模型。 |
| `answers` | `answers`（类型化） | 每个问题一个，按键对应。未提出的键、缺少的答案或类型与问题不符，都是服务商错误。 |
| `noul.noul` | `PredicateAnswer.probability`（类型化） | Google 将其限制在 0.01 到 0.99；KPP 按发送的值保留。 |
| `choice.choice` | `ChoiceAnswer.option`（类型化） | 具有该键的选项的索引。 |
| `choice.probabilities` | `ChoiceAnswer.probabilities`（类型化） | 按选项排列；回复中没有条目的为 `None`。 |
| `score.score` | `ScoreAnswer.score`（类型化） | |
| `score.probabilities` | `ScoreAnswer.probabilities`（类型化） | 按级别索引排列。 |
| `score.legend` | 形式标记 `legend` | 回复跨格式时根据请求的级别重新构建。 |
| `confidence` | `Answer.confidence`（类型化） | 见[置信度](#置信度)。 |
| `usage.total_input_tokens` | `usage.input_tokens`（类型化） | 减去 `total_cached_tokens`，后者为 `usage.cache_read_tokens`。 |
| `usage.input_tokens_by_modality` | `usage.input_image_tokens`、`input_audio_tokens`（类型化） | 图片、视频和文档 token 计为图片 token，与 Google 指南的计法相同。 |
| `usage.total_output_tokens` | `usage.output_tokens`（类型化） | 在 Google 为 0。 |
| `usage` | `usage.raw` | 整个用量在相同格式中原样返回。 |
| `prompt_feedback.block_reason` | 无 | 输入被拦截：`ContentFiltered`，见[错误](#错误)。没有原因时，`prompt_feedback` 保留在 extras 中。 |
| 以整数写出的数字 | 形式标记 `integers` | 只要值不变，就再次以整数写出。 |
| 未知键 | extras | 在每一层。 |

编码器按问题顺序在应用的键下写出答案，并从 `EncodeCtx::request` 中应用的请求取得每个选项键和级别。它不写出 `model`。来自其他格式的回复得到计入缓存的 `total_input_tokens`、以文本为其余部分的 `input_tokens_by_modality`、按报告值写出的 `total_output_tokens`，以及 `total_tokens`。

### 置信度

来自 Gemini 的回复保留其发送的 `confidence`。来自其他格式的答案使用 Google 的规则：选择题和评分题取最高概率，是非题取 `max(p, 1 − p)`（D8）。`confidence` 总是写出，因为 Google 的 SDK 要求有它。

### 拒答

Gemini 的格式没有拒答这种答案。来自 OpenAI 的含拒答的回复以错误到达 Gemini 应用：`ContentFiltered`，消息中列出被拒答的问题（D9）。

## 流式传输

决策不以流式传输。流编码器对每个事件都返回 `Unsupported`（`streaming`）；流解码器报告一个带 `Unsupported`（`streaming`）的 `StreamEvent::Error`，之后不再报告任何内容。

## 错误

错误使用 Google 的信封，读写方式与 [Gemini](gemini.md#错误) 相同。被拦截的输入是一个 400，其 `code` 不是 HTTP 状态码，而是一个安全代码（`safety`、`prohibited_content`、`blocklist`、`image_safety`、`content_blocked`）。KPP 将其读为 `ContentFiltered`。带 `prompt_feedback.block_reason` 的回复没有回答任何问题，因此也是 `ContentFiltered`（D10）。

Gemini 的错误按原样到达 Gemini 应用。其他任何 `ContentFiltered` 都写成 `{"error": {"code", "message"}}`，状态码为 400。其代码为拦截原因对应的代码，否则为 `safety`。其他所有错误都以 Google 的信封和其类别对应的状态码写出。

## 模型

该编解码器读写 Gemini 的模型列表，如 Gemini 页面的[模型](gemini.md#模型)部分所述。

## 使用此格式的服务商

### Google AI Studio

`POST /v1beta/decisions`，只有 `gemini-decision-1`：1 到 1,000 个问题，2 到 255 个选项和级别，输入可以是文本、图片、音频、视频和 PDF。`EgressCtx::with_base_path` 发送到其他路径。Gemini Enterprise Agent Platform 尚不提供决策（D21）。
