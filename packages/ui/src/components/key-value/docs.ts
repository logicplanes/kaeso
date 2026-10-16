import type { ComponentDocs } from "../../component-docs";

export const docs = {
  KeyValue: {
    description:
      "Label and value pairs (13 secondary label, 14 value), copyable values. Side by side for sheets; label above value in columns (`grid`) where a sheet is wide enough for two or more to a row. A text value that doesn't fit ends in an ellipsis and shows whole in a tooltip.",
    props: {
      columns: "The grid layout's items to a row, from 480px of its container's width.",
      items: "The pairs to show.",
      layout: "Keys beside values in columns, in a grid, or stacked above them.",
    },
  },
} satisfies ComponentDocs;
