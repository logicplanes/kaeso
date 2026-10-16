import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Skeleton: {
    description: "A placeholder in the shape of the content that is loading.",
    props: {
      shape: "The shape it stands in for: a line of text, a control, a circle or a block.",
    },
  },
  SkeletonLine: {
    description: "Lines of text: the last one shorter.",
    props: {
      lines: "How many lines of text it stands in for.",
    },
  },
} satisfies ComponentDocs;
