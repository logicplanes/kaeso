import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ResourceList: {
    description: "A console list of resources: search, group headers, row actions and a selection bar.",
    props: {
      "aria-label": "The list's accessible name.",
      bulkActions: "Actions for selected rows: [{label, icon, onClick(ids)}].",
      emptyAction: "The action shown when there are no items.",
      emptyDescription: "The sentence shown when there are no items.",
      emptyTitle: 'Defaults to "Nothing here yet" in the provider\'s language.',
      groups: "The groups of items.",
      onOpen: "Called when an item is opened.",
      toolbar: "The filters and display options above the list.",
    },
  },
} satisfies ComponentDocs;
