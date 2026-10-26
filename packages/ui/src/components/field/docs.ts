import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Field: {
    description:
      "The aria wiring a control needs, whether it sits in a <Field> or wraps itself in one through its label/description/error props. In the second case the ids are derived here exactly as Field derives them, so the label, help and error name the control.",
    props: {
      description: "One sentence of help under the label.",
      error:
        "The error message, shown under the control in the danger color: it marks the field invalid and is read out with the description. After a failed submit, move focus to the first invalid field so its error is heard.",
      id: "The id of the control inside, so the label points to it.",
      invalid: "Marks the field invalid without a message.",
      label: "The field's label.",
      required: "Marks the field as required.",
    },
  },
  Fieldset: {
    description: "A group of fields under one legend, which names the group for screen readers.",
    props: {
      description: "One sentence of help under the heading; it also describes the group.",
      legend: "The heading of the group of fields, read out as the group's name.",
    },
  },
} satisfies ComponentDocs;
