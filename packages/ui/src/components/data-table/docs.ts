import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  DataTable: {
    description: "A table for many rows: sorting, selection, loading and empty states, more rows on demand, and virtual scrolling for long lists.",
    props: {
      activeRowId: "The id of a row to show as active (open in a sheet).",
      "aria-label": "The table's accessible name.",
      columns: "The columns, as TanStack Table column definitions.",
      data: "The rows.",
      density: "How much room each row gets.",
      empty: "What to show when there are no rows, usually an `Empty`.",
      error:
        "The rows failed to load: the table says so where its rows would be and offers Try again (`onRetry`). Errors are toasts, so the value isn't shown; toast the message where the request fails.",
      freshRowIds: "Rows that just arrived (a live list): each shows a fill that fades out over a second.",
      getRowId: "Returns a row's stable id, for selection and keys.",
      hasMore: "Whether more rows can be loaded.",
      loading: "Shows skeleton rows while the first page loads.",
      loadingMore: "Shows that more rows are loading.",
      manualSorting: "Sorting happens on the server: the data arrives sorted.",
      onHiddenColumnsChange: "Called with the ids of the columns left out at the table's width, so a kept column can show what they held.",
      onLoadMore: "Called to load the next rows.",
      onRetry: "Shows a Retry button when loading failed.",
      onRowClick: "Called when a row is clicked; rows then show that they can be opened.",
      onSelectionChange: "Called with the selected rows.",
      onSortingChange: "Called with the new sort order.",
      ref: "A ref to the outer element.",
      rowMenu: "Items for a row's context menu (right-click).",
      selectable: "Adds a checkbox to every row.",
      selection: "The selected rows, when you control them.",
      sorting: "The sort order, when you control it.",
      stickyFirstColumn: "Keeps the first column in view while scrolling sideways.",
      virtualizeAbove: "Above this many rows, only the visible rows are rendered.",
    },
  },
} satisfies ComponentDocs;
