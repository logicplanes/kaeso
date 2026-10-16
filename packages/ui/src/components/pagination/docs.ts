import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Pagination: {
    description: "Moves through long lists by pages, by cursor, or with a Load more button.",
    props: {
      hasMore: "Whether there is a next page.",
      hasPrevious: "Whether there is a previous page.",
      loading: "Shows that a page is loading.",
      onLoadMore: "Called by the Load more button.",
      onNext: "Called to go to the next page.",
      onPageChange: "Called with the chosen page number.",
      onPrevious: "Called to go to the previous page.",
      page: "pages:",
      pageCount: "The number of pages, for numbered pages.",
      variant: "Numbered pages, previous and next by cursor, or a Load more button.",
    },
  },
} satisfies ComponentDocs;
