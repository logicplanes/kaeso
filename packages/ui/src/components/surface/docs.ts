import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Surface: {
    description: "One of the library's surfaces, for content that needs the look of a panel, popover, dialog or well.",
    props: {
      kind: "Which surface it looks like: panel, well, popover, menu, tooltip, dialog, sheet or group.",
    },
  },
} satisfies ComponentDocs;
