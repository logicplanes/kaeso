import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Figure: {
    description:
      "A diagram or image set apart from the text: centred on a well, with a caption. An image keeps its own width, so its text stays readable, and the figure scrolls sideways, with a fade at the edge, when that is wider than the column.",
    props: {
      caption: "A few words under the figure that say what it shows, such as a diagram's title.",
      children: "What the figure shows, usually a ThemeImage.",
      focus: "Where a figure wider than its column starts, as a share of its width: 0.5, the middle, by default.",
      label: "The name of the figure's scrolling region, read by screen readers; give each figure on a page its own.",
      surface: "The figure's background: a well like a code block's, or none.",
    },
  },
} satisfies ComponentDocs;
