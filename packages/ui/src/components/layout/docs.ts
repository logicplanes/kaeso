import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Grid: {
    description: "A responsive grid.",
    props: {
      align: "How items line up within their cells.",
      as: "The element or component to render as, such as `section` or `ul`.",
      columns: "The number of columns.",
      gap: "The space between items, in pixels on the spacing scale.",
      justify: "How items are spread along the main axis.",
    },
  },
  Inline: {
    description: "A horizontal row that wraps.",
    props: {
      align: "How items line up across the row.",
      as: "The element or component to render as, such as `section` or `ul`.",
      gap: "The space between items, in pixels on the spacing scale.",
      justify: "How items are spread along the main axis.",
      wrap: "Lets items wrap onto the next line.",
    },
  },
  Stack: {
    description: "A vertical stack.",
    props: {
      align: "How items line up across the column.",
      as: "The element or component to render as, such as `section` or `ul`.",
      gap: "The space between items, in pixels on the spacing scale.",
      justify: "How items are spread along the main axis.",
    },
  },
} satisfies ComponentDocs;
