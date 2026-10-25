import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ThemeMenu: {
    description: "System, light or dark, stored as `kaeso:theme`.",
    props: {
      size: "The button's height.",
      side: "Where the menu opens: below the button (`bottom`), or above it (`top`) when the button sits at the foot of a sidebar.",
      align: "Which edge of the button the menu lines up with: its end (`end`) or its start (`start`), for a button at the left edge.",
      variant: "A filled button (`secondary`) or a quiet one (`ghost`).",
    },
  },
} satisfies ComponentDocs;
