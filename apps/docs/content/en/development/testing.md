---
title: Testing
description: Every check a change must pass, what each test suite covers and how to run it.
order: 4
---

Every change comes with tests that assert on what a person or an app sees. Commands run from the repository root. Suites that need databases need the [development stack](stack.md).

## Before a change is done

Every change passes these:

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

Then run these for the parts you changed:

| Changed | Also run |
| --- | --- |
| KPP | The [SDK conformance suite](#sdk-conformance), `cargo test --doc -p kaeso-kpp -p kaeso-codecs`, the [fuzz targets' checks](#fuzzing) and `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| The gateway | The [SDK suite](#sdk-suite) |
| `deploy/`, a Dockerfile, packaging | The [install test](#install-test) and `docker build -f deploy/console.Dockerfile .` |
| `.github/workflows` | `actionlint` |
| An interface | Look at every screen you changed in light and dark, at desktop and phone widths, in English, German and Chinese |

[Continuous integration](#continuous-integration) runs the same checks on every pull request.

## TypeScript

| Command | Checks |
| --- | --- |
| `pnpm lint` | ESLint or oxlint in every package, with Kaeso's own rules: layout-only class names, no colours outside the tokens, no text written straight into the console. For the docs, every page with `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, after building the libraries in `packages/` |
| `pnpm test` | Vitest with coverage thresholds in both libraries, the console and the docs. Then the tests of `scripts/`, among them the [version check](releasing.md#the-version-in-the-repository) |
| `pnpm i18n` | Every text of the console has German and Chinese with the same placeholders, and nothing is left over ([translations](translations.md#checking)) |
| `pnpm knip` | Unused files, exports and dependencies |
| `pnpm build` | Every production build and its bundle budgets. Both sites' builds then run the [site check](releasing.md#the-sites) |
| `pnpm licenses:check` | After a build: the license of every package whose code ships ([license policy](contributing.md#add-a-dependency)) |
| `pnpm release:check` | After a build: both libraries packed as npm would, with every file they must ship, at one version |
| `pnpm e2e` | Playwright in the console, the docs and the showcase, one after another. The showcase's suite tests its build, so run `pnpm build` first |

`pnpm --filter <package> <script>` runs one package's script. The packages are `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`. After you change a library, run `pnpm build:packages` before a filtered run. Each package's coverage thresholds are in its `vitest.config.ts`, and they only ever go up.

Playwright needs Chromium, installed once:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### The console

Unit tests run in happy-dom against the Platform API, stubbed per test (`apps/console/src/test/network.ts`, with fixtures in `src/test/fixtures/`). `renderApp(path)` mounts the whole console at an address, and `renderUi` mounts a single component. Tests assert on what a person sees and on the requests sent, and they pass in any order. To check the order, run `pnpm --filter kaeso-console exec vitest run --sequence.shuffle`. Test files under `src/routes` start with `-`, so the route generator skips them.

The end-to-end suite never touches the development stack. It starts its own, the Compose project `kaeso-console-e2e`, with the gateway on `127.0.0.1:28080` and the console on http://localhost:3100. The stack and its data are removed when the run ends. To keep it running between runs, start it yourself and tell the suite:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

The specs build on each other, starting with `01-public`, which sets the install up. CI runs them in three parts, and a new spec runs in part 3 unless `PART_1` or `PART_2` in `.github/workflows/ci.yml` names it. A test fails on any error the browser reports, including a `console.error`, so specs import `test` and `expect` from `apps/console/e2e/support.ts`.

### The documentation site

The unit tests and the end-to-end suite use the fixture pages in `apps/docs/e2e/fixtures/content`, never the real pages, so editing documentation never breaks a test. `pnpm --filter kaeso-docs e2e` runs Playwright, axe included, against a production build of the fixtures, in every language and at phone width. `pnpm --filter kaeso-docs check:content` and the build check the real pages. The diagram renderer's tests draw in Chromium, so `pnpm test` needs the Chromium that Playwright installs.

### The UI library and the showcase

Component tests run in happy-dom with Testing Library. The showcase has no unit tests. Its end-to-end suite runs Playwright against its production build, so run `pnpm build` first. It checks accessibility with axe in both themes, and the keyboard and focus rings. It also checks the tokens' type scale, motion and geometry, every component page at phone width, the German and Chinese pages, and the sitemap and page heads.

## Rust

Install the Cargo tools once:

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| Command | Checks |
| --- | --- |
| `cargo fmt --all --check` | Formatting (`rustfmt.toml`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints, among them no `unwrap`, `expect`, `panic!` or panicking indexing outside tests (`Cargo.toml`, `clippy.toml`) |
| `cargo test --workspace --all-features` | Every test |
| `cargo coverage-kpp` | The tests of `kaeso-kpp` and `kaeso-codecs` under `cargo llvm-cov`, with their coverage threshold |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | The examples in both crates' READMEs, their documentation on docs.rs, which `cargo llvm-cov` doesn't run |
| `cargo coverage-gateway` | The tests of every other crate except `kpp-conformance` and `xtask`, with their coverage threshold |
| `cargo test -p xtask -p kpp-conformance` | The repository tasks and the conformance server, which the coverage commands leave out |
| `cargo deny check` | Licenses, advisories, bans and sources of every dependency (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP's stricter policy: no runtime or HTTP crates in the library |
| `cargo sqlx prepare --workspace --check` | The offline query data in `.sqlx/` matches the queries |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` is current |

The coverage commands are aliases in `.cargo/config.toml`, which also holds their thresholds and the files they leave out. Tests that need the databases fail, rather than skip, when the databases aren't there.

`cargo sqlx prepare --check` needs `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` with the migrations applied. The stack's gateway applies them when it starts. Without it, run `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`.

`kaeso-kpp` and `kaeso-codecs` name the oldest Rust they support as `rust-version` in their `Cargo.toml`, and CI tests each crate with it. To do the same, install that version with `rustup toolchain install <version>` and run `cargo +<version> test --locked -p <crate>`.

### Test databases

Every test creates its own databases on the development PostgreSQL, ClickHouse and Valkey, so tests run in parallel without sharing state. `kaeso-testkit` names them and cleans them up:

| Name | Made by | Dropped |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in a test, or `kaeso-testkit fresh-services --temporary` | When the test ends, also on a panic, or by the sweep |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, for a gateway you [run from source](stack.md#run-the-gateway-from-source) | Only by `kaeso-testkit drop-services dev_<uuid>` |

The first test in a process sweeps away `t_` databases that are older than six hours and no longer in use. To keep a failed test's databases, run it with `KAESO_TEST_KEEP_DATABASES=1`, and it prints their names.

### Snapshots and references

Some tests compare against reviewed `insta` snapshots. When one changes, review the diff and update what depends on it:

| Snapshot | Holds | Also update |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Every right and built-in role | [Rights](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Every setting with its default and description | [Configuration](../self-hosting/configuration.md) and `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | The outcomes of the cross-dialect matrix | The dialect's mapping page ([KPP internals](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | The text and HTML of every email | |

Two more tests guard text. `crates/kaeso-server/tests/error_copy.rs` checks every message the Platform API can send against the [error copy rules](architecture/gateway.md#error-messages). `crates/kaeso-server/tests/docs_paths.rs` checks that every repository path and relative link in the Markdown files at the root and in `crates/`, `deploy/`, `e2e/`, `fuzz/` and `.github/` points at something that exists.

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

This suite checks that the SDKs accept everything KPP's encoders produce. It builds and starts `kpp-conformance`, a server in `crates/kpp-conformance`, on a random port. To use a server you started with `cargo run --release -p kpp-conformance`, set `KPP_CONFORMANCE_URL=http://127.0.0.1:8787`.

```mermaid
sequenceDiagram
  accTitle: A conformance request
  accDescr: The SDK sends a request whose model names a source dialect and a fixture. The server decodes it with the endpoint's codec, decodes the fixture with the source dialect's codec into KPP, encodes that with the endpoint's codec and returns the response, stream or error for the SDK to parse.
  participant SDK
  participant Server as kpp-conformance
  SDK->>Server: Request, model "dialect:fixture"
  Note over Server: Decode with the endpoint's codec
  Note over Server: Decode the fixture with the<br>source dialect's codec into KPP
  Note over Server: Encode with the endpoint's codec
  Server-->>SDK: Response, SSE stream or error
  Note over SDK: Parses it
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

Every response and stream [fixture](kpp/fixtures.md) of every generation dialect goes through every generation endpoint, streamed and not. The tests compare what the SDK parsed with what KPP decoded (`GET /_expected`), so a translation that loses text, tool arguments or readable reasoning fails. They also check that structured output stays valid JSON and that every error fixture raises the SDK's `APIStatusError` with the right status (`e2e/conformance/test_errors.py`). Model listings must page and filter as the SDKs expect (`e2e/conformance/test_models.py`). `GET /_scenarios` lists every scenario the server offers.

### SDK suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

This suite runs the SDKs against the gateway, on the development databases. It builds the gateway and `kaeso-testkit`, starts the mock provider and the gateway, and sets up users, projects, provider keys, models and API keys through the Platform API. The tests cover streaming, keepalives, tools, images, PDFs, reasoning, both directions across providers, token counting, embeddings, model listings, every error class, limits, SDK retries and disconnects.

The mock provider (`crates/kaeso-testkit/src/mock_provider.rs`) chooses its reply from markers in the request:

| Marker | Effect |
| --- | --- |
| `[mock:fixture=NAME]` in the prompt | Replies with that fixture of the provider's format |
| `[mock:gate=NAME]` in the prompt | Holds the request until the test opens the gate |
| A model id starting with `unserved-` | The format's model not found |
| `[mock:gate=NAME]` in the provider key | Holds `GET /v1/models` and `GET /v1/models/{id}` |
| `[mock:models=absent]` in the provider key | Answers both models endpoints with a bare 404 |
| `[mock:models=list-only]` in the provider key | Answers only `GET /v1/models/{id}` with a bare 404 |
| `[mock:models=endless]` in the provider key | Keeps the Anthropic listing on its first page |

`MockProvider::gate()` makes a gate and returns its marker. `wait_for(n)` returns once `n` requests have reached it, and `open()` lets them all through. That is how a test holds a request in flight without a sleep.

### Install test

```bash
e2e/.venv/bin/pytest -q e2e/install
```

This suite builds the gateway and mock provider images and runs `deploy/compose.yaml` from a clean state, with the overrides in `e2e/install`. It completes the first-start setup and calls the install with both SDKs. It also checks that PostgreSQL and ClickHouse give the gateway no rights it doesn't need, and that Valkey refuses connections without its password. A stand-in serves the console's address. `KAESO_TEST_CONSOLE_IMAGE` runs the real console image instead, and `KAESO_TEST_GATEWAY_IMAGE` and `KAESO_TEST_MOCK_IMAGE` use images that are already built.

## Fuzzing

`fuzz/` holds one [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) target per KPP decoder, `decode_<dialect>`, and one for the SSE parser, `sse`. The decoder targets check that every decoder accepts any bytes without panicking, and that encoding the result into every dialect doesn't panic either. The `sse` target checks that the parser yields the same items however the input is split. `fuzz/` is a Cargo workspace of its own, so `libfuzzer-sys` never reaches the library crates.

Every change checks that the targets build, and are formatted and linted:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

Running the targets needs a nightly toolchain and cargo-fuzz:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Run a target with a corpus folder and the dialect's fixtures as the seed corpus. The `sse` target takes every fixture:

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

A crash is written to `fuzz/artifacts/<target>/`. Reproduce it with `cargo +nightly fuzz run <target> <file>`, turn it into a regression test in the codec, then fix the codec.

## Benchmarks

`crates/kaeso-codecs/benches/codecs.rs` measures a full decode and encode cycle in each dialect, for a 100-message conversation and a 2,000-event stream. Before you change a codec's hot path, run the benchmarks on `dev` and again on your branch:

```bash
cargo bench -p kaeso-codecs --bench codecs
```

criterion prints the change against the previous run. CI only checks that the benchmarks build.

## Continuous integration

The CI workflow, `.github/workflows/ci.yml`, runs on every pull request, on every push to `dev` and `prod`, and inside the Release workflow. On a pull request, its first job decides which areas changed, and the jobs of unchanged areas are skipped. Everywhere else, every job runs.

The one required check is **CI**. It passes when **Changes** and **No comments** passed and every other job passed or was skipped. A pull request into `dev` can't be merged until it is green ([get it merged](contributing.md#get-it-merged)).

| Job | Runs |
| --- | --- |
| Changes | Decides which areas a pull request changes |
| No comments | No comments in source files, configuration or Markdown code blocks |
| Workflows | `actionlint` |
| Rust format, Rust lint | `cargo fmt` and `cargo clippy`, for the workspace and `fuzz/` |
| Test KPP | `cargo coverage-kpp` and the doc tests of `kaeso-kpp` and `kaeso-codecs` |
| Minimum Rust version | `cargo check` and `cargo test` of each KPP crate with its `rust-version` |
| Test gateway | `cargo coverage-gateway`, with the development stack |
| Repository tooling | `node scripts/version.mjs check`, `cargo test -p xtask -p kpp-conformance` and `cargo test -p kaeso-server --test docs_paths` |
| Query data | The PostgreSQL migrations, then `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `openapi.json` is current |
| Rust dependencies | `cargo deny check`, with the workspace's policy and KPP's |
| Fuzz targets, Benchmarks build | The fuzz targets and the benchmarks build |
| SDK conformance, SDK end-to-end | `pytest -q e2e/conformance` and `pytest -q e2e/sdk` |
| Compose config | `docker compose config` for both Compose files |
| Images (gateway, mock provider, updater, console) | Build the four images; the console's job also checks that its image serves the console with a content security policy |
| Install test | `pytest -q e2e/install` with those images |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` and `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript libraries | `pnpm build:packages`, once for every job that needs the libraries |
| TypeScript test | `pnpm test`, one job per workspace, the console's in three shards whose coverage is merged and checked in one more job |
| TypeScript build | `pnpm build`, `pnpm licenses:check` and `pnpm release:check` |
| End-to-end console, docs and showcase | Each package's `e2e`, the console's in three parts and the showcase's in shards |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |

When an end-to-end shard fails, the job **End-to-end console report** or **End-to-end showcase report** merges the shards' results into one HTML report, the artifact `playwright-console` or `playwright-showcase`.
