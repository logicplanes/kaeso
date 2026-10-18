---
title: Testing
description: Every test suite in the repository, how to run it, and what must pass before a change is done.
order: 4
---

Every change comes with tests, and they assert on what a person or an app sees. Coverage thresholds are never lowered; when coverage rises, the threshold rises with it. Commands run from the repository root, with the [development stack](local-setup.md#3-start-the-development-stack) running for suites that need databases.

## Before a change is done

These pass for every change:

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

Then, for the parts you changed:

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
| `pnpm lint` | ESLint or oxlint in every package, with Kaeso's own rules: layout-only class names, no colours outside the tokens, no text written straight into the console; for the docs, every page with `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, after building the libraries in `packages/` |
| `pnpm test` | Vitest with coverage thresholds in both libraries, the console and the docs, then the tests of the scripts in `scripts/` with `node --test`, among them the [version check](releasing.md#the-version-in-the-repository) run on the repository itself |
| `pnpm i18n` | Every text of the console has German and Chinese with the same placeholders, and nothing is left over ([translations](translations.md#checking)) |
| `pnpm knip` | Unused files, exports and dependencies |
| `pnpm build` | Every production build and its bundle budgets ([performance](performance.md)); both sites' builds then run the [site check](releasing.md#the-sites) |
| `pnpm licenses:check` | After a build: the license of every package whose code ships ([dependencies](dependencies.md#license-policy)) |
| `pnpm release:check` | After a build: packs both libraries as npm would and checks that every file they must ship is there, at one version |
| `pnpm e2e` | Playwright in the console, the docs and the showcase, one after another; the showcase's suite tests its build, so `pnpm build` comes first |

`pnpm --filter <package> <script>` runs one package's script; the packages are `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`. `pnpm typecheck`, `pnpm test` and `pnpm knip` build the libraries first; before a filtered run, run `pnpm build:packages` if the libraries changed.

Each package with tests sets thresholds for lines, statements, branches and functions in its `vitest.config.ts`: 100% for the tokens, and for the others their measured coverage rounded down to a tenth of a percent.

Playwright needs Chromium, installed once:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### The console

Unit tests run in happy-dom against the Platform API stubbed per test (`apps/console/src/test/network.ts`, fixtures in `apps/console/src/test/fixtures/`). `renderApp(path)` mounts the whole console at an address with its own router, which ends with the test; `renderUi` mounts a single component. Tests assert on what a person sees and on the requests sent, and pass in any order, which `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` checks. Test files under `src/routes` start with `-` so the route generator skips them.

The end-to-end suite never touches the development stack. It starts its own from `deploy/compose.dev.yaml`: the Compose project `kaeso-console-e2e`, on ports in the 28000 range with the gateway on `127.0.0.1:28080` and the console on http://localhost:3100. For `20-sso-only` it adds a second gateway that allows only single sign-on, on `127.0.0.1:28081` with its console on http://localhost:3101. The stack and its data are removed when the run ends. The specs build on each other, starting with `01-public`, which sets the install up. `KAESO_GATEWAY_IMAGE` uses a gateway image that is already built.

To keep the end-to-end stack running between runs, start it yourself and tell the suite:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

A test fails on any error the browser reports while it runs: an uncaught error or rejection, or a `console.error`, which is where React and Base UI warn. Specs import `test` and `expect` from `apps/console/e2e/support.ts`, which holds this check.

### The documentation site

The unit tests and the end-to-end suite use the fixture pages in `apps/docs/e2e/fixtures/content`, never the real pages, so editing documentation never breaks a test. `pnpm --filter kaeso-docs e2e` runs Playwright, axe included, against a production build of the fixtures; it also runs the site check on that build and opens the search from the phone bar at 375 by 812 pixels in every language. The real pages are checked by `pnpm --filter kaeso-docs check:content` and by the build. The site check's own tests are in `scripts/check-site.test.mjs`. The tests of the diagram renderer draw in Chromium, as the build does, so `pnpm test` needs the Chromium that Playwright installs.

### The UI library and the showcase

Component tests run in happy-dom with Testing Library. The showcase has no unit tests; its end-to-end suite runs Playwright against its production build, so `pnpm build` comes first. It checks:

- axe in light and dark, the keyboard and focus rings;
- the type scale, motion and geometry against the tokens, and menus that open in place;
- every component page at phone width without sideways scrolling;
- the German and Chinese pages, in their language before any script runs and hydrated without a mismatch;
- the search opened from the phone header at 375 by 812 pixels in every language;
- the sitemap, the head of every kind of page, and links into the repository that name the release's tag.

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
| `cargo coverage-kpp` | The tests of `kaeso-kpp` and `kaeso-codecs` under `cargo llvm-cov`, failing below 95% of lines |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | The examples in both crates' READMEs, which are their documentation on docs.rs and which `cargo llvm-cov` doesn't run |
| `cargo coverage-gateway` | The tests of every other crate except `kpp-conformance` and `xtask` under `cargo llvm-cov`, failing below 91% of lines |
| `cargo test -p xtask -p kpp-conformance` | The repository tasks and the conformance server, which the coverage commands leave out |
| `cargo deny check` | Licenses, advisories, bans and sources of every dependency (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | KPP's stricter policy (`crates/kaeso-codecs/deny.toml`): no runtime or HTTP crates in the library |
| `cargo sqlx prepare --workspace --check` | The offline query data in `.sqlx/` matches the queries |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` is current |

The coverage commands are aliases in `.cargo/config.toml`, which also lists the files they leave out, such as test helpers and the binary's `main`. Tests that need the databases fail, rather than skip, when the databases aren't there.

`cargo sqlx prepare --check` needs `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` with the migrations applied. The stack's gateway applies them when it starts; without it, run `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres`.

`kaeso-kpp` and `kaeso-codecs` declare the oldest Rust they support as `rust-version` in their `Cargo.toml`, and CI checks and tests each crate with it. To do the same, install that version with `rustup toolchain install <version>` and run `cargo +<version> test --locked -p <crate>`.

### Test databases

Every test creates its own databases on the development PostgreSQL, ClickHouse and Valkey, so tests run in parallel without sharing state. `kaeso-testkit` names them and cleans them up:

| Name | Made by | Dropped |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in a test, or `kaeso-testkit fresh-services --temporary` | When the test ends, also on a panic; otherwise by the sweep |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, for a gateway you [run from source](local-setup.md#run-the-gateway-from-source) | Only by `kaeso-testkit drop-services dev_<uuid>` |

The first test in a process sweeps away `t_` databases that are older than six hours and no longer in use. `KAESO_TEST_KEEP_DATABASES=1 cargo test …` keeps a failed test's databases and prints their names. `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` and `KAESO_TEST_VALKEY_URL` point the tests at other servers.

### Snapshots and references

Some tests compare against reviewed `insta` snapshots. When one changes, review the diff and update what depends on it:

| Snapshot | Holds | Also update |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Every right and built-in role | [Rights](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Every setting with its default and description | [Configuration](../self-hosting/configuration.md) and `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | The outcomes of the cross-dialect matrix | The dialect's mapping page ([KPP internals](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | The text and HTML of every email | |

Two more tests guard text: `crates/kaeso-server/tests/error_copy.rs` checks every message the Platform API can send against the [error copy rules](architecture/gateway.md#error-messages), and `crates/kaeso-server/tests/docs_paths.rs` checks that every repository path and relative link in the Markdown files at the root and in `crates/`, `deploy/`, `e2e/`, `fuzz/` and `.github/` points at something that exists.

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

This suite checks that the SDKs accept everything KPP's encoders produce. It builds and starts `kpp-conformance` (`crates/kpp-conformance`, a release build on a random port); `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` uses a server you started with `cargo run --release -p kpp-conformance` instead.

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

Every response and stream [fixture](kpp/fixtures.md) of every generation dialect is served through every generation endpoint, in the same dialect and across dialects, streamed and not. The tests compare what the SDK parsed with what KPP decoded (`GET /_expected`), so a translation that loses text, tool arguments or readable reasoning fails. In detail:

- **Reasoning** the SDK exposes (the summaries and text of Responses reasoning items, the text of Anthropic `thinking` blocks) equals the decoded reasoning. In Responses streams, the `response.reasoning_summary_text.delta` events add up to each `response.reasoning_summary_text.done` and to the final response's summaries.
- **Structured output** is still valid JSON when the SDK reads it.
- **Errors:** every error fixture of every dialect, served through the Anthropic, Chat and Responses endpoints, raises the SDK's `APIStatusError` with the status of its KPP error kind in that format, keeps `retry-after` on rate limits, and uses the format's error body (`e2e/conformance/test_errors.py`).
- **Model listings:** the models endpoints serve a fixed catalogue of five models, one of them retired, paged with `ModelList::page`. `e2e/conformance/test_models.py` follows every page with the Anthropic SDK's auto-paging forward (`limit=2`) and backward (`before_id`), filters by `lifecycle`, expects `BadRequestError` for an unknown cursor or `limit=0`, lists with the OpenAI SDK, retrieves models, and expects `NotFoundError` for unknown ids in both SDKs.

`GET /_scenarios` lists every scenario the server offers.

### SDK suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

This suite needs the development databases. It builds the gateway and `kaeso-testkit`, creates fresh databases, starts the mock provider and the gateway, and sets up the first user, an organization and project, provider keys, catalogue models, an alias and API keys through the Platform API. The tests cover streaming, keep-alives through a quiet provider, tools, images, PDFs, reasoning, both directions across providers, token counting, embeddings, model listings, every error class, limits, SDK retries and disconnects.

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

`MockProvider::gate()` makes a gate and returns its marker; `wait_for(n)` returns once `n` requests have reached it, and `open()` lets them all through. That is how a test holds a request in flight without a sleep.

### Install test

```bash
e2e/.venv/bin/pytest -q e2e/install
```

This suite builds the gateway and mock provider images from `deploy/gateway.Dockerfile` and runs `deploy/compose.yaml` from a clean state with the overrides in `e2e/install`. It completes the first-start setup, calls the install with both SDKs, and checks that PostgreSQL and ClickHouse give the gateway no rights it doesn't need and that Valkey refuses connections without its password. A stand-in serves the console's address; `KAESO_TEST_CONSOLE_IMAGE` runs the real console image instead, and `KAESO_TEST_GATEWAY_IMAGE` and `KAESO_TEST_MOCK_IMAGE` use images that are already built.

## Fuzzing

`fuzz/` holds one [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) target per KPP decoder, plus the SSE parser. It is a Cargo workspace of its own, outside the repository's workspace as cargo-fuzz expects, so `libfuzzer-sys` never reaches the library crates.

| Target | Checks |
| --- | --- |
| `decode_anthropic_messages`, `decode_openai_chat`, `decode_openai_responses`, `decode_openai_embeddings` | Every decoder of the dialect accepts any bytes without panicking: requests of all three kinds, replies, errors, model listings, and streams fed in chunks whose size the input decides. Encoding what it decoded into every dialect doesn't panic either |
| `sse` | The SSE parser never panics and yields the same items however the input is split, and writing the parsed items back gives the same items when parsed again |

The shared logic is in `fuzz/src/lib.rs`; each file in `fuzz/fuzz_targets/` names its dialect. Every change checks that the targets build and are formatted and linted:

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

```bash
mkdir -p fuzz/corpus/sse
cargo +nightly fuzz run sse fuzz/corpus/sse crates/kaeso-codecs/fixtures -- -max_total_time=600
```

A crash is written to `fuzz/artifacts/<target>/`. Reproduce it with `cargo +nightly fuzz run <target> <file>`, turn it into a regression test in the codec, then fix the codec. [Performance](performance.md#fuzzing) records the last full run.

## Continuous integration

The CI workflow (`.github/workflows/ci.yml`) runs on every pull request, on every push to `dev` and `prod`, and inside the Release workflow. The Release workflow's own pushes, its release commit and the release brought into `dev`, don't start it again, since the release has already passed every job.

On a pull request, the first job decides which areas changed, and the jobs of unchanged areas are skipped; in every other case every job runs. The one required check is **CI**: it passes when **Changes** and **No comments** passed and every other job passed or was skipped, and fails when one failed, timed out or was cancelled. A pull request into `dev` can't be merged until it is green ([how changes reach dev](contributing.md#how-changes-reach-dev)).

| Job | Runs |
| --- | --- |
| Changes | Decides which areas a pull request changes: Rust, TypeScript, deployment, the repository's own files, the workflows |
| No comments | No comments in source files, configuration or Markdown code blocks |
| Workflows | `actionlint` |
| Rust format | `cargo fmt --all --check`, for the workspace and `fuzz/` |
| Rust lint | `cargo clippy` with every warning denied, for the workspace and `fuzz/` |
| Test KPP | `cargo coverage-kpp` and `cargo test --doc -p kaeso-kpp -p kaeso-codecs` |
| Minimum Rust version | `cargo check` and `cargo test` of `kaeso-kpp` and of `kaeso-codecs`, each with the Rust version its `rust-version` names |
| Test gateway | `cargo coverage-gateway`, with the development stack |
| Repository tooling | `node scripts/version.mjs check`, `cargo test -p xtask -p kpp-conformance` and `cargo test -p kaeso-server --test docs_paths` |
| Query data | The PostgreSQL migrations, then `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | `cargo deny check`, with the workspace's policy and KPP's |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | `pytest -q e2e/sdk`, with the development stack |
| Compose config | `docker compose config` for `deploy/compose.yaml` and `deploy/compose.dev.yaml` |
| Images (gateway), Images (mock provider), Images (console) | Build the three images side by side, labelled with the versions in `Cargo.toml` and `apps/console/package.json`; the console's job also checks that its image answers `/healthz`, serves the console with a content security policy, and answers 404 for a missing file under `/assets/` |
| Install test | `pytest -q e2e/install` with those images, the real console included |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` and `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript libraries | `pnpm build:packages`, once; the TypeScript tests and the end-to-end jobs use what it builds |
| TypeScript test (tokens, ui, docs, showcase, scripts) | `pnpm test` split by workspace, one job each: the tests of `packages/tokens`, `packages/ui`, `apps/docs` and `apps/showcase`, and `node --test 'scripts/*.test.mjs'`, each with its package's coverage thresholds and its coverage in the job's summary |
| TypeScript test (console 1/3, 2/3, 3/3) | The console's unit tests in three shards (`vitest run --shard`), each recording its coverage |
| TypeScript test (console coverage) | Merges the three shards (`vitest run --merge-reports --coverage`): fails if a test failed or the merged coverage is below the console's thresholds |
| TypeScript build | `pnpm build`, `pnpm licenses:check` and `pnpm release:check`; End-to-end showcase tests the showcase it builds |
| End-to-end console (1/3, 2/3, 3/3) | `pnpm --filter kaeso-console e2e` in three parts against the gateway image, each part on a stack of its own |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase (chromium 1/8 to 8/8, webkit 1/1) | `pnpm --filter kaeso-showcase e2e` against the showcase built by TypeScript build: the `chromium` project in eight shards (`playwright test --shard`), the `webkit` project in a job of its own |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | The required check |

### How the jobs are scheduled

Each job starts as soon as the jobs it uses have finished, and none builds what another job has already built:

- **TypeScript libraries** builds `packages/` once and hands the build on as an artifact to the TypeScript tests and the end-to-end jobs, which no longer build it themselves. It runs whenever one of them runs, so for Rust and deployment changes too, which End-to-end console tests. **TypeScript build** hands on the showcase it builds the same way.
- The three image jobs run side by side. End-to-end console starts once Images (gateway) and TypeScript libraries have finished; the install test waits for all three images.
- The console's unit tests take longest of all TypeScript tests, so they run in three shards. Each shard runs with its thresholds off, since a third of the tests covers only part of the code; **TypeScript test (console coverage)** merges what the shards recorded and applies the thresholds of `apps/console/vitest.config.ts` to the whole, as `pnpm test` does.
- The showcase's tests are independent of each other. The `chromium` project runs in eight shards with Playwright's `--shard`; the `webkit` project, which has only the navigation sheet's tests, runs in a job of its own, so only that job installs WebKit and the system packages it needs.
- The console's specs build on each other, so `--shard` can't split them. Each part starts its own stack and runs `01-public` first, then the specs whose data comes only from the specs before them in the same part: part 1 the project's pages (`02-models` to `08-palette`, `11-instance`, `13-keyboard`, `16-period`), part 2 the organization (`02-models`, `04-members`, `10-org`, `12-lifecycle`, `19-roles`), part 3 every other spec. A new spec runs in part 3 unless `PART_1` or `PART_2` in the workflow names it, and a name there without a spec fails the job.
- When a shard or part fails, **End-to-end showcase report** or **End-to-end console report** merges the shards' blob reports into one HTML report, the artifact `playwright-showcase` or `playwright-console`. These two jobs aren't part of the required check.

Caches keep repeated work short: the pnpm store (keyed on `pnpm-lock.yaml`), Playwright's browsers (keyed on the Playwright version; on a hit only the system packages are installed), Rust's build output (written on `dev`, read by every pull request) and the layers of every image (the `gha` cache of Docker Buildx, also read from `dev`).
