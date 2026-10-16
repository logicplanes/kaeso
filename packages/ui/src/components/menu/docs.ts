import type { ComponentDocs } from "../../component-docs";

export const docs = {
  ContextMenu: {
    description: "A dropdown menu: 16px radius, 34px rows, typeahead, Home/End, loops.",
    props: {
      children: "The area that opens the menu on a right click or long press.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      modal: "Blocks the rest of the page while the menu is open.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
    },
  },
  Menubar: {
    description: "A row of menus, as in a desktop app's title bar.",
    props: {
      children: "The menus.",
      onOpenChange: "Called when a menu opens or closes.",
    },
  },
} satisfies ComponentDocs;
