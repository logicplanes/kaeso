# Notices

Kaeso includes no code adapted from other projects. If code is ever adapted from an MIT- or Apache-licensed project, its attribution goes here. Every dependency's license is checked by `cargo deny` and `pnpm licenses:check` against the [license policy](https://docs.kaeso.app/development/contributing#add-a-dependency).

## Data

- `crates/kaeso-core/data/models-dev.json`: a snapshot of [models.dev](https://models.dev) (`https://models.dev/api.json`), MIT licensed, © models.dev contributors, source at https://github.com/anomalyco/models.dev. Its provenance and fetch date are in [`crates/kaeso-core/data/README.md`](crates/kaeso-core/data/README.md).

## Fonts and icons

- [Inter](https://rsms.me/inter/), SIL Open Font License 1.1, and [Geist Mono](https://vercel.com/font), SIL Open Font License 1.1: unmodified subsets of their Fontsource builds, self-hosted by `@logicplanes/kaeso-ui` and shipped with their license files (`packages/ui/src/styles/fonts/`).
- [Hugeicons](https://hugeicons.com) free, Stroke Rounded, MIT licensed: the icon set of every Kaeso interface.

## Provider marks

The provider marks in `packages/ui/src/components/kaeso/provider-marks.ts` identify each built-in provider in the console. Each is the provider's trademark, redrawn in one colour on a 24px grid:

- OpenAI and Anthropic: the monochrome marks of [LobeHub Icons](https://github.com/lobehub/lobe-icons) (`@lobehub/icons-static-svg`), MIT licensed, © 2023 LobeHub.
- DeepSeek: the whale from the logo on [deepseek.com](https://www.deepseek.com).
- SpaceX: the mark of [console.x.ai](https://console.x.ai) and [x.ai](https://x.ai), cropped to a square as in the console's own app icon.
- Together AI: the mark from the logo on [together.ai](https://www.together.ai), in one colour.
- Groq: the bolt from the logo on [groq.com](https://groq.com).
- OpenRouter: the mark from the logo on [openrouter.ai](https://openrouter.ai).
- Mistral: the icon from the [Mistral AI brand assets](https://mistral.ai/brand), in one colour.

## Wire formats

KPP implements the OpenAI and Anthropic wire formats from the vendors' public documentation and specifications:

- the OpenAI OpenAPI specification (MIT), https://github.com/openai/openai-openapi;
- the Anthropic API documentation, https://docs.anthropic.com;
- payload shapes cross-checked against the official SDKs, https://github.com/openai/openai-python (Apache-2.0) and https://github.com/anthropics/anthropic-sdk-python (MIT).

KPP's design draws lessons from these projects; no code from them is included:

- TensorZero (Apache-2.0): `Unknown` content blocks, provider-tagged reasoning, an end-to-end provider matrix.
- LiteLLM Rust core (MIT): flattened `extra` maps on wire structs, declining unsupported requests before sending them.
- Vercel AI SDK provider specification: content parts, stream lifecycle events, provider options.
