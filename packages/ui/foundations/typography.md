# Typography

Inter for the interface, Geist Mono for ids, keys and code. Fourteen pixels is the base; nothing readable is smaller than twelve. Sizes, line heights and tracking: `../ai/tokens.md`.

## Scale

Ten steps, each with its own line height and tracking. Larger sizes tighten; small sizes open up slightly.

| Utility         | Use                                       |
| --------------- | ----------------------------------------- |
| `text-micro`    | Key caps, tiny counts                     |
| `text-caption`  | Group labels, column headers, field hints |
| `text-small`    | Descriptions, secondary cells, metadata   |
| `text-body`     | The base: labels, rows, controls          |
| `text-body-lg`  | Lead sentences, settings titles           |
| `text-title-sm` | Section titles                            |
| `text-title`    | Dialog and sheet titles                   |
| `text-title-lg` | Page titles                               |
| `text-display`  | The home page only                        |
| `text-metric`   | Big numbers, tabular                      |

## Families

Inter for everything people read: "Request logs, keys and usage for every model you run." Geist Mono (`mono`) for what people copy or compare character by character: `req_01JAR2N7`, `sk-kso-AbC123…wxyz`.

## Numbers

Numbers that align in columns use tabular figures (`tabular`). Money keeps its exact decimal string from the API and is formatted only for display; token counts use thin grouping; durations pick their unit (`840 ms`, `2.4 s`). Use the formatters in `@logicplanes/kaeso-ui/format`.

## Rules

- Three weights: 400, 500 and 600. No faux bold, no light weights.
- Dark mode renders 500 and 600 twenty units lighter, so both themes look equally heavy.
- Sentence case everywhere, including buttons and titles.
- Never a serif. Every control and portal inherits the interface family.
