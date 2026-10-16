import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ThemeMenu: {
    description: "System, light or dark, stored as `kaeso:theme`.",
    props: {
      size: "The button's height.",
      variant: "A filled button (`secondary`) or a quiet one (`ghost`).",
    },
  },
} satisfies ComponentDocs;
