import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Table: {
    description:
      'A plain table for small amounts of data; use `DataTable` for sorting, selection and many rows. It has no row boxes, so its text sits on the gutter, not 12px in; its rows are 40px like DataTable\'s and grow when a cell wraps. Wider than its room, it scrolls sideways inside its own box under an edge fade, and the box takes focus as a group named "Table, scrolls sideways". Keep ids and model names on one line with `className="whitespace-nowrap"` on their cells.',
    props: {
      density: "12px between columns (`default`), or 8px (`compact`) for reference tables of five or more columns.",
    },
  },
} satisfies ComponentDocs;
