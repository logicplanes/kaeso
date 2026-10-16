import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Switch: {
    description: "A pill track and a round thumb that travels in 180ms.",
    props: {
      "aria-label": "The accessible name, for when there is no visible label.",
      "aria-labelledby": "The id of the element that names it, when its label is elsewhere on the page.",
      checked: "Whether it is checked, when you control it.",
      defaultChecked: "Whether it starts checked, when the component keeps its own state.",
      description: "One sentence of help under the label.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      disabledReason: "Why it is disabled, shown as a tooltip; it stays focusable so people can read it.",
      id: "The id of the control, so a label elsewhere can point to it.",
      label: "The switch's label.",
      loading: "Shows that the change is being saved and ignores clicks until it is.",
      name: "The field name the value is sent under in a form.",
      onCheckedChange: "Called with the new state when it is checked or unchecked.",
      readOnly: "Shows the value without letting people change it.",
      size: "The switch's size.",
    },
  },
} satisfies ComponentDocs;
