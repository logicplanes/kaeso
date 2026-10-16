import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Slider: {
    description: "A 4px pill track, an ink range and a 16px ink thumb; a value bubble while dragging; ticks for steps; ranges.",
    props: {
      "aria-label": "The accessible name, for when there is no visible label.",
      defaultValue: "The value, or two values for a range, on first render.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      formatValue: "Formats the value in the bubble shown while dragging. Defaults to the number in the provider's language.",
      getAriaLabel: "Returns the accessible name of each thumb of a range.",
      max: "The highest value allowed.",
      min: "The lowest value allowed.",
      name: "The field name the value is sent under in a form.",
      onValueChange: "Called with the value while it is dragged.",
      onValueCommitted: "Called with the value when dragging ends.",
      step: "The steps the value moves in.",
      ticks: "Marks under the track at every step (or at these values).",
      value: "The value, or two values for a range, when you control it.",
    },
  },
} satisfies ComponentDocs;
