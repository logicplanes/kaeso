# Accessibility

Every component works with a keyboard, a screen reader, a coarse pointer and the system's contrast and motion settings. Keep it that way when you compose them.

## Foundations

- Behavior comes from Base UI: roles, states, focus management and keyboard interaction follow the WAI-ARIA practices.
- **Targets**: nothing interactive is smaller than 32px; a control drawn smaller (a 24px chip action) still takes a 32px hit area (`tap-target`). With a coarse pointer, hit areas grow to 44px and rows and menu items grow to 44px tall.
- **Text**: 14px base, 12px minimum. Every text and background pair is measured against WCAG 2.2 (4.5:1 for text, 3:1 for large text and essential graphics).
- **Zoom stays with the person**: with a coarse pointer, every field's text is at least 16px, so a phone never zooms the page when a field takes focus, and a quick second tap on a control is a tap, not a zoom. Pinch zoom always works; never set `maximum-scale` or `user-scalable=no`.
- **Color is never alone**: every status carries a word or an icon.

## Focus

Focus rings are 2px, follow the control's shape, and appear for keyboard focus only, so a clicked control never looks stuck. Focus never changes a control's size. Dialogs and sheets trap focus and return it to their trigger when they close. A dialog opens on its first field; without one, on its primary action, or on the safe action (Cancel, Back) when the primary one destroys; never on the dialog box. A collection is one tab stop: arrows move inside it, and Tab leaves it. Every app frame starts with a skip link to its content.

## Keyboard

Each component's keys are in the registry and on its page. Across the library:

| Key            | Action                                                                    |
| -------------- | ------------------------------------------------------------------------- |
| Tab, Shift Tab | Move between controls                                                     |
| Arrow keys     | Move within a group: menu items, tabs, segments, list rows, toolbar items |
| Enter, Space   | Activate                                                                  |
| Escape         | Close the overlay, clear the selection                                    |
| ⌘K             | Command palette                                                           |
| [              | Collapse or expand the sidebar                                            |

## Screen readers

Icon-only buttons require an accessible name; their tooltip repeats it. A control without a visible label takes `aria-label`. Toasts are status messages, errors are alerts. Loading regions are marked busy. Charts have a text summary and a table view.

## Preferences

- `prefers-reduced-motion`: transforms are removed; only opacity changes.
- `prefers-contrast: more`: rings use the strong line and secondary text becomes primary.
- `prefers-color-scheme`: the system theme is the default.
- `forced-colors: active`: rings drawn as shadows disappear, so every control and raised surface gets a 1px system-colour outline instead, keyboard focus a 2px `Highlight` outline, selected and current items a 2px `Highlight` outline, and highlighted rows a `Highlight` fill. Data colours that carry meaning keep their colour (`data-mark`); lines drawn as fills use `forced-line`.

## Testing

axe runs on every showcase page and open overlay with zero violations allowed, and keyboard walks assert that focus moves and returns correctly (`pnpm audit:all`).
