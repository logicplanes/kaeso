---
title: 测试
description: 变更必须通过的每一项检查、每个测试套件覆盖的内容，以及运行方式。
order: 4
---

每项变更都带有测试，断言人或应用看到的内容。命令都在仓库根目录运行。需要数据库的测试套件依赖[开发栈](stack.md)。

## 完成变更之前

每项变更都要通过这些命令：

```bash
pnpm lint
pnpm format:check
pnpm typecheck
pnpm test
pnpm i18n
pnpm knip
pnpm build
pnpm licenses:check
pnpm release:check
pnpm e2e
```

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage-kpp
cargo coverage-gateway
cargo test -p xtask -p kpp-conformance
cargo deny check
cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check
cargo sqlx prepare --workspace --check
cargo run -q -p kaeso -- openapi | diff - openapi.json
```

然后针对改动的部分运行：

| 改动 | 还要运行 |
| --- | --- |
| KPP | [SDK 一致性](#sdk-一致性)套件、`cargo test --doc -p kaeso-kpp -p kaeso-codecs` 以及[模糊测试目标的检查](#模糊测试) |
| 网关 | [SDK 测试套件](#sdk-测试套件) |
| `deploy/`、Dockerfile、打包 | [安装测试](#安装测试)和 `docker build -f deploy/console.Dockerfile .` |
| `.github/workflows` | `actionlint` |
| 界面 | 在浅色和深色主题、桌面和手机宽度下，用英语、德语和中文查看每个改动过的界面 |

[持续集成](#持续集成)会对每个拉取请求运行同样的检查。

## TypeScript

| 命令 | 检查内容 |
| --- | --- |
| `pnpm lint` | 每个包中的 ESLint 或 oxlint，带有 Kaeso 自己的规则：类名只用于布局，令牌之外不出现颜色，控制台中不直接写文本。对文档，用 `check:content` 检查每个页面 |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | 构建 `packages/` 中的库之后运行 TypeScript |
| `pnpm test` | 两个库、控制台和文档中带覆盖率阈值的 Vitest。然后运行 `scripts/` 的测试，其中包括[版本检查](releasing.md#仓库中的版本号) |
| `pnpm i18n` | 控制台的每条文本都有占位符相同的德语和中文，且没有多余的条目（[翻译](translations.md#检查)） |
| `pnpm knip` | 未使用的文件、导出和依赖 |
| `pnpm build` | 每个生产构建及其打包预算。随后两个网站的构建会运行 `scripts/check-site.mjs`，检查站点地图和每个页面的头部 |
| `pnpm licenses:check` | 构建之后：每个代码随产品发布的包的许可证（[许可证政策](contributing.md#添加依赖)） |
| `pnpm release:check` | 构建之后：两个库按 npm 的方式打包，包含必须发布的每个文件，且版本一致 |
| `pnpm e2e` | 依次运行控制台、文档和展示站点中的 Playwright。展示站点的套件测试其构建产物，所以先运行 `pnpm build` |

`pnpm --filter <package> <script>` 运行某个包的脚本。这些包是 `kaeso-console`、`kaeso-docs`、`kaeso-showcase`、`@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens`。修改某个库之后，在按包过滤运行之前先运行 `pnpm build:packages`。每个包的覆盖率阈值在其 `vitest.config.ts` 中，只升不降。

Playwright 需要 Chromium，只需安装一次：

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### 控制台

单元测试在 happy-dom 中运行，Platform API 按测试打桩（`apps/console/src/test/network.ts`，fixture 在 `src/test/fixtures/`）。`renderApp(path)` 在某个地址挂载整个控制台，`renderUi` 挂载单个组件。测试断言人看到的内容和发出的请求，并且以任意顺序都能通过。要检查顺序，运行 `pnpm --filter kaeso-console exec vitest run --sequence.shuffle`。`src/routes` 下的测试文件以 `-` 开头，路由生成器会跳过它们。

端到端套件从不触碰开发栈。它启动自己的 Compose 项目 `kaeso-console-e2e`，网关在 `127.0.0.1:28080`，控制台在 http://localhost:3100 上。运行结束时会移除这个栈及其数据。要在多次运行之间保留它，自己启动并告知套件：

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

各个 spec 前后依赖，从设置实例的 `01-public` 开始。CI 分五部分运行它们；除非 `.github/workflows/ci.yml` 中的 `PART_1`、`PART_2`、`PART_3` 或 `PART_4` 列出了新的 spec，它都会在第 5 部分运行。浏览器报告的任何错误（包括 `console.error`）都会让测试失败，所以 spec 从 `apps/console/e2e/support.ts` 导入 `test` 和 `expect`。

### 文档站点

单元测试和端到端套件使用 `apps/docs/e2e/fixtures/content` 中的 fixture 页面，从不使用真实页面，所以编辑文档永远不会让测试失败。`pnpm --filter kaeso-docs e2e` 针对 fixture 的生产构建运行 Playwright（包括 axe），覆盖每种语言和手机宽度。真实页面由 `pnpm --filter kaeso-docs check:content` 和构建检查。图示渲染器的测试在 Chromium 中绘制，因此 `pnpm test` 需要 Playwright 安装的 Chromium。

### UI 库与展示站点

组件测试在 happy-dom 中使用 Testing Library 运行。展示站点没有单元测试。它的端到端套件针对生产构建运行 Playwright，所以先运行 `pnpm build`。它在两种主题下用 axe 检查无障碍，并检查键盘操作和焦点环。它还检查令牌的字号阶梯、动效和几何尺寸、手机宽度下的每个组件页面、德语和中文页面，以及站点地图和页面头部。

## Rust

先安装一次 Cargo 工具：

```bash
cargo install cargo-deny cargo-llvm-cov cargo-nextest sqlx-cli --locked
```

| 命令 | 检查内容 |
| --- | --- |
| `cargo fmt --all --check` | 格式（`rustfmt.toml`） |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | lint，其中包括测试之外不允许 `unwrap`、`expect`、`panic!` 或会 panic 的索引（`Cargo.toml`、`clippy.toml`） |
| `cargo test --workspace --all-features` | 所有测试 |
| `cargo coverage-kpp` | 在 `cargo llvm-cov` 下运行 `kaeso-kpp` 和 `kaeso-codecs` 的测试，并检查其覆盖率阈值 |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | 两个 crate 的 README 中的示例，即它们在 docs.rs 上的文档，`cargo llvm-cov` 不会运行这些示例 |
| `cargo coverage-gateway` | 在 `cargo llvm-cov nextest` 下运行除 `kpp-conformance` 和 `xtask` 外所有其他 crate 的测试，每个测试在独立进程中且全部同时运行，并检查其覆盖率阈值 |
| `cargo test -p xtask -p kpp-conformance` | 覆盖率命令不包含的仓库任务和一致性服务器 |
| `cargo deny check` | 每个依赖的许可证、安全公告、禁用项和来源（`deny.toml`） |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP 更严格的政策：库中不允许运行时或 HTTP crate |
| `cargo sqlx prepare --workspace --check` | `.sqlx/` 中的离线查询数据与查询一致 |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` 是最新的 |

覆盖率命令是 `.cargo/config.toml` 中的别名，该文件也包含它们的阈值和排除的文件。需要数据库的测试在数据库不可用时会失败，而不是跳过。

`cargo sqlx prepare --check` 需要 `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso`，并且迁移已经执行。开发栈中的网关启动时会执行迁移；没有网关时，运行 `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`。

`kaeso-kpp` 和 `kaeso-codecs` 在各自的 `Cargo.toml` 中用 `rust-version` 声明支持的最低 Rust 版本，CI 会用这个版本测试每个 crate。要自己验证，用 `rustup toolchain install <version>` 安装该版本，再运行 `cargo +<version> test --locked -p <crate>`。

### 测试数据库

每个测试都会在开发用的 PostgreSQL、ClickHouse 和 Valkey 上创建自己的数据库，因此测试可以并行运行而不共享状态。`kaeso-testkit` 负责命名和清理：

| 名称 | 创建者 | 删除时机 |
| --- | --- | --- |
| `t_<uuid>` | 测试中的 `TestServices::start`，或 `kaeso-testkit fresh-services --temporary` | 测试结束时（panic 时也会），或由清扫删除 |
| `dev_<uuid>` | `kaeso-testkit fresh-services`，供[从源码运行的网关](stack.md#从源码运行网关)使用 | 只能通过 `kaeso-testkit drop-services dev_<uuid>` 删除 |

一个进程中的第一个测试会清扫超过六小时且不再使用的 `t_` 数据库。要保留失败测试的数据库，用 `KAESO_TEST_KEEP_DATABASES=1` 运行它，测试会输出这些数据库的名称。

### 快照与参考文档

有些测试与经过审阅的 `insta` 快照比较。快照变化时，审阅差异并更新依赖它的内容：

| 快照 | 内容 | 同时更新 |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | 每个权限和内置角色 | [权限](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | 每项设置及其默认值和说明 | [配置](../self-hosting/configuration.md)和 `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | 跨方言矩阵的结果 | 该方言的映射页面（[KPP 内部实现](kpp/index.md#测试)） |
| `crates/kaeso-mail/src/snapshots/` | 每封邮件的文本和 HTML | |

另有两个测试守护文本。`crates/kaeso-server/tests/error_copy.rs` 按照[错误文案规则](architecture/gateway.md#错误消息)检查 Platform API 可能发送的每条消息。`crates/kaeso-server/tests/docs_paths.rs` 检查根目录以及 `crates/`、`deploy/`、`e2e/`、`fuzz/` 和 `.github/` 中 Markdown 文件里的每个仓库路径和相对链接都指向存在的内容。

## 端到端测试

`e2e/` 中的套件用 `e2e/requirements.txt` 中固定版本的 OpenAI 和 Anthropic 官方 Python SDK，测试 KPP 的编码器、网关和全新实例。先设置一次它们的虚拟环境：

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK 一致性

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

该套件检查 SDK 能否接受 KPP 编码器产生的一切。它会构建 `crates/kpp-conformance` 中的服务器 `kpp-conformance`，并在随机端口上启动。要使用通过 `cargo run --release -p kpp-conformance` 自己启动的服务器，设置 `KPP_CONFORMANCE_URL=http://127.0.0.1:8787`。

```mermaid
sequenceDiagram
  accTitle: 一次一致性测试请求
  accDescr: SDK 发送一个请求，其模型名指定源方言和 fixture。服务器用端点的编解码器解码请求，用源方言的编解码器把 fixture 解码为 KPP，再用端点的编解码器编码，并返回响应、流或错误，由 SDK 解析。
  participant SDK
  participant Server as kpp-conformance
  SDK->>Server: 请求，model "dialect:fixture"
  Note over Server: 用端点的编解码器解码
  Note over Server: 用源方言的编解码器<br>把 fixture 解码为 KPP
  Note over Server: 用端点的编解码器编码
  Server-->>SDK: 响应、SSE 流或错误
  Note over SDK: 解析
```

| 端点 | 编解码器 | SDK 调用 |
| --- | --- | --- |
| `POST /v1/messages` | `anthropic_messages` | `messages.create`、`messages.stream` |
| `POST /v1/messages/count_tokens` | `anthropic_messages` | `messages.count_tokens` |
| `POST /v1/chat/completions` | `openai_chat` | `chat.completions.create`、`chat.completions.stream` |
| `POST /v1/responses` | `openai_responses` | `responses.create`、`responses.stream` |
| `POST /v1/responses/input_tokens` | `openai_responses` | `responses.input_tokens.count` |
| `POST /v1/embeddings` | `openai_embeddings` | `embeddings.create` |
| `GET /v1/models`、`GET /v1/models/{id}` | 带 `anthropic-version` 请求头时为 `anthropic_messages`，否则为 `openai_chat` | `models.list`、`models.retrieve` |

每个生成类方言的每个响应和流式 [fixture](kpp/fixtures.md) 都会通过每个生成类端点，包括流式和非流式。测试把 SDK 解析出的内容与 KPP 解码出的内容（`GET /_expected`）比较，因此丢失文本、工具参数或可读推理的转换都会失败。测试还检查结构化输出仍是有效的 JSON，每个错误 fixture 都会以正确的状态码引发 SDK 的 `APIStatusError`（`e2e/conformance/test_errors.py`），模型列表的分页和过滤也符合 SDK 的预期（`e2e/conformance/test_models.py`）。`GET /_scenarios` 列出服务器提供的每个场景。

### SDK 测试套件

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

该套件在开发数据库上用 SDK 测试网关。它会构建网关和 `kaeso-testkit`，启动模拟服务商和网关，并通过 Platform API 设置用户、项目、服务商密钥、模型和 API 密钥。测试覆盖流式传输、保活、工具、图片、PDF、推理、跨服务商的双向转换、Token 计数、嵌入、模型列表、每类错误、限额、SDK 重试和断开连接。

模拟服务商（`crates/kaeso-testkit/src/mock_provider.rs`）根据请求中的标记选择回复：

| 标记 | 效果 |
| --- | --- |
| 提示中的 `[mock:fixture=NAME]` | 用服务商格式中的该 fixture 回复 |
| 提示中的 `[mock:gate=NAME]` | 挂起请求，直到测试打开闸门 |
| 提示中的 `[mock:steps=NAME]` | 先发送流的第一个事件，测试每推进一次闸门再发送一个 |
| 以 `unserved-` 开头的模型 ID | 该格式的“模型不存在”错误 |
| 服务商密钥中的 `[mock:gate=NAME]` | 挂起 `GET /v1/models` 和 `GET /v1/models/{id}` |
| 服务商密钥中的 `[mock:models=absent]` | 两个模型端点都只返回一个裸 404 |
| 服务商密钥中的 `[mock:models=list-only]` | 只有 `GET /v1/models/{id}` 返回裸 404 |
| 服务商密钥中的 `[mock:models=endless]` | 让 Anthropic 的模型列表停留在第一页 |

`MockProvider::gate()` 创建一个闸门并返回它的标记。`wait_for(n)` 在 `n` 个请求到达后返回，`open()` 放行所有请求。测试就是这样在不使用 sleep 的情况下让请求保持在进行中。

### 安装测试

```bash
e2e/.venv/bin/pytest -q e2e/install
```

该套件构建网关和模拟服务商的镜像，并以 `e2e/install` 中的覆盖配置从干净状态运行 `deploy/compose.yaml`。它完成首次启动设置，并用两个 SDK 调用该实例。它还检查 PostgreSQL 和 ClickHouse 没有授予网关不需要的权限，以及 Valkey 会拒绝不带密码的连接。控制台的地址由一个替身提供。`KAESO_TEST_CONSOLE_IMAGE` 改为运行真实的控制台镜像，`KAESO_TEST_GATEWAY_IMAGE` 和 `KAESO_TEST_MOCK_IMAGE` 则使用已经构建好的镜像。

## 模糊测试

`fuzz/` 为每个 KPP 解码器提供一个 [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) 目标 `decode_<dialect>`，并为 SSE 解析器提供目标 `sse`。解码器目标检查每个解码器接受任意字节都不会 panic，并且把结果编码为每种方言时也不会 panic。`sse` 目标检查无论输入如何切分，解析器都产生相同的条目。`fuzz/` 是一个独立的 Cargo 工作区，因此 `libfuzzer-sys` 永远不会进入库 crate。

每项变更都会检查这些目标能够构建，并且通过格式和 lint 检查：

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

运行这些目标需要 nightly 工具链和 cargo-fuzz：

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

运行目标时，指定一个语料目录，并把该方言的 fixture 作为种子语料。`sse` 目标使用所有 fixture：

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

崩溃会写入 `fuzz/artifacts/<target>/`。用 `cargo +nightly fuzz run <target> <file>` 复现它，把它变成编解码器中的回归测试，然后修复编解码器。

## 持续集成

CI 工作流 `.github/workflows/ci.yml` 在每个拉取请求、每次推送到 `dev` 和 `prod` 时运行，也在 Release 工作流中运行。在拉取请求上，它的第一个作业判断哪些领域有改动，未改动领域的作业会被跳过。在其他所有情况下，每个作业都会运行。

唯一必需的检查是 **CI**。当 **Changes** 和 **No comments** 通过，且其他每个作业都通过或被跳过时，它才通过。指向 `dev` 的拉取请求在它变绿之前无法合并（[合并](contributing.md#合并)）。

| 作业 | 运行内容 |
| --- | --- |
| Changes | 判断拉取请求改动了哪些领域 |
| No comments | 源文件、配置和 Markdown 代码块中没有注释 |
| Workflows | `actionlint` |
| Rust format、Rust lint | 对工作区和 `fuzz/` 运行 `cargo fmt` 和 `cargo clippy` |
| Test KPP | `cargo coverage-kpp` 以及 `kaeso-kpp` 和 `kaeso-codecs` 的文档测试 |
| Minimum Rust version | 用各自的 `rust-version` 对每个 KPP crate 运行 `cargo check` 和 `cargo test` |
| Test gateway | 在开发栈上运行 `cargo coverage-gateway` |
| Repository tooling | `node scripts/version.mjs check`、`cargo test -p xtask -p kpp-conformance` 和 `cargo test -p kaeso-server --test docs_paths` |
| Query data | 先执行 PostgreSQL 迁移，再运行 `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `openapi.json` 是最新的 |
| Rust dependencies | 用工作区和 KPP 的政策运行 `cargo deny check` |
| Fuzz targets | 模糊测试目标能够构建 |
| SDK conformance、SDK end-to-end | `pytest -q e2e/conformance` 和 `pytest -q e2e/sdk` |
| Compose config | 对两个 Compose 文件运行 `docker compose config` |
| Images（gateway、mock provider、updater、console） | 构建四个镜像；控制台的作业还检查其镜像以内容安全策略提供控制台 |
| Install test | 用这些镜像运行 `pytest -q e2e/install` |
| TypeScript lint | `pnpm format:check`、`pnpm lint`、`pnpm i18n` 和 `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript libraries | `pnpm build:packages`，为所有需要这些库的作业构建一次 |
| TypeScript test | `pnpm test`，每个工作区一个作业；控制台分六个分片，其覆盖率由另一个作业合并并检查 |
| TypeScript build | `pnpm build`、`pnpm licenses:check` 和 `pnpm release:check` |
| End-to-end console、docs 和 showcase | 每个包的 `e2e`；控制台分五部分，展示站点分多个分片，其无障碍检查使用单独的分片 |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |

端到端分片失败时，**End-to-end console report** 或 **End-to-end showcase report** 作业会把各分片的结果合并成一份 HTML 报告，即制品 `playwright-console` 或 `playwright-showcase`。
