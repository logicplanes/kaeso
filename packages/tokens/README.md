# @logicplanes/kaeso-tokens

Kaeso's design tokens: CSS variables, the Tailwind CSS v4 theme, TypeScript constants and JSON. Pure white and pure black canvases, a tint-free grey ramp, and color only for data and status. `@logicplanes/kaeso-ui` is built on them; use them directly where the components cannot go, such as emails or native code.

## Install

```bash
pnpm add @logicplanes/kaeso-tokens
```

| Import                                  | What it is                                                                                  |
| --------------------------------------- | ------------------------------------------------------------------------------------------- |
| `@logicplanes/kaeso-tokens/tokens.css`  | `--k-*` CSS variables, light on `:root`, dark by system preference or `[data-theme="dark"]` |
| `@logicplanes/kaeso-tokens/theme.css`   | The Tailwind v4 theme: only token values exist as utilities                                 |
| `@logicplanes/kaeso-tokens/tokens.json` | Every token, resolved per mode, for build tools                                             |
| `@logicplanes/kaeso-tokens`             | The same values as TypeScript constants                                                     |

How light and dark work: [ui.kaeso.app/theming](https://ui.kaeso.app/theming). `@logicplanes/kaeso-ui` ships every value as `ai/tokens.md` and the rules for using them in `foundations/tokens.md`.

## License

MIT. The Kaeso name and logo are not covered by the license.
