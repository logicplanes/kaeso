---
title: Local setup
description: Go from a fresh clone to the console with sample data and passing tests.
order: 1
---

Clone Kaeso, start the development stack, fill it with sample data and run the tests. Every command runs from the repository root.

## Before you begin

- [Git](https://git-scm.com).
- [Node.js](https://nodejs.org) 24, the version in `.node-version`.
- [rustup](https://rustup.rs). It installs the Rust toolchain pinned in `rust-toolchain.toml` the first time you run `cargo` in the repository.
- A Docker engine with Compose 2.24 or newer: Docker Engine on Linux, or [Colima on macOS](stack.md#run-docker-with-colima-on-macos).
- [Python](https://www.python.org) 3.13, for the [end-to-end tests](testing.md#end-to-end-tests) only.

## Turn on Corepack

Corepack ships with Node.js and provides the exact pnpm version the repository pins:

```bash
corepack enable
```

`pnpm --version` now prints the version named in `package.json`.

## Get the code

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## Set it up

One command checks what the repository needs, installs the dependencies, builds the UI library, starts the development stack and fills it with sample data:

```bash
pnpm bootstrap
```

It stops before it changes anything when a prerequisite is missing or too old, when Docker doesn't answer, or when another program holds a port the stack needs. It then says how to fix that, such as `corepack enable` or the variable that moves the port. Run it again at any time: an install that is set up keeps its data.

The first run builds the gateway from your checkout, which takes several minutes; later runs reuse Docker's cache. The sample data is the organization Northwind Labs with five projects, six members, ten keys, models, aliases and limits. A few thousand support emails and Slack questions go through the gateway, which the mock provider answers with written replies. Then come 70 days of request history: each app and person with its own models, clients and hours, content where the project keeps it, response cache hits, fallbacks while a provider is overloaded and a limit reached during a sale.

| Option | What it does |
| --- | --- |
| `--check` | Only checks the prerequisites, the free disk and the ports |
| `--all` | Also installs the browsers, the Cargo tools and the Python packages the [full test suites](testing.md) need |
| `--no-seed` | Starts the stack without sample data; open the setup link it prints to set Kaeso up by hand |

With `KAESO_SEED_WHOLE_DAYS=1`, the history ends with yesterday instead of ten minutes ago, for a demo that moves its dates forward. `pnpm stack:up` and `pnpm seed` run the stack and the sample data on their own ([development stack](stack.md)).

## Open the console

Build the libraries and serve the console:

```bash
pnpm dev
```

The console runs at http://localhost:3000 and reloads as you save. Sign in as `owner@example.com`, the organization's owner and an instance admin, or as `lin@example.com`, a developer. Both use the password `correct horse battery staple`. Emails arrive in Mailpit at http://127.0.0.1:58025.

## Run the tests

Run the TypeScript and the Rust tests:

```bash
pnpm test
cargo test --workspace --all-features
```

Both finish without failures. The Rust tests create databases of their own on the stack, so they leave the sample data alone.

## Next steps

- [Contributing](contributing.md): Make a change and get your pull request merged.
- [Development stack](stack.md): Change ports, run a second stack or run the gateway from source.
- [Testing](testing.md): Run every check a change must pass.
- [Architecture](architecture/index.md): Find your way around the gateway, the console and the docs.
