import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ResourceList: {
    description:
      "A console list of resources: search, group headers, row actions and a selection bar. A search announces how many rows it leaves, and its no-match state offers Clear search.",
    props: {
      "aria-label": "The list's accessible name.",
      bulkActions: "Actions for selected rows: [{label, icon, onClick(ids)}].",
      emptyAction: "The action shown when there are no items.",
      emptyDescription: "The sentence shown when there are no items.",
      emptyTitle:
        'What will appear, such as "No keys yet". Pass it: the default names the collection by its `aria-label` ("No keys yet", in the provider\'s language), which only reads well for a plural noun.',
      groups: "The groups of items.",
      onOpen: "Called when an item is opened.",
      toolbar: "The filters and display options above the list.",
    },
  },
} satisfies ComponentDocs;
