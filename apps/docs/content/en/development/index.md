---
title: Development
description: How Kaeso's source is organized, and how to build, test and change it.
order: 6
---

Kaeso is developed in the open in one repository, [github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso): the gateway, KPP, the console, the UI library and this documentation, released together under one version number. These pages are for people who work on Kaeso itself.

New here? [Local setup](local-setup.md) takes you from a fresh clone to a running install with sample data and passing tests.

## The repository

| Path | What it holds | Built with |
| --- | --- | --- |
| `crates/` | The Cargo workspace: the gateway (`kaeso` and the `kaeso-*` crates), KPP (`kaeso-kpp`, `kaeso-codecs`), the SDK conformance server (`kpp-conformance`) and repository tasks (`xtask`) | Rust |
| `apps/console` | The web console | TypeScript, React |
| `apps/docs` | This documentation, served at docs.kaeso.app | Markdown, TypeScript |
| `apps/showcase` | The UI library's showcase: every component in every state, run locally | Astro, React |
| `packages/ui`, `packages/tokens` | The component library and the design tokens every interface is built from | TypeScript, React |
| `e2e/` | End-to-end tests with the official OpenAI and Anthropic Python SDKs, and the install test | Python |
| `fuzz/` | Fuzz targets for every KPP decoder | Rust |
| `deploy/` | The Dockerfiles, the production `compose.yaml` and the development stack | Docker |
| `openapi.json` | The Platform API document, generated from the gateway's code | |

Cargo builds the Rust workspace and pnpm the TypeScript workspace (`pnpm-workspace.yaml`); both run from the repository root.

## What a release ships

| Artifact | Built from |
| --- | --- |
| The image `ghcr.io/logicplanes/kaeso-gateway` | `deploy/gateway.Dockerfile` |
| The image `ghcr.io/logicplanes/kaeso-console` | `deploy/console.Dockerfile` |
| `compose.yaml`, `kaeso.example.toml` and `env.example`, attached to the GitHub release | `deploy/compose.yaml`, pinning both images of the release, `deploy/kaeso.example.toml` and `deploy/.env.example` |
| The crates `kaeso-kpp` and `kaeso-codecs` on crates.io | `crates/kaeso-kpp`, `crates/kaeso-codecs` |
| The npm packages `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens` | `packages/ui`, `packages/tokens` |

The documentation and the showcase are published as websites. [Releasing](releasing.md) describes how a release is made.

## Pages

| Page | What it covers |
| --- | --- |
| [Local setup](local-setup.md) | Prerequisites, the development stack, sample data, and running each part from source |
| [Architecture](architecture/index.md) | How the parts fit together, the gateway, the console, the documentation site and the emails |
| [KPP internals](kpp/index.md) | The canonical format, the codecs, and every wire format field by field |
| [Testing](testing.md) | Every test suite, how to run it, and what must pass before a change is done |
| [Translations](translations.md) | How the console, the UI library, the emails and the docs speak English, German and Chinese |
| [Writing documentation](documentation.md) | How the pages of this site are written, linked and translated |
| [Dependencies and licenses](dependencies.md) | Every dependency, why it is there, and the license policy |
| [Performance](performance.md) | Bundle budgets, codec benchmarks and fuzzing results |
| [Releasing](releasing.md) | Versions, the release workflow and what it publishes |
| [Contributing](contributing.md) | Issues, pull requests and the rules every change follows |
