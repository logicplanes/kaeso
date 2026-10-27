import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ThemeImage: {
    description: "An image drawn twice, once for each theme: it follows the site's theme setting, not only the system's, and prints the light one.",
    props: {
      alt: "What the image shows, read by screen readers and shown when it can't load.",
      "aria-describedby": "The id of a longer description, such as a diagram's text alternative.",
      dark: "The image's address for the dark theme.",
      height:
        "The image's own height in pixels. Pass it with `width` so the page keeps the image's place before it loads; without both, the content below moves when it arrives.",
      light: "The image's address for the light theme and for print.",
      width: "The image's own width in pixels; the image shrinks to its container, never grows past this.",
    },
  },
} satisfies ComponentDocs;
