import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Popover: {
    description:
      "A floating panel next to its trigger: radius 16, raised; collision-aware and never wider than the screen. Its content takes a title, or an aria-label, which names it.",
    props: {
      children: "The trigger and the panel's content.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      trapFocus: "Trap focus only when the popover holds a form.",
    },
  },
} satisfies ComponentDocs;
