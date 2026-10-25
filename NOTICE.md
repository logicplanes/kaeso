# Notices

Kaeso includes no code adapted from other projects. If code is ever adapted from an MIT- or Apache-licensed project, its attribution goes here. Every dependency's license is checked by `cargo deny` and `pnpm licenses:check` against the [license policy](https://docs.kaeso.app/development/contributing#add-a-dependency).

## Data

- `crates/kaeso-core/data/models-dev.json`: a snapshot of [models.dev](https://models.dev) (`https://models.dev/api.json`), MIT licensed, © models.dev contributors, source at https://github.com/anomalyco/models.dev. Its provenance and fetch date are in [`crates/kaeso-core/data/README.md`](crates/kaeso-core/data/README.md).

## Fonts and icons

- [Inter](https://rsms.me/inter/), SIL Open Font License 1.1, and [Geist Mono](https://vercel.com/font), SIL Open Font License 1.1: unmodified subsets of their Fontsource builds, self-hosted by `@logicplanes/kaeso-ui` and shipped with their license files (`packages/ui/src/styles/fonts/`).
- [Hugeicons](https://hugeicons.com) free, Stroke Rounded, MIT licensed: the icon set of every Kaeso interface.

## Provider marks

The provider marks in `packages/ui/src/components/kaeso/provider-marks.ts` are their owners' trademarks. Kaeso shows them only to identify which provider a key, model or request belongs to (nominative use), and implies no affiliation or endorsement. Each comes from the owner's own vector, unchanged except for uniform scaling and translation onto a 24px grid, in one colour: the owner's black and white versions where it publishes them (`providerMark` in `packages/tokens/src/tokens.ts`), otherwise the theme's text colour.

- OpenAI: the Blossom from [openai.com](https://openai.com)'s icon, black on light and white on dark, following the [OpenAI design guidelines](https://openai.com/brand).
- Anthropic: the A mark from the logo on [anthropic.com](https://www.anthropic.com), in the text colour as on Anthropic's site; see its [trademark guidelines](https://www.anthropic.com/legal/trademark-guidelines).
- DeepSeek: the whale from the logo on [deepseek.com](https://www.deepseek.com), in the text colour as on DeepSeek's site.
- SpaceX: the mark of [x.ai](https://x.ai), cropped to a square as in xAI's own app and console icons, black on light and white on dark as in those icons; see the [brand guidelines](https://x.ai/legal/brand-guidelines).
- Together AI: the black and white logos from the [Together AI brand assets](https://www.together.ai/brand).
- Groq: the bolt from [GroqCloud](https://console.groq.com)'s logo and favicon, in the text colour; see the [trademark policy](https://groq.com/trademark-policy).
- OpenRouter: the Ink glyph (`#03080A`) on light and the Cloud glyph (`#FCFCFE`) on dark from the [OpenRouter brand assets](https://openrouter.ai/brand).
- Mistral: the black (`#161525`) and white icons from the [Mistral AI brand assets](https://mistral.ai/brand).

Groq is a trademark of Groq LLC and/or its affiliates. Kaeso is not affiliated with or endorsed by any of these providers.

## Client marks

The client marks in `packages/ui/src/components/kaeso/client-button.tsx` are their owners' trademarks: Claude of Anthropic, Cursor of Anysphere, and GitHub Copilot of GitHub. Kaeso shows them only to say which client a set of install steps is for (nominative use), and implies no affiliation or endorsement. Each is the [Simple Icons](https://simpleicons.org) vector (CC0 1.0) of that mark, unchanged, in the theme's text colour.

## Wire formats

KPP implements the OpenAI and Anthropic wire formats from the vendors' public documentation and specifications:

- the OpenAI OpenAPI specification (MIT), https://github.com/openai/openai-openapi;
- the Anthropic API documentation, https://docs.anthropic.com;
- payload shapes cross-checked against the official SDKs, https://github.com/openai/openai-python (Apache-2.0) and https://github.com/anthropics/anthropic-sdk-python (MIT).

KPP's design draws lessons from these projects; no code from them is included:

- TensorZero (Apache-2.0): `Unknown` content blocks, provider-tagged reasoning, an end-to-end provider matrix.
- LiteLLM Rust core (MIT): flattened `extra` maps on wire structs, declining unsupported requests before sending them.
- Vercel AI SDK provider specification: content parts, stream lifecycle events, provider options.
