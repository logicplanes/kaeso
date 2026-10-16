import type { ComponentDocs } from "../../component-docs";

export const docs = {
  GroupHeader: {
    description:
      "A quiet caption row above its group: title, count and a disclosure chevron, set in from the edges like the rows. No band: space separates the groups, and the + circle appears with hover or focus.",
    props: {
      addLabel: "The + circle's name. Defaults to \"Add\" in the provider's language.",
      collapsed: "Whether the group is collapsed.",
      count: "How many items the group has.",
      first: "The list's first row (no hairline above it in a card).",
      glyph: "A mark before the name, such as a status dot.",
      indent: "In a selectable list, line the title up with the row titles after the checkbox column.",
      onAdd: "Shows an Add button and is called when it is pressed.",
      onToggle: "Called when the group is collapsed or expanded.",
      title: "The group's name.",
    },
  },
  List: {
    description:
      "The console list: 44px rows on the panel, no lines, metadata on the right, keyboard-first. An ARIA grid of one-cell rows, so rows may hold actions.",
    props: {
      "aria-label": "The list's accessible name.",
      "data-force-row": "Force a state on the first row for state matrices: hover, focus, selected.",
      density: "How much room each row gets.",
      empty:
        'Shown when there are no rows and the list isn\'t loading: usually an `Empty` (`size="md"` in a settings column, where it sits inside the list\'s card; `size="compact"` on an edge-to-edge page).',
      groups: "…rows under group headers.",
      idWidth: "A fixed width for the id column when ids differ in length, so titles align. By default the id takes its own width.",
      items: "Rows without groups, or…",
      loading: "Loading rows shaped like items, while the rows aren't known yet.",
      onOpen: "Called when a row is opened with a click or Enter.",
      onSelectedChange: "Called with the selected rows; rows then get checkboxes.",
      selectable: "Selection: checkboxes appear on hover, and on every row once something is selected.",
      selected: "The ids of the selected rows.",
      selectionActions: "Actions in the floating selection bar.",
      selectionBar: "The floating selection bar; off where the host shows the count itself, as in a dialog.",
      virtualizeAbove: "Virtualize above this many rows (100).",
    },
  },
  ListRow: {
    description: "One row: checkbox, glyph, id, title, subtitle, metadata, date, avatar, hover actions.",
    props: {
      active: "Marks the row that has keyboard focus.",
      density: "How much room the row gets.",
      depth: "Indents a child row under its parent.",
      idWidth: "The width of the id column in pixels, so ids line up.",
      index: "The row's position, for keyboard navigation.",
      item: "The row's data.",
      lastChild: "Marks the last child of a parent, for the connecting line.",
      narrow: "The list is narrow (a phone): the title, the subtitle and the metadata with the date are lines of their own.",
      onFocus: "Called when the row gets focus.",
      onOpen: "Called when the row is opened.",
      onSelect: "Called when the row is selected or unselected.",
      selectable: "Shows the row's checkbox.",
      selected: "Marks the row as selected.",
      selecting: "Shows checkboxes on every row while some are selected.",
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
