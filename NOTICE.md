# Notices

Kaeso includes no code adapted from other projects. If code is ever adapted from an MIT- or Apache-licensed project, its attribution goes here. Every dependency's license is checked by `cargo deny` and `pnpm licenses:check` against the [license policy](https://docs.kaeso.app/development/contributing#add-a-dependency).

## Trademarks

All trademarks, logos and product names belong to their owners. Kaeso is not affiliated with, endorsed or sponsored by any provider, client or other company it names. Kaeso names them and shows their marks only to identify the service a key, model, request or set of install steps belongs to (nominative use).

Some providers' terms don't permit their logos in an interface like Kaeso's without their consent: the [membership agreement](https://www.alibabacloud.com/help/en/legal/latest/alibaba-cloud-international-website-membership-agreement) of Alibaba Cloud, which owns QwenCloud, granting no right to its trademarks and logos, Anthropic's [trademark guidelines](https://www.anthropic.com/legal/trademark-guidelines), Cerebras's [terms of service](https://www.cerebras.ai/terms-of-service), which grant no license or right to its trademarks and logos, Cohere's [terms of use](https://cohere.com/terms-of-use), Groq's [trademark policy](https://groq.com/trademark-policy), Z.ai's [terms of use](https://docs.z.ai/legal-agreement/terms-of-use), which forbid using its logos without its prior written consent, and MiniMax's [terms of service](https://platform.minimax.io/protocol/terms-of-service), which forbid using its logos without its prior written approval, as does its [brand page](https://www.minimax.io/brand-vi), which also asks not to separate or alter its logo, and Google's [brand terms](https://partnermarketinghub.withgoogle.com/brands/google/trademarks-and-terms/terms-and-conditions/), which license its brand features only where Google approves a request. Kaeso shows their marks anyway, only to identify the provider, in the owner's shapes and colours, a colour adjusted only as far as it needs to stay visible as listed under [Provider marks](#provider-marks). Mistral's [brand page](https://mistral.ai/brand) asks not to recolour its icon, not to place it on a coloured background and not to frame it; Kaeso shows the gradient icon in its own colours on the same neutral tile as every other mark. An owner who wants its mark taken out can [open an issue](https://github.com/logicplanes/kaeso/issues); Kaeso then shows the provider's first letter instead, as it does for providers without a mark.

## Data

- `crates/kaeso-core/data/kpmr.json`: a snapshot of the catalogue of [KPMR](https://kpmr.kaeso.app), the Kaeso Provider & Model Registry, which is derived from [models.dev](https://models.dev) (`https://models.dev/api.json`), MIT licensed, © models.dev contributors, source at https://github.com/anomalyco/models.dev. Its provenance and revision are in [`crates/kaeso-core/data/README.md`](crates/kaeso-core/data/README.md).

## Fonts and icons

- [Inter](https://rsms.me/inter/), SIL Open Font License 1.1, and [Geist Mono](https://vercel.com/font), SIL Open Font License 1.1: unmodified subsets of their Fontsource builds, self-hosted by `@logicplanes/kaeso-ui` and shipped with their license files (`packages/ui/src/styles/fonts/`).
- [Hugeicons](https://hugeicons.com) free, Stroke Rounded, MIT licensed: the icon set of every Kaeso interface.

## Provider marks

The provider marks in `packages/ui/src/components/kaeso/provider-marks.ts` are their owners' trademarks. Kaeso shows them only to identify which provider a key, model or request belongs to (nominative use), and implies no affiliation or endorsement. Each comes from the owner's own vector, unchanged except for uniform scaling and translation onto a 24px grid, in the owner's colours (`providerMark` in `packages/tokens/src/tokens.ts`) or, for a mark its owner shows in black or white, the theme's text colour. Where an owner's colour falls below 3:1 against white or against the mark's dark tile (`#141414`), Kaeso darkens it on light or lightens it on dark only as far as 3:1, keeping its hue; every such change is listed below.

- OpenAI: the Blossom from [openai.com](https://openai.com)'s icon, black on light and white on dark, following the [OpenAI design guidelines](https://openai.com/brand).
- Anthropic: the A mark from the logo on [anthropic.com](https://www.anthropic.com), in the text colour as on Anthropic's site, which draws its logo in the text colour; see its [trademark guidelines](https://www.anthropic.com/legal/trademark-guidelines).
- DeepSeek: the whale from the logo on [deepseek.com](https://www.deepseek.com), in DeepSeek's blue: `#4D6BFE` on light, from the logo in the [DeepSeek-V2 repository](https://github.com/deepseek-ai/DeepSeek-V2/blob/main/figures/logo.svg) and deepseek.com's brand colour, and `#6799FE` on dark, deepseek.com's brand colour in its dark theme.
- SpaceX: the mark of [x.ai](https://x.ai), cropped to a square as in xAI's own app and console icons, black on light and white on dark as in those icons; see the [brand guidelines](https://x.ai/legal/brand-guidelines).
- Together AI: the three-part logo from the Color+Black and Color+White SVGs of the logo suite (`together-ai-logo-suite.zip`, logo dated 021026) on the [Together AI brand assets](https://www.together.ai/brand), each part in its colour: magenta `#EF2CC1` and orange `#FC4C02` on light and dark, lavender `#CAAEF5` on dark and darkened to `#A488CD` on light.
- Groq: the bolt from [GroqCloud](https://console.groq.com)'s logo and favicon, in Groq's orange `#F43E01` (`--color-brand` on [groq.com](https://groq.com) and its favicon) on light and dark; see the [trademark policy](https://groq.com/trademark-policy).
- OpenRouter: the Ink glyph (`#03080A`) on light and the Cloud glyph (`#FCFCFE`) on dark from the [OpenRouter brand assets](https://openrouter.ai/brand).
- Mistral: the gradient icon from the [Mistral AI brand assets](https://mistral.ai/brand), whose page asks to prefer it, as the page draws it (its five rows in `#FFAF01`, `#FF8204`, `#FA500F`, `#E61300` and `#C4001D`), unchanged on light and dark because the page asks not to recolour it.
- Vercel: the black and white Vercel Symbol from the [Vercel brand assets](https://vercel.com/geist/brands), black on light and white on dark, as the brand page asks.
- Cohere: the colour symbol (`cohere_symbol_colour_rgb.svg`) from the press kit on Cohere's [newsroom](https://cohere.com/newsroom), each of its three shapes in its colour: green `#355146` on light, lightened to `#4B685C` on dark; lilac `#D18EE2` and coral `#FF7759` on dark, darkened to `#BE7CCF` and `#F26B4E` on light.
- Ollama Cloud: the llama from Ollama's logo, as [Simple Icons](https://simpleicons.org) 16.36.0 draws it from the [mark Ollama published](https://github.com/ollama/ollama/issues/2152#issuecomment-1905286922), in the text colour as on [ollama.com](https://ollama.com).
- Moonshot: the K of Kimi, Moonshot AI's models, as [Simple Icons](https://simpleicons.org) 16.36.0 draws it, coloured as the K-only SVGs of the [Kimi brand guidelines](https://moonshotai.github.io/Branding-Guide) colour it: the K in the text colour (black on light, white on dark) and the dot in Kimi blue `#1783FF` on both.
- QwenCloud: the symbol from QwenCloud's logo on [docs.qwencloud.com](https://docs.qwencloud.com) (`/logo/light.svg`), its six shapes scaled to the 24-unit square, each with the logo's own violet-to-pink gradient from `#4F21FF` to `#D75BFE` across that shape; on dark the violet is lightened to `#543AFF`, where QwenCloud's own dark logo (`/logo/dark.svg`) is white.
- Cerebras: the C mark, `Cerebras C logo.svg` from the logo download (`Cerebras Logos`) on Cerebras's [press kit](https://www.cerebras.ai/company/press-kit), moved to the middle of the 24px grid: its four arcs in Cerebras orange `#F05A28` on light and dark, and the inner c in the text colour, black on light and white on dark, as Cerebras draws the lettering in its logos for white and for black backgrounds.
- Z.ai: the Z of Z.ai's mark, the icon [z.ai](https://z.ai) serves (`https://z-cdn.chatglm.cn/z-ai/static/logo.svg`), without the dark tile it sits on there and scaled to the width of the 24px grid, in the text colour: black on light and white on dark, as the mark is one colour. Z.ai publishes no brand page.
- Google AI Studio: the AI Studio product icon that [aistudio.google.com](https://aistudio.google.com) serves (`https://www.gstatic.com/images/branding/productlogos/ai_studio/v1/192px.svg`), its two shapes scaled to the 24px grid. That SVG fills the shapes with raster gradients, so each shape gets a linear gradient through the colours sampled from it: green `#00B95C` to blue `#3186FF` on the upper shape, and blue to yellow `#FBBC04` to red `#FC413D` on the lower one. On light the green is darkened to `#00AB55` and the yellow to `#BD8D03` to stand out from white.
- Gemini Enterprise Agent Platform: the Gemini product icon that the platform's documentation shows as the product's logo on [docs.cloud.google.com](https://docs.cloud.google.com/gemini-enterprise-agent-platform) (`https://www.gstatic.com/images/branding/productlogos/gemini_2025/v1/192px.svg`); Google Cloud's [icon library](https://cloud.google.com/icons) offers no icon under the platform's current name. Its four-pointed shape is scaled to the 24px grid and filled in blue `#3285FF`, the colour sampled at its centre; that SVG fills the shape with a raster gradient, so the upper, lower and left points each get a linear gradient from that blue to the colour sampled at the point: red `#FA423F`, green `#14BA64` and yellow `#FAC70B`. On light the green is darkened to `#12AA5B` and the yellow to `#B59007` to stand out from white.
- MiniMax: the symbol from MiniMax's logo, as the SVG in the header of [MiniMax's documentation](https://platform.minimax.io/docs) draws it (`logo/light.svg`), without its wordmark and scaled to the width of the 24px grid, with the logo's own gradient from magenta `#E21680` to orange `#FF633A`; on light the orange is darkened to `#FF6036` to stand out from white. MiniMax's [brand page](https://www.minimax.io/brand-vi) offers the logo only as PNG and Illustrator files.

Groq is a trademark of Groq LLC and/or its affiliates. Google AI Studio, Gemini, Gemini Enterprise Agent Platform, Google Cloud, the AI Studio icon and the Gemini icon are trademarks or distinctive brand features of Google LLC. See [Trademarks](#trademarks) for the providers whose terms restrict the use of their logos.

## Client marks

The client marks in `packages/ui/src/components/kaeso/client-button.tsx` are their owners' trademarks: Claude of Anthropic, Cursor of Anysphere, and GitHub Copilot of GitHub. Kaeso shows them only to say which client a set of install steps is for (nominative use), and implies no affiliation or endorsement. Each is the [Simple Icons](https://simpleicons.org) vector (CC0 1.0) of that mark, unchanged, in the theme's text colour.

## Wire formats

KPP implements the OpenAI and Anthropic wire formats from the vendors' public documentation and specifications:

- the OpenAI OpenAPI specification (MIT), https://github.com/openai/openai-openapi;
- the Anthropic API documentation, https://docs.anthropic.com;
- the Gemini API documentation and its discovery document, https://ai.google.dev/api;
- payload shapes cross-checked against the official SDKs, https://github.com/openai/openai-python (Apache-2.0), https://github.com/anthropics/anthropic-sdk-python (MIT) and https://github.com/googleapis/python-genai (Apache-2.0).

KPP's design draws lessons from these projects; no code from them is included:

- TensorZero (Apache-2.0): `Unknown` content blocks, provider-tagged reasoning, an end-to-end provider matrix.
- LiteLLM Rust core (MIT): flattened `extra` maps on wire structs, declining unsupported requests before sending them.
- Vercel AI SDK provider specification: content parts, stream lifecycle events, provider options.
