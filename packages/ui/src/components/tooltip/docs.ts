import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Tooltip: {
    description: "A short label for a control, shown on hover and focus. Never on disabled elements without a reason.",
    props: {
      children: "The trigger: a single element that accepts a ref.",
      content: "What the tooltip says. Keep it to the control's name or one short sentence.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      delay: "How long the pointer has to rest before it opens, in milliseconds.",
      disabled: "Never shows the tooltip.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      shape: "A pill for a short label, or a box for several lines.",
      shortcut: 'A shortcut shown after the text: `["mod", "k"]` or `[["g"], ["m"]]`.',
      side: "The side it opens on.",
      sideOffset: "Distance from the trigger in px.",
    },
  },
  TooltipProvider: {
    description: "Tooltips open after 500ms, and instantly when moving between tooltipped siblings.",
    props: {
      children: "The app, so every tooltip shares one delay.",
      delay: "How long the pointer has to rest before the first tooltip opens, in milliseconds.",
    },
  },
  TooltipSurface: {
    description: "A static tooltip surface for documentation and state matrices.",
    props: {
      children: "The tooltip's text.",
      shape: "A pill for a short label, or a box for several lines.",
      shortcut: "The keyboard shortcut shown after the text.",
    },
  },
} satisfies ComponentDocs;
