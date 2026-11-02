import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  SwapText: {
    description: "Text that changes with state without changing size: all alternatives sit in one grid cell, only the current one is visible.",
    props: {
      align: "Aligns the shorter labels at the start or in the center of the reserved width.",
      children: "What is shown now.",
      reserve: 'Every text this place can show ("Copy page", "Copied"). The width fits the widest, so a state change never resizes the control.',
      wrap: "Lets longer texts, such as a field's hint, wrap within their column; the height then fits the tallest, so nothing below moves.",
    },
  },
} satisfies ComponentDocs;
