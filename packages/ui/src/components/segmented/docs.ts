import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Segmented: {
    description:
      "Two to five mutually exclusive options: 24h/7d/30d, List/Board, providers. Options wider than the room scroll sideways, with the chosen one in view.",
    props: {
      "aria-label": "The control's accessible name.",
      defaultValue: "The chosen option on first render.",
      disabled: "Turns off every segment.",
      fullWidth: "Fills the width of its container.",
      onValueChange: "Called with the chosen option.",
      options: "The options, as segments. A segment with a label is named by it; an icon-only one needs its `aria-label`, also shown as its tooltip.",
      size: "The control's height.",
      value: "The chosen option, when you control it.",
    },
  },
} satisfies ComponentDocs;
