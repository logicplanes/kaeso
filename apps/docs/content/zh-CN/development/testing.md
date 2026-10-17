---
title: 测试
description: 仓库中的每个测试套件、如何运行，以及完成变更之前必须通过哪些检查。
order: 4
---

每个变更都附带测试，测试断言的是用户或应用实际看到的内容。覆盖率阈值从不降低；覆盖率提高时，阈值也随之提高。下面的所有命令都在仓库根目录下运行；需要数据库的测试套件要求[开发环境栈](local-setup.md#3-启动开发环境栈)正在运行。

## 完成变更之前

TypeScript 工作区：

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

Rust 工作区：

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

然后运行所修改部分的测试套件：网关、KPP 或打包相关的变更运行[端到端测试](#端到端测试)，打包相关的变更还要运行 `docker build -f deploy/console.Dockerfile .`，KPP 的变更还要运行[模糊测试构建](#模糊测试)、[基准测试](performance.md#kpp)和 `cargo test --doc -p kaeso-kpp -p kaeso-codecs`。在浅色和深色主题下、在桌面和手机宽度下、用英语、德语和中文查看修改过的每个界面。[持续集成](#持续集成)会在每个拉取请求上运行同样的检查。

## TypeScript

| 命令 | 检查内容 |
| --- | --- |
| `pnpm lint` | 在每个包中运行 ESLint 或 oxlint，并带有 Kaeso 自己的规则：类名只用于布局、令牌之外不使用颜色、不在控制台界面中直接写入文本；对于文档，用 `check:content` 检查每个页面 |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | 先构建 `packages/` 中的库，再运行 TypeScript 类型检查 |
| `pnpm test` | 带覆盖率阈值的 Vitest，在两个库、控制台和文档中运行；随后用 `node --test` 运行仓库 `scripts/` 中脚本的测试，例如变更日志脚本的测试；版本检查的测试也会检查仓库本身（[仓库中的版本号](releasing.md#仓库中的版本号)） |
| `pnpm i18n` | 控制台的每条文本都有德语和中文版本，占位符一致且没有多余条目（[翻译](translations.md#检查)） |
| `pnpm knip` | 未使用的文件、导出和依赖 |
| `pnpm build` | 所有生产构建，最后检查打包体积预算（[性能](performance.md)）；两个网站的构建随后运行站点检查（[网站](releasing.md#网站)） |
| `pnpm licenses:check` | 构建之后：代码随产品发布的每个包的许可证（[依赖与许可证](dependencies.md#许可证政策)） |
| `pnpm release:check` | 构建之后：像 npm 一样打包两个库，检查它们必须发布的每个文件都在其中，且版本一致 |
| `pnpm e2e` | 依次在控制台、文档和展示站点中运行 Playwright；展示站点的测试套件针对其构建产物运行，因此要先运行 `pnpm build` |

`pnpm --filter <package> <script>` 只运行单个包的脚本；包有 `kaeso-console`、`kaeso-docs`、`kaeso-showcase`、`@logicplanes/kaeso-ui` 和 `@logicplanes/kaeso-tokens`。`pnpm typecheck`、`pnpm test` 和 `pnpm knip` 会先构建这些库；如果库有变化，在单独运行某个包之前先运行 `pnpm build:packages`。

每个带测试的包都在自己的 `vitest.config.ts` 中为行、语句、分支和函数设置覆盖率阈值：令牌包为 100%，其他包为实测覆盖率，向下取整到 0.1 个百分点。

Playwright 需要 Chromium，只需安装一次：

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### 控制台

单元测试在 happy-dom 中运行，每个测试各自模拟 Platform API（`apps/console/src/test/network.ts`，fixture 位于 `apps/console/src/test/fixtures/`）。`renderApp(path)` 在某个地址挂载整个控制台，并使用一个独立的路由器，它随测试结束而结束，因此一个测试导航到的内容不会影响下一个测试；`renderUi` 只挂载单个组件。测试断言用户看到的内容和发出的请求，并且以任意顺序运行都能通过，可用 `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` 检查。`src/routes` 下的测试文件以 `-` 开头，因此路由生成器会跳过它们。

端到端测试套件从不使用开发环境栈。它会用 `deploy/compose.dev.yaml` 启动自己的栈，即使用 28000 段端口的 Compose 项目 `kaeso-console-e2e`，网关在 `127.0.0.1:28080`；控制台由 Vite 在 http://localhost:3100 提供；供 `20-sso-only` 使用的、只通过单点登录登录的第二个网关在 `127.0.0.1:28081`，其控制台由 Vite 在 http://localhost:3101 提供；运行结束时删除这个栈及其数据。`KAESO_GATEWAY_IMAGE` 会使用已构建好的网关镜像，而不是从检出的代码构建。各个 spec 从 `01-public` 开始逐个依赖前一个，`01-public` 负责设置安装。如果想在多次运行之间保留端到端测试栈，可以手动启动它并告知测试套件：

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

测试运行期间浏览器报告的任何错误都会让测试失败：未捕获的错误或 rejection，或者 `console.error`（React 和 Base UI 会通过它发出警告）。spec 从 `apps/console/e2e/support.ts` 导入 `test` 和 `expect`，这项检查就在其中。

### 文档站点

单元测试和端到端测试套件使用 `apps/docs/e2e/fixtures/content` 中的 fixture 页面，从不使用真实页面，因此修改文档永远不会让测试失败。`pnpm --filter kaeso-docs e2e` 针对这些页面的生产构建运行 Playwright（包括 axe）。真实页面由 `pnpm --filter kaeso-docs check:content` 和构建过程检查。该套件还会对其构建运行站点检查，并在 375×812 像素下以每种语言从手机顶栏打开搜索；站点检查自身的测试位于 `scripts/check-site.test.mjs`。

### UI 库与展示站点

组件测试使用 Testing Library 在 happy-dom 中运行，断言用户看到的内容。展示站点没有单元测试；它的端到端测试套件针对其生产构建运行 Playwright，因此要先运行 `pnpm build`：在浅色和深色主题下运行 axe，检查键盘操作和焦点环，对照令牌检查字号层级、动效和几何尺寸，检查菜单在原位打开，检查每个组件页面在手机宽度下不会横向滚动，检查德语和中文页面在任何脚本运行之前就以其语言呈现，且水合时没有不一致，在 375×812 像素下以每种语言从手机顶栏打开搜索，检查站点地图和每类页面的 head，并检查指向仓库的链接都使用该版本的标签。

## Rust

先安装一次 Cargo 工具：

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| 命令 | 检查内容 |
| --- | --- |
| `cargo fmt --all --check` | 代码格式（`rustfmt.toml`） |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | lint 规则，其中包括在测试之外禁止 `unwrap`、`expect`、`panic!` 和可能 panic 的索引（`Cargo.toml`、`clippy.toml`） |
| `cargo test --workspace --all-features` | 所有测试 |
| `cargo coverage-kpp` | 在 `cargo llvm-cov` 下运行 `kaeso-kpp` 和 `kaeso-codecs` 的测试，行覆盖率低于 95% 时失败 |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | `kaeso-kpp` 和 `kaeso-codecs` 的 README 中的示例，即这两个 crate 在 docs.rs 上的文档；`cargo llvm-cov` 不会运行它们 |
| `cargo coverage-gateway` | 在 `cargo llvm-cov` 下运行除 `kpp-conformance` 和 `xtask` 之外其他所有 crate 的测试，行覆盖率低于 91% 时失败 |
| `cargo test -p xtask -p kpp-conformance` | 覆盖率命令不包含的仓库任务和一致性服务器 |
| `cargo deny check` | 每个依赖的许可证、安全公告、禁用项和来源（`deny.toml`） |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP 更严格的策略（`crates/kaeso-codecs/deny.toml`）：库中不得包含运行时或 HTTP crate |
| `cargo sqlx prepare --workspace --check` | `.sqlx/` 中的离线查询数据与查询一致 |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` 是最新的 |

覆盖率命令是 `.cargo/config.toml` 中的别名，该文件也列出了它们排除的内容，例如测试辅助代码和二进制文件的 `main`。需要数据库的测试在数据库不可用时会失败，而不是跳过。

`kaeso-kpp` 和 `kaeso-codecs` 在各自的 `Cargo.toml` 中用 `rust-version` 写明能构建它们的最低 Rust 版本，CI 会用该版本检查和测试每个 crate。要在本地做同样的检查，用 `rustup toolchain install` 安装该版本，然后运行 `cargo +<version> test --locked -p <crate>`。

`cargo sqlx prepare --check` 需要设置 `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso`，并且已应用迁移。开发环境栈中的网关启动时会应用迁移；没有它时，运行 `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`。

### 测试数据库

每个测试都会在开发用的 PostgreSQL、ClickHouse 和 Valkey 上创建自己的数据库，因此测试可以并行运行而不共享状态。`kaeso-testkit` 负责为它们命名和清理：

| 名称 | 创建者 | 删除时机 |
| --- | --- | --- |
| `t_<uuid>` | 测试中的 `TestServices::start`，或 `kaeso-testkit fresh-services --temporary` | 测试结束时，包括测试 panic 时；否则由清理过程删除 |
| `dev_<uuid>` | `kaeso-testkit fresh-services`，用于[从源代码运行](local-setup.md#从源代码运行网关)的网关 | 仅在请求时删除：`kaeso-testkit drop-services dev_<uuid>` |

进程中的第一个测试会删除超过六小时且已不再使用的 `t_` 数据库。`KAESO_TEST_KEEP_DATABASES=1 cargo test …` 会保留失败测试的数据库并输出它们的名称。`KAESO_TEST_POSTGRES_URL`、`KAESO_TEST_CLICKHOUSE_URL` 和 `KAESO_TEST_VALKEY_URL` 让测试连接到其他服务器。

### 快照与参考文档

有些测试使用 `insta` 与经过审阅的快照进行比较。快照变化时，审阅差异并更新依赖它的内容：

| 快照 | 内容 | 同时更新 |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | 每项权限和每个内置角色 | [权限](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | 每项设置及其默认值和说明 | [配置](../self-hosting/configuration.md)和 `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | 跨方言矩阵的结果 | 该方言的映射页面（[KPP 内部实现](kpp/index.md#测试)） |
| `crates/kaeso-mail/src/snapshots/` | 每封邮件的文本和 HTML | |

`crates/kaeso-server/tests/error_copy.rs` 按照[错误消息规则](architecture/gateway.md#错误消息)检查 Platform API 可能发送的每条消息，`crates/kaeso-server/tests/docs_paths.rs` 检查仓库根目录以及 `crates/`、`deploy/`、`e2e/`、`fuzz/` 和 `.github/` 中的 Markdown 文件里提到的每个仓库路径和每个相对链接都指向存在的内容。

## 端到端测试

`e2e/` 中的测试套件使用官方 OpenAI 和 Anthropic Python SDK（版本固定在 `e2e/requirements.txt` 中），针对 KPP 的编码器、网关和一个全新的安装运行。先创建一次它们的虚拟环境：

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK 一致性

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

检查 SDK 是否接受 KPP 编码器产生的一切内容。该测试套件会构建并启动 `kpp-conformance`（`crates/kpp-conformance`，release 构建，监听随机端口）；设置 `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` 则会改为针对用 `cargo run --release -p kpp-conformance` 手动启动的服务器运行。

```text
SDK request ──▶ server: decode with the endpoint's codec (ingress decode)
                 model = "<source dialect>:<fixture>"
                 fixture ──▶ decode with the SOURCE codec (egress decode) ──▶ KPP
                 KPP ──▶ encode with the endpoint's codec (ingress encode)
SDK parses ◀── response / SSE stream / error
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

每个生成类方言的每个回复和流 [fixture](kpp/fixtures.md) 都会经由每个生成类端点提供，包括同方言和跨方言、流式和非流式。测试将 SDK 解析出的内容（文本、工具调用）与 KPP 解码出的内容（`GET /_expected`）进行比较，因此丢失文本、工具参数或可读推理的转换都会失败：

- SDK 暴露的推理（Responses 推理项的摘要和文本、Anthropic `thinking` 块的文本）必须等于解码出的推理；在 Responses 流中，`response.reasoning_summary_text.delta` 事件拼接起来必须等于每个 `response.reasoning_summary_text.done`，也必须等于最终回复中的摘要；
- fixture 为结构化输出的回复，在 SDK 读取时必须仍是有效的 JSON；
- 每个方言的每个错误 fixture 经由 Anthropic、Chat 和 Responses 端点提供时，都必须触发 SDK 的 `APIStatusError`，其状态码为该 KPP 错误类型在该格式中对应的状态码；速率限制错误必须保留 `retry-after`，并使用该格式的错误体（`e2e/conformance/test_errors.py`）；
- 模型端点提供一个固定的五个模型的目录，其中一个已停用，用 `ModelList::page` 分页：`e2e/conformance/test_models.py` 使用 Anthropic SDK 的自动分页向前（`limit=2`）和向后（`before_id`）遍历每一页，按 `lifecycle` 筛选，对未知游标或 `limit=0` 期望 `BadRequestError`，用 OpenAI SDK 列出模型、获取模型，并在两个 SDK 中对未知 ID 期望 `NotFoundError`。

`GET /_scenarios` 列出服务器提供的每个场景。

### SDK 测试套件

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

需要开发数据库。它会构建网关和 `kaeso-testkit`，创建新的数据库，启动模拟服务商和网关，并通过 Platform API 设置第一个用户、一个组织和项目、服务商密钥、目录中的模型、一个别名和 API 密钥。测试覆盖流式、经由静默服务商的保活、工具、图片、PDF、推理、跨服务商的双向转换、Token 计数、嵌入、模型列表、每一类错误、限额、SDK 重试和断开连接。

模拟服务商根据请求中的标记选择回复（`crates/kaeso-testkit/src/mock_provider.rs`）；`[mock:fixture=NAME]` 指定该服务商格式的一个 fixture，`[mock:gate=NAME]` 则让请求停住，直到测试打开这道闸门。以 `unserved-` 开头的模型 ID 会收到该格式的“找不到模型”。服务商密钥中的标记作用于 `GET /v1/models` 和 `GET /v1/models/{id}`：`[mock:gate=NAME]` 让它们停住，`[mock:models=absent]` 对两者都只回复一个裸 404，`[mock:models=list-only]` 只对后者这样回复，`[mock:models=endless]` 让 Anthropic 列表一直停在第一页。`MockProvider::gate()` 创建一道闸门，并返回写入提示词的标记；`wait_for(n)` 在有 `n` 个请求到达后返回，`open()` 放行所有请求。测试就是这样不靠 sleep 让请求保持在处理中的。

### 安装测试

```bash
e2e/.venv/bin/pytest -q e2e/install
```

从 `deploy/gateway.Dockerfile` 构建网关和模拟服务商的镜像，从干净状态运行 `deploy/compose.yaml`（使用 `e2e/install` 中的覆盖配置），完成首次启动设置，用两个 SDK 调用这个安装，并检查 PostgreSQL 和 ClickHouse 没有给网关任何它不需要的权限，以及 Valkey 拒绝不带密码的连接。控制台的地址由一个替身提供；`KAESO_TEST_CONSOLE_IMAGE` 会改为运行真实的控制台镜像，`KAESO_TEST_GATEWAY_IMAGE` 和 `KAESO_TEST_MOCK_IMAGE` 则使用已构建好的镜像。每当打包方式、某个 Dockerfile 或 `deploy/compose.yaml` 发生变化时，都要运行它。

## 模糊测试

`fuzz/` 为每个 KPP 解码器以及 SSE 解析器各提供一个 [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) 目标。它是一个独立的 Cargo 工作区，按照 cargo-fuzz 的要求不包含在仓库的工作区中，因此 `libfuzzer-sys` 永远不会进入库 crate。

| 目标 | 检查内容 |
| --- | --- |
| `decode_anthropic_messages`、`decode_openai_chat`、`decode_openai_responses`、`decode_openai_embeddings` | 该方言的每个解码器（三类请求、回复、错误、模型列表，以及按输入决定大小分块送入的流）接受任意字节都不会 panic，将解码结果编码为每种方言也不会 panic |
| `sse` | SSE 解析器从不 panic，并且无论输入如何切分，产生的条目都相同；将解析出的条目写回后再次解析，得到的条目也相同 |

共享逻辑在 `fuzz/src/lib.rs` 中；`fuzz/fuzz_targets/` 中的每个文件指定自己的方言。每个变更都要检查这些目标能够构建，并通过格式和 lint 检查：

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

运行它们需要 nightly 工具链和 cargo-fuzz：

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

运行一个目标时，指定一个语料库文件夹，并以该方言的 fixture 作为种子语料库；`sse` 目标使用所有 fixture：

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

CI 工作流（`.github/workflows/ci.yml`）在每个拉取请求、每次推送到 `dev` 和 `prod`、合并队列中以及 Release 工作流调用时运行。Release 工作流自己的推送，即它的发布提交和带回 `dev` 的版本，不会再次触发它：该版本已经通过了所有作业。在拉取请求上，第一个作业判断哪些区域有变化，未变化区域的作业会被跳过；其他情况下运行所有作业。唯一的必需检查是 **CI**：当 **Changes** 和 **No comments** 通过，且其他每个作业都通过或被跳过时，它才通过。

| 作业 | 运行内容 |
| --- | --- |
| Changes | 判断拉取请求修改了哪些区域：Rust、TypeScript、部署、仓库自身的文件、工作流 |
| No comments | 源文件、配置和 Markdown 代码块中没有注释 |
| Workflows | `actionlint` |
| Rust format | 对工作区和 `fuzz/` 运行 `cargo fmt --all --check` |
| Rust lint | 对工作区和 `fuzz/` 运行 `cargo clippy`，所有警告都视为错误 |
| Test KPP | `cargo coverage-kpp` 和 `cargo test --doc -p kaeso-kpp -p kaeso-codecs` |
| Minimum Rust version | 分别用 `kaeso-kpp` 和 `kaeso-codecs` 的 `rust-version` 所写的 Rust 版本，对它们运行 `cargo check` 和 `cargo test` |
| Test gateway | `cargo coverage-gateway`，使用开发环境栈 |
| Repository tooling | `node scripts/version.mjs check`、`cargo test -p xtask -p kpp-conformance` 和 `cargo test -p kaeso-server --test docs_paths` |
| Query data | 先应用 PostgreSQL 迁移，再运行 `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | `cargo deny check`，分别使用工作区的策略和 KPP 的策略 |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | `pytest -q e2e/sdk`，使用开发环境栈 |
| Compose config | 对 `deploy/compose.yaml` 和 `deploy/compose.dev.yaml` 运行 `docker compose config` |
| Images | 构建网关、模拟服务商和控制台的镜像，并标注 `Cargo.toml` 和 `apps/console/package.json` 中的版本；检查控制台镜像响应 `/healthz`，提供带内容安全策略的控制台，并对 `/assets/` 下不存在的文件返回 404 |
| Install test | 使用这些镜像（包括真实的控制台镜像）运行 `pytest -q e2e/install` |
| TypeScript lint | `pnpm format:check`、`pnpm lint`、`pnpm i18n` 和 `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript test | `pnpm test`，并在作业摘要中列出每个包的覆盖率 |
| TypeScript build | `pnpm build`、`pnpm licenses:check` 和 `pnpm release:check` |
| End-to-end console | 针对网关镜像运行 `pnpm --filter kaeso-console e2e` |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase | 针对 TypeScript build 构建的展示站点运行 `pnpm --filter kaeso-showcase e2e` |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | 必需检查 |
