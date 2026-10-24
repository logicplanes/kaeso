---
title: Fixture 与录制
nav: Fixture
description: 编解码器测试所用的真实载荷，以及如何从服务商录制新的载荷。
order: 9
---

每项编解码器变更都从一个测试样本（fixture）开始：一个真实的请求体、回复、流或错误，位于 `crates/kaeso-codecs/fixtures`。每个 fixture 都会经过自己的编解码器做往返测试，跨方言矩阵把每个生成类 fixture 送入其他每个方言。[SDK 一致性](../testing.md#sdk-一致性)测试套件把它们提供给官方 SDK，网关的模拟服务商用它们作答，模糊测试目标也以它们为起点。

## 目录结构

| 文件夹 | 方言 |
| --- | --- |
| `anthropic_messages/` | Anthropic Messages（`/v1/messages`、`/v1/messages/count_tokens`） |
| `openai_chat/` | OpenAI Chat Completions（`/v1/chat/completions`） |
| `openai_responses/` | OpenAI Responses（`/v1/responses`、`/v1/responses/input_tokens`） |
| `openai_embeddings/` | OpenAI Embeddings（`/v1/embeddings`） |

每个方言文件夹都有 `requests/`、`responses/` 和 `errors/`，端点支持流式时还有 `streams/`。

| 文件 | 内容 |
| --- | --- |
| `requests/<name>.json` | 原样的请求体 |
| `requests/<name>.headers.json` | 随该请求发送、在允许列表中的转发请求头 |
| `responses/<name>.json` | 原样的 200 响应体 |
| `streams/<name>.sse` | 原样的 SSE 字节 |
| `errors/<name>.json` 和 `errors/<name>.meta.json` | 错误响应体，以及 `{"status": …, "headers": {…}}`。当编解码器以另一种响应体回复应用时（例如把 SpaceX 的字符串 `error` 改为 OpenAI 错误格式），`"encoded"` 保存该响应体，往返测试与它比较 |

部分端点有自己的子文件夹：Anthropic 的 `count_tokens/` 和 OpenAI Responses 的 `input_tokens/`，分别位于 `requests/` 和 `responses/` 中；以及用于模型列表的 `models/`（`anthropic_messages/models/`，以及三个 OpenAI 编解码器共用的 `openai_chat/models/`）。

其他使用某个方言的服务商（例如兼容 OpenAI 的 API）的录制内容放在 `<dialect>/providers/<id>/` 中，以该服务商在 `crates/kaeso-core/src/provider.rs` 中的 ID 命名。该文件夹有同样的 `requests/`、`responses/`、`streams/` 和 `errors/` 子文件夹，只在服务商有对应文件时才创建。`requests/` 存放只有该服务商接受的请求；其中每个名称都必须与该方言自己的请求 fixture 不同。

往返测试、解码器测试和跨方言矩阵会连同方言自己的 fixture 一起读取每个服务商文件夹，并把其中的文件命名为 `providers/<id>/<name>`。因此，新的服务商录制会给矩阵快照添加条目；审查这些条目，并用 `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix` 接受它们。SDK 一致性测试套件只提供方言自己的 fixture；当测试以 `providers/<id>/<name>` 指定某个服务商的文件时，网关的模拟服务商会提供该文件。

- `unknown-*` fixture 有意带有虚构的字段、块或事件，用于测试 `Extras` 和 `Unknown` 的处理。
- 编解码器必须拒绝的 fixture（`stateful-*`、`n-2*`、`input-audio*`）测试的是它们的 `Unsupported` 错误，而不是往返。

## 来源记录

每个方言文件夹及其中的 `requests/`、`responses/`、`streams/`、`errors/` 和 `models/` 文件夹都有一个 `README.md`，列出每个文件、它覆盖的内容以及它的来源：

| 标记 | 来源 |
| --- | --- |
| `doc:` | 服务商文档中的示例 |
| `sdk:` | 官方 SDK 中的测试或录制内容 |
| `schema:` | 根据固定版本的 OpenAPI schema 构造的请求体 |
| `RECORDED.md` | 用 `cargo xtask record` 进行的真实录制，附带模型和时间 |

方言文件夹的 `README.md` 还会注明其 fixture 遵循的固定版本规范和 SDK 版本。它还会注明这些 fixture 共有的约定，例如流与其响应之间一致的 ID 和 Token 计数。每添加一个新的 fixture，都要在其中添加一行。

服务商文件夹有自己的一个 `README.md`。其中包括：

- 服务商的名称和 ID；
- 它的 API 参考及阅读日期；
- 录制时使用的基础 URL 和模型；
- 其子文件夹中每个文件的一行。

每个子文件夹的 `RECORDED.md` 在开头几行注明该服务商。

## 录制真实响应

`cargo xtask record` 把请求 fixture 发送给真实的服务商，并保存返回的内容。它需要主动运行，只在本地机器上运行，从不在 CI 中运行，并且需要环境中有该服务商的 API 密钥：

```bash
export ANTHROPIC_API_KEY=…
cargo xtask record anthropic_messages tools-single thinking-enabled-budget
cargo xtask record anthropic_messages --count --all
```

```bash
export OPENAI_API_KEY=…
cargo xtask record openai_chat --all --dry-run
cargo xtask record openai_responses --count --all
```

`crates/kaeso-core/src/provider.rs` 中的其他任何服务商都以同样的方式用 `--provider` 录制，通常还要加上 `--model`，因为请求 fixture 中写的是 OpenAI 和 Anthropic 的模型：

```bash
export <ID>_API_KEY=…
cargo xtask record openai_chat --provider <id> --model <model> --dry-run \
  tools-single stream-include-usage
cargo xtask record openai_chat --provider <id> --model <model> \
  tools-single stream-include-usage
```

| 选项 | 作用 |
| --- | --- |
| `<dialect>` | `anthropic_messages`、`openai_chat`、`openai_responses` 或 `openai_embeddings` |
| `<names>` | 要发送的请求 fixture，不带 `.json` |
| `--all` | 该方言的所有请求 fixture，但不包括 `stateful-*`、`unknown-*`、`n-2*` 和 `input-audio*`：服务商会拒绝它们，或者它们带有虚构的字段 |
| `--count` | Token 计数端点及其位于 `requests/count_tokens/` 或 `requests/input_tokens/` 的 fixture；仅限 `anthropic_messages` 和 `openai_responses`，并且仅限 `count_tokens` 为该方言的服务商 |
| `--dry-run` | 打印将要发送的内容，不实际发送 |
| `--provider <id>` | 要录制的服务商，按其 ID 指定；它的 `dialects` 中必须列有该方言。默认情况下，`anthropic_messages` 为 `anthropic`，其他方言为 `openai` |
| `--model <id>` | 发送此模型，而不是各请求 fixture 中写的模型，并把它记录到 `RECORDED.md` 中 |

- 它读取 `requests/<name>.json`（对其他服务商还会读取 `providers/<id>/requests/<name>.json`），以及存在时的 `<name>.headers.json`，后者中在允许列表内的请求头（`anthropic-beta`、`anthropic-version`、`openai-beta`）会一并发送。Anthropic 请求带有 `anthropic-version: 2023-06-01`，除非该文件指定了其他版本。
- 对 2xx 回复，它写入 `responses/recorded-<name>.json`（使用 `--count` 时位于 `responses/count_tokens/` 或 `responses/input_tokens/`）；请求带有 `"stream": true` 时，写入 `streams/recorded-<name>.sse`（收到的原始字节）；出错时，写入 `errors/recorded-<name>.json`（响应体不是 JSON 时为 `.txt`）和 `recorded-<name>.meta.json`（状态码以及可能存在的 `retry-after`）。随后它会在该文件夹的 `RECORDED.md` 中添加一行，记录模型和 UTC 时间。`openai` 和 `anthropic` 录制到方言自己的文件夹中，其他每个服务商录制到 `providers/<id>/` 中。JSON 响应体以两个空格缩进，并保留服务商的键顺序。
- 密钥来自 `<ID>_API_KEY`，即服务商 ID 的大写形式，连字符换成下划线：`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`，服务商 `fireworks-ai` 则为 `FIREWORKS_AI_API_KEY`。`<ID>_BASE_URL` 替换服务商的默认基础 URL，即网关使用的那个。密钥放在该服务商的认证请求头中：`authorization: Bearer` 或 `x-api-key`。
- API 密钥从环境中读取，从不写入文件；请求头也从不保存。
- 写入之前，它会从所有携带云签名凭据的 URL 中删除查询字符串（AWS Signature Version 4、Google Cloud 签名 URL、Azure 共享访问签名）。网页搜索可能返回这类预签名链接。其余内容逐字节保持不变。

代码位于 `crates/xtask/src/record.rs`。提交之前要审查每个录制的文件：其中包含模型输出的任何内容，并且不得含有个人数据。往返测试会自动发现录制的 fixture。
