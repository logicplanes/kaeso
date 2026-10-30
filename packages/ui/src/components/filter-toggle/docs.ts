import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  FilterToggle: {
    description:
      "An on-or-off filter as a pill that sits in a row of selects: quiet like an unset select when off, a check and full text when on. Use it instead of a switch in filter rows and toolbars.",
    props: {
      label: "What the filter keeps, such as “Agents”.",
      pressed: "Whether the filter is on.",
      onPressedChange: "Called with the new state when it is pressed.",
      size: "Its height, matched to the selects beside it.",
    },
  },
} satisfies ComponentDocs;
