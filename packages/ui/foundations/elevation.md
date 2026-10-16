# Elevation

Few levels, each with one job. Lines are rings drawn with shadows; depth comes from tone first and shadow second. Values: `../ai/tokens.md`.

## Levels

| Utility          | Use                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------- |
| `shadow-ring`    | A hairline inside the edge: badges, tags, key caps, provider tiles.                   |
| `shadow-frame`   | A hairline outside the edge, for content that reaches it: previews, embedded screens. |
| `shadow-control` | Secondary buttons, inputs, selects: a ring and a shadow you feel more than see.       |
| `shadow-segment` | The raised item inside a segmented track.                                             |
| `shadow-panel`   | The content panel on the chrome.                                                      |
| `shadow-raised`  | Menus, popovers, tooltips.                                                            |
| `shadow-dialog`  | Dialogs, sheets, the command palette.                                                 |
| `shadow-toast`   | The toast capsule.                                                                    |

## Rules

- Never a CSS border on a control: rings are box-shadows, so they never change the box size.
- A hairline is the same on all four sides. Controls and small marks draw it inside their own edge, so a scroller or a truncating table cell never cuts a side off; cards, tables and overlays, whose content reaches their edge, draw it outside, opaque and above their shadow. No top light: a highlight along one edge reads as that side drawn thicker.
- In light mode, shadows carry the lift. In dark mode they disappear into black, so a lighter ring does it instead.
- One overlay level at a time. A menu inside a dialog uses the raised level, not a deeper one.
