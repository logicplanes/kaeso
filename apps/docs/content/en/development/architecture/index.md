---
title: Architecture
description: How the gateway, KPP, the console, the UI library and the documentation fit together in one repository.
order: 2
---

```text
                     crates/kaeso-kpp, crates/kaeso-codecs
                                    │
apps ─▶ /v1 ─────────▶ gateway (crates/kaeso, crates/kaeso-*) ─▶ OpenAI, Anthropic
                         │    │
console ─▶ /platform/v1 ─┘    └─▶ openapi.json ─▶ the console's API client
   ▲
packages/ui, packages/tokens ─▶ apps/console, apps/docs, apps/showcase
```

- **The gateway uses KPP** for every model request. KPP decodes the app's format into its canonical format and encodes it for the provider, and back. The gateway never parses or builds a provider's JSON itself, and it depends on KPP by path inside the Cargo workspace, so a change to both is one pull request.
- **The console uses the Platform API** and nothing else, so everything it does, a script can do too. Its API client is generated from `openapi.json`, which the gateway generates from its code, so the two change together.
- **The console, the docs and the showcase are drawn with the UI library.** Every visible element comes from `@logicplanes/kaeso-ui`, built on `@logicplanes/kaeso-tokens`, both used from the workspace (`workspace:*`). The gateway's emails use the same token values.
- **One version.** Every part shares the version number of the release, and the release's `compose.yaml` pins the gateway and console images of that release.

## Pages

| Page | What it covers |
| --- | --- |
| [Gateway](gateway.md) | The request path, the crates, configuration, and the rules the gateway's code follows |
| [Console](console.md) | Data, styling, bundles and errors in the web console |
| [Documentation site](docs-site.md) | How the Markdown pages become docs.kaeso.app |
| [Emails](emails.md) | How the gateway builds the emails it sends |
| [KPP internals](../kpp/index.md) | The canonical format and the codecs |

## The Rust workspace

| Crate | Job |
| --- | --- |
| `kaeso` | The gateway binary: its command line (`serve`, `migrate`, `master-key`, `user`, `setup-link`, `healthcheck`, `openapi`), startup and shutdown. The only crate that may use `anyhow`. |
| `kaeso-server` | The HTTP surfaces: routers for `/v1`, `/platform/v1`, health and metrics; extractors, error mapping, the OpenAPI document, CSRF checks, request ids, body limits; the services behind both APIs, among them the limit counters as Valkey Lua scripts; configuration, background jobs and telemetry. |
| `kaeso-core` | Domain rules without I/O: ids, rights and roles, key format, model resolution, the catalogue, capability checks, limits, costs, the stream reply assembler. Tested without databases. Its `data/` holds the models.dev snapshot shipped with every release. |
| `kaeso-store` | Storage: PostgreSQL through sqlx, with the offline query data in `.sqlx/` at the repository root, ClickHouse, and the Valkey connection with its cache invalidation; the migrations for PostgreSQL and ClickHouse, run under a lock. |
| `kaeso-upstream` | Provider transport: HTTP clients, auth headers, base URLs, timeouts, streaming bodies and cancellation. |
| `kaeso-crypto` | Envelope encryption (AES-256-GCM), the master key ring, passwords, TOTP and secret wrapper types. |
| `kaeso-mail` | Email: SMTP delivery and every email in one layout, in three languages ([emails](emails.md)). |
| `kaeso-testkit` | Test-only: development and test databases, the mock provider built from KPP's fixtures, a mock identity provider. |
| `kaeso-kpp`, `kaeso-codecs` | KPP's types and codecs, published to crates.io ([KPP internals](../kpp/index.md)). |
| `kpp-conformance` | A server that serves KPP's encoders to the official Python SDKs for the [conformance suite](../testing.md#sdk-conformance). |
| `xtask` | Repository tasks: `cargo xtask models-dev` refreshes the models.dev snapshot, `cargo xtask record` [records provider responses](../kpp/fixtures.md#recording-live-responses). |

Every crate shares the workspace's version, edition and lints in the root `Cargo.toml`. Only `kaeso-kpp` and `kaeso-codecs` are published; the others set `publish = false`.

## The TypeScript workspace

| Package | Folder | Job |
| --- | --- | --- |
| `kaeso-console` | `apps/console` | The web console, a static React app served beside the gateway ([console](console.md)) |
| `kaeso-docs` | `apps/docs` | This documentation ([documentation site](docs-site.md)) |
| `kaeso-showcase` | `apps/showcase` | Every component of the UI library in every state, with its guides; it runs locally with `pnpm --filter kaeso-showcase dev` |
| `@logicplanes/kaeso-ui` | `packages/ui` | The component library: React components on Base UI, styled with Tailwind CSS and the tokens |
| `@logicplanes/kaeso-tokens` | `packages/tokens` | The design tokens as CSS variables, a Tailwind theme, TypeScript constants and JSON |

The apps read the libraries' builds, so `pnpm build:packages` runs before anything that needs them. The UI library ships its own rules and component registry for the people and tools that use it (`packages/ui/AGENTS.md`, `packages/ui/foundations/`, `packages/ui/ai/`); its showcase documents every component.

## In production

```text
browser, apps ─▶ https://kaeso.example.com ─▶ Caddy ─▶ /v1/*, /platform/* ─▶ gateway
                                                   └▶ everything else ─▶ console (static files)
gateway ─▶ PostgreSQL (state), ClickHouse (request logs), Valkey (counters, caches)
```

The console is a set of static files served by its own small container, on the same domain as the gateway, so the session cookie is first-party and there is no CORS. [Self-hosting](../../self-hosting/index.md) describes an install.
