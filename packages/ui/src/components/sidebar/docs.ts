import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Sidebar: {
    description: "The app's navigation: the wordmark, groups of links, an account menu at the foot, collapsible and resizable.",
    props: {
      "aria-label": "Defines a string value that labels the current element.",
      resizable: "Draggable right edge (232–280px), remembered.",
    },
  },
  "Sidebar.Provider": {
    description:
      "Holds the sidebar's state for everything inside it: whether it shows as a rail, is collapsed or peeks, and its width. Wrap the app frame in it once.",
    props: {
      children: "The app frame, and everything else that reads or changes the sidebar's state.",
      defaultRail: "Starts as a rail of icons instead of the full sidebar, until the person widens it.",
      shortcut: "Lets the [ key collapse and open the sidebar on screens 1024px and wider, except while typing or in a dialog. On by default.",
      storageKey: "Remembers the rail, width, collapsed state and open groups in local storage under this key, so they survive a reload.",
    },
  },
} satisfies ComponentDocs;
