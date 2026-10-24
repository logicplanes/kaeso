import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Table: {
    description: "A plain table for small amounts of data; use `DataTable` for sorting, selection and many rows.",
    props: {
      density: "12px between columns (`default`), or 8px (`compact`) for reference tables of five or more columns.",
    },
  },
} satisfies ComponentDocs;
