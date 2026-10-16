---
title: Testing
description: Every test suite in the repository, how to run it, and what must pass before a change is done.
order: 4
---

Every change comes with tests, and tests assert on what a person or an app sees. Coverage thresholds are never lowered; when coverage rises, the threshold rises with it. Every command below runs from the repository root, with the [development stack](local-setup.md#3-start-the-development-stack) running where a suite needs databases.

## Before a change is done

The TypeScript workspace:

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

The Rust workspace:

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

Then run the suites of the parts you changed: the [end-to-end tests](#end-to-end-tests) for the gateway, KPP or packaging, `docker build -f deploy/console.Dockerfile .` for packaging, and the [fuzz build](#fuzzing) and [benchmarks](performance.md#kpp) for KPP. Look at every interface you changed in light and dark, at desktop and phone widths, in English, German and Chinese. [Continuous integration](#continuous-integration) runs the same checks on every pull request.

## TypeScript

| Command | What it checks |
| --- | --- |
| `pnpm lint` | ESLint or oxlint in every package, with Kaeso's own rules: layout-only class names, no colours outside the tokens, no text written straight into the console's interface; for the docs, every page with `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, after building the libraries in `packages/` |
| `pnpm test` | Vitest with coverage thresholds, in both libraries, the console and the docs |
| `pnpm i18n` | Every text of the console in German and Chinese, with the same placeholders and nothing left over ([translations](translations.md#checking)) |
| `pnpm knip` | Unused files, exports and dependencies |
| `pnpm build` | Every production build, ending with its bundle budgets ([performance](performance.md)) |
| `pnpm licenses:check` | After a build: the license of every package whose code is shipped ([dependencies](dependencies.md#license-policy)) |
| `pnpm release:check` | After a build: packs both libraries as npm would and checks that every file they must ship is there, at one version |
| `pnpm e2e` | Playwright in the console, the docs and the showcase, one after the other; the showcase's suite runs against its build, so `pnpm build` comes first |

`pnpm --filter <package> <script>` runs one package's script alone; the packages are `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`. `pnpm typecheck`, `pnpm test` and `pnpm knip` build the libraries first; run `pnpm build:packages` before a filtered run when they changed.

Every package with tests sets thresholds for lines, statements, branches and functions in its `vitest.config.ts`: 100% for the tokens, and for the others their measured coverage, rounded down to a tenth of a percent.

Playwright needs Chromium, installed once:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### The console

Unit tests run in happy-dom against the Platform API stubbed per test (`apps/console/src/test/network.ts`, with fixtures in `apps/console/src/test/fixtures/`). `renderApp(path)` mounts the whole console at an address with a router of its own, which ends with the test, so nothing a test navigates to reaches the next one; `renderUi` mounts a single component. Tests assert on what a person sees and on the requests sent, and pass in any order, which `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` checks. Test files under `src/routes` start with `-`, so the route generator skips them.

The end-to-end suite never touches the development stack. It starts a stack of its own from `deploy/compose.dev.yaml`, the Compose project `kaeso-console-e2e` on the 28000 ports with the gateway on `127.0.0.1:28080`, serves the console with Vite on http://localhost:3100, and removes the stack and its data when the run ends. `KAESO_GATEWAY_IMAGE` uses a gateway image that is already built instead of building one from the checkout. The specs build on each other from `01-public`, which sets the install up. To keep an end-to-end stack running between runs, start it by hand and tell the suite:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

A test fails on any error the browser reports while it runs: an uncaught error or rejection, or a `console.error`, where React and Base UI warn. Specs import `test` and `expect` from `apps/console/e2e/support.ts`, which holds this check.

### The documentation site

The unit tests and the end-to-end suite use the fixture pages in `apps/docs/e2e/fixtures/content`, never the real pages, so a change to the documentation never breaks a test. `pnpm --filter kaeso-docs e2e` runs Playwright, axe included, against a production build of those pages. The real pages are checked by `pnpm --filter kaeso-docs check:content` and by the build.

### The UI library and the showcase

Component tests run in happy-dom with Testing Library and assert on what a person sees. The showcase has no unit tests; its end-to-end suite runs Playwright against its production build, so `pnpm build` comes first: axe in the light and dark themes, the keyboard and focus rings, the type scale, motion and geometry against the tokens, menus that open in place, and every component page at phone width without sideways scrolling.

## Rust

Install the Cargo tools once:

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| Command | What it checks |
| --- | --- |
| `cargo fmt --all --check` | Formatting (`rustfmt.toml`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints, among them no `unwrap`, `expect`, `panic!` or panicking indexing outside tests (`Cargo.toml`, `clippy.toml`) |
| `cargo test --workspace --all-features` | Every test |
| `cargo coverage-kpp` | The tests of `kaeso-kpp` and `kaeso-codecs` under `cargo llvm-cov`, failing below 95% of lines |
| `cargo coverage-gateway` | The tests of every other crate but `kpp-conformance` and `xtask` under `cargo llvm-cov`, failing below 91% of lines |
| `cargo test -p xtask -p kpp-conformance` | The repository tasks and the conformance server, which the coverage commands leave out |
| `cargo deny check` | Licenses, advisories, bans and sources of every dependency (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP's stricter policy (`crates/kaeso-codecs/deny.toml`): no runtime or HTTP crates in the library |
| `cargo sqlx prepare --workspace --check` | The offline query data in `.sqlx/` matches the queries |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` is current |

The coverage commands are aliases in `.cargo/config.toml`, which also lists what they leave out, such as test helpers and the binary's `main`. Tests that need the databases fail instead of skipping when the databases aren't there.

`cargo sqlx prepare --check` needs `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` with the migrations applied. The stack's gateway applies them when it starts; without it, run `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`.

### Test databases

Every test creates databases of its own on the development PostgreSQL, ClickHouse and Valkey, so tests run in parallel without sharing state. `kaeso-testkit` names them and cleans them up:

| Name | Made by | Dropped |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in a test, or `kaeso-testkit fresh-services --temporary` | When the test ends, also when it panics; otherwise by the sweep |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, for a gateway you [run from source](local-setup.md#run-the-gateway-from-source) | Only on request: `kaeso-testkit drop-services dev_<uuid>` |

The first test in a process drops `t_` databases older than six hours that are no longer in use. `KAESO_TEST_KEEP_DATABASES=1 cargo test …` keeps a failed test's databases and prints their names. `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` and `KAESO_TEST_VALKEY_URL` point the tests at other servers.

### Snapshots and references

Some tests compare against reviewed snapshots with `insta`. When one changes, review the diff and update what depends on it:

| Snapshot | Holds | Also update |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Every right and built-in role | [Rights](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Every setting with its default and description | [Configuration](../self-hosting/configuration.md) and `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | The outcomes of the cross-dialect matrix | The mapping page of the dialect ([KPP internals](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | The text and HTML of every email | |

`crates/kaeso-server/tests/error_copy.rs` checks every message the Platform API can send against the [error copy rules](architecture/gateway.md#error-messages), and `crates/kaeso-server/tests/docs_paths.rs` checks that every repository path and relative link in the Markdown files at the root and in `crates/`, `deploy/`, `e2e/`, `fuzz/` and `.github/` points at something that exists.

## End-to-end tests

The suites in `e2e/` run the official OpenAI and Anthropic Python SDKs, pinned in `e2e/requirements.txt`, against KPP's encoders, the gateway and a fresh install. Set up their virtual environment once:

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK conformance

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

Checks that the SDKs accept everything KPP's encoders produce. The suite builds and starts `kpp-conformance` (`crates/kpp-conformance`, a release build on a random port); `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` runs it against a server started by hand with `cargo run --release -p kpp-conformance` instead.

```text
SDK request ──▶ server: decode with the endpoint's codec (ingress decode)
                 model = "<source dialect>:<fixture>"
                 fixture ──▶ decode with the SOURCE codec (egress decode) ──▶ KPP
                 KPP ──▶ encode with the endpoint's codec (ingress encode)
SDK parses ◀── response / SSE stream / error
```

| Endpoint | Codec | SDK calls |
| --- | --- | --- |
| `POST /v1/messages` | `anthropic_messages` | `messages.create`, `messages.stream` |
| `POST /v1/messages/count_tokens` | `anthropic_messages` | `messages.count_tokens` |
| `POST /v1/chat/completions` | `openai_chat` | `chat.completions.create`, `chat.completions.stream` |
| `POST /v1/responses` | `openai_responses` | `responses.create`, `responses.stream` |
| `POST /v1/responses/input_tokens` | `openai_responses` | `responses.input_tokens.count` |
| `POST /v1/embeddings` | `openai_embeddings` | `embeddings.create` |
| `GET /v1/models`, `GET /v1/models/{id}` | `anthropic_messages` with an `anthropic-version` header, otherwise `openai_chat` | `models.list`, `models.retrieve` |

Every response and stream [fixture](kpp/fixtures.md) of every generation dialect is served through every generation endpoint, in the same dialect and across dialects, streamed and not. The tests compare what the SDK parsed (text, tool calls) with what KPP decoded (`GET /_expected`), so a translation that loses text, tool arguments or readable reasoning fails:

- the reasoning the SDK exposes (Responses reasoning items' summaries and text, Anthropic `thinking` blocks' text) must equal the decoded reasoning, and in Responses streams the `response.reasoning_summary_text.delta` events must add up to each `response.reasoning_summary_text.done` and to the final response's summaries;
- replies whose fixture is structured output must still be valid JSON when the SDK reads them;
- every error fixture of every dialect, served through the Anthropic, Chat and Responses endpoints, must raise the SDK's `APIStatusError` with the status that belongs to its KPP error kind in that format, keep `retry-after` on rate limits, and use the format's error body (`e2e/conformance/test_errors.py`);
- the models endpoints serve a fixed catalogue of five models, one of them retired, paged with `ModelList::page`: `e2e/conformance/test_models.py` follows every page with the Anthropic SDK's auto-paging forward (`limit=2`) and backward (`before_id`), filters by `lifecycle`, expects `BadRequestError` for an unknown cursor or `limit=0`, lists with the OpenAI SDK, retrieves models, and expects `NotFoundError` for unknown ids in both SDKs.

`GET /_scenarios` lists every scenario the server offers.

### SDK suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

Needs the development databases. It builds the gateway and `kaeso-testkit`, creates fresh databases, starts the mock provider and the gateway, and sets up the first user, an organization and project, provider keys, catalogue models, an alias and API keys through the Platform API. The tests cover streaming, keep-alives through a quiet provider, tools, images, PDFs, reasoning, both directions across providers, token counting, embeddings, model listings, every error class, limits, SDK retries and disconnects.

The mock provider chooses its reply from markers in the request (`crates/kaeso-testkit/src/mock_provider.rs`); `[mock:fixture=NAME]` names a fixture of the provider's format.

### Install test

```bash
e2e/.venv/bin/pytest -q e2e/install
```

Builds the gateway and mock provider images from `deploy/gateway.Dockerfile`, runs `deploy/compose.yaml` from a clean state with the overrides in `e2e/install`, completes the first-start setup, and calls the install with both SDKs. A stand-in serves the console's address; `KAESO_TEST_CONSOLE_IMAGE` runs the real console image instead, and `KAESO_TEST_GATEWAY_IMAGE` and `KAESO_TEST_MOCK_IMAGE` use images that are already built. Run it whenever packaging, a Dockerfile or `deploy/compose.yaml` changes.

## Fuzzing

`fuzz/` holds one [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) target per KPP decoder, plus the SSE parser. It is a Cargo workspace of its own, left out of the repository's workspace as cargo-fuzz expects, so `libfuzzer-sys` never reaches the library crates.

| Target | Checks |
| --- | --- |
| `decode_anthropic_messages`, `decode_openai_chat`, `decode_openai_responses`, `decode_openai_embeddings` | Every decoder of the dialect (requests of all three kinds, replies, errors, model listings, streams fed in chunks whose size the input decides) accepts any bytes without panicking, and encoding the model listings and stream events it decoded doesn't panic either |
| `sse` | The SSE parser never panics and yields the same items however the input is split |

The shared logic is in `fuzz/src/lib.rs`; each file in `fuzz/fuzz_targets/` names its dialect. Every change checks that the targets build and are formatted and linted:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

Running them needs a nightly toolchain and cargo-fuzz:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Run a target with a corpus folder and the dialect's fixtures as the seed corpus; the `sse` target takes every fixture:

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

```bash
mkdir -p fuzz/corpus/sse
cargo +nightly fuzz run sse fuzz/corpus/sse crates/kaeso-codecs/fixtures -- -max_total_time=600
```

A crash is written to `fuzz/artifacts/<target>/`. Reproduce it with `cargo +nightly fuzz run <target> <file>`, turn it into a regression test in the codec, then fix the codec. [Performance](performance.md#fuzzing) records the last full run.

## Continuous integration

The CI workflow (`.github/workflows/ci.yml`) runs on every pull request, on every push to `dev` and `prod`, in the merge queue, and from the Release workflow. On a pull request, its first job decides which areas changed, and the jobs of unchanged areas are skipped; everything else runs every job. The one required check is **CI**: it passes when **Changes** and **No comments** passed and every other job passed or was skipped.

| Job | Runs |
| --- | --- |
| Changes | Decides which areas a pull request changes: Rust, TypeScript, the documentation, deployment, the repository's own files, the workflows |
| No comments | No comments in source files, configuration or Markdown code blocks |
| Workflows | `actionlint` |
| Rust format | `cargo fmt --all --check`, for the workspace and `fuzz/` |
| Rust lint | `cargo clippy` with every warning denied, for the workspace and `fuzz/` |
| Test KPP | `cargo coverage-kpp` |
| Test gateway | `cargo coverage-gateway`, with the development stack |
| Repository tooling | `cargo test -p xtask -p kpp-conformance` and `cargo test -p kaeso-server --test docs_paths` |
| Query data | The PostgreSQL migrations, then `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | `cargo deny check`, with the workspace's policy and KPP's |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | `pytest -q e2e/sdk`, with the development stack |
| Compose config | `docker compose config` for `deploy/compose.yaml` and `deploy/compose.dev.yaml` |
| Images | Builds the gateway, mock provider and console images, and checks that the console image answers `/healthz` and serves the console with a content security policy |
| Install test | `pytest -q e2e/install` with those images, the real console included |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` and `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript test | `pnpm test`, with each package's coverage in the job's summary |
| TypeScript build | `pnpm build`, `pnpm licenses:check` and `pnpm release:check` |
| End-to-end console | `pnpm --filter kaeso-console e2e` against the gateway image |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase | `pnpm --filter kaeso-showcase e2e` against the showcase built by TypeScript build |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | The required check |
