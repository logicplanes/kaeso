import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Flow: {
    description: 'A node-and-connector diagram: the route "app → Kaeso → provider / model".',
    props: {
      animated: "Dots that travel along the connectors. Off by default; never on loaded pages that need calm.",
      "aria-label": "The accessible name of the diagram.",
      columns: "Columns of nodes, left to right.",
      edges: "Connections as [from, to] node ids.",
      node: "The shape of each step: a pill or a box.",
    },
  },
} satisfies ComponentDocs;
