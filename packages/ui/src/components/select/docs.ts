import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Select: {
    description: "A pill that opens a list on the menu surface: check on the selected item, groups, typeahead.",
    props: {
      appearance: "A filled field (`default`), a quiet one for toolbars (`ghost`), or a quiet one in full text colour for column headers (`header`).",
      "aria-label": "The accessible name, for when there is no visible label.",
      defaultValue: "The value on first render, when the component keeps its own value.",
      description: "One sentence of help under the label.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      fullWidth: "Width of the trigger follows its content (default) or fills the container.",
      icon: "Leading icon in the trigger.",
      id: "The id of the control, so a label elsewhere can point to it.",
      invalid: "Shows it as invalid; pair it with a message that says what to fix.",
      label: "The visible label.",
      layout: "The label above the control (`vertical`) or beside it (`horizontal`).",
      multiple: "Allows choosing more than one option; `value` is then a list.",
      name: "The field name the value is sent under in a form.",
      onOpenChange: "Called when the list opens or closes.",
      onValueChange: "Called with the chosen value, or the list of values with `multiple`.",
      open: "Whether the list is open, when you control it.",
      optional: "Adds “Optional” to the label, for forms where most fields are required.",
      options: "The options to choose from.",
      placeholder: 'Defaults to "Not set" in the provider\'s language.',
      required: "Marks it as required in a form.",
      size: "The height of the field.",
      value: "The chosen value, or a list of values with `multiple`, when you control it.",
      error: "The error message, shown under the control in the danger color; it marks the control invalid and is read out with its description.",
    },
  },
} satisfies ComponentDocs;
