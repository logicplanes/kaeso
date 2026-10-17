---
title: Gateway
description: The request path, configuration, model data and the rules the gateway's Rust code follows.
order: 1
---

The gateway is the one server of an install: apps call it with the official OpenAI and Anthropic SDKs, and the console and scripts manage it through the Platform API. It is the Rust workspace in `crates/`, built into one binary, `kaeso`. What happens to a request from the outside is under [request flow](../../concepts/request-flow.md); this page is the inside.

## The request path

```text
app ─▶ /v1 ─▶ 1 key ─▶ 2 decode (KPP) ─▶ 3 model ─▶ 4 limits ─▶ 5 provider key ─▶ 6 encode (KPP) + send ─▶ provider
app ◀──────── 8 encode reply (KPP) ◀── 7 decode reply (KPP) ◀───────────────────────────────────────────── provider
                                               └─▶ 9 charge usage (Valkey, PostgreSQL) · 10 log (ClickHouse)

console, scripts ─▶ /platform/v1 ─▶ session cookie | API key with platform access | platform key
                                  ─▶ exactly one right per endpoint ─▶ service ─▶ PostgreSQL + audit event
```

The model API lives in `crates/kaeso-server/src/model_api`, the Platform API in `crates/kaeso-server/src/platform`, and the services both use in `crates/kaeso-server/src/services`. The crates are listed under [architecture](index.md#the-rust-workspace).

## Rules

Every change to the gateway follows these rules; most of them are enforced by lints or tests.

1. **Wire formats live only in KPP.** The gateway never parses or builds OpenAI or Anthropic request, response, stream, error or model-listing JSON by hand; `kaeso_codecs::dispatch` does. The gateway's own JSON is the Platform API and its error envelope.
2. **No passthrough.** Every model request is decoded to KPP and encoded again, even when the app and the provider speak the same format.
3. **No retries and no fallbacks.** A failed provider call fails, in the app's format.
4. **Never drop data silently.** What KPP refuses (`Unsupported`) goes back to the app as an error in its own format, before anything is sent to a provider. A `KppError` is never caught and ignored.
5. **No panics in library crates.** No `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, panicking indexing or `unsafe` outside tests and the binary's startup; the workspace lints in `Cargo.toml` and `#![forbid(unsafe_code)]` enforce it.
6. **Secrets never leave memory unprotected.** Provider keys, master keys, API key secrets, session ids, sign-in tokens, TOTP secrets and passwords are never logged, traced, printed, stored in ClickHouse, returned by an API or put into an error message. They live in redacting wrapper types.
7. **Money is never a float.** `rust_decimal::Decimal` in Rust, `NUMERIC` in PostgreSQL, `Decimal` in ClickHouse; live spend counters in Valkey are integer nano-dollars, rounded up.
8. **Every Platform API change is audited, and every endpoint checks exactly one right** from the catalogue in `crates/kaeso-core/src/rights/catalogue.rs` ([rights](../../concepts/rights.md)), except `me/*` and `auth/*`, and the instance admin flag for `instance/*`. Routes are registered with their right, so a missing check fails a test.
9. **Organizations are isolated.** Every PostgreSQL and ClickHouse query that reads an organization's data filters by the organization from the authenticated context, never from user input alone.
10. **Migrations only move forward** and run at startup under a lock. They live in `crates/kaeso-store/migrations/postgres` (sqlx) and `crates/kaeso-store/migrations/clickhouse` (ordered SQL files). 0.1.0's schema is one initial set, nine files for PostgreSQL and two for ClickHouse; a schema change is a new file after them, never an edit to a released one. A migration that needs a manual step makes the release a major one.
11. **The OpenAPI document is generated.** Operation summaries and descriptions are written in `#[utoipa::path(summary = …, description = …)]`, type descriptions in `#[schema(description = …)]`, and descriptions of fields, variants and query parameters in `#[doc = "…"]` attributes, the only form utoipa reads there. `cargo run -q -p kaeso -- openapi > openapi.json` writes the document again.
12. **Tests are deterministic.** Clocks and random sources are injected, and nothing sleeps to synchronise; tests poll with deadlines.

## Configuration

Every setting is one row of the table in `crates/kaeso-server/src/config/settings.rs`: its TOML key, its environment variable, its type, its default, whether it is a secret, and a one-sentence description. Provider settings are one row as well, `providers.<id>.<field>`, which the table repeats for every provider in `crates/kaeso-core/src/provider.rs` and every field in `PROVIDER_FIELDS`; [provider settings](../../self-hosting/configuration.md#provider-settings) has the naming rule. Loading merges the defaults, the TOML file named by `KAESO_CONFIG` and the `KAESO_*` variables, including `_FILE` variables for secrets, in `config/load.rs` over the `toml` crate, with errors that always name the setting.

Tests check that every environment name follows its key, that required settings and secrets have no default, that every description is a sentence, that every provider variable maps back to its provider and field and no field's name ends in `_` and another field's name, and that `deploy/kaeso.example.toml` sets every setting but the secrets to its default and loads, and they render the table into a reviewed snapshot (`crates/kaeso-server/tests/snapshots/`). A change to a setting changes that snapshot, and [configuration](../../self-hosting/configuration.md) is updated from it. A configuration crate such as figment would add a second source of truth for keys and defaults, so there is none.

## Provider ids

A provider id is lowercase letters and digits, with single hyphens between them: `^[a-z0-9]+(-[a-z0-9]+)*$`. The rule lives in one place, `ProviderId` and `ID_PATTERN` in `crates/kaeso-core/src/provider.rs`, and every id that enters the gateway goes through it: configuration keys and variables, provider keys and catalogue models in the Platform API (whose OpenAPI document gives the pattern as the `ProviderId` schema), the supplement and the models.dev import. A compile-time check holds every id in `PROVIDERS` to it, and the PostgreSQL CHECK constraints on the `provider` columns of `provider_credentials`, `catalogue_models` and `usage_daily` use the same pattern, which a store test compares. models.dev entries are read under the provider's own id; every provider in the shipped snapshot follows the rule, which a test checks.

## Model data

`crates/kaeso-core/data` holds the model data shipped with every release:

| File | Holds |
| --- | --- |
| `models-dev.json` | A snapshot of [models.dev](https://models.dev), filtered to the providers Kaeso calls, so a fresh install works without network access. The folder's `README.md` records when it was fetched and its SHA-256. |
| `supplement.toml` | Fields models.dev doesn't have, with every entry's source in the folder's `README.md`: `adaptive_thinking` for Anthropic models, `forced_tool_choice: false` for models that refuse a forced tool choice, `efforts` where models.dev lists none, and `egress_dialect`, the format Kaeso sends to an OpenAI model. |

A catalogue entry's value is its override, else models.dev, else the supplement, else empty. When the gateway syncs, every 24 hours by default, it applies the same filter and stores the result in PostgreSQL; at startup the newer of the stored and the built-in snapshot wins. Only the providers in `crates/kaeso-core/src/provider.rs` are kept.

Refresh the snapshot before a release:

```bash
cargo xtask models-dev
```

It downloads `https://models.dev/api.json` (`--url` or `MODELS_DEV_URL` changes the address), filters it, checks that the gateway can read the result, writes the snapshot, the table in `crates/kaeso-core/data/README.md` and the fetch time in `crates/kaeso-core/src/models_dev.rs`, and prints the new hash. Tests check that the three agree.

## Logged replies

With content logging on, the gateway stores each reply in KPP form, the serde JSON of `kaeso_kpp::Response`, in the `reply` column of ClickHouse's `request_content`. The Platform API returns it as stored, and the console's log inspector draws it. An organization may keep content for up to 36,500 days, so a reply logged today outlives many releases, and every later console must still read it.

- **0.1.0's form has no version field.** Snapshots in `crates/kaeso-server/src/services/snapshots/` pin it for representative replies: text with citations, tool calls, reasoning in every form, a refusal, unknown blocks, and stop reasons with and without a value. A serde change to a KPP type fails `logged_replies_keep_the_v0_1_form`.
- **A change to that form is a format change.** From the first one on, the gateway writes a `format` number into the reply object, `2` for the second form, and the console learns the new form while keeping its renderer of every older one. Accepting the changed snapshots alone is never enough.
- **The console reads only what it knows.** A reply without `format` is drawn with the current renderer; any `format` value is shown as the stored JSON with a note, so a console older than the gateway never breaks (`isKnownReplyForm` in `apps/console/src/features/project/logs/render.ts`).

## Error messages

The console shows the Platform API's `message` as it is, in a toast, so every message is one short sentence:

- At most 90 characters as written in the code, placeholders included, and at most 120 once filled in. It ends with a period.
- One sentence: join two thoughts with a semicolon (`Your session has ended; sign in again.`) or drop the second. No abbreviations such as "e.g.", no line breaks.
- Plain and specific: say what is wrong and, when it fits, what to do. Name fields as code spans (`` `amount` must be positive. ``) and put the field in `param`.
- No internal names (ClickHouse, Valkey, configuration keys, module paths) and no trailing technical detail such as provider, SMTP or serde errors: log those with `tracing` and keep the message short.
- Internal causes (`ApiError::internal`, the `#[error]` of internal types, log lines) start with a lowercase letter; messages for people start with a capital letter or a code span.

`crates/kaeso-server/tests/error_copy.rs` scans the code behind the Platform API and fails on any message that breaks these rules, and the integration tests check every Platform API answer they receive (`crates/kaeso-server/tests/common/copy.rs`). The model API keeps its providers' error formats, so its messages are not bound by these rules.
