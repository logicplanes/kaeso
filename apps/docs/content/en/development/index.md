---
title: Development
description: How Kaeso's source is organized, and how to build, test and change it.
order: 6
---

These pages are for people who work on Kaeso itself. Everything lives in one repository, [github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso), and is released together under one version. To get a working checkout, start with [local setup](local-setup.md).

## The repository

| Path | Holds | Built with |
| --- | --- | --- |
| `crates/` | The gateway (`kaeso` and the `kaeso-*` crates), KPP (`kaeso-kpp`, `kaeso-codecs`), the SDK conformance server (`kpp-conformance`) and repository tasks (`xtask`) | Rust, Cargo |
| `apps/console` | The web console | TypeScript, React |
| `apps/docs` | This documentation, served at docs.kaeso.app | Markdown, TypeScript |
| `apps/showcase` | The UI library's showcase at ui.kaeso.app: every component in every state | Astro, React |
| `packages/ui`, `packages/tokens` | The component library and the design tokens every interface is built from | TypeScript, React |
| `e2e/` | The official OpenAI and Anthropic Python SDKs against KPP, the gateway and a fresh install | Python |
| `fuzz/` | Fuzz targets for every KPP decoder | Rust |
| `deploy/` | The Dockerfiles, the production `compose.yaml` and the development stack | Docker |
| `openapi.json` | The Platform API document, generated from the gateway's code | |

Cargo builds the Rust workspace and pnpm the TypeScript workspace (`pnpm-workspace.yaml`). Both run from the repository root.

## What a release ships

| Artifact | Built from |
| --- | --- |
| The images `ghcr.io/logicplanes/kaeso-gateway` and `ghcr.io/logicplanes/kaeso-console` | `deploy/gateway.Dockerfile`, `deploy/console.Dockerfile` |
| `compose.yaml`, `kaeso.example.toml` and `env.example`, attached to the GitHub release | `deploy/compose.yaml` with both images pinned to the release, `deploy/kaeso.example.toml`, `deploy/.env.example` |
| The crates `kaeso-kpp` and `kaeso-codecs` on crates.io | `crates/kaeso-kpp`, `crates/kaeso-codecs` |
| The npm packages `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens` | `packages/ui`, `packages/tokens` |

The documentation and the showcase are deployed as static sites from the release's tag. [Releasing](releasing.md) covers the whole process.

## Pages

| Page | Covers |
| --- | --- |
| [Contributing](contributing.md) | Issues, pull requests and the rules every change follows |
| [Local setup](local-setup.md) | Prerequisites, the development stack, sample data, and running each part from source |
| [Architecture](architecture/index.md) | How the gateway, the console, the documentation site and the emails fit together |
| [KPP internals](kpp/index.md) | The canonical format, the codecs, and every wire format field by field |
| [Testing](testing.md) | Every test suite, how to run it, and what CI requires |
| [Translations](translations.md) | How every interface speaks English, German and Simplified Chinese, and the glossary |
| [Writing documentation](documentation.md) | How pages of this site are written, linked and translated |
| [Dependencies and licenses](dependencies.md) | Every direct dependency, why it is there, and the license policy |
| [Performance](performance.md) | Bundle budgets, codec benchmarks and fuzzing results |
| [Releasing](releasing.md) | Versions, the changelog, the Release workflow and what it publishes |
