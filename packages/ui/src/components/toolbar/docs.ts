import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Toolbar: {
    description: "A row of controls above a list or a chart.",
    props: {
      "aria-label": "Defines a string value that labels the current element.",
      size: "The bar's height: the default, or inset in a panel.",
    },
  },
} satisfies ComponentDocs;
