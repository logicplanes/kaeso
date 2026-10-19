# Layout

One edge per page. The components bring their own insets, so a page composed of them lines up without a margin of its own. Values: `../ai/tokens.md`.

## The gutter

| Where                                         | Gutter                                                                                   |
| --------------------------------------------- | ---------------------------------------------------------------------------------------- |
| The content panel, at every width             | 24px: page header, toolbar, page body, lists, tables, dialogs, sheets and the phone bar  |
| The sidebar                                   | 16px; its items pad their content 12px, so icons and the wordmark sit 28px in            |
| Settings groups, inset lists and inset tables | 20px inside the group                                                                    |
| Menus, selects and the command palette        | Their own inset (6px), with rows padded 10px; a popup is a surface, not part of the page |

## Boxes sit on the gutter

Every box starts and ends on the gutter. A box is anything whose edge you can see, at rest or under the pointer: a button, an input, a chip, a segmented track, a pill tab (the selected one is filled), the hover and selected fill of a list row or a table row, the hover fill of a group header or a disclosure, a settings group. Nothing bleeds into the gutter: it stays empty at every width, no fill touches the panel's edge, and no focus ring is cut off by a scroller.

Text inside a box is padded by the box: 12px in list rows, table cells, pill tabs, chips, group headers and disclosures. A page so has two vertical lines and no others:

- **The gutter**, where every box and every piece of plain text starts: the page title, headings, labels, descriptions, chart axes.
- **The gutter plus 12px**, where the text inside boxes starts: a row's title, a tab's label, a column's values.

The same holds at the right edge: the last box ends on the gutter, the last column's text 12px inside it.

## Exceptions

- **Text in running text.** A link or an inline copyable value inside a sentence keeps its 2 to 4px of hit padding around the text. It is part of the line, not a box.
- **Controls inside a box on the gutter.** Inside a box that already sits on the gutter, such as a table's header cell, a settings group's header or a banner, a control may use that box's own padding and keep its label on the box's text line: a column's sort button keeps its label on the column's edge.
- **Pane dividers.** Hairlines that split the panel into panes, such as the playground's columns, run from edge to edge; each pane keeps the 24px gutter inside it.
- **The wordmark** sits 28px in and 28px down in the sidebar and on sign-in screens, on the sidebar's content line. On the phone bar it sits on the page's 24px gutter, like everything under it.

## Vertical rhythm

- The page header is 52px and the toolbar under it 48px; neither draws a line.
- A list or table starts 8px under the toolbar (`AppFrame.Body padding="list"`); a content page 16px under the header (`padding="page"`).
- List rows are 44px (36px compact, 56px with a second line); table rows 40px; group headers 36px, with 16px above every one after the first.
- A row's height never depends on its content: badges, pills and avatars fit inside it.

## Numbers and truncation

- Numeric columns are right-aligned and use tabular figures, so digits line up from row to row.
- A column is wide enough for its header in every language, and for the values people compare: a provider's name, a status, a cost. Truncate only what is long by nature (ids, messages, user agents), and show the whole value on hover.
- On a phone, drop columns before truncating the ones that stay.
