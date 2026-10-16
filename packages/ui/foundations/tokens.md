# Tokens

Every value in the library is a token. Components use role names through CSS variables and the Tailwind theme; raw values never appear in a component. Every value, light and dark: `../ai/tokens.md`.

## Using tokens

Import the stylesheet once. It defines the `--k-*` variables for light and dark, and a Tailwind theme in which only token values exist: `bg-panel`, `text-fg-secondary`, `h-36`, `rounded-pill`. Classes like `bg-blue-500` or `p-7` are not generated.

```css
@import "tailwindcss";
@import "@logicplanes/kaeso-ui/tailwind.css";
```

The values are also exported as JSON (`@logicplanes/kaeso-tokens/tokens.json`) and TypeScript (`@logicplanes/kaeso-tokens`).

## Lengths

Every length a component may use is on a fixed list; the class suffix is the pixel value (`h-36` is 36px). A length off the list does not exist. The list is at the end of `../ai/tokens.md`.

## Rules

- Use the role, never the step: `text-fg-secondary`, not a gray.
- No hex or rgb literals, no arbitrary Tailwind values or variants, no `dark:`: both themes come from the same role names.
- Change a token, not a component, when a value is wrong everywhere.
