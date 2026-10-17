import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ThemeSelect: {
    description: "A select for system, light or dark, stored as `kaeso:theme`, with its label in the provider's language.",
    props: {
      appearance: "A quiet pill for sidebars and toolbars (`ghost`) or a raised one with a ring (`default`).",
      fullWidth: "Width of the trigger follows its content (default) or fills the container, as in a sidebar's footer.",
      size: "The height of the field.",
    },
  },
} satisfies ComponentDocs;
