import type { ComponentDocs } from "../../component-docs";

export const docs = {
  BrandWordmark: {
    description:
      "The wordmark as `Sidebar.Brand` and AppFrame's phone bar draw it: a link to the app's home when there is one (its focus ring 4px around the glyphs), the plain wordmark otherwise.",
    props: {
      href: "Where the wordmark links to.",
      label: "The wordmark's accessible name.",
    },
  },
  Sidebar: {
    description: "The app's navigation: the wordmark, groups of links, an account menu at the foot, collapsible and resizable.",
    props: {
      "aria-label": "Defines a string value that labels the current element.",
      resizable: "Draggable right edge (220–320px), remembered.",
    },
  },
  "Sidebar.Provider": {
    description: "Provided by NavSheet: how the sidebar's own toggle closes the sheet it floats in.",
    props: {},
  },
} satisfies ComponentDocs;
