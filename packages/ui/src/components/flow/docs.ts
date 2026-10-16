import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Flow: {
    description: 'A node-and-connector diagram: the route "app → Kaeso → provider / model".',
    props: {
      animated: "Dots that travel along the connectors. Off by default; never on loaded pages that need calm.",
      "aria-label":
        'The accessible name of the diagram: what it shows, such as "Route of checkout requests". Defaults to "Flow diagram"; screen readers hear each node and each connection after it.',
      columns: "Columns of nodes, left to right.",
      edges: "Connections as [from, to] node ids.",
      node: "The shape of each step: a pill or a box.",
    },
  },
} satisfies ComponentDocs;
