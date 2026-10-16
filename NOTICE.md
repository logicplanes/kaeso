# Notices

Kaeso includes no code adapted from other projects. If code is ever adapted from an MIT- or Apache-licensed project, its attribution goes here. Every dependency and its license is listed on the [dependencies](https://docs.kaeso.app/development/dependencies) page.

## Data

- `crates/kaeso-core/data/models-dev.json`: a snapshot of [models.dev](https://models.dev) (`https://models.dev/api.json`), MIT licensed, © models.dev contributors, source at https://github.com/anomalyco/models.dev. Its provenance and fetch date are in [`crates/kaeso-core/data/README.md`](crates/kaeso-core/data/README.md).

## Fonts and icons

- [Inter](https://rsms.me/inter/), SIL Open Font License 1.1, and [Geist Mono](https://vercel.com/font), SIL Open Font License 1.1: unmodified subsets of their Fontsource builds, self-hosted by `@logicplanes/kaeso-ui` and shipped with their license files (`packages/ui/src/styles/fonts/`).
- [Hugeicons](https://hugeicons.com) free, Stroke Rounded, MIT licensed: the icon set of every Kaeso interface.

## Wire formats

KPP implements the OpenAI and Anthropic wire formats from the vendors' public documentation and specifications:

- the OpenAI OpenAPI specification (MIT), https://github.com/openai/openai-openapi;
- the Anthropic API documentation, https://docs.anthropic.com;
- payload shapes cross-checked against the official SDKs, https://github.com/openai/openai-python (Apache-2.0) and https://github.com/anthropics/anthropic-sdk-python (MIT).

KPP's design draws lessons from these projects; no code from them is included:

- TensorZero (Apache-2.0): `Unknown` content blocks, provider-tagged reasoning, an end-to-end provider matrix.
- LiteLLM Rust core (MIT): flattened `extra` maps on wire structs, declining unsupported requests before sending them.
- Vercel AI SDK provider specification: content parts, stream lifecycle events, provider options.
