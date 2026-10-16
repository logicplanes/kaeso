---
title: 依赖与许可证
description: 每个直接依赖、引入它的原因、它的许可证，以及维护这一政策的检查。
order: 7
---

Kaeso 采用 MIT 许可证，它发布的一切都只依赖采用宽松许可证的代码。新增依赖时，要在同一个拉取请求中为它在本页添加一行，写明需要它的原因及其许可证。

## 许可证政策

| 发布内容 | 允许的许可证 | 检查方式 |
| --- | --- | --- |
| 网关 | MIT、MIT-0、Apache-2.0（也包括带 LLVM 例外的版本）、BSD-2-Clause、BSD-3-Clause、ISC、Unicode-3.0、Unicode-DFS-2016、Zlib，以及下面列出的[例外](#例外) | 使用根目录的 `deny.toml` 运行 `cargo deny check` |
| `kaeso-kpp`、`kaeso-codecs` | 同一份列表，但没有例外，且完全不使用运行时或 HTTP crate | 使用 `crates/kaeso-codecs/deny.toml` 运行 `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` |
| `@logicplanes/kaeso-ui`、`@logicplanes/kaeso-tokens` | 对其生产依赖：MIT、ISC、BSD-2-Clause、BSD-3-Clause、Apache-2.0、0BSD | `pnpm licenses:check`（`scripts/licenses.mjs`） |
| 控制台和文档 | 对代码进入其构建产物的每个包：MIT、ISC、BSD 和 Apache-2.0 | 在 `pnpm build` 之后运行 `pnpm licenses:check` |

`cargo deny check` 还会拒绝来自未知注册表或 Git 仓库的 crate、通配符版本以及存在安全公告的 crate。Kaeso 发布的任何内容都绝不允许包含 GPL、LGPL 和 AGPL 代码。npm 包在各 `package.json` 文件中固定为确切版本，Cargo 的版本锁定在 `Cargo.lock` 中。

## Rust

### 网关

| Crate | 用途 | 许可证 |
| --- | --- | --- |
| `kaeso-kpp`、`kaeso-codecs` | KPP 类型和所有线路格式，来自同一工作区 | MIT |
| `tokio` | 异步运行时 | MIT |
| `axum` | HTTP 服务器 | MIT |
| `tower` | 中间件：超时、请求体大小限制 | MIT |
| `http`、`http-body-util` | HTTP 类型和请求体 | MIT |
| `reqwest` | 服务商传输，使用 rustls、HTTP/2 和流式请求体 | MIT OR Apache-2.0 |
| `futures`、`tokio-util`、`bytes` | 流、取消、字节缓冲区 | MIT（`bytes`、`tokio-util`），MIT OR Apache-2.0（`futures`） |
| `sqlx` | PostgreSQL，带编译时检查的查询和迁移 | MIT OR Apache-2.0 |
| `clickhouse` | 官方 ClickHouse 客户端，用于请求日志，以及在 `kaeso-testkit` 中删除测试数据库 | MIT OR Apache-2.0 |
| `redis` | Valkey 客户端，带连接管理器和 Lua 脚本 | BSD-3-Clause |
| `serde`、`serde_json` | 序列化 | MIT OR Apache-2.0 |
| `humantime-serde` | 配置中的时长 | MIT OR Apache-2.0 |
| `toml` | 配置文件和 models.dev 补充数据 | MIT OR Apache-2.0 |
| `clap` | `kaeso` 和 `cargo xtask` 的命令行 | MIT OR Apache-2.0 |
| `tracing`、`tracing-subscriber` | 结构化 JSON 日志 | MIT |
| `metrics`、`metrics-exporter-prometheus` | Prometheus 指标 | MIT |
| `utoipa`、`utoipa-axum` | 生成的 OpenAPI 文档 | MIT OR Apache-2.0 |
| `aes-gcm` | 服务商密钥的信封加密 | Apache-2.0 OR MIT |
| `argon2` | 密码哈希（Argon2id） | MIT OR Apache-2.0 |
| `sha2`、`subtle` | 密钥和令牌的哈希、`cargo xtask models-dev` 中 models.dev 快照的哈希、常量时间比较 | MIT OR Apache-2.0（`subtle`：BSD-3-Clause） |
| `rand` | 随机机密和令牌 | MIT OR Apache-2.0 |
| `zeroize`、`secrecy` | 擦除和脱敏内存中的机密 | Apache-2.0 OR MIT |
| `totp-rs` | TOTP（RFC 6238） | MIT |
| `openidconnect` | 基于 OpenID Connect 的单点登录；其 HTTP 请求经过网关自己带 SSRF 防护的客户端 | MIT |
| `lettre` | SMTP | MIT |
| `maud` | 邮件的 HTML：在编译时检查、会转义每个值的模板（[邮件](architecture/emails.md)） | MIT OR Apache-2.0 |
| `resvg`（仅测试） | 在测试中渲染文字标志的 SVG，以检查邮件中的图片是否为最新 | Apache-2.0 OR MIT |
| `uuid` | UUIDv7 主键 | Apache-2.0 OR MIT |
| `time` | UTC 时间戳 | MIT OR Apache-2.0 |
| `rust_decimal` | 金额，从不使用浮点数 | MIT |
| `rust_decimal_macros`（仅测试） | 测试中的十进制字面量 | MIT |
| `base64`、`url`、`ipnet`、`indexmap`、`percent-encoding` | 数据处理辅助 | MIT OR Apache-2.0 |
| `idna` | 以 ASCII 形式比较允许的邮箱域名，无论输入时使用何种大小写或文字 | MIT OR Apache-2.0 |
| `moka` | 带存活时间的进程内缓存 | MIT OR Apache-2.0 |
| `thiserror` | 库中的类型化错误 | MIT OR Apache-2.0 |
| `anyhow` | 仅用于二进制程序中的错误 | MIT OR Apache-2.0 |
| `rstest`、`insta`、`proptest` | 测试 | MIT OR Apache-2.0（`insta`：Apache-2.0） |
| `rsa`、`chrono` | `kaeso-testkit` 中的模拟身份提供方，仅用于测试 | MIT OR Apache-2.0 |

### KPP

`kaeso-kpp` 和 `kaeso-codecs` 的库代码只能使用 `serde`、`serde_json`（启用 `raw_value`）、`bytes`、`thiserror`、`indexmap`、`base64` 和 `mime`。其他任何依赖都需要在这里添加一行。

| Crate | 使用方 | 原因 |
| --- | --- | --- |
| `serde`（启用 `derive`） | 两者 | KPP 类型的稳定 JSON 形式；Wire 结构体 |
| `serde_json`（启用 `raw_value`、`float_roundtrip`） | 两者 | JSON；`RawValue` 用于逐字节一致的原始负载；`float_roundtrip` 让浮点数解析回的值与打印出的值完全一致，KPP 的属性测试依赖这一点 |
| `indexmap`（启用 `serde`） | 两者 | 用于 extras 的保序映射 |
| `thiserror` | 两者 | 错误类型 |
| `base64` | `kaeso-codecs` | Data URL 和嵌入向量 |

测试、基准测试和工具可以使用更多依赖，但绝不作为库的常规依赖：

| Crate | 使用方 | 原因 |
| --- | --- | --- |
| `insta`（启用 `json`） | `kaeso-codecs` 的测试 | 跨方言矩阵的快照 |
| `proptest` | `kaeso-codecs` 的测试 | 属性测试：SSE 分块拆分、KPP 往返 |
| `criterion` | `kaeso-codecs` 的基准测试 | [基准测试](performance.md#kpp) |
| `libfuzzer-sys` | `fuzz/`，一个独立的工作区 | cargo-fuzz 目标 |
| `axum`、`tokio`、`serde` | `crates/kpp-conformance`，从不发布 | 一致性测试服务器，从不进入库的依赖图，由 KPP 的 `deny.toml` 强制保证 |

### 例外

许可证宽松但不在 KPP 列表中的传递依赖 crate。每一项都只针对该 crate 在根目录的 `deny.toml` 中放行。

| Crate | 引入方 | 许可证 | 可以接受的原因 |
| --- | --- | --- | --- |
| `quoted_printable` | `lettre` | 0BSD | 等同于公有领域，没有任何条件 |
| `webpki-roots`、`webpki-root-certs` | `sqlx`、`reqwest`（rustls） | CDLA-Permissive-2.0 | 以数据形式提供的 Mozilla CA 证书列表；宽松许可，没有 copyleft |
| `xxhash-rust` | `redis` | BSL-1.0 | Boost Software License：宽松许可，只在源代码分发中要求署名 |

### 忽略的安全公告

| 公告 | Crate | 不适用的原因 |
| --- | --- | --- |
| [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071)（Marvin 攻击） | `rsa`，经由 `openidconnect` 引入 | 该时序侧信道影响 RSA 私钥操作。网关只用身份提供方的公钥验证 ID 令牌的签名；私钥只由 `kaeso-testkit` 中的模拟身份提供方在测试中使用。当 `rsa` 发布修复时会重新评估。 |

### 考虑过但未使用

| Crate | 不使用的原因 |
| --- | --- |
| `figment` | 配置由一张设置表驱动，其测试会检查名称、默认值和描述，并渲染出一份经过审阅的参考（[配置](architecture/gateway.md#配置)）。有了这张表，合并默认值、TOML 文件和 `KAESO_*` 变量只需在 `toml` 之上写几行代码，且错误消息总会指明是哪项设置；figment 会为键和默认值引入第二个事实来源。 |

## TypeScript

### UI 库

| 包 | 许可证 | 原因 |
| --- | --- | --- |
| `@base-ui/react` | MIT | 无样式层：为每个交互组件提供语义、焦点管理、键盘交互和定位 |
| `@logicplanes/kaeso-tokens` | MIT | 每个组件使用的取值，来自 `packages/tokens` |
| `@tanstack/react-table` | MIT | `DataTable` 的排序、调整大小、选择和列状态 |
| `@tanstack/react-virtual` | MIT | `DataTable` 和 `List` 超过 100 行时的虚拟化行 |
| `clsx`、`tailwind-merge` | MIT | 类名组合；`tailwind-merge` 了解只含令牌的主题，因此使用方的布局类名能干净地胜出 |
| `d3-array`、`d3-scale`、`d3-shape`、`d3-time`、`d3-sankey` | ISC、BSD-3-Clause（`d3-sankey`） | 图表的比例尺、形状和布局，以 React SVG 绘制 |
| `marked` | MIT | `Markdown`（`@logicplanes/kaeso-ui/markdown`、`@logicplanes/kaeso-ui/playground`、`ConversationView` 的 `markdown`）背后的 Markdown 词法分析器：只用其词法单元，从不使用其 HTML 渲染器；不在主入口中 |
| `react-day-picker` | MIT | `Calendar`、`DatePicker` 和 `DateRangePicker` 的日历网格和日期逻辑 |
| `shiki` | MIT | `CodeBlock` 中的语法高亮，延迟加载，语法显式指定 |
| `uqr` | MIT | `QRCode` 的二维码，用于双重验证设置 |

对等依赖：`react` 和 `react-dom`（MIT），以及 `@hugeicons/react` 和 `@hugeicons/core-free-icons`（MIT），这是唯一的图标集。

### 字体

| 字体 | 许可证 | 位置 |
| --- | --- | --- |
| Inter（可变字体，latin 和 latin-ext 子集） | SIL OFL 1.1，无保留字体名称 | `packages/ui/src/styles/fonts/`，来自 Fontsource 构建；随 `LICENSE-Inter.txt` 一起发布在 `dist/fonts/` 中 |
| Geist Mono（可变字体，latin 和 latin-ext 子集） | SIL OFL 1.1，无保留字体名称 | `packages/ui/src/styles/fonts/`，来自 Fontsource 构建；随 `LICENSE-GeistMono.txt` 一起发布在 `dist/fonts/` 中 |

字体文件是未经修改的子集，并保留原名。`pnpm release:check` 会拒绝缺少这两个许可证文件之一的包。

### 构建、测试和展示站点工具

这些都不属于任何已发布的包或发布的构建产物。

| 包 | 许可证 | 原因 |
| --- | --- | --- |
| `vite`、`@vitejs/plugin-react` | MIT | 构建，库中每个组件一个入口 |
| `typescript`、`tsx` | Apache-2.0、MIT | 类型、声明文件和令牌构建 |
| `tailwindcss`、`@tailwindcss/vite`、`@tailwindcss/cli` | MIT | 样式引擎；主题由设计令牌生成 |
| `react-docgen-typescript` | MIT | 组件注册表和属性表格 |
| `vitest`、`@vitest/coverage-v8`、`happy-dom` | MIT | 带覆盖率的单元测试和组件测试 |
| `@testing-library/react`、`@testing-library/user-event`、`@testing-library/jest-dom` | MIT | 从用户所见出发的测试 |
| `@playwright/test` | Apache-2.0 | 端到端测试 |
| `@axe-core/playwright`、`axe-core` | MPL-2.0 | 端到端测试中的无障碍检查 |
| `pngjs` | MIT | 在展示站点的几何检查中读取截图 |
| `astro`、`@astrojs/react`、`@astrojs/mdx` | MIT | 展示站点 |
| `eslint`、`oxlint`、`prettier`、`prettier-plugin-astro`、`knip` | MIT、MIT、MIT、MIT、ISC | lint、格式化和未使用代码检查 |
| `@types/*` | MIT | 类型定义 |

`axe-core` 采用 MPL-2.0，这是一种文件级的 copyleft 许可证；它只在测试中运行，从不被打包。展示站点的构建会通过 Astro 引入更多工具，其中包括用于图像处理的 `sharp`（附带采用 LGPL-3.0 的 libvips），以及采用 MPL-2.0 的 `lightningcss`；它们在构建时运行，其代码都不会进入任何包或发布的站点。

## Python 测试工具

端到端测试和 KPP 的 SDK 一致性测试套件会把 `e2e/requirements.txt` 中固定版本的包安装到 `e2e/.venv` 的虚拟环境中。它们从不进入镜像或二进制程序。

| 包 | 版本 | 原因 | 许可证 |
| --- | --- | --- | --- |
| `openai` | 3.29.0 | 被测的官方 OpenAI SDK | Apache-2.0 |
| `anthropic` | 1.13.0 | 被测的官方 Anthropic SDK | MIT |
| `httpx` | 0.28.1 | 测试套件准备阶段的 Platform API 调用 | BSD-3-Clause |
| `pytest` | 9.1.1 | 测试运行器 | MIT |
