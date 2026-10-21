import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  DisplayOptions: {
    description: "Display options: view, grouping and ordering, toggles, properties.",
    props: {
      footer: "Content at the bottom of the options, such as a reset button.",
      inline: "Render the panel inline (documentation and state matrices).",
      properties:
        "The properties shown for each item; `hidden` names those that are on but left out because they don't fit, such as table columns at a narrow width.",
      rows: "Rows of choices, such as grouping and ordering.",
      toggles: "Switches, such as showing archived items.",
      trigger: "The look of the button that opens the options.",
      view: "A choice between views, such as list and board.",
    },
  },
} satisfies ComponentDocs;
