---
title: Dependencies and licenses
description: Every direct dependency, why it is there, its license, and the checks that keep the policy.
order: 7
---

Kaeso is MIT licensed, and everything it ships depends only on permissively licensed code. A new dependency gets a row on this page in the same pull request, with why it is needed and its license.

## License policy

| What ships | Allowed licenses | Checked by |
| --- | --- | --- |
| The gateway | MIT, MIT-0, Apache-2.0 (also with the LLVM exception), BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Unicode-DFS-2016, Zlib, and the [exceptions](#exceptions) below | `cargo deny check` with the root `deny.toml` |
| `kaeso-kpp`, `kaeso-codecs` | The same list without exceptions, and no runtime or HTTP crates at all | `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` with `crates/kaeso-codecs/deny.toml` |
| `@logicplanes/kaeso-ui`, `@logicplanes/kaeso-tokens` | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD, for their production dependencies | `pnpm licenses:check` (`scripts/licenses.mjs`) |
| The console and the docs | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD, for every package whose code is in their build | `pnpm licenses:check`, after `pnpm build` |

`cargo deny check` also refuses crates from unknown registries or Git repositories, wildcard versions, yanked crates and crates with security advisories. GPL, LGPL and AGPL code is never allowed in anything Kaeso ships. npm packages are pinned to exact versions in the `package.json` files, and Cargo versions are locked in `Cargo.lock`.

## Rust

### The gateway

| Crate | Used for | License |
| --- | --- | --- |
| `kaeso-kpp`, `kaeso-codecs` | KPP types and every wire format, from the same workspace | MIT |
| `tokio` | The async runtime | MIT |
| `axum` | The HTTP server | MIT |
| `tower` | Middleware: timeouts, body limits | MIT |
| `http` | HTTP types | MIT OR Apache-2.0 |
| `reqwest` | Provider transport with rustls, HTTP/2 and streaming bodies | MIT OR Apache-2.0 |
| `futures`, `tokio-util`, `bytes` | Streams, cancellation, byte buffers | MIT (`bytes`, `tokio-util`), MIT OR Apache-2.0 (`futures`) |
| `sqlx` | PostgreSQL with compile-time checked queries and migrations | MIT OR Apache-2.0 |
| `clickhouse` | The official ClickHouse client, for request logs and for dropping test databases in `kaeso-testkit` | MIT OR Apache-2.0 |
| `redis` | The Valkey client, with a connection manager and Lua scripts | BSD-3-Clause |
| `serde`, `serde_json` | Serialization | MIT OR Apache-2.0 |
| `humantime-serde` | Durations in the configuration | MIT OR Apache-2.0 |
| `toml` | The configuration file and the models.dev supplement | MIT OR Apache-2.0 |
| `clap` | The command lines of `kaeso` and `cargo xtask` | MIT OR Apache-2.0 |
| `tracing`, `tracing-subscriber` | Structured JSON logs | MIT |
| `metrics`, `metrics-exporter-prometheus` | Prometheus metrics | MIT (`metrics`), MIT AND Apache-2.0 (`metrics-exporter-prometheus`) |
| `utoipa`, `utoipa-axum` | The generated OpenAPI document | MIT OR Apache-2.0 |
| `aes-gcm` | Envelope encryption of provider keys | Apache-2.0 OR MIT |
| `argon2` | Password hashing (Argon2id) | MIT OR Apache-2.0 |
| `sha2`, `subtle` | Key and token hashes, the models.dev snapshot hash in `cargo xtask models-dev`, constant-time comparison | MIT OR Apache-2.0 (`subtle`: BSD-3-Clause) |
| `rand` | Random secrets and tokens | MIT OR Apache-2.0 |
| `zeroize`, `secrecy` | Wiping and redacting secrets in memory | Apache-2.0 OR MIT |
| `totp-rs` | TOTP (RFC 6238) | MIT |
| `openidconnect` | Single sign-on with OpenID Connect; its HTTP requests go through the gateway's own SSRF-guarded client | MIT |
| `lettre` | SMTP | MIT |
| `maud` | The HTML of emails: templates checked at compile time that escape every value ([emails](architecture/emails.md)) | MIT OR Apache-2.0 |
| `resvg` (tests only) | Renders the wordmark SVGs in a test, to check that the email images are current | Apache-2.0 OR MIT |
| `uuid` | UUIDv7 primary keys | Apache-2.0 OR MIT |
| `time` | Timestamps in UTC | MIT OR Apache-2.0 |
| `rust_decimal` | Money, never a float | MIT |
| `rust_decimal_macros` (tests only) | Decimal literals in tests | MIT |
| `base64`, `url`, `ipnet`, `percent-encoding` | Data helpers | MIT OR Apache-2.0 |
| `idna` | Allowed email domains compared in their ASCII form, whatever case or script they were typed in | MIT OR Apache-2.0 |
| `moka` | In-process caches with a time to live | (MIT OR Apache-2.0) AND Apache-2.0 |
| `thiserror` | Typed errors in the libraries | MIT OR Apache-2.0 |
| `anyhow` | Errors in the binary only | MIT OR Apache-2.0 |
| `rstest`, `insta`, `proptest` | Tests | MIT OR Apache-2.0 (`insta`: Apache-2.0) |
| `http-body-util` (tests only) | Reading response bodies in the server's tests | MIT |
| `rsa`, `chrono` | The mock identity provider in `kaeso-testkit`, in tests only | MIT OR Apache-2.0 |

### KPP

The library code of `kaeso-kpp` and `kaeso-codecs` may use only `serde`, `serde_json` (with `raw_value`), `bytes`, `thiserror`, `indexmap`, `base64` and `mime`. Anything else needs a row here.

| Crate | Used by | Why | License |
| --- | --- | --- | --- |
| `serde` (with `derive`) | Both | The stable JSON form of KPP types; wire structs | MIT OR Apache-2.0 |
| `serde_json` (with `raw_value`, `float_roundtrip`) | Both | JSON; `RawValue` for byte-exact raw payloads; `float_roundtrip` so a float parses to exactly the value that was printed, which the KPP property tests depend on | MIT OR Apache-2.0 |
| `indexmap` (with `serde`) | Both | Order-preserving maps for extras | Apache-2.0 OR MIT |
| `thiserror` | Both | Error types | MIT OR Apache-2.0 |
| `base64` | `kaeso-codecs` | Data URLs and embedding vectors | MIT OR Apache-2.0 |

Tests, benchmarks and tools may use more, never as a library's normal dependency:

| Crate | Used by | Why | License |
| --- | --- | --- | --- |
| `insta` (with `json`) | `kaeso-codecs` tests | Snapshots of the cross-dialect matrix | Apache-2.0 |
| `proptest` | `kaeso-codecs` tests | Property tests: SSE chunk splitting, KPP round trips | MIT OR Apache-2.0 |
| `criterion` | `kaeso-codecs` benchmarks | [Benchmarks](performance.md#kpp) | Apache-2.0 OR MIT |
| `libfuzzer-sys` | `fuzz/`, a workspace of its own | The cargo-fuzz targets | (MIT OR Apache-2.0) AND NCSA |
| `axum`, `tokio`, `serde`, `serde_json` | `crates/kpp-conformance`, never published | The conformance server, never in the library graph, which KPP's `deny.toml` enforces | MIT (`axum`, `tokio`), MIT OR Apache-2.0 |

### Exceptions

Transitive crates whose license is permissive but not on KPP's list. Each is allowed for that crate only, in the root `deny.toml`.

| Crate | Pulled in by | License | Why it is acceptable |
| --- | --- | --- | --- |
| `quoted_printable` | `lettre` | 0BSD | Public-domain equivalent, with no conditions at all |
| `webpki-roots`, `webpki-root-certs` | `sqlx`, `reqwest` (rustls) | CDLA-Permissive-2.0 | Mozilla's CA certificate list as data; permissive, no copyleft |
| `xxhash-rust` | `redis` | BSL-1.0 | The Boost Software License: permissive, attribution only in source distributions |

### Ignored advisories

| Advisory | Crate | Why it doesn't apply |
| --- | --- | --- |
| [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) (Marvin attack) | `rsa`, through `openidconnect` | The timing side channel affects RSA private-key operations. The gateway only verifies ID-token signatures with identity providers' public keys; private keys are used only by the mock identity provider in `kaeso-testkit`, in tests. It is reviewed again when `rsa` ships a fix. |

### Considered and not used

| Crate | Why not |
| --- | --- |
| `figment` | Configuration is driven by one settings table, whose tests check the names, defaults and descriptions and render a reviewed reference ([configuration](architecture/gateway.md#configuration)). With that table, merging defaults, the TOML file and `KAESO_*` variables is a few lines over `toml`, with errors that always name the setting; figment would add a second source of truth for keys and defaults. |

## TypeScript

### The UI library

| Package | License | Why |
| --- | --- | --- |
| `@base-ui/react` | MIT | The headless layer: semantics, focus management, keyboard interaction and positioning for every interactive component |
| `@logicplanes/kaeso-tokens` | MIT | The values every component uses, from `packages/tokens` |
| `@tanstack/react-table` | MIT | Sorting, resizing, selection and column state for `DataTable` |
| `@tanstack/react-virtual` | MIT | Virtualized rows in `DataTable` and `List` over 100 rows |
| `clsx`, `tailwind-merge` | MIT | Class composition; `tailwind-merge` knows the token-only theme, so a consumer's layout classes win cleanly |
| `d3-array`, `d3-scale`, `d3-shape`, `d3-time`, `d3-sankey` | ISC, BSD-3-Clause (`d3-sankey`) | Scales, shapes and layouts for the charts, drawn as React SVG |
| `marked` | MIT | The Markdown lexer behind `Markdown` (`@logicplanes/kaeso-ui/markdown`, `@logicplanes/kaeso-ui/playground`, `ConversationView`'s `markdown`): tokens only, never its HTML renderer; outside the main entry |
| `react-day-picker` | MIT | The calendar grid and date logic for `Calendar`, `DatePicker` and `DateRangePicker` |
| `shiki` | MIT | Syntax highlighting in `CodeBlock`, loaded lazily with explicit grammars |
| `uqr` | MIT | QR codes for `QRCode`, in two-factor setup |

Peer dependencies: `react` and `react-dom` (MIT), and `@hugeicons/react` and `@hugeicons/core-free-icons` (MIT), the only icon set.

### Fonts

| Font | License | Where |
| --- | --- | --- |
| Inter (variable, latin and latin-ext subsets) | SIL OFL 1.1, no reserved font name | `packages/ui/src/styles/fonts/`, from the Fontsource build; shipped in `dist/fonts/` with `LICENSE-Inter.txt` |
| Geist Mono (variable, latin and latin-ext subsets) | SIL OFL 1.1, no reserved font name | `packages/ui/src/styles/fonts/`, from the Fontsource build; shipped in `dist/fonts/` with `LICENSE-GeistMono.txt` |

The font files are unmodified subsets and keep their names. `pnpm release:check` refuses a package without both license files.

### The console

| Package | License | Why |
| --- | --- | --- |
| `@logicplanes/kaeso-ui`, `@hugeicons/react`, `@hugeicons/core-free-icons` | MIT | Every component and icon, from `packages/ui` |
| `react`, `react-dom` | MIT | The interface |
| `@tanstack/react-router` | MIT | File-based routes with typed search parameters, each route split into its own chunks |
| `@tanstack/react-query` | MIT | Server state: caching, refetching and mutations against the Platform API |
| `openapi-fetch` | MIT | The typed Platform API client, over the types generated from `openapi.json` |
| `marked` | MIT | The Markdown lexer behind a logged request's conversation: tokens only, drawn with library components |
| `zod` | MIT | The peer the OpenAI and Anthropic SDKs of `pnpm seed` ask for; not in the console's build |

### The documentation site

| Package | License | Why |
| --- | --- | --- |
| `@logicplanes/kaeso-ui`, `@logicplanes/kaeso-tokens`, `@hugeicons/react`, `@hugeicons/core-free-icons` | MIT | Every component and icon, from `packages/ui` and `packages/tokens` |
| `react`, `react-dom` | MIT | The interface |
| `@tanstack/react-router` | MIT | One route tree per language; every address loads its page's chunk |
| `marked` (build only) | MIT | The Markdown lexer of the content build; pages are compiled at build time, so it never reaches the browser |
| `image-size` (build only) | MIT | The width and height of every image, so pages don't shift as images load |

### Build, test and showcase tools

None of these is part of a published package or of the console's or the documentation's build.

| Package | License | Why |
| --- | --- | --- |
| `vite`, `@vitejs/plugin-react` | MIT | Builds, with one entry point per component in the library |
| `typescript`, `tsx` | Apache-2.0, MIT | Types, declarations and the token build |
| `tailwindcss`, `@tailwindcss/vite`, `@tailwindcss/cli` | MIT | The styling engine; the theme is generated from the tokens |
| `react-docgen-typescript` | MIT | The component registry and the props tables |
| `@tanstack/router-plugin`, `@tanstack/router-generator` | MIT | The console's route tree, generated from `src/routes`, and its code splitting |
| `openapi-typescript`, `@redocly/openapi-core` | MIT | The console's Platform API types, generated from `openapi.json` |
| `openai`, `@anthropic-ai/sdk` | Apache-2.0, MIT | Sample traffic through the gateway in `pnpm seed` |
| `vitest`, `@vitest/coverage-v8`, `happy-dom` | MIT | Unit and component tests with coverage |
| `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom` | MIT | Tests from what a person sees |
| `@playwright/test` | Apache-2.0 | End-to-end tests |
| `@axe-core/playwright`, `axe-core` | MPL-2.0 | Accessibility checks in the end-to-end tests |
| `pngjs` | MIT | Reads screenshots in the showcase's geometry checks |
| `astro`, `@astrojs/react`, `@astrojs/mdx` | MIT | The showcase |
| `eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-jsx-a11y`, `globals` | MIT | Lint for the console and the documentation site, with their own rules in `eslint/` |
| `oxlint`, `prettier`, `prettier-plugin-astro`, `knip` | MIT, MIT, MIT, ISC | Lint for the libraries and the showcase, formatting, and unused-code checks |
| `@types/*` | MIT | Type definitions |

`axe-core` is MPL-2.0, a file-level copyleft license; it runs only in tests and is never bundled. The showcase build pulls in further tools through Astro, among them `sharp` with libvips under LGPL-3.0 for image processing and `lightningcss` under MPL-2.0; they run at build time, and none of their code reaches a package or a published site.

## Python test tools

The end-to-end tests and KPP's SDK conformance suite install the pinned packages of `e2e/requirements.txt` into a virtual environment in `e2e/.venv`. They never reach an image or a binary.

| Package | Version | Why | License |
| --- | --- | --- | --- |
| `openai` | 3.29.0 | The official OpenAI SDK under test | Apache-2.0 |
| `anthropic` | 1.13.0 | The official Anthropic SDK under test | MIT |
| `httpx` | 0.28.1 | Platform API calls in the suites' setup | BSD-3-Clause |
| `pytest` | 9.1.1 | The test runner | MIT |
