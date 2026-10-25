---
title: Development
description: Build, test and change Kaeso, from a fresh clone to a merged pull request.
order: 6
---

These pages are for people who change Kaeso itself. Everything lives in one repository, [logicplanes/kaeso](https://github.com/logicplanes/kaeso), and ships together under one version.

| Path | Holds | Built with |
| --- | --- | --- |
| `crates/` | The gateway, the updater, KPP (`kaeso-kpp`, `kaeso-codecs`), the SDK conformance server and repository tasks (`xtask`) | Rust, Cargo |
| `apps/console` | The web console | TypeScript, React |
| `apps/docs` | This documentation | Markdown, TypeScript |
| `apps/showcase` | The UI library's showcase at ui.kaeso.app | Astro, React |
| `packages/ui`, `packages/tokens` | The component library and the design tokens every interface is built from | TypeScript, React |
| `e2e/` | The official OpenAI and Anthropic Python SDKs against KPP, the gateway and a fresh install | Python |
| `fuzz/` | Fuzz targets for every KPP decoder | Rust |
| `deploy/` | The Dockerfiles, the self-hosting `compose.yaml` and the development stack | Docker |
| `openapi.json` | The Platform API document, generated from the gateway's code | |

Cargo builds the Rust workspace and pnpm the TypeScript workspace, both from the repository root.

## Next steps

- [Local setup](local-setup.md): Go from a fresh clone to the console with sample data and passing tests.
- [Contributing](contributing.md): Report a problem, propose a change and get your pull request merged.
- [Development stack](stack.md): Change ports, run a second stack or run the gateway from source.
- [Testing](testing.md): Run the checks a change must pass, and find the suite that covers your code.
- [Gateway benchmark](benchmark.md): Measure the time and memory Kaeso adds, from single requests to thousands of streams.
- [Translations](translations.md): Write every word in English, German and Simplified Chinese.
- [Writing documentation](documentation.md): Write and translate the pages of this site.
- [Architecture](architecture/index.md): Learn how the gateway, the console, the docs and the updater are built.
- [KPP internals](kpp/index.md): Change the codecs that translate between the API formats.
- [Releasing](releasing.md): Version, publish and deploy a release.
