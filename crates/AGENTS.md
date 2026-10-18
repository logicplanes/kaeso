# AGENTS.md

Rules for the Rust workspace: the gateway, the updater and KPP. The root [`AGENTS.md`](../AGENTS.md) holds the setup, the definition of done and the rules for the whole repository.

## What lives here

| Crate             | Job                                                                                                                                                                                       |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `kaeso`           | The binary: CLI (`serve`, `migrate`, `master-key`, `user`, `setup-link`, `healthcheck`, `openapi`), configuration, telemetry, startup and shutdown. The only crate that may use `anyhow`. |
| `kaeso-server`    | axum routers for `/v1`, `/platform/v1`, health and metrics; extractors, error mapping, OpenAPI (utoipa), CSRF, request ids, body limits, background jobs; the services behind them, among them the limit counters as Valkey Lua scripts (`src/services/limits.rs`). Handlers stay thin. |
| `kaeso-core`      | Domain rules without I/O: ids, rights and roles, key format, model resolution, catalogue merge, capabilities, limits, costs, the stream reply assembler, retention; the model data.        |
| `kaeso-store`     | PostgreSQL (sqlx, offline query data in `.sqlx/`), ClickHouse, the Valkey client and its invalidation channel, and the migrations of both databases. |
| `kaeso-upstream`  | Provider transport: reqwest clients, auth headers, base URLs, timeouts, streaming bodies, cancellation.                                                                                   |
| `kaeso-crypto`    | Envelope encryption (AES-256-GCM), the master key ring, passwords, TOTP, redacting secret types.                                                                                          |
| `kaeso-mail`      | SMTP via lettre; every email in one layout, in English, German and Simplified Chinese.                                                                                                    |
| `kaeso-update`    | Updates without I/O: the release manifest, the update policy (channels, major versions, steps by hand) and the updater's protocol, shared by the gateway and the updater.                 |
| `kaeso-updater`   | The optional updater binary: verifies a release's signed provenance and switches a Compose install to it, through a Docker socket proxy. The only crate that talks to Docker.         |
| `kaeso-testkit`   | Test-only: development and test databases, the mock provider built from KPP's fixtures, a mock identity provider.                                                                        |
| `kaeso-kpp`       | KPP's types, `validate()` and `required_capabilities()`. No codec logic, no I/O. Published on crates.io.                                                                                  |
| `kaeso-codecs`    | One sans-IO codec per dialect, plus `dispatch`, `cross`, `continuity`, `keepalive`, `sse` and `testing`; the fixtures in `crates/kaeso-codecs/fixtures`. Published on crates.io.          |
| `kpp-conformance` | The server the SDK conformance suite in `e2e/conformance` runs against. Never published.                                                                                                  |
| `xtask`           | `cargo xtask models-dev` refreshes the models.dev snapshot; `cargo xtask record` records live provider responses with keys from the environment. Never runs in CI.                      |

The request path, the crates and every design decision are in [gateway architecture](https://docs.kaeso.app/development/architecture/gateway) and [KPP internals](https://docs.kaeso.app/development/kpp).

## Gateway rules

1. **Wire formats live only in KPP.** Never parse or build OpenAI or Anthropic request, response, stream, error or model-listing JSON by hand; `kaeso_codecs::dispatch` does. The gateway's own JSON is the Platform API and its error envelope.
2. **No passthrough.** Decode every model request to KPP and encode it again, even when app and provider speak the same format.
3. **No retries and no fallbacks.** A failed provider call fails, in the app's format.
4. **Never drop data silently.** What KPP refuses (`Unsupported`) goes back to the app as an error in its own format before anything is sent. Never catch and ignore a `KppError`.
5. **Secrets never leave memory unprotected.** Provider keys, master keys, API key secrets, session ids, sign-in tokens, TOTP secrets and passwords are never logged, traced, printed, stored in ClickHouse, returned by an API or put into an error message. Keep them in the redacting wrapper types.
6. **Money is never a float.** `rust_decimal::Decimal` in Rust, `NUMERIC` in PostgreSQL, `Decimal` in ClickHouse; live spend counters in Valkey are integer nano-dollars, rounded up.
7. **One right per endpoint, every change audited.** Every Platform API endpoint checks exactly one right from `crates/kaeso-core/src/rights/catalogue.rs`, except `me/*` and `auth/*`, and the instance admin flag for `instance/*`. Register routes with their right so a missing check fails a test.
8. **Organizations are isolated.** Every PostgreSQL and ClickHouse query that reads an organization's data filters by the organization from the authenticated context, never from user input alone.
9. **Migrations only move forward** and run at startup under a lock: `crates/kaeso-store/migrations/postgres` (sqlx) and `crates/kaeso-store/migrations/clickhouse` (ordered SQL files). A migration that needs a manual step makes the release a major one.
10. **The OpenAPI document is generated.** Write operation summaries and descriptions in `#[utoipa::path(summary = …, description = …)]`, type descriptions in `#[schema(description = …)]`, and field, variant and query parameter descriptions in `#[doc = "…"]` attributes. Regenerate with `cargo run -q -p kaeso -- openapi > openapi.json` and change the console in the same pull request.
11. **Every setting is one row** of `crates/kaeso-server/src/config/settings.rs`: key, environment variable, type, default, secret flag and a one-sentence description. Provider settings are one pattern row, `providers.<id>.<field>`, repeated for every provider in `crates/kaeso-core/src/provider.rs` and every field in `PROVIDER_FIELDS`; no field name may end in `_` and another field's name.
12. **Provider ids follow one rule**, `ProviderId` and `ID_PATTERN` in `crates/kaeso-core/src/provider.rs` (lowercase letters and digits, single hyphens between them). Every id that enters the gateway goes through it, and the PostgreSQL CHECK constraints use the same pattern.
13. **The logged reply form is pinned.** `request_content.reply` holds a reply's serde JSON, read by every later console; a change to the snapshots in `crates/kaeso-server/src/services/snapshots/` is a format change that adds a `format` number to the reply and teaches the console the new form.

## Error copy

The console shows the Platform API's `message` as it is, in a toast, so every message is one short sentence:

- At most 90 characters as written in the code, placeholders included, and 120 once filled in. It ends with a period.
- One sentence: join two thoughts with a semicolon (`Your session has ended; sign in again.`) or drop the second. No `e.g.`, no line breaks.
- Say what is wrong and, when it fits, what to do. Name fields as code spans and put the field in `param`.
- No internal names (ClickHouse, Valkey, configuration keys, module paths) and no trailing technical detail: log those with `tracing`.
- Internal causes (`ApiError::internal`, `#[error]` of internal types, log lines) start lowercase; copy starts with a capital letter or a code span.

`crates/kaeso-server/tests/error_copy.rs` and `crates/kaeso-server/tests/common/copy.rs` enforce it. The console translates every message it can show (`apps/console/src/api/gateway-errors.ts`); `error_copy.rs` fails on a message without a rule there, so a new message comes with its German and Chinese in the same pull request. The model API (`/v1`) keeps its providers' error formats.

## KPP rules

1. **Never silently drop information.** Every field and block of an incoming payload ends up in exactly one of: a typed KPP field, a dialect-bound `Extras` entry, an `Unknown` item, or a `KppError` of kind `Unsupported`. A match arm that discards data is a bug. Drop a hint across dialects only where [field classes](https://docs.kaeso.app/development/kpp#field-classes) and the [extras allowlist](https://docs.kaeso.app/development/kpp/cross-format#extras) say so.
2. **Never guess a wire format.** Every mapping names its source on the dialect's mapping page: the provider documentation or the OpenAPI schema at the pinned spec commit, or the official SDK types. When the sources are ambiguous, choose `Unsupported` and open an issue.
3. **Fixtures before code.** Every codec change starts with a real payload in `crates/kaeso-codecs/fixtures/<dialect>/`, a provenance row in that folder's `README.md`, and a failing test. Fixtures never contain real keys or personal data.
4. **Round trip is the core test.** For every fixture, decode → KPP → encode reproduces the original as an equal JSON value (streams: an equal event sequence). If exact equality is impossible, the test says why next to the fixture.
5. **Additive public API.** `kaeso-kpp` and `kaeso-codecs` are published: public enums and growable structs are `#[non_exhaustive]`, enum variants wrap `#[non_exhaustive]` structs with constructors, and the `Codec` trait is sealed. Prefer new optional fields and variants over changing existing ones.
6. **Small, I/O-free dependencies.** Library crates use `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` and `mime`; anything else needs the maintainer's agreement and a one-line reason in the pull request's description. `crates/kaeso-codecs/deny.toml` bans `tokio`, `reqwest`, `hyper` and `axum` from them.
7. **Error messages are for the end user of Kaeso:** which field the target does not support and what to do.
8. **Ask the maintainer first** before changing the public KPP type design, adding a dialect or endpoint, or deciding a cross-format question differently from [cross-format](https://docs.kaeso.app/development/kpp/cross-format).
9. **Update the dialect's mapping page** in `apps/docs/content`, in all three languages, in the same pull request.

## Both

- **No panics in library code.** No `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, panicking indexing or `unsafe` outside tests and the binary's startup (workspace lints, `#![forbid(unsafe_code)]`). Fix the code instead of adding `#[allow]`.
- **Use the property and snapshot tools.** `kaeso_codecs::testing` for JSON and SSE comparisons, `insta` for snapshots, `proptest` for properties; a case a property test finds becomes an explicit regression test.
- **Review snapshot diffs** and update what depends on them: rights and roles (`crates/kaeso-core/src/rights/snapshots/`) and settings (`crates/kaeso-server/tests/snapshots/`) change their documentation pages; cross-dialect outcomes (`crates/kaeso-codecs/tests/snapshots/`, `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix`) change the mapping pages; emails live in `crates/kaeso-mail/src/snapshots/`.
- **Compare performance** when touching hot paths: run `cargo bench -p kaeso-codecs --bench codecs` on `dev` and on your branch ([benchmarks](https://docs.kaeso.app/development/testing#benchmarks)).
- **Refresh the model data** with `cargo xtask models-dev`; never edit `crates/kaeso-core/data/models-dev.json` by hand.
- Tests that need databases always fail, never skip, when the development stack is down.
