# Kaeso UI for agents

How an agent builds Kaeso screens with `@logicplanes/kaeso-ui`. Read this before writing interface code. It ships in the package (`node_modules/@logicplanes/kaeso-ui/AGENTS.md`) and is the page [ui.kaeso.app/for-agents](https://ui.kaeso.app/for-agents).

## Load it in your project

So your agent reads these rules every time it works on the interface:

- **Claude Code:** add this line to your project's `CLAUDE.md`:

  ```
  @node_modules/@logicplanes/kaeso-ui/AGENTS.md
  ```

- **Codex, Cursor and other agents that read `AGENTS.md`:** add this line to your project's `AGENTS.md`:

  ```
  Before writing interface code, read node_modules/@logicplanes/kaeso-ui/AGENTS.md and follow it.
  ```

## Foundations

The design system behind the components, in `node_modules/@logicplanes/kaeso-ui/foundations/`. Read the one that matches what you are doing:

| File                           | Read it when                                      |
| ------------------------------ | ------------------------------------------------- |
| `foundations/tokens.md`        | Writing any class: only token values exist        |
| `foundations/color.md`         | Choosing a surface, a text role or a status color |
| `foundations/typography.md`    | Setting text, numbers, ids or code                |
| `foundations/shape.md`         | Choosing a radius, nesting surfaces               |
| `foundations/elevation.md`     | Drawing a line, a ring or a raised surface        |
| `foundations/motion.md`        | Animating anything, showing loading               |
| `foundations/icons.md`         | Placing an icon                                   |
| `foundations/writing.md`       | Writing any label, message, empty state or error  |
| `foundations/accessibility.md` | Building anything interactive                     |
| `foundations/psychology.md`    | Reviewing a screen: the 27 perception rules       |

## Machine-readable

- `node_modules/@logicplanes/kaeso-ui/ai/component-registry.md`: every component, its import, props, types, defaults and variants, generated from the types. The same as JSON in `ai/component-registry.json`.
- `node_modules/@logicplanes/kaeso-ui/ai/component-guides.md`: for every component, what it is for, keyboard, accessibility, do and don't, and every example's code. Read a component's section before using it for the first time.
- `node_modules/@logicplanes/kaeso-ui/ai/tokens.md`: every token value, light and dark. `ai/icons.md`: the icon vocabulary.
- [ui.kaeso.app/llms.txt](https://ui.kaeso.app/llms.txt): these rules and every component, in plain text.
- [ui.kaeso.app/components.json](https://ui.kaeso.app/components.json): the registry, online.
- Every component page on ui.kaeso.app has **Copy page**, which copies it as Markdown.

## Setup

With Tailwind CSS v4, in the app's stylesheet:

```css
@import "tailwindcss";
@import "@logicplanes/kaeso-ui/tailwind.css";
```

Without Tailwind, import `@logicplanes/kaeso-ui/styles.css` once. Then wrap the app once:

```tsx
import { ThemeProvider, TooltipProvider, Toaster } from "@logicplanes/kaeso-ui";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <TooltipProvider>
        {children}
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>
  );
}
```

## The rules

1. **Compose, never design.** The console is built only from `@logicplanes/kaeso-ui`. If something is missing, it is added to the library, not drawn in the app.
2. **`className` is for layout only**: margin, width, grid placement, flex. Never color, radius, shadow, font or padding on a component; use its props and variants.
3. **Only token values exist.** `bg-panel`, `text-fg-secondary`, `h-36`, `rounded-pill`. There is no `bg-blue-500`, no `p-7`, no arbitrary values, no `dark:`.
4. **Wrap inputs in `Field`** (or give them `label`) for the label, hint and error. A bare input without a label or an `aria-label` is a bug.
5. **One primary button per view.** Everything else is secondary or ghost.
6. **No decorative cards and no decorative icons.** Content sits on the panel; empty states and dialogs carry words.
7. **Color only for data and status**, and a status always has a word.
8. **Use the formatters** from `@logicplanes/kaeso-ui/format` for money, tokens, durations and dates. Money stays an exact decimal string.
9. **Use the icon vocabulary**: `icons.key`, `icons.model`, never an icon picked by look.
10. **Copy follows the voice**: sentence case, a verb first on buttons, no exclamation points or emoji, errors that say what to do next.
11. **Leave room.** Let the panel breathe: the components bring their own spacing, so don't pack screens with extra wrappers, borders or headings. Fewer elements, each one clear.

## Which component for which job

| Job                                      | Use                                                             |
| ---------------------------------------- | --------------------------------------------------------------- |
| App layout                               | `AppFrame` with `Sidebar`, `PageHeader`, `AppFrame.Body`        |
| Settings page                            | `AppFrame.Settings`, `SettingsGroup`, `SettingsRow`             |
| A collection people scan and act on      | `List` with group headers                                       |
| A collection people compare by column    | `DataTable` (virtualized, sortable)                             |
| Filters above a collection               | `Toolbar` with `FilterChip`, `SearchInput`, `DisplayOptions`    |
| A choice among 2–4 views                 | `Segmented`; page sections: `PillTabs`; detail sections: `Tabs` |
| One of many options                      | `Select`; searchable: `Combobox`                                |
| A confirmation of an irreversible action | `AlertDialog`; deletion: the `DeleteResource` block             |
| A task in several steps                  | `StepDialog`                                                    |
| Feedback after an action                 | `toast()`, with Undo for reversible actions                     |
| A secret shown once                      | `SecretReveal`                                                  |
| Numbers at a glance                      | `MetricRow` with `Stat`; over time: `TimeseriesChart`           |
| Nothing to show yet                      | `Empty` with one action                                         |

## A screen

```tsx
import { AppFrame, Sidebar, PageHeader, Toolbar, FilterChip, SearchInput, DataTable, Button, icons } from "@logicplanes/kaeso-ui";

export function RequestLogs({ rows }: { rows: LogRow[] }) {
  return (
    <AppFrame brandHref="/" sidebar={<ConsoleSidebar current="logs" />}>
      <PageHeader start={<AppFrame.MenuButton />} title="Request logs" actions={<Button variant="secondary">Export CSV</Button>} />
      <Toolbar>
        <FilterChip property="Status" operator="is" value="2xx" />
        <SearchInput aria-label="Search logs" placeholder="Search logs" />
      </Toolbar>
      <AppFrame.Body>
        <DataTable data={rows} columns={columns} />
      </AppFrame.Body>
    </AppFrame>
  );
}
```
