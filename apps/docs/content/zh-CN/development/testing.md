---
title: 测试
description: 仓库中的每个测试套件、如何运行，以及完成变更之前必须通过哪些检查。
order: 4
---

每个变更都附带测试，测试断言用户或应用看到的内容。覆盖率阈值从不降低；覆盖率提高时，阈值随之提高。命令都在仓库根目录下运行；需要数据库的套件要求[开发环境栈](local-setup.md#3-启动开发环境栈)正在运行。

## 完成变更之前

每个变更都要通过以下命令：

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

然后根据修改的部分运行：

| 修改了 | 另外运行 |
| --- | --- |
| KPP | [SDK 一致性](#sdk-一致性)套件、`cargo test --doc -p kaeso-kpp -p kaeso-codecs`、[模糊测试目标的检查](#模糊测试)和 `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| 网关 | [SDK 测试套件](#sdk-测试套件) |
| `deploy/`、Dockerfile、打包 | [安装测试](#安装测试)和 `docker build -f deploy/console.Dockerfile .` |
| `.github/workflows` | `actionlint` |
| 界面 | 在浅色和深色主题下、在桌面和手机宽度下，用英语、德语和中文查看每个修改过的界面 |

[持续集成](#持续集成)会在每个拉取请求上运行相同的检查。

## TypeScript

| 命令 | 检查内容 |
| --- | --- |
| `pnpm lint` | 在每个包中运行 ESLint 或 oxlint，并带有 Kaeso 自己的规则：类名只用于布局、令牌之外不出现颜色、控制台中不直接写文本；对文档则用 `check:content` 检查每个页面 |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | 先构建 `packages/` 中的库，再运行 TypeScript |
| `pnpm test` | 在两个库、控制台和文档中运行带覆盖率阈值的 Vitest，然后用 `node --test` 运行 `scripts/` 中脚本的测试，其中包括对仓库本身运行的[版本号检查](releasing.md#仓库中的版本号) |
| `pnpm i18n` | 控制台的每段文字都有占位符相同的德语和中文，且没有多余的条目（[翻译](translations.md#检查)） |
| `pnpm knip` | 未使用的文件、导出和依赖 |
| `pnpm build` | 每个生产构建及其打包预算（[性能](performance.md)）；两个网站的构建随后运行[网站检查](releasing.md#网站) |
| `pnpm licenses:check` | 构建之后：每个代码随产品发布的包的许可证（[依赖](dependencies.md#许可证政策)） |
| `pnpm release:check` | 构建之后：像 npm 那样打包两个库，并检查它们必须包含的每个文件都在，且版本一致 |
| `pnpm e2e` | 依次在控制台、文档和展示站点中运行 Playwright；展示站点的套件测试其构建结果，因此要先运行 `pnpm build` |

`pnpm --filter <package> <script>` 只运行一个包的脚本；包名为 `kaeso-console`、`kaeso-docs`、`kaeso-showcase`、`@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens`。`pnpm typecheck`、`pnpm test` 和 `pnpm knip` 会先构建库；如果库有变化，请在按包过滤运行之前先运行 `pnpm build:packages`。

每个有测试的包都在其 `vitest.config.ts` 中为行、语句、分支和函数设置阈值：令牌包为 100%，其他包为实测覆盖率向下取整到十分之一个百分点。

Playwright 需要 Chromium，只需安装一次：

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### 控制台

单元测试在 happy-dom 中运行，每个测试各自模拟 Platform API（`apps/console/src/test/network.ts`，fixture 位于 `apps/console/src/test/fixtures/`）。`renderApp(path)` 在某个地址挂载整个控制台，使用随测试结束的独立路由器；`renderUi` 挂载单个组件。测试断言用户看到的内容和发出的请求，并且以任意顺序都能通过，`pnpm --filter kaeso-console exec vitest run --sequence.shuffle` 会检查这一点。`src/routes` 下的测试文件以 `-` 开头，以便路由生成器跳过它们。

端到端套件从不使用开发环境栈。它从 `deploy/compose.dev.yaml` 启动自己的栈：Compose 项目 `kaeso-console-e2e`，使用 28000 段的端口，网关在 `127.0.0.1:28080`，控制台在 http://localhost:3100 。`20-sso-only` 还会增加第二个只允许单点登录的网关，位于 `127.0.0.1:28081`，其控制台在 http://localhost:3101 。运行结束时会删除该栈及其数据。这些用例相互依赖，从负责完成安装设置的 `01-public` 开始。`KAESO_GATEWAY_IMAGE` 可以使用已经构建好的网关镜像。

要让端到端栈在多次运行之间保持运行，请自行启动它并告知套件：

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

运行期间浏览器报告的任何错误都会让测试失败：未捕获的错误或 rejection，或 `console.error`（React 和 Base UI 通过它发出警告）。用例从 `apps/console/e2e/support.ts` 导入 `test` 和 `expect`，这项检查就在其中。

### 文档站点

单元测试和端到端套件使用 `apps/docs/e2e/fixtures/content` 中的 fixture 页面，从不使用真实页面，因此修改文档永远不会让测试失败。`pnpm --filter kaeso-docs e2e` 针对这些 fixture 的生产构建运行 Playwright（包括 axe）；它还会对该构建运行网站检查，并在每种语言下以 375 × 812 像素从手机顶栏打开搜索。真实页面由 `pnpm --filter kaeso-docs check:content` 和构建来检查。网站检查本身的测试位于 `scripts/check-site.test.mjs`。图示渲染器的测试与构建一样在 Chromium 中绘制，因此 `pnpm test` 需要 Playwright 安装的 Chromium。

### UI 库与展示站点

组件测试在 happy-dom 中用 Testing Library 运行。展示站点没有单元测试；它的端到端套件针对其生产构建运行 Playwright，因此要先运行 `pnpm build`。它检查：

- 浅色和深色主题下的 axe、键盘操作和焦点环；
- 字号体系、动效和几何尺寸是否符合令牌，以及菜单是否在原位打开；
- 每个组件页面在手机宽度下没有横向滚动；
- 德语和中文页面在任何脚本运行之前就已是对应语言，并且水合时没有不一致；
- 在每种语言下以 375 × 812 像素从手机页眉打开搜索；
- 站点地图、每类页面的 head，以及指向仓库并带有该版本标签的链接。

## Rust

先安装一次 Cargo 工具：

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| 命令 | 检查内容 |
| --- | --- |
| `cargo fmt --all --check` | 格式（`rustfmt.toml`） |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | lint，其中包括测试之外不得使用 `unwrap`、`expect`、`panic!` 或可能 panic 的索引（`Cargo.toml`、`clippy.toml`） |
| `cargo test --workspace --all-features` | 所有测试 |
| `cargo coverage-kpp` | 在 `cargo llvm-cov` 下运行 `kaeso-kpp` 和 `kaeso-codecs` 的测试，行覆盖率低于 95% 时失败 |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | 两个 crate 的 README 中的示例；这些 README 就是它们在 docs.rs 上的文档，`cargo llvm-cov` 不会运行它们 |
| `cargo coverage-gateway` | 在 `cargo llvm-cov` 下运行除 `kpp-conformance` 和 `xtask` 之外其他所有 crate 的测试，行覆盖率低于 91% 时失败 |
| `cargo test -p xtask -p kpp-conformance` | 覆盖率命令排除在外的仓库任务和一致性服务器 |
| `cargo deny check` | 每个依赖的许可证、安全公告、禁用项和来源（`deny.toml`） |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP 更严格的政策（`crates/kaeso-codecs/deny.toml`）：库中不得有运行时或 HTTP 相关的 crate |
| `cargo sqlx prepare --workspace --check` | `.sqlx/` 中的离线查询数据与查询一致 |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` 是最新的 |

覆盖率命令是 `.cargo/config.toml` 中的别名，该文件也列出了它们排除的文件，例如测试辅助代码和二进制程序的 `main`。需要数据库的测试在数据库不可用时会失败，而不是跳过。

`cargo sqlx prepare --check` 需要已应用迁移的 `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso`。开发环境栈中的网关启动时会应用迁移；如果没有运行网关，请运行 `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`。

`kaeso-kpp` 和 `kaeso-codecs` 在各自的 `Cargo.toml` 中用 `rust-version` 声明所支持的最低 Rust 版本，CI 会用该版本检查和测试每个 crate。要自己验证，请用 `rustup toolchain install <version>` 安装该版本，然后运行 `cargo +<version> test --locked -p <crate>`。

### 测试数据库

每个测试都会在开发用的 PostgreSQL、ClickHouse 和 Valkey 上创建自己的数据库，因此测试可以并行运行而不共享状态。`kaeso-testkit` 负责命名和清理：

| 名称 | 创建者 | 删除时机 |
| --- | --- | --- |
| `t_<uuid>` | 测试中的 `TestServices::start`，或 `kaeso-testkit fresh-services --temporary` | 测试结束时，panic 时也会删除；否则由清理过程删除 |
| `dev_<uuid>` | `kaeso-testkit fresh-services`，用于[从源代码运行](local-setup.md#从源代码运行网关)的网关 | 只能通过 `kaeso-testkit drop-services dev_<uuid>` 删除 |

进程中的第一个测试会清理超过六小时且不再使用的 `t_` 数据库。`KAESO_TEST_KEEP_DATABASES=1 cargo test …` 会保留失败测试的数据库并打印其名称。`KAESO_TEST_POSTGRES_URL`、`KAESO_TEST_CLICKHOUSE_URL` 和 `KAESO_TEST_VALKEY_URL` 可以让测试使用其他服务器。

### 快照与参考文档

有些测试会与审阅过的 `insta` 快照比较。快照变化时，请审阅差异，并更新依赖它的内容：

| 快照 | 内容 | 同时更新 |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | 每项权限和每个内置角色 | [权限](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | 每项设置及其默认值和说明 | [配置](../self-hosting/configuration.md)和 `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | 跨方言矩阵的结果 | 该方言的映射页面（[KPP 内部实现](kpp/index.md#测试)） |
| `crates/kaeso-mail/src/snapshots/` | 每封邮件的文本和 HTML | |

另有两个测试守护文字：`crates/kaeso-server/tests/error_copy.rs` 按照[错误文案规则](architecture/gateway.md#错误消息)检查 Platform API 可能发送的每条消息；`crates/kaeso-server/tests/docs_paths.rs` 检查根目录以及 `crates/`、`deploy/`、`e2e/`、`fuzz/` 和 `.github/` 中 Markdown 文件里的每个仓库路径和相对链接都指向存在的内容。

## 端到端测试

`e2e/` 中的套件用 OpenAI 和 Anthropic 的官方 Python SDK（版本固定在 `e2e/requirements.txt` 中）测试 KPP 的编码器、网关和全新安装。先创建一次虚拟环境：

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK 一致性

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

该套件检查 SDK 是否接受 KPP 编码器产生的一切。它会构建并启动 `kpp-conformance`（`crates/kpp-conformance`，随机端口上的 release 构建）；设置 `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` 则改用你通过 `cargo run --release -p kpp-conformance` 启动的服务器。

```mermaid
sequenceDiagram
  accTitle: 一次一致性测试请求
  accDescr: SDK 发送一个请求，其模型名指定源方言和夹具。服务器用端点的编解码器解码请求，用源方言的编解码器把夹具解码为 KPP，再用端点的编解码器编码，并返回响应、流或错误，由 SDK 解析。
  participant SDK
  participant Server as kpp-conformance
  SDK->>Server: 请求，model "dialect:fixture"
  Note over Server: 用端点的编解码器解码
  Note over Server: 用源方言的编解码器<br>把夹具解码为 KPP
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

每个生成类方言的每个响应和流式 [fixture](kpp/fixtures.md) 都会通过每个生成类端点提供，包括同方言和跨方言、流式和非流式。测试把 SDK 解析出的内容与 KPP 解码出的内容（`GET /_expected`）进行比较，因此丢失文本、工具参数或可读推理内容的转换都会失败。具体来说：

- **推理**：SDK 暴露的推理内容（Responses 推理项的摘要和文本、Anthropic `thinking` 块的文本）与解码出的推理内容一致。在 Responses 流中，`response.reasoning_summary_text.delta` 事件拼接起来等于每个 `response.reasoning_summary_text.done`，也等于最终响应中的摘要。
- **结构化输出**：SDK 读取后仍是有效的 JSON。
- **错误**：每个方言的每个错误 fixture 通过 Anthropic、Chat 和 Responses 端点提供时，都会引发 SDK 的 `APIStatusError`，状态码对应该格式中的 KPP 错误类型，限流时保留 `retry-after`，并使用该格式的错误响应体（`e2e/conformance/test_errors.py`）。
- **模型列表**：模型端点提供一个固定的目录，包含五个模型，其中一个已退役，用 `ModelList::page` 分页。`e2e/conformance/test_models.py` 用 Anthropic SDK 的自动分页向前（`limit=2`）和向后（`before_id`）遍历每一页，按 `lifecycle` 过滤，对未知游标或 `limit=0` 期望 `BadRequestError`，用 OpenAI SDK 列出模型、获取模型，并在两个 SDK 中对未知 ID 期望 `NotFoundError`。

`GET /_scenarios` 列出服务器提供的每个场景。

### SDK 测试套件

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

该套件需要开发用的数据库。它会构建网关和 `kaeso-testkit`，创建全新的数据库，启动模拟服务商和网关，并通过 Platform API 设置第一个用户、一个组织和项目、服务商密钥、目录模型、一个别名和 API 密钥。测试覆盖流式传输、服务商静默时的保活、工具、图片、PDF、推理、跨服务商的双向转换、Token 计数、嵌入、模型列表、每类错误、限额、SDK 重试和断开连接。

模拟服务商（`crates/kaeso-testkit/src/mock_provider.rs`）根据请求中的标记选择回复：

| 标记 | 效果 |
| --- | --- |
| 提示中的 `[mock:fixture=NAME]` | 用服务商格式中的该 fixture 回复 |
| 提示中的 `[mock:gate=NAME]` | 挂起请求，直到测试打开闸门 |
| 以 `unserved-` 开头的模型 ID | 该格式的“模型不存在”错误 |
| 服务商密钥中的 `[mock:gate=NAME]` | 挂起 `GET /v1/models` 和 `GET /v1/models/{id}` |
| 服务商密钥中的 `[mock:models=absent]` | 两个模型端点都只返回一个裸 404 |
| 服务商密钥中的 `[mock:models=list-only]` | 只有 `GET /v1/models/{id}` 返回裸 404 |
| 服务商密钥中的 `[mock:models=endless]` | 让 Anthropic 的模型列表停留在第一页 |

`MockProvider::gate()` 创建一个闸门并返回它的标记；`wait_for(n)` 在 `n` 个请求到达后返回，`open()` 放行所有请求。测试就是这样在不使用 sleep 的情况下让请求保持在进行中。

### 安装测试

```bash
e2e/.venv/bin/pytest -q e2e/install
```

该套件用 `deploy/gateway.Dockerfile` 构建网关和模拟服务商的镜像，并以 `e2e/install` 中的覆盖配置从干净状态运行 `deploy/compose.yaml`。它完成首次启动设置，用两个 SDK 调用该安装，并检查 PostgreSQL 和 ClickHouse 没有授予网关不需要的权限，以及 Valkey 会拒绝不带密码的连接。控制台的地址由一个替身提供；`KAESO_TEST_CONSOLE_IMAGE` 改为运行真实的控制台镜像，`KAESO_TEST_GATEWAY_IMAGE` 和 `KAESO_TEST_MOCK_IMAGE` 则使用已经构建好的镜像。

## 模糊测试

`fuzz/` 为每个 KPP 解码器以及 SSE 解析器各提供一个 [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) 目标。它是一个独立的 Cargo 工作区，按照 cargo-fuzz 的要求放在仓库工作区之外，因此 `libfuzzer-sys` 永远不会进入库 crate。

| 目标 | 检查内容 |
| --- | --- |
| `decode_anthropic_messages`、`decode_openai_chat`、`decode_openai_responses`、`decode_openai_embeddings` | 该方言的每个解码器接受任意字节都不会 panic：三类请求、回复、错误、模型列表，以及按输入决定的块大小分块送入的流。把解码结果编码成每种方言也不会 panic |
| `sse` | SSE 解析器从不 panic，无论输入如何切分都产生相同的条目；把解析出的条目写回后再次解析，得到的条目也相同 |

共享逻辑位于 `fuzz/src/lib.rs`；`fuzz/fuzz_targets/` 中的每个文件对应一种方言。每个变更都要检查这些目标能够构建，并通过格式检查和 lint：

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

运行目标时，指定一个语料文件夹，并把该方言的 fixture 作为种子语料。`sse` 目标使用所有 fixture：

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

```bash
mkdir -p fuzz/corpus/sse
cargo +nightly fuzz run sse fuzz/corpus/sse crates/kaeso-codecs/fixtures -- -max_total_time=600
```

崩溃会写入 `fuzz/artifacts/<target>/`。用 `cargo +nightly fuzz run <target> <file>` 复现它，把它变成编解码器中的回归测试，然后修复编解码器。[性能](performance.md#模糊测试)记录了最近一次完整运行的结果。

## 持续集成

CI 工作流（`.github/workflows/ci.yml`）在每个拉取请求上、每次推送到 `dev` 和 `prod` 时，以及在 Release 工作流内部运行。Release 工作流自己的推送（发布提交以及合入 `dev` 的版本）不会再次触发它，因为该版本已经通过了每个作业。

在拉取请求上，第一个作业判断哪些领域发生了变化，未变化领域的作业会被跳过；其他情况下会运行所有作业。唯一的必需检查是 **CI**：当 **Changes** 和 **No comments** 通过、其他每个作业都通过或被跳过时，它才通过；只要有一个作业失败、超时或被取消，它就失败。合入 `dev` 的拉取请求必须等它通过后才能合并（[变更如何进入 dev](contributing.md#变更如何进入-dev)）。

| 作业 | 运行内容 |
| --- | --- |
| Changes | 判断拉取请求修改了哪些领域：Rust、TypeScript、部署、仓库自身的文件、工作流 |
| No comments | 源文件、配置和 Markdown 代码块中没有注释 |
| Workflows | `actionlint` |
| Rust format | 对工作区和 `fuzz/` 运行 `cargo fmt --all --check` |
| Rust lint | 对工作区和 `fuzz/` 运行 `cargo clippy`，所有警告都视为错误 |
| Test KPP | `cargo coverage-kpp` 和 `cargo test --doc -p kaeso-kpp -p kaeso-codecs` |
| Minimum Rust version | 分别用 `kaeso-kpp` 和 `kaeso-codecs` 的 `rust-version` 所指定的 Rust 版本运行 `cargo check` 和 `cargo test` |
| Test gateway | 在开发环境栈上运行 `cargo coverage-gateway` |
| Repository tooling | `node scripts/version.mjs check`、`cargo test -p xtask -p kpp-conformance` 和 `cargo test -p kaeso-server --test docs_paths` |
| Query data | 运行 PostgreSQL 迁移，然后运行 `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | 按工作区的政策和 KPP 的政策运行 `cargo deny check` |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | 在开发环境栈上运行 `pytest -q e2e/sdk` |
| Compose config | 对 `deploy/compose.yaml` 和 `deploy/compose.dev.yaml` 运行 `docker compose config` |
| Images (gateway)、Images (mock provider)、Images (console) | 并行构建这三个镜像，并标注 `Cargo.toml` 和 `apps/console/package.json` 中的版本；控制台的作业还会检查其镜像能响应 `/healthz`、以内容安全策略提供控制台，并对 `/assets/` 下不存在的文件返回 404 |
| Install test | 用这些镜像（包括真实的控制台）运行 `pytest -q e2e/install` |
| TypeScript lint | `pnpm format:check`、`pnpm lint`、`pnpm i18n` 和 `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript test (tokens, ui, docs, showcase, scripts) | 按工作区拆分的 `pnpm test`，每个工作区一个作业：`packages/tokens`、`packages/ui`、`apps/docs` 和 `apps/showcase` 的测试，以及 `node --test 'scripts/*.test.mjs'`，各自使用所属包的覆盖率阈值，覆盖率显示在作业摘要中 |
| TypeScript test (console 1/3, 2/3, 3/3) | 控制台的单元测试，分成三个分片（`vitest run --shard`），每个分片记录自己的覆盖率 |
| TypeScript test (console coverage) | 合并三个分片（`vitest run --merge-reports --coverage`）：有测试失败或合并后的覆盖率低于控制台的阈值时失败 |
| TypeScript build | `pnpm build`、`pnpm licenses:check` 和 `pnpm release:check`；端到端作业测试的就是这里构建出的库和站点 |
| End-to-end console (1/3, 2/3, 3/3) | 针对网关镜像分三部分运行 `pnpm --filter kaeso-console e2e`，每部分使用自己的栈 |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase (1/6 至 6/6) | 针对 TypeScript build 构建的展示站点，分六个分片（`playwright test --shard`）运行 `pnpm --filter kaeso-showcase e2e` |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | 必需检查 |

### 作业如何编排

每个作业在它所依赖的作业完成后立即开始，任何作业都不会重复构建其他作业已经构建好的内容：

- **TypeScript build** 只构建一次库和两个站点，并以制品的形式传给其他作业。只要 End-to-end console 运行，它就会运行，因此 Rust 和部署的变更也会触发它。
- 三个镜像作业并行运行。Images (gateway) 和 TypeScript build 完成后，End-to-end console 立即开始；安装测试等待全部三个镜像。
- 控制台的单元测试是所有 TypeScript 测试中耗时最长的，因此分成三个分片运行。每个分片在关闭阈值的情况下运行，因为三分之一的测试只覆盖部分代码；**TypeScript test (console coverage)** 合并各分片记录的覆盖率，并像 `pnpm test` 一样，把 `apps/console/vitest.config.ts` 中的阈值应用到整体上。
- 展示站点的测试彼此独立，用 Playwright 的 `--shard` 分成六个分片运行。
- 控制台的用例相互依赖，因此无法用 `--shard` 拆分。每部分启动自己的栈，先运行 `01-public`，再运行那些数据只来自同一部分中前面用例的用例：第 1 部分是项目的页面（`02-models` 至 `08-palette`、`11-instance`、`13-keyboard`、`16-period`），第 2 部分是组织（`02-models`、`04-members`、`10-org`、`12-lifecycle`、`19-roles`），第 3 部分是其余所有用例。新用例只要没有被工作流中的 `PART_1` 或 `PART_2` 列出，就会在第 3 部分运行；那里列出的名称如果没有对应的用例，作业就会失败。
- 某个分片或部分失败时，**End-to-end showcase report** 或 **End-to-end console report** 会把各分片的 blob 报告合并成一个 HTML 报告，即制品 `playwright-showcase` 或 `playwright-console`。这两个作业不属于必需检查。

缓存让重复的工作保持简短：pnpm 存储（以 `pnpm-lock.yaml` 为键）、Playwright 的浏览器（以 Playwright 版本为键；命中时只安装系统软件包）、Rust 的构建输出（在 `dev` 上写入，每个拉取请求都会读取），以及每个镜像的层（Docker Buildx 的 `gha` 缓存，同样从 `dev` 读取）。
