# Notices

Kaeso includes no code adapted from other projects. If code is ever adapted from an MIT- or Apache-licensed project, its attribution goes here. Every dependency's license is checked by `cargo deny` and `pnpm licenses:check` against the [license policy](https://docs.kaeso.app/development/contributing#add-a-dependency).

## Data

- `crates/kaeso-core/data/models-dev.json`: a snapshot of [models.dev](https://models.dev) (`https://models.dev/api.json`), MIT licensed, © models.dev contributors, source at https://github.com/anomalyco/models.dev. Its provenance and fetch date are in [`crates/kaeso-core/data/README.md`](crates/kaeso-core/data/README.md).

## Fonts and icons

- [Inter](https://rsms.me/inter/), SIL Open Font License 1.1, and [Geist Mono](https://vercel.com/font), SIL Open Font License 1.1: unmodified subsets of their Fontsource builds, self-hosted by `@logicplanes/kaeso-ui` and shipped with their license files (`packages/ui/src/styles/fonts/`).
- [Hugeicons](https://hugeicons.com) free, Stroke Rounded, MIT licensed: the icon set of every Kaeso interface.

## Provider marks

The provider marks in `packages/ui/src/components/kaeso/provider-marks.ts` identify each built-in provider in the console. Each is its owner's trademark, drawn from the owner's own vector unchanged except for uniform scaling onto a 24px grid, in the owner's one-colour version for each theme (`providerMark` in `packages/tokens/src/tokens.ts`):

- OpenAI: the Blossom from [openai.com](https://openai.com)'s icon, black on light and white on dark, following the [OpenAI design guidelines](https://openai.com/brand).
- DeepSeek: the whale from the logo on [deepseek.com](https://www.deepseek.com), in the text colour as on DeepSeek's own site; DeepSeek publishes no logo guidelines.
- SpaceX: the mark of [x.ai](https://x.ai) and [console.x.ai](https://console.x.ai), uncropped, black on light and white on dark as in the console's icons, following the [brand guidelines](https://x.ai/legal/brand-guidelines).
- Together AI: the black and white logos from the [Together AI brand assets](https://www.together.ai/brand).
- OpenRouter: the Ink glyph (`#03080A`) on light and the Cloud glyph (`#FCFCFE`) on dark from the [OpenRouter brand assets](https://openrouter.ai/brand).
- Mistral: the black (`#161525`) and white icons from the [Mistral AI brand assets](https://mistral.ai/brand).

Anthropic's [trademark guidelines](https://www.anthropic.com/legal/trademark-guidelines) allow its logo only in approved materials, and Groq's [trademark policy](https://groq.com/trademark-policy) allows its logo in an interface only under a licence, so both show a letter.

## Wire formats

KPP implements the OpenAI and Anthropic wire formats from the vendors' public documentation and specifications:

- the OpenAI OpenAPI specification (MIT), https://github.com/openai/openai-openapi;
- the Anthropic API documentation, https://docs.anthropic.com;
- payload shapes cross-checked against the official SDKs, https://github.com/openai/openai-python (Apache-2.0) and https://github.com/anthropics/anthropic-sdk-python (MIT).

KPP's design draws lessons from these projects; no code from them is included:

- TensorZero (Apache-2.0): `Unknown` content blocks, provider-tagged reasoning, an end-to-end provider matrix.
- LiteLLM Rust core (MIT): flattened `extra` maps on wire structs, declining unsupported requests before sending them.
- Vercel AI SDK provider specification: content parts, stream lifecycle events, provider options.
