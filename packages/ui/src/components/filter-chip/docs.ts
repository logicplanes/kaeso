import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  FilterChip: {
    description: 'A filter as one pill: "Status is 2xx ×", each part opening a menu.',
    props: {
      defaultValuesOpen: "Open the value menu when the chip appears: a filter just added asks for its value at once.",
      groups:
        "Choices as ready-made searchable list groups, in place of `values`: rows with marks and details and headings with logos, such as `useModelGroups` or `useProviderGroups` build.",
      multiple: "Several values may be chosen.",
      onOperatorChange: "Called with the chosen comparison.",
      onRemove: "Called when the filter is removed.",
      onSelectedChange: "Called with the chosen values.",
      onValuesOpenChange: "Called when the value menu opens or closes, so a filter just added can go away when it closes without a value.",
      operator: '"is", "is not", "is any of"',
      operators: "The comparisons people can choose, such as “is” and “is not”.",
      property: '"Status"',
      propertyIcon: "The icon of the property the filter applies to.",
      searchable: "Gives the value menu a search field however few choices it has.",
      selected: "The chosen values.",
      value: 'The value, as shown: "2xx", or "2 models".',
      width: "How wide the searchable list opens: `sm` for providers, `lg` for models with their details.",
      values:
        "Choices for the value menu: past 8 it gets a search field, past 50 it opens a fast searchable list grouped by each choice's `group`, such as models by provider.",
    },
  },
} satisfies ComponentDocs;
