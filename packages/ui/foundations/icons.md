# Icons

Hugeicons, Stroke Rounded, one icon per Kaeso concept. The same idea always looks the same. Every name: `../ai/icons.md`.

## Vocabulary

Use the names, not the icons: `icons.key` always means a key, `icons.model` a model. Import `icons` from `@logicplanes/kaeso-ui` and pass one to a component's `icon`, `leadingIcon` or `trailingIcon`, or to `<Icon icon={icons.key} size="md" />`. Sizes: `xs`, `sm`, `md`, `lg`, `xl`.

## Rules

- An icon accompanies a word or names a button that has an accessible label. Decorative icons are hidden from assistive technology.
- No decorative icons: empty states, dialogs and badges carry words, not symbols.
- Icons sit quieter than their label (text secondary) and take the label's color when active.
- Stroke icons next to text are centered on the cap height, not the box.
- Every icon draws a 1.5px stroke at every size, the weight of the medium text beside it: a hairline beside a label reads as a different, lighter thing.
- An icon that changes with state, as Copy turns to Copied, cross-fades: the old one shrinks to a quarter and blurs out while the new one grows in, in `--k-duration-swap`; with reduced motion only the opacity changes. `Icon` does it whenever its `icon` changes after the first render.
