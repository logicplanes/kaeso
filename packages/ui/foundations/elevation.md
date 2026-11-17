# Elevation

Kaeso Vanguard is flat. Structure is hairlines; depth comes from tone first, and only what floats above the page casts a shadow. Values: `../ai/tokens.md`.

## Levels

| Utility          | Use                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------- |
| `shadow-ring`    | A hairline inside the edge: badges, tags, key caps, provider tiles.                   |
| `shadow-frame`   | A hairline outside the edge, for content that reaches it: previews, embedded screens. |
| `shadow-control` | Secondary buttons, inputs, selects: a hairline ring, no drop shadow.                  |
| `shadow-segment` | The raised item inside a segmented track.                                             |
| `shadow-panel`   | A panel set apart on a band; the app frame itself is one ground split by a hairline.  |
| `shadow-raised`  | Menus, popovers, tooltips.                                                            |
| `shadow-dialog`  | Dialogs, sheets, the command palette.                                                 |
| `shadow-toast`   | The toast capsule.                                                                    |
| `image-outline`  | Pictures: 1px of pure black or white at 10% inside the edge, in the picture's shape.  |

## Rules

- Never a CSS border on a control: rings are box-shadows, so they never change the box size.
- A hairline is the same on all four sides. Controls and small marks draw it inside their own edge, so a scroller or a truncating table cell never cuts a side off; cards, tables and overlays, whose content reaches their edge, draw it outside, opaque and above their shadow. No top light: a highlight along one edge reads as that side drawn thicker.
- Nothing that sits on the page casts a shadow, and no fill has a gradient or a highlight: primary and danger buttons are flat ink and flat red.
- In light mode, one long, quiet shadow under a hairline carries an overlay's lift. In dark mode they disappear into black, so a lighter ring does it instead.
- A picture takes `image-outline`, never a ring: a grey line on a photo reads as dirt on its edge, while a translucent one deepens whatever colour the edge already has. Logos and transparent art take none.
- One overlay level at a time. A menu inside a dialog uses the raised level, not a deeper one.
- A modal sheet dims the page half as much as a dialog does, so the list it inspects stays readable behind it.
