# Shape

Kaeso Vanguard: pills for everything on one line, cut plates for everything that holds something. A container's corners are barely rounded (4 to 16px), so a pill reads as a control and a plate as a surface at a glance. Values: `../ai/tokens.md`.

## Radii

| Utility                                   | Use                                                                            |
| ----------------------------------------- | ------------------------------------------------------------------------------ |
| `rounded-pill`                            | Every single-line control: buttons, inputs, selects, tabs, tags, sidebar items |
| `rounded-panel`                           | The content panel of the app frame                                             |
| `rounded-menu`                            | Menus and listbox popups                                                       |
| `rounded-item`                            | Rows inside menus and lists (menu radius minus its 6px inset)                  |
| `rounded-popover`                         | Popovers and hover cards                                                       |
| `rounded-dialog`                          | Dialogs and the command palette                                                |
| `rounded-sheet`                           | Sheets, on the side that faces the content                                     |
| `rounded-group`                           | Settings groups                                                                |
| `rounded-well`                            | Code wells, preview frames and image thumbnails                                |
| `rounded-tooltip`                         | Tooltips that wrap; a single-line tooltip is a pill                            |
| `rounded-palette`, `rounded-palette-item` | The command palette and its rows                                               |
| `rounded-checkbox`                        | Checkboxes                                                                     |
| `rounded-org`                             | Organization avatars                                                           |
| `rounded-kbd`                             | Key caps                                                                       |
| `rounded-code`                            | Inline code                                                                    |
| `rounded-bar`, `rounded-cell`             | Bars in charts, heatmap cells                                                  |
| `rounded-avatar`                          | Small square marks                                                             |
| `rounded-field`                           | Token fields that wrap                                                         |
| `rounded-textarea`                        | Textareas                                                                      |
| `rounded-cell-lg`                         | One-time code cells                                                            |
| `rounded-tooltip-lg`                      | Not used by tooltips any more; use `rounded-tooltip`                           |
| `rounded-frame`                           | An app frame embedded in a page                                                |
| `rounded-toast`                           | A toast that wraps                                                             |

## Concentric corners

An inner corner is the outer corner minus the inset, so curves stay parallel. A 10px menu with a 6px inset holds 4px items; a 16px frame with an 8px inset holds an 8px panel.

## Rules

- A pill is a perfect stadium: its radius is at least half its height.
- Nothing mixes shapes: a square button never sits next to a pill.
- No decorative cards. Content sits on the panel, divided by hairlines; only the surfaces above it, and the flat plates that hold a figure, code or a picture, have corners of their own.
- Plates are cut, not soft: never a container corner above 16px.
