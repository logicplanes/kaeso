import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ThemeMenu: {
    description: "System, light or dark, stored as `kaeso:theme`.",
    props: {
      display:
        "One circle that opens the choices (`menu`), or the three choices side by side as icons with the current one raised (`segmented`), for the foot of a sidebar where one tap changes the theme.",
      size: "The button's or the track's height.",
      side: "Where the menu opens: below the button (`bottom`), or above it (`top`) when the button sits at the foot of a sidebar.",
      align: "Which edge of the button the menu lines up with: its end (`end`) or its start (`start`), for a button at the left edge.",
      variant: "A filled button (`secondary`) or a quiet one (`ghost`).",
    },
  },
} satisfies ComponentDocs;
