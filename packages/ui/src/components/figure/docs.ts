import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Figure: {
    description: "A diagram or image set apart from the text: centred on a well, with a caption, scrolling sideways only when it can't shrink to fit.",
    props: {
      caption: "A few words under the figure that say what it shows, such as a diagram's title.",
      children: "What the figure shows, usually a ThemeImage.",
      label: "The name of the figure's scrolling region, read by screen readers.",
      surface: "The figure's background: a well like a code block's, or none.",
    },
  },
} satisfies ComponentDocs;
