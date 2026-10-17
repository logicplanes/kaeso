---
title: Local setup
description: Go from a fresh clone to a running install with sample data and passing tests, step by step.
order: 1
---

This page sets up everything on your machine: the gateway built from source with its databases, the console, sample data, and the tests. Every command runs from the repository root.

## What you need

| Tool | Version | Used for |
| --- | --- | --- |
| [Git](https://git-scm.com) | any recent | Getting the code |
| [Node.js](https://nodejs.org) | 24, as in `.node-version` | The console, the UI library, the docs and the scripts |
| pnpm | 11, pinned in `package.json` and installed by Corepack | The TypeScript workspace |
| [Rust](https://rustup.rs) through rustup | 1.99, pinned in `rust-toolchain.toml` and installed on first use | The gateway and KPP |
| [Docker](https://docs.docker.com/get-docker/) with Compose | Docker Compose 2.24 or newer | The development stack |
| [Python](https://www.python.org) | 3.13 | Only for the [end-to-end tests](testing.md#end-to-end-tests) |

Corepack comes with Node.js. Turn it on once, and it provides the exact pnpm version the repository pins:

```bash
corepack enable
```

rustup reads `rust-toolchain.toml` the first time you run `cargo` in the repository and installs Rust 1.99 with rustfmt, clippy and llvm-tools. `rustup show` does it ahead of time.

## 1. Get the code

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## 2. Install the dependencies

```bash
pnpm install
```

This installs every package of the TypeScript workspace: the console, the docs, the showcase and the libraries in `packages/`. Cargo fetches the Rust dependencies by itself on the first build.

## 3. Start the development stack

```bash
pnpm stack:up
```

This builds the gateway's image from your checkout (`kaeso-dev-gateway`, from `deploy/gateway.Dockerfile`) and starts it in Docker with PostgreSQL, ClickHouse, Valkey, Mailpit, a mock model provider and a mock identity provider, then waits until every service is healthy. The first build compiles the gateway and takes several minutes; later ones reuse Docker's cache.

| Service | What it is | On your machine |
| --- | --- | --- |
| `gateway` | The gateway built from your checkout | `127.0.0.1:18080` |
| `postgres`, `clickhouse`, `valkey` | The gateway's databases, also used by the Rust tests | `127.0.0.1:55432`, `127.0.0.1:58123`, `127.0.0.1:56379` |
| `mail` | Mailpit, which catches sign-in links, invitations and password resets | `http://127.0.0.1:58025`, SMTP on `127.0.0.1:51025` |
| `provider` | A mock OpenAI and Anthropic API that answers every request without calling a real provider | Inside the stack only |
| `idp` | A mock OpenID Connect provider for single sign-on | `http://idp.localhost:18091` |
| `master-key` | Writes the gateway's master key into a volume before it starts | |

The stack is the Compose project `kaeso-dev`, defined in `deploy/compose.dev.yaml`. A second clone can run a stack of its own next to it, as described under Run a second stack below. Every model request goes to the mock provider, so no real provider key is needed.

## 4. Fill it with sample data

```bash
pnpm seed
```

This sets the install up through the Platform API, the way a person would, and fills it: the organization Northwind Labs with five projects, six members, keys, models, aliases and limits, a few hundred requests sent through the gateway to the mock provider, and 70 days of earlier request history.

To set the install up by hand instead, skip this step: `pnpm stack:up` prints a setup link (`http://localhost:3000/setup#token=…`) while the install has no users.

## 5. Open the console

```bash
pnpm dev
```

This builds the libraries in `packages/`, starts the stack (rebuilding the gateway when its source changed), and serves the console at http://localhost:3000. It reloads as you save.

Sign in as `owner@example.com`, an instance admin and the organization's owner, or `lin@example.com`, a developer. Both have the password `correct horse battery staple`. Emails the gateway sends arrive in Mailpit at http://127.0.0.1:58025.

```text
browser ─▶ http://localhost:3000 (Vite) ─▶ /platform, /v1 sent on to the gateway on 127.0.0.1:18080
                                         └▶ everything else: the console
gateway ─▶ provider (mock OpenAI and Anthropic), mail (Mailpit), idp (mock single sign-on)
```

The gateway's public URL is the development server's address, so the session cookie is first-party and the `Origin` the browser sends is the one the gateway expects. Production serves the console and the gateway on one domain the same way, behind one reverse proxy.

## 6. Run the tests

The TypeScript tests:

```bash
pnpm test
```

The Rust tests, which use the stack's databases:

```bash
cargo test --workspace --all-features
```

Every Rust test creates databases of its own, so tests run in parallel and leave the sample data alone. [Testing](testing.md) lists every suite, the end-to-end tests and the checks a change must pass.

## Stop the stack

```bash
pnpm stack:down
```

This removes the stack with its data, including the Rust tests' databases; the next `pnpm stack:up` starts empty.

## Change ports and images

The stack's ports come from `apps/console/scripts/stack.mjs`. These variables change them: `KAESO_GATEWAY_PORT`, `KAESO_POSTGRES_PORT`, `KAESO_CLICKHOUSE_PORT`, `KAESO_VALKEY_PORT`, `KAESO_MAIL_PORT`, `KAESO_MAIL_SMTP_PORT`, `KAESO_IDP_PORT` and `KAESO_PUBLIC_URL`. The Rust tests find the databases through `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` and `KAESO_TEST_VALKEY_URL` when they are elsewhere.

To run the stack against a gateway image built elsewhere, such as a release, name it; the stack then uses it instead of building:

```bash
KAESO_GATEWAY_IMAGE=ghcr.io/logicplanes/kaeso-gateway:{{version}} pnpm stack:up
```

## Run a second stack

A second clone gets a stack of its own when it has another project name and other ports. `KAESO_STACK_PROJECT` names the Compose project (`kaeso-dev` by default) and the gateway's image after it; `pnpm stack:up`, `pnpm seed` and `pnpm stack:down` then work on that project alone. In the second clone:

```bash
export KAESO_STACK_PROJECT=kaeso-two KAESO_PUBLIC_URL=http://localhost:3001
export KAESO_GATEWAY_PORT=18180 KAESO_IDP_PORT=18191 KAESO_MAIL_PORT=58125 KAESO_MAIL_SMTP_PORT=51125
export KAESO_POSTGRES_PORT=55532 KAESO_CLICKHOUSE_PORT=58223 KAESO_VALKEY_PORT=56479
pnpm dev --port 3001
```

`pnpm seed` in the same shell fills that stack. Point the Rust tests at its databases with the `KAESO_TEST_…` variables above.

## Run the gateway from source

For a gateway you can restart in seconds, with a debugger or extra logging, run it with Cargo against the stack's databases. Start the mock provider in a second terminal first, so no request reaches a real provider:

```bash
cargo run -p kaeso-testkit -- mock-provider 127.0.0.1:18090
```

In the first terminal, create fresh databases and export their settings, point both providers at the mock, add a public URL, a master key and email that is only logged, then start the gateway:

```bash
export $(cargo run -q -p kaeso-testkit -- fresh-services)
export KAESO_PROVIDERS_OPENAI_BASE_URL=http://127.0.0.1:18090
export KAESO_PROVIDERS_ANTHROPIC_BASE_URL=http://127.0.0.1:18090
export KAESO_PUBLIC_URL=http://localhost:3000
export KAESO_LISTEN=127.0.0.1:8080
export KAESO_MASTER_KEYS="$(cargo run -q -p kaeso -- master-key generate)"
export KAESO_MAIL_TRANSPORT=log
cargo run -p kaeso -- serve
```

Serve the console against it from a third terminal:

```bash
KAESO_GATEWAY_URL=http://127.0.0.1:8080 pnpm --filter kaeso-console dev
```

Then open the setup link from the gateway's log. To call the real providers instead, leave out the two `KAESO_PROVIDERS_…` lines. The databases stay until you drop them from the first terminal:

```bash
cargo run -q -p kaeso-testkit -- drop-services "$KAESO_CLICKHOUSE_DATABASE"
```

Every setting is listed under [configuration](../self-hosting/configuration.md).

## Generated code

Two things are generated rather than written:

- **`openapi.json`**, the Platform API document at the repository root, comes from the gateway's code. After changing the Platform API, write it again and commit it with the change:

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

- **The console's API client and route tree.** `pnpm --filter kaeso-console generate` writes the client's types from `openapi.json` and the route tree from `apps/console/src/routes`. The console's `dev`, `build`, `typecheck`, `test` and `e2e` scripts run it first, and neither output is committed.

The console and the gateway therefore change together: a change to the Platform API updates `openapi.json` and the console in the same pull request.

## The documentation site

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

The site runs at http://localhost:4480 and reloads as you save a page in `apps/docs/content`. `pnpm --filter kaeso-docs check:content` checks every page, link and translation without starting it. [Writing documentation](documentation.md) has the rest.

## The UI library

```bash
pnpm build:packages
pnpm --filter kaeso-showcase dev
```

The showcase of every component runs at http://localhost:4321 and reloads as you change the library's source. The console and the docs read the library's build instead: after changing `packages/ui` or `packages/tokens`, run `pnpm build:packages` and restart their development servers.
