import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Skeleton: {
    description:
      "A placeholder in the shape of the content that is loading. It is hidden from screen readers, so the region it stands in for sets `aria-busy` while it loads (List, DataTable and StatTile do). In forced colours it keeps an outline.",
    props: {
      lines: "Stands in for that many lines of text, the last one shorter; it takes the place of `shape`.",
      shape: "The shape it stands in for: a line of text, a control, a circle or a block.",
    },
  },
} satisfies ComponentDocs;
