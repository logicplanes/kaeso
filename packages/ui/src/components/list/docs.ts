import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  GroupHeader: {
    description:
      "A quiet caption row above its group: title, count and a disclosure chevron, set in from the edges like the rows. On the panel there is no band: 16px of space separates the groups. In a card the header is a `fill-1` band with a hairline, the card's way of separating groups. The + circle appears with hover or focus.",
    props: {
      active: "Holds the list's one tab stop: Tab reaches this header's toggle and no other row. Set by `List`.",
      addLabel: "The + circle's name. Defaults to \"Add\" in the provider's language.",
      collapsed: "Whether the group is collapsed.",
      count: "How many items the group has.",
      first: "The list's first row: no space above it on the panel, no hairline above it in a card.",
      glyph: "A mark before the name, such as a status dot.",
      indent: "In a selectable list, line the title up with the row titles after the checkbox column; the hover fill still starts on the gutter.",
      index: "The header's position in the list, for keyboard navigation.",
      onAdd: "Shows an Add button and is called when it is pressed.",
      onFocus: "Called when the header's toggle or its add button gets focus.",
      onToggle: "Called when the group is collapsed or expanded.",
      title: "The group's name.",
    },
  },
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
  ListRow: {
    description: "One row: checkbox, glyph, id, title, subtitle, metadata, date, avatar, hover actions.",
    props: {
      active: "Holds the list's one tab stop: Tab reaches this row and no other. Set by `List`.",
      density: "How much room the row gets.",
      depth: "Indents a child row under its parent.",
      idWidth: "The width of the id column in pixels, so ids line up.",
      index: "The row's position, for keyboard navigation.",
      item: "The row's data.",
      lastChild: "Marks the last child of a parent, for the connecting line.",
      narrow:
        "The list is narrow (a phone): the title and the subtitle are lines of their own, and the metadata, the date, the avatar and the actions share one line under them, so titles keep the row's width.",
      onFocus: "Called when the row gets focus.",
      onOpen: "Called when the row is opened.",
      onSelect: "Called when the row is selected or unselected.",
      selectable: "Shows the row's checkbox.",
      selected: "Marks the row as selected.",
      selecting: "Shows checkboxes on every row while some are selected.",
      wrapSubtitle: "Lets a subtitle under its title wrap. Set by `List`.",
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
  SelectionBar: {
    description:
      "The floating capsule with the selection count and its actions: the toast's material (an ink capsule on white, graphite on black), so the two bottom bars of the console read as one family.",
    props: {
      children: "The actions for the selected items.",
      count: "How many items are selected.",
      onClear: "Called when the selection is cleared.",
    },
  },
} satisfies ComponentDocs;
