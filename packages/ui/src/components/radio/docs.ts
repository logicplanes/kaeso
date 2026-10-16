import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Radio: {
    description: "A radio button with its label and description, inside a `RadioGroup`.",
    props: {
      "aria-describedby": "The ids of the elements that describe it.",
      "aria-label": "The accessible name, for when there is no visible label.",
      "aria-labelledby": "Names the circle by other text, such as the first line of a row whose label holds more.",
      description: "One sentence of help under the label.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      id: "The id of the control, so a label elsewhere can point to it.",
      label: "The visible label.",
      value: "The value this option stands for.",
    },
  },
  "Radio.Circle": {
    description: "The circle alone: 18px; selected is an ink donut around a panel center.",
    props: {
      "aria-describedby": "The ids of the elements that describe it.",
      "aria-label": "The accessible name, for when there is no visible label.",
      "aria-labelledby": "Names the circle by other text, such as the first line of a row whose label holds more.",
      description: "One sentence of help under the label.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      id: "The id of the control, so a label elsewhere can point to it.",
      label: "The visible label.",
      value: "The value this option stands for.",
    },
  },
  RadioCards: {
    description: "Choices with descriptions as rows in a settings group.",
    props: {
      "aria-label": "The accessible name, for when there is no visible label.",
      "aria-labelledby": "The id of the element that names it, when its label is elsewhere on the page.",
      children: "The cards, one per option.",
      defaultValue: "The chosen option on first render.",
      disabled: "Turns off every card.",
      name: "The field name the value is sent under in a form.",
      onValueChange: "Called with the chosen option.",
      orientation: "Lays the items out in a row (`horizontal`) or a column (`vertical`).",
      value: "The chosen option, when you control it.",
    },
  },
} satisfies ComponentDocs;
