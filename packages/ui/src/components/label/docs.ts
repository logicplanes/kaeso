import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Label: {
    description: "The label of a form control.",
    props: {
      children: "The label's text.",
      optional: 'Adds a quiet "Optional" after the label.',
      size: "The label's text size.",
    },
  },
} satisfies ComponentDocs;
