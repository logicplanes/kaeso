# Color

A monochrome interface on pure white and pure black. Color appears only in data, in small status marks next to a word, and in highlighted code. Values: `../ai/tokens.md`.

## Neutrals

Two tint-free ramps. Components use the role names, never a step, so both themes are designed rather than inverted.

| Role             | Use                                                                |
| ---------------- | ------------------------------------------------------------------ |
| `bg-panel`       | The brightest surface: pure white, pure black. Content lives here. |
| `bg-chrome`      | The window behind the panel. The sidebar sits on it.               |
| `bg-raised`      | Controls, menus, popovers: a surface above the panel.              |
| `raised-hover`   | A raised control under the pointer: secondary buttons, list items. |
| `raised-pressed` | A raised control while it is pressed.                              |
| `fill-1`         | Quiet fills: inputs at rest, code wells, group headers.            |
| `fill-2`         | Hover on ghost controls and rows.                                  |
| `fill-3`         | Pressed and current: the active sidebar pill.                      |
| `line-subtle`    | Separators inside groups.                                          |
| `line`           | Rings around controls and groups.                                  |
| `line-strong`    | Hovered rings, table header rules.                                 |
| `switch-thumb`   | An off switch's thumb: white in light, light grey in dark.         |
| `text`           | Titles, labels, values.                                            |
| `text-secondary` | Descriptions, metadata, icons at rest.                             |
| `text-tertiary`  | Placeholders, counts, axis labels.                                 |
| `text-disabled`  | Disabled labels.                                                   |
| `ink`            | The primary button and selected marks.                             |
| `ink-hover`      | The primary button under the pointer.                              |
| `ink-pressed`    | The primary button while pressed, or while its menu is open.       |
| `focus`          | Keyboard focus rings.                                              |

## Status

Success, warning, danger and info are marks, never surfaces: a colored word, an icon, the tint behind a banner, the destructive button. Each has a solid, a `-text` and a `-tint` variable. A status is always readable without its color. The danger fill is `#dc2626` in both themes, so white text on it passes AA.

## Code

Highlighted code has its own palette, used only inside `CodeBlock`, `DiffView`, `JsonView` and the code blocks of `Markdown`: calm, deep colours in light mode and soft ones in dark, each at least 4.5:1 on the code well, on a highlighted line and on a diff line. Components never colour interface text with it.

| Role          | Colours                                                                     |
| ------------- | --------------------------------------------------------------------------- |
| `keyword`     | `const`, `def`, `if`, `return`, SQL verbs, Dockerfile instructions          |
| `string`      | Strings, template and f-strings, heredocs, YAML and `.env` values           |
| `constant`    | Numbers, `true`, `false`, `null`, escapes, CSS units, command-line flags    |
| `comment`     | Comments, in italic                                                         |
| `function`    | Function and method names, shell commands, Markdown headings and links      |
| `type`        | Types, classes, namespaces, INI and TOML sections                           |
| `property`    | Object, JSON and YAML keys, TOML, INI and `.env` keys, HTTP headers, fields |
| `tag`         | HTML, XML and JSX tags                                                      |
| `attribute`   | Attributes, CSS selectors, decorators, annotations, preprocessor lines      |
| `variable`    | Shell, PHP, Ruby, PowerShell, SCSS and GraphQL variables                    |
| `punctuation` | Brackets, operators, separators, a shell session's prompt                   |
| `inserted`    | Added lines of a diff                                                       |
| `deleted`     | Removed lines of a diff                                                     |

Each role is a `--k-code-<role>` variable and a `text-code-<role>` class. In high-contrast mode comments and punctuation take the text colour.

## Data

Eight series in a fixed order, balanced in lightness so none shouts. Token kinds always take the same series: input blue, output emerald, cache read a soft blue edged in input's blue, cache write amber, reasoning violet. A soft tint is too faint to stand on the panel alone, so it always carries a 1px edge in its series colour. Partial periods draw faint with a dashed edge, on bars and lines alike. Marks thinner than 2px use a darker shade in light mode to keep 3:1.

## Rules

- The interface is monochrome. A color that is not data, status or highlighted code is a bug.
- A status mark sits next to a word or an icon; color alone never carries meaning.
- Dark mode is designed on its own: the panel is the darkest surface, controls are lifted with a ring and a faint top light, and weights render twenty units lighter.
- Lines are rings drawn with box-shadow, one device pixel, and rarer than space or tone.
