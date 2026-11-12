---
title: System One
description: TypeSafe 的 System One 决策格式的每个字段如何映射到 KPP 以及如何映射回来。
order: 8
---

在这里查阅 TypeSafe 的 System One 格式的任意字段：它在 KPP 中的位置、各服务商如何接收它，以及映射的来源。修改编解码器读写该字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::system_one::SystemOne`（`crates/kaeso-codecs/src/system_one/`）
- **端点：**`POST /v1/systemone`，以 JSON 或 `multipart/form-data` 上传发送，以及 OpenAI 的[模型端点](#模型)
- **Fixture：**`crates/kaeso-codecs/fixtures/system_one/`，根据 TypeSafe 的 OpenAPI 文档和文档构建，没有录制的

**来源。** TypeSafe 的 OpenAPI 文档 `https://api.typesafe.ai/openapi.json` 0.2.0 版（`SystemOneRequest`、`Question`、`NoulCriteria`、`Asset`、`SystemOneResponse`、`Answer`、`HTTPBadRequestError`、`HTTPValidationError`），以及它关于 [System One](https://docs.typesafe.ai/api/system-one)、[图片](https://docs.typesafe.ai/images)、[错误](https://docs.typesafe.ai/api/errors)和[置信度](https://docs.typesafe.ai/confidence)的页面，阅读于 2026-11-12。

网关尚未转发决策请求；编解码器已为此做好准备。

## 表格的读法

- **类型化**：一个类型化的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `system_one`；其他方言会拒绝它们（[Extras](cross-format.md#extras)）。
- **形式标记**：编解码器私有的 `Extras::form` 标记；`verbatim` 保留已声明键的 `null` 写法。

## 请求：`SystemOneRequest`

决策请求解码为 `Request::Decide(DecideRequest)`。问题保持原有顺序，回复的 `answers[i]` 回答 `questions[i]`。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `DecideRequest.model`（类型化） | 必填。 |
| `state` 字符串 | `input = DecisionInput::Text`（类型化） | |
| `state` 对象或数组 | `input = DecisionInput::Json`（类型化） | 逐字节保留，包括键的顺序。 |
| 带 `image_url` 部分的 `state` | `input = DecisionInput::Json` 和 `assets`（类型化） | 任意深度的每个部分都成为一个资产，使用新的引用 `kaeso-image-<n>-ref`，写在该部分原来的位置；部分自身的字段作为资产的 extras 保留，形式标记 `state_part`。Perplexity、OpenRouter 和 Baseten 以这种方式接收图片（[图片](#图片)）。 |
| `questions` | `questions`（类型化） | 由具名问题组成的对象；其他任何内容都是 `InvalidRequest`。每个键是 `Question.name`。 |
| `questions.*.type` | `Question.kind`（类型化） | `noul` 对应 `Predicate`，`choice`、`score` 和 `box` 保持原名。其他类型是 `InvalidRequest`。 |
| `questions.*.instructions` 字符串 | `Instructions::Text`（类型化） | |
| `questions.*.instructions` 对象或数组 | `Instructions::Json`（类型化） | |
| `questions.*.instructions` 为 `null` | 无，形式标记（`verbatim`） | |
| noul 的 `criteria` 对象 | `Predicate.when_true`、`.when_false`（类型化） | `true` 和 `false` 为 `Description::Text` 或 `Description::Json`；`null` 是形式标记（`verbatim`）。对象本身是形式标记 `criteria_object`，因此 `{}` 能原样返回。 |
| noul 的 `criteria` 为 `null` | 形式标记（`verbatim`） | |
| choice 的 `criteria` 对象 | `Choice.options`（类型化） | 每个键按顺序成为 `ChoiceValue::Text`；其描述与 noul 相同。描述为 `null` 时是选项上的形式标记 `null`。 |
| score 的 `criteria` 数组 | `Score.levels`（类型化） | 每一项是该级别的 `description`；级别 `i` 的分值为 `i`。 |
| box 的 `criteria` | extras | box 问题没有 criteria；即使发送了也保留在问题上。 |
| `assets` | `assets`（类型化） | 以引用为键，保持顺序。 |
| `assets.*.type` | 无 | 必须是 `image`，否则为 `InvalidRequest`；缺失时为形式标记 `no_type`。 |
| `assets.*.format` | 来源的媒体类型 | `png` 为 `image/png`，`jpg` 和 `jpeg` 为 `image/jpeg`，原样保存在形式标记 `format` 中。其他值是 `InvalidRequest`。 |
| `assets.*.data_base64` | `Asset.source = Source::Base64`（类型化） | 需要 `format`。不是 base64 的文本是 `InvalidRequest`。 |
| `assets.*.part` | `Asset.source = Source::Base64`（类型化） | 仅限上传：所命名部分的字节，是请求体的一个切片。形式标记 `part`、`part_type` 和 `filename`。旁边的 `data_base64` 作为 extras 保留，因为 TypeSafe 会忽略它。 |
| 未知键 | extras | 在每一层：请求、问题、criteria、资产。 |
| 请求头 `typesafe-danger-zone-images` | extras `header:typesafe-danger-zone-images` | 只转发给 System One。 |
| 请求头 `typesafe-danger-zone-long-context` | extras `header:typesafe-danger-zone-long-context` | 只转发给 System One。 |

上传的第一个部分是 `request`，即 JSON 请求。其他每个部分都是一张图片，恰好由一个资产在 `part` 中指名，类型为 `image/png`、`image/jpeg`、`image/jpg`、`application/octet-stream` 或没有类型。缺失、未使用或重复的部分，或其他类型，都是 `InvalidRequest`。上传只在发往 TypeSafe 时仍以上传形式发送；其他服务商以 base64 接收图片。

`DecideRequest::validate()` 要求有模型、至少一个问题、每个问题至少一个选项或级别、名称不重复，并且每个资产都在状态、说明或描述的某个字符串中被引用。对象的键从不计入，因为 TypeSafe 不会替换它们。`decode_request` 拒绝其他所有类型的请求。

## 图片

每张图片都是一个由字符串引用的资产。它发往何处，由 `EgressCtx::system_one_profile` 中服务商的 `SystemOneProfile` 决定：

| `MediaPlacement` | 写成 | 拒绝 |
|---|---|---|
| `Assets` | 带 `data_base64` 的 `assets`，或再次作为上传的部分 | 图片 URL |
| `State` | 在状态中每个作为完整字符串的引用处写入带 data URL 的 `image_url` 部分 | 较长字符串中的引用、说明或描述中的引用 |
| `StateOrAssets` | 应用发送部分之处写成部分，其余写成 `assets` | `assets` 中的图片 URL |
| `Conversation` | `state.messages` 下的一条用户消息：每个字符串是 `text` 部分，每张图片是 `image_url` 部分 | 状态不是文本、字符串列表或 `{"messages": …}` 时其中的图片 |
| `Root` | 顶层的 `images` 列表，由 data URL 组成，按顺序从状态数组中取出 | 不是状态数组完整元素的任何引用 |
| `None` | 无 | 所有图片 |

发送之前，编解码器从每张图片的头部读取其格式和尺寸，不进行解码（`kaeso_codecs::image_size`），并拒绝配置文件所拒绝的内容。图片从不转换或缩放。拒绝信息会指明 `image URL`、`image format`、`image size` 或 `animated image`，字段位于 `param`。

## 问题与选项

| KPP | 写成 | 说明 |
|---|---|---|
| `Question.name` | 问题的键 | 当每个问题都有名称、名称不重复且服务商可接受时保留；否则每个问题都以 `q0`…`qN-1` 发送，并在 `adapted` 中记为 `names`。回复按位置读回。 |
| `Instructions::Blocks` | 一个字符串，或由字符串、JSON 值和图片引用组成的数组 | 图片成为资产。 |
| `ChoiceValue::Bool` | `"true"`、`"false"` | 在 `adapted` 中记为 `booleans`。同一问题中同时出现 `true` 和 `"true"` 为 `Unsupported`（`repeated choice value`）。 |
| 带 `label` 和文本 `description` 的 `Level` | `"label: description"` | 在 `adapted` 中记为 `levels`。只有标签时写出标签。 |
| `QuestionKind::Box` | `box` | 配置文件不回答 box 问题时为 `Unsupported`（`box question`）。 |
| `DecideRequest.cache`、`Block.cache` | 无 | System One 没有对应位置的提示，在 `left_out` 中记为 `cache`。 |
| `DecideRequest.user` | 无 | 一个提示，在 `left_out` 中记为 `user`。 |
| `DecisionInput::Messages` | 状态 | 只接受一条用户消息；其文本、JSON 和图片块成为一个字符串或数组。更多消息为 `Unsupported`（`conversation input`）。 |

## 回复：`SystemOneResponse`

解码器从 `EgressCtx` 中取得每个问题发送时使用的键，并按该键读取答案。回答了未提出的问题、缺少答案、选择了未提供的选项或级别，或答案类型不符，都是服务商错误。

| 线路字段 | KPP | 说明 |
|---|---|---|
| `model` | `DecideResponse.model`（类型化） | 实际回答的模型，例如 `jev-latest` 对应的 `jev-1.14.0`。 |
| `answers.*` noul | `AnswerValue::Predicate(PredicateAnswer { probability })`（类型化） | `noul` 是“是”的概率。 |
| `answers.*` choice | `AnswerValue::Choice(ChoiceAnswer { option, probabilities })`（类型化） | `option` 是所选选项的索引；`probabilities` 每个选项一项，回复中没有的为 `None`。 |
| `answers.*` score | `AnswerValue::Score(ScoreAnswer { score, probabilities })`（类型化） | `probabilities` 按级别排列，来自键 `"0"`、`"1"` 等。 |
| `answers.*.legend` | 形式标记 `legend` | 回复跨格式时，根据请求的级别重新构建。 |
| `answers.*` box | `AnswerValue::Box(BoxAnswer)`（类型化） | 边界以比例表示；`confidence: null` 是形式标记 `null_confidence`。 |
| `answers.*.confidence` | `Answer.confidence`（类型化） | 见[置信度](#置信度)。 |
| `answers.*` refusal | `AnswerValue::Refusal`（类型化） | TypeSafe 从不发送；用于写出被其他格式拒答的答案。 |
| 以整数写出的数字 | 形式标记 `integers` | 只要值不变，就再次以整数写出。 |
| `usage.input_tokens` | `usage.input_tokens`（类型化） | 包括图片。 |
| `usage.output_tokens` | `usage.output_tokens`（类型化） | TypeSafe 会统计但不收费。 |
| `usage` | `usage.raw` | 其他键，例如 OpenRouter 的 `cost`，在相同格式中原样返回。 |
| 未知键 | extras | 在每一层。 |

答案按问题顺序，以应用自己的键写出；编码器从 `EncodeCtx::request` 中应用的请求取得这些键。

### 置信度

来自 System One 服务商的回复保留其发送的 `confidence`。来自其他格式的答案使用 TypeSafe 的[文档公式](https://docs.typesafe.ai/confidence)：选择题为 `(p_max − 1/n) / (1 − 1/n)`，评分题为 `max(0, 1 − Σ p_i·|i − m| / MAD_unif)`，只有一个选项或级别时为 1。是非题没有置信度，box 保留服务商的值或 `null`。

Perplexity 计算评分题置信度时，以到最可能级别的距离为基准，而不是到中间的距离，因此其值与 TypeSafe 的不同。它的回复保留发送时的值。

## 流式传输

决策不进行流式传输。流编码器对每个事件都返回 `Unsupported`（`streaming`）；流解码器报告一个带 `Unsupported`（`streaming`）的 `StreamEvent::Error`，之后不再报告任何内容。

## 错误

| 状态码 | 响应体 | KPP |
|---|---|---|
| 400 | `{"detail": "…"}` 或 `{"detail": {"message": …}}` | `InvalidRequest`；响应体提到 `max_tokens_exceeded` 时为 `ContextTooLong` |
| 401 | `{"detail": …}` | `Authentication` |
| 413 | `{"detail": …}` | `RequestTooLarge` |
| 415 | `{"detail": …}` | `InvalidRequest` |
| 422 | `{"detail": [{"loc", "msg", "type", "input"?, "ctx"?}]}` | `InvalidRequest`，第一个问题去掉 `body` 后的位置写入 `param` |
| 429 | `{"detail": …}`、`retry-after`、`retry-after-ms` | 带等待时间的 `RateLimited` |
| 529 | `{"detail": …}` | `Overloaded` |

没有 `detail` 的响应体按 OpenAI 错误读取，这涵盖 OpenRouter 和 Perplexity 的 `error` 封装以及 Cloudflare 的封装。System One 错误原样到达 System One 应用。其他任何错误都写成 `{"detail": message}`，状态码取决于其种类：带 `param` 的 `InvalidRequest` 为 422，附带 `detail[{loc, msg, type: "value_error"}]`；其他无效或不支持的请求为 400；计费问题为 402；服务商过载为 529。

## 模型

TypeSafe 的 `GET /v1/models` 返回 `{"models": [{name, description, release_date}]}`，没有任何请求头能区分 TypeSafe 的 SDK 和 OpenAI 的 SDK。本编解码器读写 OpenAI 的模型列表，如 OpenAI Chat Completions 页面的[模型](openai-chat.md#模型)一节所述。

## 使用此格式的服务商

每个服务商都有一个 `SystemOneProfile`，TypeSafe 以外的服务商把 fixture 放在 `fixtures/system_one/providers/<id>/` 中（[Fixture](fixtures.md)）。

### TypeSafe

`SystemOneProfile::TYPESAFE`，`POST /v1/systemone`。图片放在 `assets` 中或作为上传的部分，PNG 和 JPEG，每边 280 到 1568 像素，最大 16 MiB，只接受静态图片；请求包含图片或 box 问题时都会带上 `TypeSafe-Danger-Zone-Images: true`。box 问题需要 Jev 1.14。长上下文请求头只在应用发送时才转发。最多 255 个选项和 10 个级别。

### OpenRouter

`SystemOneProfile::OPENROUTER`，`https://openrouter.ai/api` 下的 `POST /v1/systemone`。来源：[Jev 指南](https://openrouter.ai/docs/guides/community/jev)、[Decisions API 参考](https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request)和多模态决策指南，阅读于 2026-11-12。OpenRouter 接收状态中的 `image_url` 部分和 `assets` 中的图片，但不接收上传。它的回复会添加 `id`、`provider` 和 `usage.cost`，这些保留在 extras 中。

### Perplexity

`SystemOneProfile::PERPLEXITY`，`POST /v1/decisions`，请求体为 System One 格式。来源：[Decisions API 快速入门](https://docs.perplexity.ai/docs/decisions/quickstart)，阅读于 2026-11-12。图片是状态中任意位置的 `image_url` 部分，使用 PNG、JPEG 或 WebP 的 data URL，最多 8 张。Perplexity 不回答 box 问题，并拒绝它未记录的顶层字段，因此应用自己的字段在发送前即为 `Unsupported`。不会发送 `mode` 和 `max_staleness`。

### Baseten

`SystemOneProfile::BASETEN`，`POST /v1/systemone`。来源：[System One 参考](https://docs.baseten.co/reference/inference-api/system-one)，阅读于 2026-11-12。图片放在 `state.messages` 下的一条用户消息中，PNG、JPEG 或 WebP，最多 16 张；`service_tier` 保留在 extras 中。不支持 box 问题。

### Fireworks AI

`SystemOneProfile::FIREWORKS`，`https://api.fireworks.ai/inference` 下的 `POST /v1/systemone`，只接受文本，不支持 box 问题。该模型处于私有预览阶段，因此配置文件依据 LiteLLM 的服务商页面，而不是 Fireworks 自己的页面。

### Cloudflare Workers AI

`SystemOneProfile::CLOUDFLARE_WORKERS_AI`，`POST /ai/run/@cf/cloudflare/{model}`，依据 [Clef 模型页面](https://developers.cloudflare.com/workers-ai/models/clef/)。请求体以短名称指定模型，例如 `clef`。一个请求最多 64 个问题，键由字母、数字、`_`、`.` 和 `-` 组成；顶层 `images` 列表中最多 4 张 PNG、JPEG 或 WebP 图片，每张最大 4 MiB。回复是 Cloudflare 封装中的 `result`。网关尚未向其发送请求。

### 其他服务器

`SystemOneProfile::GENERIC` 接受所有问题类型，在其图片位置写成部分时发送图片 URL，且不检查图片限制。它的路径来自 `EgressCtx::with_base_path`，图片位置来自 `SystemOneProfile::with_media`，适用于 vLLM、llama.cpp 和该格式的其他服务器。
