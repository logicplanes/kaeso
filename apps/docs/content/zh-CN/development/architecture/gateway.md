---
title: 网关
description: 请求路径、配置、模型数据，以及网关的 Rust 代码遵循的规则。
order: 1
---

网关是一套安装中唯一的服务器。应用用 OpenAI 和 Anthropic 的官方 SDK 调用它；控制台和脚本通过 Platform API 管理它。它就是 `crates/` 中的 Rust 工作区，编译成一个可执行文件 `kaeso`。[请求流程](../../concepts/request-flow.md)从外部描述一个请求；本页描述内部。

## 请求路径

```text
app ─▶ /v1 ─▶ 1 key ─▶ 2 decode (KPP) ─▶ 3 model ─▶ 4 limits ─▶ 5 provider key ─▶ 6 encode (KPP) + send ─▶ provider
app ◀──────── 8 encode reply (KPP) ◀── 7 decode reply (KPP) ◀───────────────────────────────────────────── provider
                                               └─▶ 9 charge usage (Valkey, PostgreSQL) · 10 log (ClickHouse)

console, scripts ─▶ /platform/v1 ─▶ session cookie | API key with platform access | platform key
                                  ─▶ exactly one right per endpoint ─▶ service ─▶ PostgreSQL + audit event
```

| 代码 | 位置 |
| --- | --- |
| 模型 API（`/v1`） | `crates/kaeso-server/src/model_api` |
| Platform API（`/platform/v1`） | `crates/kaeso-server/src/platform` |
| 两者共用的服务 | `crates/kaeso-server/src/services` |

[架构](index.md#rust-工作区)列出了所有 crate。

## 规则

每次修改网关都要遵循这些规则；其中大多数由 lint 和测试强制执行。

1. **wire 格式只存在于 KPP 中。** 网关从不手动解析或构建 OpenAI 或 Anthropic 的请求、响应、流、错误或模型列表 JSON，这些都由 `kaeso_codecs::dispatch` 完成。网关自己的 JSON 只有 Platform API 及其错误信封。
2. **不做透传。** 每个模型请求都会解码为 KPP 再重新编码，即使应用和服务商使用同一种格式。
3. **不重试，不回退。** 对服务商的调用失败就是失败，并以应用的格式返回。
4. **绝不悄悄丢弃数据。** KPP 拒绝的内容（`Unsupported`）会在向服务商发送任何内容之前，以应用自己的格式作为错误返回。`KppError` 从不被捕获后忽略。
5. **库 crate 中不出现 panic。** 除测试和可执行文件的启动阶段外，不使用 `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!`、可能 panic 的索引或 `unsafe`。`Cargo.toml` 中的工作区 lint 和 `#![forbid(unsafe_code)]` 强制执行这一点。
6. **机密信息绝不以未受保护的形式离开内存。** 服务商密钥、主密钥、API 密钥的机密部分、会话 ID、登录令牌、TOTP 密钥和密码，绝不会被写入日志或追踪、打印、存入 ClickHouse、由 API 返回或写进错误消息。它们保存在会自动脱敏的包装类型中。
7. **金额绝不使用浮点数。** Rust 中用 `rust_decimal::Decimal`，PostgreSQL 中用 `NUMERIC`，ClickHouse 中用 `Decimal`；Valkey 中的实时支出计数器是向上取整的整数纳美元。
8. **每个端点一个权限，每次变更都有审计。** Platform API 的每个端点都恰好检查 `crates/kaeso-core/src/rights/catalogue.rs` 中的一个权限（[权限](../../concepts/rights.md)），`me/*` 和 `auth/*` 除外；`instance/*` 检查实例管理员标志。路由与其权限一起注册，因此缺少检查会导致测试失败。
9. **组织之间相互隔离。** 每个读取组织数据的 PostgreSQL 和 ClickHouse 查询，都按已认证上下文中的组织过滤，绝不只依据用户输入。
10. **迁移只向前推进**，并在启动时于锁保护下运行：`crates/kaeso-store/migrations/postgres`（sqlx）和 `crates/kaeso-store/migrations/clickhouse`（按顺序执行的 SQL 文件）。已发布的文件绝不修改；修改 schema 就新增一个文件。需要手动步骤的迁移会使该版本成为主版本。
11. **OpenAPI 文档是生成的。** 操作的摘要和描述写在 `#[utoipa::path(summary = …, description = …)]` 中，类型描述写在 `#[schema(description = …)]` 中，字段、变体和查询参数的描述写在 `#[doc = "…"]` 属性中，这是 utoipa 在这些位置唯一读取的形式。`cargo run -q -p kaeso -- openapi > openapi.json` 会重新写出该文档。
12. **测试是确定性的。** 时钟和随机源通过注入提供，任何代码都不通过休眠来同步；测试按截止时间轮询。

## 配置

每项设置都是 `crates/kaeso-server/src/config/settings.rs` 表格中的一行：TOML 键、环境变量、类型、默认值、是否为机密，以及一句话描述。服务商设置只有一行模式 `providers.<id>.<field>`，表格会对 `crates/kaeso-core/src/provider.rs` 中的每个服务商和 `PROVIDER_FIELDS` 中的每个字段展开这一行（命名规则见[服务商设置](../../self-hosting/configuration.md#服务商设置)）。

`config/load.rs` 合并默认值、`KAESO_CONFIG` 指定的 TOML 文件和 `KAESO_*` 变量，包括用于机密的 `_FILE` 变量。它直接使用 `toml` crate，每个错误都会指明对应的设置。项目不使用 figment 这类配置 crate，因为那会为键和默认值引入第二个真相来源。

测试检查以下几点：

- 每个环境变量都与其键对应，每个服务商变量都能唯一对应回其服务商和字段；
- 没有字段名以 `_` 加另一个字段名结尾，以免变量名冲突；
- 必填设置和机密没有默认值，每条描述都是一个句子；
- `deploy/kaeso.example.toml` 可以加载，并把机密以外的每项设置都设为其默认值。

测试还会把这张表渲染成一个经过审阅的快照，存放在 `crates/kaeso-server/tests/snapshots/` 中。修改一项设置会改变这个快照，[配置](../../self-hosting/configuration.md)页面随之更新。

## 服务商 ID

服务商 ID 由小写字母和数字组成，中间可用单个连字符分隔：`^[a-z0-9]+(-[a-z0-9]+)*$`。这条规则只定义在一个地方，即 `crates/kaeso-core/src/provider.rs` 中的 `ProviderId` 和 `ID_PATTERN`，进入网关的每个 ID 都要经过它：配置键和变量、Platform API 中的服务商密钥和目录模型（其 OpenAPI 文档以 `ProviderId` schema 给出该模式）、补充数据以及 models.dev 的导入。

有三项检查确保规则只有一处来源：

- 编译期检查让 `PROVIDERS` 中的每个 ID 都符合该规则；
- PostgreSQL 在 `provider_credentials`、`catalogue_models` 和 `usage_daily` 的 `provider` 列上的 CHECK 约束使用相同的模式，由一个存储层测试进行比对；
- 由于 models.dev 的条目按服务商自己的 ID 读取，一个测试会检查随附 models.dev 快照中的每个服务商都符合该规则。

## 模型数据

`crates/kaeso-core/data` 存放每个版本附带的模型数据：

| 文件 | 内容 |
| --- | --- |
| `models-dev.json` | [models.dev](https://models.dev) 的快照，只保留 Kaeso 调用的服务商，使新安装在无网络时也能工作。该目录的 `README.md` 记录了获取时间和 SHA-256。 |
| `supplement.toml` | models.dev 没有的字段，每个条目的来源都列在该目录的 `README.md` 中：Anthropic 模型的 `adaptive_thinking`，拒绝强制工具选择的模型的 `forced_tool_choice: false`，models.dev 未列出时的 `efforts`，以及 `egress_dialect`，即 Kaeso 发给 OpenAI 模型时使用的格式。 |

目录条目的取值顺序是：它的覆盖值，否则 models.dev，否则补充数据，否则为空。网关默认每 24 小时同步一次 models.dev（`KAESO_MODELS_DEV_INTERVAL`），使用相同的过滤规则并把结果存入 PostgreSQL；启动时，存储的快照和内置快照中较新的那个生效。只保留 `crates/kaeso-core/src/provider.rs` 中的服务商。

发布前刷新快照：

```bash
cargo xtask models-dev
```

该任务下载 `https://models.dev/api.json`（`--url` 或 `MODELS_DEV_URL` 可更改地址），进行过滤，并检查网关能否读取结果。随后它写入快照、`crates/kaeso-core/data/README.md` 中的表格以及 `crates/kaeso-core/src/models_dev.rs` 中的获取时间，并打印新的哈希值。测试会检查这三者一致。

## 记录的回复

开启内容日志后，网关以 KPP 形式（即 `kaeso_kpp::Response` 的 serde JSON）把每条回复存入 ClickHouse 中 `request_content` 的 `reply` 列。Platform API 按存储原样返回，控制台的日志查看器负责绘制。一个组织最多可以保留内容 36,500 天，所以今天记录的回复会跨越很多个版本，之后的每个控制台版本都必须仍能读取它。

- **第一种形式没有版本字段。** `crates/kaeso-server/src/services/snapshots/` 中的快照为有代表性的回复固定了这种形式：带引用的文本、工具调用、各种形式的推理、拒绝回答、未知块，以及有值和无值的停止原因。对 KPP 类型的 serde 改动会导致 `logged_replies_keep_the_v0_1_form` 失败。
- **改动这种形式就是格式变更。** 从第一次变更开始，网关会在回复对象中写入一个 `format` 编号，第二种形式为 `2`；控制台学会新形式，同时保留每种旧形式的渲染器。只接受改变后的快照永远不够。
- **控制台只读取它认识的形式。** 没有 `format` 的回复用当前渲染器绘制；带有任意 `format` 值的回复以存储的 JSON 加一条说明显示，因此比网关旧的控制台也不会出错（`apps/console/src/features/project/logs/render.ts` 中的 `isKnownReplyForm`）。

## 错误消息

控制台在提示消息中原样显示 Platform API 的 `message`，因此每条消息都是一个简短的句子：

- 按代码中的写法最多 90 个字符（含占位符），填入值后最多 120 个字符，以句号结尾。
- 只写一句话：两个意思用分号连接（`Your session has ended; sign in again.`），或者删掉第二个。不用“e.g.”之类的缩写，不换行。
- 说明哪里出了问题，合适时说明该怎么做。字段以代码格式写出（`` `amount` must be positive. ``），并把字段放进 `param`。
- 不出现内部名称（ClickHouse、Valkey、配置键、模块路径），也不附带服务商、SMTP 或 serde 错误之类的技术细节：这些用 `tracing` 记录。
- 内部原因（`ApiError::internal`、内部类型的 `#[error]`、日志行）以小写字母开头；面向人的消息以大写字母或代码片段开头。

`crates/kaeso-server/tests/error_copy.rs` 会扫描 Platform API 背后的代码，任何违反这些规则的消息都会使其失败；集成测试则检查收到的每个 Platform API 响应（`crates/kaeso-server/tests/common/copy.rs`）。模型 API 沿用服务商的错误格式，因此不受这些规则约束。
