import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Portal: {
    description: "Renders children into document.body after mount. Overlays use Base UI's own portals.",
    props: {
      children: "The content to render in the nearest portal container.",
      container: "Defaults to document.body.",
    },
  },
} satisfies ComponentDocs;
