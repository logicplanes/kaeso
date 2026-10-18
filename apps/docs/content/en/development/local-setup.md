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

## Install the dependencies

Install the console, the docs, the showcase and the libraries in `packages/`:

```bash
pnpm install
```

Cargo fetches the Rust crates on the first build.

## Start the development stack

Build the gateway from your checkout and start it with its databases and mock services:

```bash
pnpm stack:up
```

The first build compiles the gateway and takes several minutes; later builds reuse Docker's cache. Once every service is healthy, the command prints a setup link, `http://localhost:3000/setup#token=…`.

## Add sample data

Set the install up through the Platform API and fill it with sample data:

```bash
pnpm seed
```

You get the organization Northwind Labs with five projects, six members, keys, models, an alias and limits. It also sends a few hundred requests to the mock provider and adds 70 days of earlier request history. To set the install up by hand instead, skip this step and open the setup link.

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
