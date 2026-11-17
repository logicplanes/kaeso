import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ContextMenu: {
    description: "The same menu, opened by a right click, a long press or the context-menu key on the area it wraps.",
    props: {
      children:
        "The area that opens the menu on a right click or long press. When it holds nothing focusable, it becomes a focus stop, so Shift F10 and the context-menu key reach it; the focus returns to it when the menu closes.",
      onOpenChange: "Called when it opens or closes.",
    },
  },
  Menu: {
    description:
      "A dropdown menu: a 10px plate with 4px rows inset 6px, 34px rows that grow when a label wraps, hairlines between groups that run edge to edge, labels on one column (an item without an icon lines up with those that have one), typeahead, Home/End, loops. An item's shortcut and hint describe it rather than name it. Checkbox and radio items show a check when chosen; the group's label says whether it takes one or several.",
    props: {
      children: "The trigger and the menu's content.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      modal: "Blocks the rest of the page while the menu is open.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
    },
  },
} satisfies ComponentDocs;
