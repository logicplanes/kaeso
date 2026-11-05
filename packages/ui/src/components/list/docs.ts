import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  List: {
    description:
      "The console list: 44px rows on the panel (48px in a card, 56px with a second line), no lines, metadata on the right, keyboard-first. An ARIA grid of one-cell rows with one tab stop: arrows move between rows, Space selects, Enter opens, the right arrow reaches a row's actions and Escape comes back. A row's text can be selected and copied; a click that selects text doesn't open the row.",
    props: {
      "aria-label": "The list's accessible name.",
      "data-force-row": "Force a state on the first row for state matrices: hover, pressed, focus.",
      density: "How much room each row gets.",
      empty:
        'Shown when there are no rows and the list isn\'t loading: usually an `Empty` (`size="md"` in a settings column, where it sits inside the list\'s card; `size="compact"` on an edge-to-edge page).',
      groups: "…rows under group headers.",
      idWidth: "A fixed width for the id column when ids differ in length, so titles align. By default the id takes its own width.",
      items: "Rows without groups, or…",
      loading: "Loading rows shaped like items, under a group header's shape when the list has groups, while the rows aren't known yet.",
      onOpen: "Called when a row is opened with a click or Enter. Enter with ⌘ or Ctrl is left to the surrounding form.",
      onSelectedChange: "Called with the selected rows; rows then get checkboxes.",
      selectable: "Selection: checkboxes appear on hover, and on every row once something is selected.",
      selected: "The ids of the selected rows.",
      selectionActions: "Actions in the floating selection bar.",
      selectionBar:
        "The floating selection bar; off where the host shows the count itself, as in a dialog. Rows reached with the arrow keys scroll clear of it.",
      wrapSubtitle: "Lets a subtitle under its title wrap instead of ending with an ellipsis, for descriptions people read in full.",
      virtualizeAbove: "Virtualize above this many rows (100).",
    },
  },
  SelectionAction: {
    description: "An action pill for the selection bar, drawn like the toast's action. `danger` colors the label for destructive actions.",
    props: {
      children: "The action's label.",
      danger: "Marks a destructive action.",
      icon: "The action's icon.",
      onClick: "Called when the action is chosen.",
    },
  },
} satisfies ComponentDocs;
