import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  FilterChip: {
    description: 'A filter as one pill: "Status is 2xx ×", each part opening a menu.',
    props: {
      defaultValuesOpen: "Open the value menu when the chip appears: a filter just added asks for its value at once.",
      multiple: "Several values may be chosen.",
      onOperatorChange: "Called with the chosen comparison.",
      onRemove: "Called when the filter is removed.",
      onSelectedChange: "Called with the chosen values.",
      operator: '"is", "is not", "is any of"',
      operators: "The comparisons people can choose, such as “is” and “is not”.",
      property: '"Status"',
      propertyIcon: "The icon of the property the filter applies to.",
      selected: "The chosen values.",
      value: 'The value, as shown: "2xx", or "2 models".',
      values: "Choices for the value menu.",
    },
  },
} satisfies ComponentDocs;
