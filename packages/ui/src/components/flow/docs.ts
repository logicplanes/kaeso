import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Flow: {
    description:
      'A node-and-connector diagram: the route "app → Kaeso → provider / model". A diagram wider than its column scrolls sideways with a fade at the edge and takes focus.',
    props: {
      animated:
        "When the route changes, a dot runs once along each connector, to show where requests now go. Never on first render and never with reduced motion.",
      "aria-label":
        'The accessible name of the diagram: what it shows, such as "Route of checkout requests". Defaults to "Flow diagram"; screen readers hear each node and each connection after it.',
      columns: "Columns of nodes, left to right.",
      edges: "Connections as [from, to] node ids.",
      node: "The shape of each step: a pill or a box.",
    },
  },
} satisfies ComponentDocs;
