---
title: KPP 内部实现
description: KPP 的类型和编解码器是如何构建的，面向 kaeso-kpp 和 kaeso-codecs 的贡献者。
order: 3
nav: KPP 内部实现
---

KPP，即 Kaeso Provider Protocol，是每个模型请求都要经过的规范格式。网关把应用发来的内容解码为 KPP，再把 KPP 编码后发给服务商，回复沿同一路径返回。调用 Kaeso 的人能看到的部分在[格式转换](../../concepts/format-translation.md)中介绍；这些页面面向修改 KPP 的人。

| 页面 | 内容 |
| --- | --- |
| [跨格式转换](cross-format.md) | 数据从一种 API 格式转到另一种格式时遵循的规则及其理由 |
| [字段类别](field-classes.md) | 每个 KPP 字段属于语义字段还是提示字段 |
| [Extras 允许列表](extras-allowlist.md) | 请求转换格式时可以丢弃的 Extras |
| [Anthropic Messages](anthropic-messages.md)、[OpenAI Chat Completions](openai-chat.md)、[OpenAI Responses](openai-responses.md)、[OpenAI Embeddings](openai-embeddings.md) | 每种格式的每个 wire 字段，以及它在 KPP 中的位置 |
| [Fixture 与录制](fixtures.md) | 编解码器测试所用的真实载荷，以及如何录制新的载荷 |

## Crate 与模块

| Crate | 内容 |
| --- | --- |
| `kaeso-kpp` | 只包含 KPP 类型：请求、回复、流事件、错误、Token 计数、嵌入和模型列表，以及 `validate()` 和 `required_capabilities()`。没有编解码逻辑，也没有 I/O。 |
| `kaeso-codecs` | 每种 API 格式一个 sans-IO 编解码器（代码中称为方言，即 `DialectId`），以及它们共用的构件。编解码器把字节转换为 KPP 值，把 KPP 值转换为字节；流按 chunk 逐块送入并产生事件。 |

两个 crate 都发布在 crates.io 上。它们的库代码只能使用 `serde`、`serde_json`、`bytes`、`thiserror`、`indexmap`、`base64` 和 `mime`；`cargo deny` 禁止它们使用 `tokio`、`reqwest`、`hyper` 和 `axum`（[依赖与许可证](../dependencies.md#kpp)）。

| 模块 | 方言 |
| --- | --- |
| `anthropic_messages` | Anthropic Messages（`/v1/messages`、`/v1/messages/count_tokens`） |
| `openai_chat` | OpenAI Chat Completions（`/v1/chat/completions`） |
| `openai_responses` | OpenAI Responses（`/v1/responses`、`/v1/responses/input_tokens`） |
| `openai_embeddings` | OpenAI Embeddings（`/v1/embeddings`） |
| `openai_models` | OpenAI Models API，由三个 OpenAI 编解码器共用 |

共享模块有 `codec`（契约）、`dispatch`（运行时按 `DialectId` 选择的编解码函数）、`wire`（serde 构件）、`sse`（事件流的解析器和写入器）、`cross`（跨方言数据的规则）、`continuity`（随对话传递的推理）、`keepalive`（防止实时流陷入静默的帧）、`models`（模型列表辅助函数）和 `testing`（往返比较，使用这些编解码器的其他 crate 也可以用）。

## 设计原则

- **无状态。** `GenerateRequest` 始终携带完整的对话。
- **消息由有序的内容块组成。** 块的顺序自始至终保持不变。
- **字节要紧的地方保留原始数据。** 工具参数、JSON schema、未知块和引用都是 `RawJson`；不透明的推理按收到时的样子保存，并标记其 `Origin`。
- **语义或提示。** 每个字段要么是语义字段（目标必须遵守，否则以 `Unsupported` 拒绝），要么是提示字段（可以忽略）。[字段类别](field-classes.md)列出了所有字段。
- **绑定来源的 Extras。** KPP 没有建模的内容都保存在 `Extras` 中，标记其来源方言，并且只写回该方言。`fields` 是语义字段：其他方言会拒绝它们，除非它们在[允许列表](extras-allowlist.md)中。`form` 只关乎呈现（字符串还是数组、`null` 还是缺失、chunk 如何分组），其他方言会忽略它。嵌套对象的未知键使用带点的键路径，例如 `function.x`。
- **发送之前拒绝。** `check()` 在发送任何内容之前返回 `Unsupported`，注明目标无法表达的功能。
- **从不静默丢弃。** 传入载荷的每个字段和块都恰好落到一个地方：类型化的 KPP 字段、绑定方言的 `Extras` 条目、`Unknown` 项，或种类为 `Unsupported` 的 `KppError`。丢弃数据的 match 分支就是 bug。
- **只做增量的公共 API。** 公共枚举和可扩展的结构体都是 `#[non_exhaustive]`；枚举变体包装带有构造函数（`TextBlock::new`、`Block::text` 等）的 `#[non_exhaustive]` 结构体，因此可以添加字段，而不会破坏构建或匹配这些值的代码。`Codec` trait 是密封的。

KPP 的值和错误都按值返回。`KppError` 和部分枚举变体较大，但一个值每个请求只构建一次，错误又属于冷路径；把它们装箱会让每个签名都更笨拙，因此工作区允许 `clippy::large_enum_variant` 和 `clippy::result_large_err`。

每个 KPP 类型都为日志和快照实现了 `serde`。这种 JSON 形式在补丁版本之间保持稳定，但它不是 wire 格式。

## 类型

这些类型的形态取决于编解码器保持无损所需要的东西：

- **`Extras`** 有三个字段：`origin`、`fields`（逐字节保存的原始 JSON）和 `form`（呈现标记）。同一方言内的往返必须重现被 KPP 规范化抹掉的写法，例如 `content` 是字符串还是数组、`null` 还是缺失，以及 chunk 边界；这些标记不携带含义，因此不会像未知的 `fields` 那样让其他方言拒绝请求。两个空的 `Extras` 无论来源如何都相等。
- **转发的请求头**（`anthropic-beta`、`anthropic-version`、`openai-beta`）保存在 `Extras::fields` 中，键以 `header:` 开头；重复的请求头用 `", "` 连接。请求体字段都不含冒号，因此这些键不会冲突，请求头也像其他 extra 一样绑定到其方言。
- **原始项。** `Box<RawValue>` 包装在 `RawJson` 中，按字节比较。未知的块、工具、工具选择、流事件和 delta 都是 `RawItem { origin, raw }`；引用是 `Citation { origin, raw }`，因为各方言的引用格式几乎没有共同之处。
- **回复。** `Reply` 是 `Generate`、`CountTokens` 或 `Embed`，`RequestKind` 也对应这三种：不知道端点，就无法区分 Anthropic 的 `/v1/messages` 和 `/v1/messages/count_tokens` 请求体。
- **工具。** `ToolDef` 是函数工具，或完整保留的未知工具；`ToolChoice` 是 `Auto`、`None`、`Required`、指定名称的工具，或未知值。函数工具的 `parameters` 是可选的，与 OpenAI 的规定一致。
- **输出与推理设置。** `OutputSpec::JsonSchema` 有可选的名称（Anthropic 的 schema 没有名称）和描述。`ReasoningConfig` 保存模式、推理强度、预算、摘要以及是否包含加密推理；它的枚举把不认识的值保存在 `Other(String)` 中，因为服务商经常新增推理强度级别。
- **推理块**保存来源、摘要、文本部分和不透明部分，因为一个 OpenAI 推理 item 同时带有 ID 和加密内容，还可能有多个文本部分，而只有摘要的块也需要靠来源来决定可以回放到哪里。
- **用量。** `Usage::input_tokens` 不含缓存读取和写入，它们是单独的字段（[用量](cross-format.md#用量)）；`Usage::raw` 保存服务商的用量对象及其来源，因此编码器只在自己的方言中叠加它。嵌入向量是 `f64`，因此服务商的值输出时保持不变。
- **错误。** `ErrorKind::Billing` 用于 Anthropic 的 `billing_error`（402）和 OpenAI 的 `insufficient_quota`，其他种类都无法如实描述它们。

## 流

KPP 流的结构是 `ResponseStart → (BlockStart → BlockDelta* → BlockStop)* → ResponseDelta* → ResponseStop`，任何位置都可能出现 `Error`，`Extra` 则携带 KPP 没有建模的方言事件。块索引是 KPP 自己的：按块开始的顺序依次为 0、1、2……。块可以交错（OpenAI 就是这样流式传输并行工具调用的），因此每个事件都注明它所属的块。每个生命周期事件都携带 `Extras`，因此同一方言内的回放能精确重现原始事件，包括各事件自己的字段，例如 OpenAI 的 `obfuscation`。

流解码器在带内报告失败：先给出到目前为止解码出的事件，最后是一个 `StreamEvent::Error`，因此同一 chunk 中先前解码的内容不会丢失。

## 编解码器契约

`Codec` 是一个由关联函数组成、在编译时选定的 trait，覆盖网关两侧的请求、回复、错误、流解码器和编码器，以及模型列表。在入站一侧，应用用该格式与 Kaeso 通信；在出站一侧，Kaeso 用它与服务商通信。网关要到每个请求时才知道两侧的方言，因此 `dispatch` 提供了接受 `DialectId` 的同一组函数；没有编解码器的方言会得到 `Unsupported`。

`decode_request` 接受一个 `RequestKind`。`EncodeCtx` 携带应用解码后的请求，因为 OpenAI Responses 会在回复中回显请求字段；它还携带一个 `created_at`，因为 sans-IO 编码器需要确定性的时钟。`EgressCtx` 携带网关对目标所知的信息：请求种类和流模式、服务商，以及模型是否推理、是否支持自适应思考；请求编码器还会在其中记下它改写过的工具 ID 和名称，由回复解码器映射回原值。

假设应用发送了一个 Anthropic Messages 请求，而模型通过 OpenAI Chat Completions 提供：

```rust
use kaeso_codecs::{EgressCtx, EncodeCtx, dispatch};
use kaeso_kpp::{DialectId, RequestKind};

let req = dispatch::decode_request(DialectId::AnthropicMessages, RequestKind::Generate, &body, &headers)?;
req.validate()?;
let mut egress = EgressCtx::new(&req).with_provider("openai");
let outgoing = dispatch::encode_request(DialectId::OpenAiChat, &req, &mut egress)?;

let reply = dispatch::decode_response(DialectId::OpenAiChat, status, &reply_headers, &reply_body, &egress)?;
let answer = dispatch::encode_response(DialectId::AnthropicMessages, &reply, &EncodeCtx::for_request(&req))?;
```

`encode_request` 会先运行目标的 `check()`。流的用法相同，使用 `dispatch::stream_decoder` 和 `dispatch::stream_encoder`；错误则使用 `dispatch::decode_error` 和 `dispatch::encode_error`。

## Wire 结构体

Wire 结构体必须保留每个字段，包括结构体没有声明的字段，还必须逐字节保留原始载荷（`RawValue`）。`#[serde(flatten)]` 无法同时做到这两点：flatten 会缓冲输入，而 `RawValue` 无法从缓冲区读取，于是工具参数和 schema 会丢失原始字节。`wire::Ext<T>` 能完成同样的工作，而且不做缓冲：

- JSON 对象中 `T` 没有声明的每个键都原样保存在 `Ext::extra` 中，并成为 KPP 的 `Extras::fields`；
- `T` 已声明、但 `T` 无法重现其收到的写法的每个键（`Option` 在 `None` 时跳过，却收到了 `"x": null`；`bool` 在为 false 时跳过，却收到了 `"flag": false`；`Vec` 在为空时跳过，却收到了 `"list": []`）都原样保存在 `Ext::verbatim` 中，并成为 KPP 的 `Extras::form`。

序列化 `Ext<T>` 时，先写 `T`，再写 `T` 没有写出的 verbatim 键，最后写 extra 键。因此解码再编码的往返是精确的；而编解码器修改某个类型化字段时（例如从另一种方言转换时），修改后的值总是优先于过时的 verbatim 写法。在编解码器必须看出区别的地方，`wire::Maybe<T>` 会区分缺失和 `null`。

在 `Ext` 内部使用的 wire 结构体遵循以下规则：不使用 `#[serde(flatten)]`、`alias`、`deny_unknown_fields`，也不使用 untagged 或 internally tagged 枚举；带标签的联合用 `wire::type_tag` 按 `"type"` 分派；含有重复键的 JSON 对象会被拒绝。只有编解码器会映射到类型化 KPP 字段的字段才会被声明；其他一切都有意不声明，作为 extras 传递。

OpenAI Responses 编解码器用普通的 serde 结构体读取类型化的值，并用一棵有序的 JSON 树（`openai_responses::node::Node`）写出，这棵树可以逐字节嵌入原始 JSON；`serde_json::Value` 则会对对象键排序，并重新格式化原始载荷。[原始写法如何保留](openai-responses.md#原始写法如何保留)介绍了它如何重现原始写法。

## 服务器发送事件

`sse::SseParser` 遵循 [WHATWG 事件流算法](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation)，但为了不丢失任何内容，有意做了以下不同处理：

- 有字段但没有 `data:` 行的帧仍会被报告，其 `data` 为空；
- 注释和未知字段会被报告，而不是被忽略；
- 在一行中间结束的流会报告为 `SseError::Truncated`，而不是被丢弃。

字节可以按任意 chunk 送入：行可以在任何位置被拆开，包括 CRLF 换行符的 `\r` 和 `\n` 之间。超过解析器上限（`sse::DEFAULT_MAX_FRAME_BYTES`，16 MiB）的帧是错误，这限制了每个流能占用的内存。

## 模型列表

模型列表（`GET /v1/models`、`GET /v1/models/{id}`）是 KPP 的一部分，包括 `ModelInfo`、`ModelLifecycle`、`ModelList` 和 `ModelsQuery`，因此网关自己永远不需要了解同一路径下的各种 wire 结构。每种方言都使用相同的路径，因此无法从路径得出入站方言：`dispatch::models_dialect` 根据请求头来选择（有 `anthropic-version` 表示 Anthropic，否则为 OpenAI）。`ModelList::page` 对每种方言都以相同的方式应用 `before_id`、`after_id`、`limit` 和生命周期过滤。

- **查询字符串**按 `&` 和 `=` 拆分，并按 `application/x-www-form-urlencoded` 进行百分号解码（`+` 表示空格）。未知参数成为 `query.<name>` 下的 extras。
- **日期。** KPP 保存 Unix 秒。RFC 3339 日期时间（Anthropic）和完整日期（OpenAI 的 `shutdown_date`）按前推格里高利历转换，不依赖日期库。亚秒位和时区偏移在转换中无法保留；只要再次编码无法重现收到的原文，就把原文保存在形式标记 `spelling` 中。9999 年之后的日期无法用 RFC 3339 表示：此时 Anthropic 的 `created_at` 取文档规定的未知值（纪元起点），其他日期取 `null`。
- **缺失或为 `null` 的字段。** 对 KPP 中没有值的字段，编码器填入文档规定的默认值；如果收到的请求体对该字段的写法不同，形式标记 `none` 会注明这一点。

## 流编码器

**Anthropic Messages。** Anthropic 一次只流式传输一个内容块，而来自其他方言的 KPP 流可能交错多个块：OpenAI 就是这样流式传输并行工具调用的，并在结尾关闭所有块。编码器是一个显式的状态机。第一个开始的块是活动块，到达时立即写出；在另一个块活动期间开始的块处于等待状态，按开始顺序缓冲；活动块结束时，写出下一个等待中的块，包括它的开始事件、缓冲的 delta，以及已经到达的结束事件。`ResponseDelta` 或 `ResponseStop` 会先关闭所有打开的块，多个 `ResponseDelta` 会合并为一个 `message_delta`，因为 SDK 每收到一个都会覆盖 `stop_reason`。Anthropic 的索引按块写出的顺序分配。不以 `ResponseStart` 开头的流会得到一个 ID 为空的 `message_start`。

**OpenAI Chat Completions。** 从 Chat 解码出的事件会借助 chunk 形式标记重新分组为原来的 chunk，因此同一方言内的回放会重现每一帧。来自其他方言的事件各自成为一个 chunk。响应一开始，流就以一个角色 chunk 开头（`delta: {"role": "assistant", "content": "", "refusal": null}`，即 OpenAI 自己的第一个 chunk），这样在模型思考期间应用也能收到流的消息；工具调用以一个头部 chunk 开始，后面跟参数 chunk；只有在应用请求了用量（`stream_options.include_usage`）时才发送用量，作为最后一个 `choices: []` 的 chunk。`ResponseStop` 写出 `data: [DONE]`；`Error` 写出 `data: {"error": …}` 并结束流，不写 `[DONE]`。

**OpenAI Responses。** 流引擎为每个 KPP 事件按 OpenAI 的发送顺序生成 SSE 事件，包括每一个冗余事件（`.done` 事件、part 事件、item 事件，以及带有全部输出 item 的最终 response 对象），并分配 `sequence_number`。解码器把同一个引擎作为影子运行：引擎在该位置会生成的每个 SSE 事件都关联到对应的 KPP 事件，差异记录为带作用范围的覆盖项；关联上了、但原始流中没有的事件记录在形式标记 `skip` 中；出现在引擎不会生成它们的位置上的事件则作为 `Extra` 传递。同一方言的流会被精确重现，而来自其他方言的流会成为完整、有效的 Responses 流。

每个流编码器也都能写出保活帧；何时写出见[保活消息](cross-format.md#保活消息)。

## 测试

| 测试套件 | 验证的内容 |
| --- | --- |
| `crates/kaeso-kpp/tests/types.rs` | 单独验证 KPP 类型：错误种类和块的名称保持稳定，以及 `validate()` 和 `required_capabilities()`。 |
| Fixture 往返（每个方言的 `tests` 模块） | `decode → KPP → encode` 把方言的每个 [fixture](fixtures.md) 重现为相等的 JSON 值（流：相等的事件序列），流还要在多种 chunk 切分下验证。必须被拒绝的 fixture 会以指定的 `Unsupported` 功能失败。 |
| `crates/kaeso-codecs/tests/cross_matrix.rs` | 对方言 A 的每个生成类 fixture 和其他每个方言 B：请求经过 `decode_A → check_B → encode_B`，并且必须能被 B 的解码器再次解码，对话内容保持不变；回复和流经过 `decode_A → encode_B → decode_B`，并且必须保留文本、拒绝和工具调用，转回 A 之后也是如此。Token 计数的请求和回复在 Anthropic 与 Responses 之间转换。结果保存为 `insta` 快照。 |
| `crates/kaeso-codecs/tests/dispatch.rs` | `dispatch` 在每种方言中都能解码服务商错误、编码带状态码的错误，嵌入编解码器会拒绝生成类回复。 |
| `crates/kaeso-codecs/tests/kpp_proptest.rs` | 随机生成的有效 KPP 请求和回复，在每种生成方言上经过编码和解码后保持不变，范围限于所有方言都支持的子集。 |
| `crates/kaeso-codecs/tests/keepalive.rs` | 应用看不到的内容仍会定期在线路上产生保活帧，并且这些帧对应用的解析器不可见。 |
| `crates/kaeso-codecs/tests/reasoning_carry.rs` | 推理发给另一方言的应用后，会在下一轮逐字节地回来，覆盖 Responses、Anthropic 和 Chat 应用。 |
| `crates/kaeso-codecs/tests/responses_on_anthropic.rs` | 使用 Anthropic 模型的 OpenAI Responses 应用，以及使用 OpenAI 模型的 Anthropic 应用：推理请求、按 OpenAI 事件顺序输出的思考流，以及工具循环。 |
| `e2e/conformance`，由 `crates/kpp-conformance` 提供服务 | 官方的 OpenAI 和 Anthropic Python SDK 能解析编码器产生的所有内容（[测试](../testing.md#sdk-一致性)）。 |
| `fuzz/` | 每个解码器都能接受任意字节而不 panic，SSE 解析器在任何 chunk 切分下都产生相同的项（[模糊测试](../testing.md#模糊测试)）。 |

`kaeso_codecs::testing` 按结构比较 JSON：键的顺序无关紧要，整数精确比较，带小数或指数的数字与其他任何数字按值比较，因此 `1` 等于 `1.0`。KPP 把 `temperature`、`top_p` 和各项惩罚保存为 `f64`，而服务商对两种写法一视同仁。

当变更改变了跨方言输出时，要审查快照的差异：运行 `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix`，然后在提交之前阅读有变化的 `.snap` 文件。

## 修改编解码器

1. **先有 fixture。** 把真实载荷加入 `crates/kaeso-codecs/fixtures/<dialect>/`，并在该文件夹的 `README.md` 中添加一行来源记录（[Fixture 与录制](fixtures.md)）。
2. **注明来源。** 每个映射都要在该方言的映射页面中注明来源：服务商的文档，或固定规范提交中的 OpenAPI schema。来源包括 [OpenAI OpenAPI 规范](https://github.com/openai/openai-openapi)、[Anthropic Messages](anthropic-messages.md) 页面中注明的 Anthropic OpenAPI 规范、官方 SDK 的类型（[openai-python](https://github.com/openai/openai-python)、[anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python)），以及服务商文档。来源含糊时，选择保守的行为，即 `Unsupported`，并提交一个 issue。
3. **先写失败的测试，再写代码。** 每个 wire 字段都映射到类型化的 KPP 字段、绑定来源的 `Extras` 条目、`Unknown` 项或 `Unsupported` 错误。
4. **更新该方言的映射页面**，三种语言都要更新，并放在同一个 pull request 中。

错误消息是写给调用 Kaeso 的人看的：它们说明目标不支持哪个字段，以及应该怎么做。
