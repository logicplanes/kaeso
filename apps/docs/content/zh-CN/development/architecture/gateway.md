---
title: 网关
description: 请求路径、配置、模型数据，以及网关的 Rust 代码遵循的规则。
order: 1
---

网关是一个安装中唯一的服务器：应用使用官方的 OpenAI 和 Anthropic SDK 调用它，控制台和脚本则通过 Platform API 管理它。它就是 `crates/` 中的 Rust 工作区，构建为一个二进制程序 `kaeso`。从外部看请求会经历什么，见[请求流程](../../concepts/request-flow.md)；本页介绍内部。

## 请求路径

```text
app ─▶ /v1 ─▶ 1 key ─▶ 2 decode (KPP) ─▶ 3 model ─▶ 4 limits ─▶ 5 provider key ─▶ 6 encode (KPP) + send ─▶ provider
app ◀──────── 8 encode reply (KPP) ◀── 7 decode reply (KPP) ◀───────────────────────────────────────────── provider
                                               └─▶ 9 charge usage (Valkey, PostgreSQL) · 10 log (ClickHouse)

console, scripts ─▶ /platform/v1 ─▶ session cookie | API key with platform access | platform key
                                  ─▶ exactly one right per endpoint ─▶ service ─▶ PostgreSQL + audit event
```

模型 API 位于 `crates/kaeso-server/src/model_api`，Platform API 位于 `crates/kaeso-server/src/platform`，两者共用的服务位于 `crates/kaeso-server/src/services`。各个 crate 列在[架构](index.md#rust-工作区)中。

## 规则

对网关的每项修改都遵循以下规则；其中大多数由 lint 或测试强制执行。

1. **线路格式只存在于 KPP 中。** 网关从不手动解析或构建 OpenAI 或 Anthropic 的请求、响应、流、错误或模型列表 JSON；这些由 `kaeso_codecs::dispatch` 完成。网关自己的 JSON 只有 Platform API 及其错误信封。
2. **不做透传。** 每个模型请求都会解码为 KPP 再重新编码，即使应用和服务商使用同一种格式也是如此。
3. **不重试，不回退。** 失败的服务商调用就是失败，并以应用的格式返回。
4. **从不静默丢弃数据。** KPP 拒绝的内容（`Unsupported`）会以应用自己的格式作为错误返回给应用，且在向服务商发送任何内容之前。`KppError` 从不会被捕获后忽略。
5. **库 crate 中没有 panic。** 除测试和二进制程序的启动代码外，不使用 `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!`、可能 panic 的索引或 `unsafe`；`Cargo.toml` 中的工作区 lint 和 `#![forbid(unsafe_code)]` 强制执行这一点。
6. **机密从不以未受保护的形式离开内存。** 服务商密钥、主密钥、API 密钥的机密部分、会话 ID、登录令牌、TOTP 机密和密码从不会被记录到日志、追踪、打印、存入 ClickHouse、由 API 返回或放入错误消息。它们保存在会自动脱敏的包装类型中。
7. **金额从不使用浮点数。** Rust 中用 `rust_decimal::Decimal`，PostgreSQL 中用 `NUMERIC`，ClickHouse 中用 `Decimal`；Valkey 中的实时花费计数器是以纳美元为单位的整数，向上取整。
8. **每项 Platform API 修改都会被审计，每个端点都恰好检查一项权限**，权限来自 `crates/kaeso-core/src/rights/catalogue.rs` 中的目录（[权限](../../concepts/rights.md)），`me/*` 和 `auth/*` 除外，`instance/*` 则检查实例管理员标志。路由与其权限一起注册，因此缺少检查会导致测试失败。
9. **组织之间相互隔离。** 每个读取组织数据的 PostgreSQL 和 ClickHouse 查询都按已认证上下文中的组织进行过滤，从不只依据用户输入。
10. **迁移只向前推进**，并在启动时在锁的保护下运行。它们位于 `crates/kaeso-store/migrations/postgres`（sqlx）和 `crates/kaeso-store/migrations/clickhouse`（按顺序排列的 SQL 文件）。需要手动步骤的迁移会使该发布成为主版本发布。
11. **OpenAPI 文档是生成的。** 操作的摘要和描述写在 `#[utoipa::path(summary = …, description = …)]` 中，类型的描述写在 `#[schema(description = …)]` 中，字段、变体和查询参数的描述则写在 `#[doc = "…"]` 属性中，这是 utoipa 在这些位置唯一读取的形式。`cargo run -q -p kaeso -- openapi > openapi.json` 会重新写出该文档。
12. **测试是确定性的。** 时钟和随机源通过注入提供，任何地方都不靠休眠来同步；测试在截止时间内轮询。

## 配置

每项设置都是 `crates/kaeso-server/src/config/settings.rs` 中表格的一行：它的 TOML 键、环境变量、类型、默认值、是否为机密，以及一句话的描述。加载时合并默认值、`KAESO_CONFIG` 指定的 TOML 文件和 `KAESO_*` 变量（包括用于机密的 `_FILE` 变量），由 `config/load.rs` 基于 `toml` crate 实现，且错误消息总会指明是哪项设置。

测试会检查每个环境变量名都与其键对应、必填设置和机密没有默认值、每条描述都是一个句子、`deploy/kaeso.example.toml` 将除机密外的每项设置都设为其默认值且能够加载，并将该表格渲染为经过审阅的快照（`crates/kaeso-server/tests/snapshots/`）。修改一项设置会改变该快照，[配置](../../self-hosting/configuration.md)页面据此更新。figment 这样的配置 crate 会为键和默认值引入第二个事实来源，因此没有使用。

## 模型数据

`crates/kaeso-core/data` 存放随每个发布附带的模型数据：

| 文件 | 内容 |
| --- | --- |
| `models-dev.json` | [models.dev](https://models.dev) 的快照，只保留 Kaeso 调用的服务商，因此全新安装无需网络访问即可工作。该文件夹的 `README.md` 记录了获取时间及其 SHA-256。 |
| `supplement.toml` | models.dev 没有的字段，每个条目的来源记录在该文件夹的 `README.md` 中：Anthropic 模型的 `adaptive_thinking`、models.dev 未列出时的 `efforts`，以及 `egress_dialect`，即 Kaeso 向 OpenAI 模型发送请求时使用的格式。 |

目录条目的值依次取其覆盖值、models.dev、补充数据，否则为空。网关同步时（默认每 24 小时一次）会应用相同的过滤规则并将结果存入 PostgreSQL；启动时，已存储的快照和内置快照中较新的那个生效。只保留 `crates/kaeso-core/src/provider.rs` 中列出的服务商。

发布前刷新快照：

```bash
cargo xtask models-dev
```

它会下载 `https://models.dev/api.json`（`--url` 或 `MODELS_DEV_URL` 可以更改地址），对其进行过滤，检查网关能否读取结果，写入快照、`crates/kaeso-core/data/README.md` 中的表格以及 `crates/kaeso-core/src/models_dev.rs` 中的获取时间，并打印新的哈希值。测试会检查三者一致。

## 错误消息

控制台会将 Platform API 的 `message` 原样显示在提示消息中，因此每条消息都是一个简短的句子：

- 按代码中的写法（含占位符）最多 90 个字符，填入值后最多 120 个字符。以句号结尾。
- 一个句子：两层意思用分号连接（`Your session has ended; sign in again.`），或者去掉第二层。不用“e.g.”之类的缩写，不换行。
- 平实而具体：说明哪里出错，合适时说明该怎么做。字段名写成代码片段（`` `amount` must be positive. ``），并把字段放入 `param`。
- 不出现内部名称（ClickHouse、Valkey、配置键、模块路径），也不在末尾附加服务商、SMTP 或 serde 错误之类的技术细节：用 `tracing` 记录这些内容，保持消息简短。
- 内部原因（`ApiError::internal`、内部类型的 `#[error]`、日志行）以小写字母开头；面向人的消息以大写字母或代码片段开头。

`crates/kaeso-server/tests/error_copy.rs` 会扫描 Platform API 背后的代码，任何违反这些规则的消息都会使其失败；集成测试也会检查收到的每个 Platform API 应答（`crates/kaeso-server/tests/common/copy.rs`）。模型 API 保留其服务商的错误格式，因此它的消息不受这些规则约束。
